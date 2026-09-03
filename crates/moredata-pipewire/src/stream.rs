//! PipeWire stream management.

use crate::{PipeWireConfig, PipeWireBackendStatus};
use moredata_audio::BackendError;
use moredata_scheduler::Scheduler;
use spa::buffer::Buffer;
use spa::pod::deserialize::PodDeserializer;
use spa::pod::Pod;
use spa::param::format::{Format, MediaType, MediaSubtype};
use spa::param::param_type::ParamType;
use spa::param::prop::Props;
use spa::param::ParamType as SpaParamType;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// PipeWire stream wrapper.
pub struct PipeWireStream {
    config: PipeWireConfig,
    core: Option<pipewire::Core>,
    stream: Option<pipewire::Stream>,
    main_loop: Arc<std::sync::Mutex<spa::MainLoop>>,
    running: std::sync::atomic::AtomicBool,
    status: Arc<Mutex<PipeWireBackendStatus>>,
}

impl PipeWireStream {
    pub fn new(
        config: &PipeWireConfig,
        main_loop: Arc<std::sync::Mutex<spa::MainLoop>>,
    ) -> Result<Self, crate::BackendError> {
        // Initialize PipeWire
        pipewire::init().map_err(|e| crate::BackendError::Device(e.to_string()))?;

        // Create core
        let core = pipewire::Core::new().map_err(|e| crate::BackendError::Device(e.to_string()))?;

        // Create stream
        let stream = pipewire::Stream::new(
            &core,
            "moredata-stream",
            pipewire::properties::properties! {
                *pipewire::keys::MEDIA_TYPE => "Audio",
                *pipewire::keys::MEDIA_CATEGORY => "Playback",
                *pipewire::keys::MEDIA_ROLE => "Music",
            },
        ).map_err(|e| crate::BackendError::Device(e.to_string()))?;

        Ok(Self {
            config: crate::PipeWireConfig::default(),
            core: Some(core),
            stream: Some(stream),
            main_loop,
            running: std::sync::atomic::AtomicBool::new(false),
            status: std::sync::Arc::new(std::sync::Mutex::new(crate::PipeWireBackendStatus::default())),
        })
    }

    pub fn connect(&mut self) -> Result<(), crate::BackendError> {
        let stream = self.stream.as_mut().unwrap();

        // Set format
        let format = spa::param::format::Format::new(
            spa::param::format::MediaType::Audio,
            spa::param::format::MediaSubtype::Raw,
            spa::param::format::FormatFlags::empty(),
        ).with_raw_audio(
            spa::param::format::RawAudioFormat::new()
                .format(spa::pod::RawAudioFormat::F32P)
                .channels(2)
                .rate(48000),
        );

        stream.set_format(&format!()).map_err(|e| crate::BackendError::Device(e.to_string()))?;

        // Connect
        stream.connect(
            pipewire::stream::Direction::Output,
            pipewire::keys::TARGET_OBJECT,
            spa::pod::Choice::None,
            &[],
        ).map_err(|e| crate::BackendError::Device(e.to_string()))?;

        self.running.store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn status(&self) -> crate::PipeWireBackendStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn stop(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Release);
        if let Some(stream) = self.stream.take() {
            let _ = stream.disconnect();
        }
    }
}