use crate::{AudioBackend, AudioStatus};

pub struct NullBackend;

impl AudioBackend for NullBackend {
    fn name(&self) -> &'static str {
        "null"
    }

    fn status(&self) -> AudioStatus {
        AudioStatus {
            backend: "null".into(),
            host: "none".into(),
            default_output: None,
            sample_rate: None,
            channels: None,
            pipewire: false,
        }
    }
}
