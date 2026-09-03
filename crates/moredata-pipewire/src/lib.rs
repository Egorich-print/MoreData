//! PipeWire backend for MoreData realtime audio platform.
//!
//! Uses M5.4 inline scheduler (zero-alloc, bounded block time, no Mutex on RT path).
//! PipeWire-specific: SPA buffer handling, quantum/rate change events,
//! connection/disconnect management without RT-path mutex.
//!
//! Requires the `pipewire-system` feature and the `libpipewire-0.3` system library.
//! Only available on Linux with PipeWire installed.

#[cfg(feature = "pipewire-system")]
mod backend;
#[cfg(feature = "pipewire-system")]
mod events;
#[cfg(feature = "pipewire-system")]
mod stream;

#[cfg(feature = "pipewire-system")]
pub use backend::{PipeWireBackend, PipeWireConfig};
#[cfg(feature = "pipewire-system")]
pub use events::{PipeWireEvent, PipeWireEventLoop};
#[cfg(feature = "pipewire-system")]
pub use stream::PipeWireStream;

use moredata_audio::{AudioBackend, AudioStatus, BackendError};
use moredata_core::EventWindow;
use moredata_scheduler::{Plan, Scheduler};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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

/// PipeWire backend implementation using M5.4 inline scheduler.
#[cfg(feature = "pipewire-system")]
pub struct PipeWireBackend {
    config: PipeWireConfig,
    scheduler: Option<Scheduler>,
    stream: Option<PipeWireStream>,
    main_loop: Option<Arc<Mutex<spa::MainLoop>>>,
    running: AtomicBool,
    status: Mutex<PipeWireBackendStatus>,
}

#[derive(Debug, Default, Clone)]
pub struct PipeWireBackendStatus {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: u32,
    pub connected: bool,
    pub quantum: u32,
    pub rate: u32,
}

#[cfg(feature = "pipewire-system")]
impl PipeWireBackend {
    pub fn new(config: PipeWireConfig) -> Result<Self, BackendError> {
        Ok(Self {
            config,
            scheduler: None,
            stream: None,
            main_loop: None,
            running: AtomicBool::new(false),
            status: Mutex::new(PipeWireBackendStatus::default()),
        })
    }

    fn init_pipeline(&mut self, graph: &moredata_core::CompiledGraph) -> Result<(), BackendError> {
        // Create main loop
        let main_loop = spa::MainLoop::new().map_err(|e| BackendError::Device(e.to_string()))?;
        self.main_loop = Some(Arc::new(Mutex::new(main_loop)));

        // Create PipeWire stream
        let stream = PipeWireStream::new(
            &self.config,
            self.main_loop.as_ref().unwrap().clone(),
        )?;
        self.stream = Some(stream);

        // Initialize scheduler with plan
        let plan = moredata_scheduler::Plan::from_graph(graph);
        let scheduler = moredata_scheduler::Scheduler::new(1, plan);
        self.scheduler = Some(scheduler);

        Ok(())
    }
}

#[cfg(feature = "pipewire-system")]
impl AudioBackend for PipeWireBackend {
    fn name(&self) -> &'static str {
        "pipewire"
    }

    fn status(&self) -> AudioStatus {
        let s = self.status.lock().unwrap();
        AudioStatus {
            backend: self.name().to_string(),
            host: "PipeWire".to_string(),
            default_output: Some("PipeWire".to_string()),
            sample_rate: Some(s.sample_rate),
            channels: Some(s.channels),
            pipewire: true,
        }
    }
}

#[cfg(feature = "pipewire-system")]
impl Drop for PipeWireBackend {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(ml) = self.main_loop.take() {
            ml.lock().unwrap().quit();
        }
    }
}

#[cfg(feature = "pipewire-system")]
impl PipeWireBackend {
    /// Run one block of audio using the inline scheduler.
    /// Called from PipeWire's process callback.
    pub fn process_block(
        &mut self,
        graph: &moredata_core::CompiledGraph,
        state: &mut moredata_runtime::ProcessState,
        frames: usize,
        out: &mut [f32],
    ) -> Result<(), BackendError> {
        if !self.running.load(Ordering::Acquire) {
            return Ok(());
        }

        if let Some(scheduler) = &self.scheduler {
            let mut window = moredata_core::EventWindow::empty();
            moredata_scheduler::Scheduler::run_block(
                &self.scheduler.as_ref().unwrap(),
                graph,
                state,
                frames,
                out,
                &mut moredata_core::EventWindow::empty(),
            );
        }
        Ok(())
    }
}

// Fallback when pipewire-system feature is not enabled
#[cfg(not(feature = "pipewire-system"))]
impl PipeWireBackend {
    pub fn new(_config: PipeWireConfig) -> Result<Self, BackendError> {
        Err(BackendError::Unsupported("PipeWire backend requires the 'pipewire-system' feature and libpipewire-0.3 system library".to_string()))
    }
}