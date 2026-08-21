use crate::{AudioBackend, AudioStatus, BackendError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use moredata_runtime::Runtime;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct CpalBackend {
    status: AudioStatus,
}

impl CpalBackend {
    pub fn probe() -> AudioStatus {
        let host = cpal::default_host();
        let host_id = format!("{:?}", host.id());
        let device = host.default_output_device();
        let name = device.as_ref().and_then(|d| d.name().ok());
        let cfg = device.as_ref().and_then(|d| d.default_output_config().ok());
        AudioStatus {
            backend: "cpal".into(),
            host: host_id,
            default_output: name,
            sample_rate: cfg.as_ref().map(|c| c.sample_rate().0),
            channels: cfg.as_ref().map(|c| c.channels()),
            pipewire: false,
        }
    }
}

impl AudioBackend for CpalBackend {
    fn name(&self) -> &'static str {
        "cpal"
    }

    fn status(&self) -> AudioStatus {
        self.status.clone()
    }
}

/// Play compiled runtime on the default output for `seconds`. Control-plane call.
///
/// cpal requires a `'static` callback, so the runtime is behind `Mutex`.
/// The callback uses `try_lock` and a stack scratch buffer — no heap in process.
pub fn play_once(rt: Runtime, seconds: f32) -> Result<(), BackendError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(BackendError::NoDevice)?;
    let config = device
        .default_output_config()
        .map_err(|e| BackendError::Device(e.to_string()))?;
    let channels = config.channels() as usize;
    let shared = Arc::new(Mutex::new(rt));
    let shared2 = shared.clone();

    let err_fn = |e: cpal::StreamError| {
        let _ = e;
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let cfg: cpal::StreamConfig = config.clone().into();
            device
                .build_output_stream(
                    &cfg,
                    move |data: &mut [f32], _| fill_f32(data, channels, &shared2),
                    err_fn,
                    None,
                )
                .map_err(|e| BackendError::Device(e.to_string()))?
        }
        cpal::SampleFormat::I16 => {
            let cfg: cpal::StreamConfig = config.clone().into();
            device
                .build_output_stream(
                    &cfg,
                    move |data: &mut [i16], _| fill_i16(data, channels, &shared2),
                    err_fn,
                    None,
                )
                .map_err(|e| BackendError::Device(e.to_string()))?
        }
        other => {
            return Err(BackendError::Unsupported(format!("{other:?}")));
        }
    };

    stream
        .play()
        .map_err(|e| BackendError::Device(e.to_string()))?;
    let dur = Duration::from_secs_f32(seconds.max(0.05));
    std::thread::sleep(dur);
    Ok(())
}

fn fill_f32(data: &mut [f32], channels: usize, rt: &Arc<Mutex<Runtime>>) {
    let channels = channels.max(1);
    let frames = data.len() / channels;
    let Ok(mut rt) = rt.try_lock() else {
        data.fill(0.0);
        return;
    };
    let mut scratch = [0.0f32; 64];
    let mut done = 0;
    while done < frames {
        let n = (frames - done).min(64);
        rt.process(&mut scratch[..n]);
        for (i, s) in scratch[..n].iter().enumerate() {
            let base = (done + i) * channels;
            for c in 0..channels {
                if base + c < data.len() {
                    data[base + c] = *s;
                }
            }
        }
        done += n;
    }
}

fn fill_i16(data: &mut [i16], channels: usize, rt: &Arc<Mutex<Runtime>>) {
    let mut f32buf = [0.0f32; 512];
    let mut offset = 0;
    while offset < data.len() {
        let n = (data.len() - offset).min(512);
        fill_f32(&mut f32buf[..n], channels, rt);
        for (o, s) in data[offset..offset + n].iter_mut().zip(f32buf[..n].iter()) {
            *o = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
        offset += n;
    }
}
