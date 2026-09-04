//! PipeWire backend for MoreData (Linux-only, work in progress).
//!
//! Always compiles on every platform. The real backend requires the
//! `pipewire-system` feature and the `libpipewire-0.3` system library (Linux).
//! Without the feature, [`PipeWireBackend::new`] returns
//! [`BackendError::Unsupported`].

use moredata_audio::{AudioBackend, AudioStatus, BackendError};

/// Configuration for the PipeWire backend.
#[derive(Debug, Clone)]
pub struct PipeWireConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: u32,
    pub client_name: String,
    pub auto_connect: bool,
}

impl Default for PipeWireConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 64,
            client_name: "MoreData".to_string(),
            auto_connect: true,
        }
    }
}

/// PipeWire backend. Uses the M5.4 inline scheduler once the real
/// stream implementation lands (M5.6, Linux only).
pub struct PipeWireBackend {
    config: PipeWireConfig,
}

impl PipeWireBackend {
    pub fn new(config: PipeWireConfig) -> Result<Self, BackendError> {
        #[cfg(feature = "pipewire-system")]
        {
            Ok(Self { config })
        }
        #[cfg(not(feature = "pipewire-system"))]
        {
            let _ = config;
            Err(BackendError::Unsupported(
                "PipeWire backend is Linux-only: rebuild with the 'pipewire-system' \
                 feature and libpipewire-0.3 dev packages installed"
                    .into(),
            ))
        }
    }

    pub fn config(&self) -> &PipeWireConfig {
        &self.config
    }
}

impl AudioBackend for PipeWireBackend {
    fn name(&self) -> &'static str {
        "pipewire"
    }

    fn status(&self) -> AudioStatus {
        AudioStatus {
            backend: self.name().into(),
            host: "PipeWire".into(),
            default_output: None,
            sample_rate: Some(self.config.sample_rate),
            channels: Some(self.config.channels),
            pipewire: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_are_sane() {
        let c = PipeWireConfig::default();
        assert_eq!(c.sample_rate, 48000);
        assert_eq!(c.channels, 2);
    }

    #[test]
    #[cfg(not(feature = "pipewire-system"))]
    fn unsupported_without_feature() {
        assert!(matches!(
            PipeWireBackend::new(PipeWireConfig::default()),
            Err(BackendError::Unsupported(_))
        ));
    }
}
