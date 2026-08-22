use crate::{AudioStatus, BackendError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use moredata_runtime::Runtime;
use moredata_runtime::link::{ControlLink, RtLink, channel};
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

impl crate::AudioBackend for CpalBackend {
    fn name(&self) -> &'static str {
        "cpal"
    }

    fn status(&self) -> AudioStatus {
        self.status.clone()
    }
}

/// A live output stream bound to an [`RtLink`]. The audio callback owns the
/// RT half exclusively (FnMut) — there is no lock anywhere in the process path.
pub struct PlaySession {
    _stream: cpal::Stream,
    ctrl: ControlLink,
}

impl PlaySession {
    /// Swap the running engine without stopping the stream (lock-free).
    pub fn publish(&self, rt: Runtime) {
        self.ctrl.publish(rt);
    }

    /// Reclaim the retired engine, if the RT thread already handed it back.
    pub fn poll_retired(&self) -> Option<Runtime> {
        self.ctrl.poll_retired()
    }
}

/// Blocking convenience: open the default output, play for `seconds`, stop.
pub fn play_once(rt: Runtime, seconds: f32) -> Result<(), BackendError> {
    let session = play(rt)?;
    std::thread::sleep(Duration::from_secs_f32(seconds.max(0.05)));
    drop(session);
    Ok(())
}

/// Open the default output and start processing `rt`. Control-plane call.
pub fn play(rt: Runtime) -> Result<PlaySession, BackendError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(BackendError::NoDevice)?;
    let config = device
        .default_output_config()
        .map_err(|e| BackendError::Device(e.to_string()))?;
    let channels = config.channels() as usize;
    let (ctrl, rt_link) = channel(rt);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, &config, channels, rt_link, |data, ch, link| {
                fill_f32(data, ch, link)
            })?
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, channels, rt_link, |data, ch, link| {
                fill_i16(data, ch, link)
            })?
        }
        other => return Err(BackendError::Unsupported(format!("{other:?}"))),
    };

    stream
        .play()
        .map_err(|e| BackendError::Device(e.to_string()))?;
    Ok(PlaySession {
        _stream: stream,
        ctrl,
    })
}

fn build_stream<T: cpal::SizedSample + 'static>(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    channels: usize,
    mut rt_link: RtLink,
    fill: fn(&mut [T], usize, &mut RtLink),
) -> Result<cpal::Stream, BackendError> {
    let cfg: cpal::StreamConfig = config.clone().into();
    device
        .build_output_stream(
            &cfg,
            move |data: &mut [T], _| fill(data, channels, &mut rt_link),
            |e: cpal::StreamError| {
                let _ = e;
            },
            None,
        )
        .map_err(|e| BackendError::Device(e.to_string()))
}

fn fill_f32(data: &mut [f32], channels: usize, rt: &mut RtLink) {
    let channels = channels.max(1);
    let frames = data.len() / channels;
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

fn fill_i16(data: &mut [i16], channels: usize, rt: &mut RtLink) {
    let mut f32buf = [0.0f32; 64];
    let frames = data.len() / channels.max(1);
    let mut done = 0;
    while done < frames {
        let n = (frames - done).min(64);
        rt.process(&mut f32buf[..n]);
        for (i, s) in f32buf[..n].iter().enumerate() {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            let base = (done + i) * channels.max(1);
            for c in 0..channels {
                if base + c < data.len() {
                    data[base + c] = v;
                }
            }
        }
        done += n;
    }
}
