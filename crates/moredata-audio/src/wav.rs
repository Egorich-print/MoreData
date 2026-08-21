use crate::BackendError;
use moredata_runtime::Runtime;
use std::path::Path;

pub fn render_wav(
    rt: &mut Runtime,
    path: &Path,
    seconds: f32,
    sample_rate: u32,
) -> Result<u64, BackendError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| BackendError::Io(e.to_string()))?;
    let total = (seconds.max(0.0) * sample_rate as f32) as usize;
    let mut buf = [0.0f32; 64];
    let mut written = 0usize;
    while written < total {
        let n = (total - written).min(64);
        rt.process(&mut buf[..n]);
        for s in buf[..n].iter() {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            writer
                .write_sample(v)
                .map_err(|e| BackendError::Io(e.to_string()))?;
        }
        written += n;
    }
    writer
        .finalize()
        .map_err(|e| BackendError::Io(e.to_string()))?;
    Ok(written as u64)
}
