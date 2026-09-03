//! AudioBackend trait. PipeWire available via separate moredata-pipewire crate;
//! cpal, wav, null always available.

mod cpal_backend;
mod null;
mod wav;

use moredata_core::Diagnostics;
use serde::Serialize;
use thiserror::Error;

pub use cpal_backend::{CpalBackend, play, play_once};
pub use null::NullBackend;
pub use wav::render_wav;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("no audio device")]
    NoDevice,
    #[error("device: {0}")]
    Device(String),
    #[error("io: {0}")]
    Io(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioStatus {
    pub backend: String,
    pub host: String,
    pub default_output: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub pipewire: bool,
}

pub trait AudioBackend {
    fn name(&self) -> &'static str;
    fn status(&self) -> AudioStatus;
}

impl From<std::io::Error> for BackendError {
    fn from(e: std::io::Error) -> Self {
        BackendError::Io(e.to_string())
    }
}

pub fn probe() -> AudioStatus {
    CpalBackend::probe()
}

pub fn merge_diag(d: Diagnostics, backend: &str) -> Diagnostics {
    let mut d = d;
    d.backend = backend.into();
    d
}
