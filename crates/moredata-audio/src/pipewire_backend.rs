//! PipeWire backend implementation for MoreData.
//!
//! This module is only compiled when the "pipewire" feature is enabled.
//! It provides a PipeWire backend that uses the M5.4 inline scheduler
//! for zero-allocation, bounded-block-time audio processing.

use crate::{AudioBackend, AudioStatus, BackendError, PipeWireConfig, PipeWireBackendStatus};
use moredata_core::{CompiledGraph, Diagnostics, EventWindow, Graph, NodeKind};
use moredata_runtime::ProcessState;
use moredata_scheduler::{Plan, Scheduler};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// PipeWire backend implementation using M5.4 inline scheduler.
/// Uses the M5.4 inline scheduler for zero-alloc, bounded-block-time audio processing.
pub struct PipeWireBackend {
    config: PipeWireConfig,
    scheduler: Option<moredata_scheduler::Scheduler>,
    graph: Option<moredata_core::Graph>,
    running: std::sync::atomic::AtomicBool,
    status: std::sync::Mutex<PipeWireBackendStatus>,
}

impl PipeWireBackend {
    pub fn new(config: PipeWireConfig) -> Result<Self, crate::BackendError> {
        Ok(Self {
            config,
            scheduler: None,
            graph: None,
            running: std::sync::atomic::AtomicBool::new(false),
            status: std::sync::Mutex::new(PipeWireBackendStatus::default()),
        }
    }

    /// Probe PipeWire availability and return status if available.
    pub fn probe() -> Result<AudioStatus, crate::BackendError> {
        // Try to connect to PipeWire
        match pipewire::init() {
            Ok(_) => {
                let core = pipewire::Core::new().map_err(|e| crate::BackendError::Device(e.to_string()))?;
                let info = core.info().map_err(|e| crate::BackendError::Device(e.to_string()))?;
                Ok(AudioStatus {
                    backend: "pipewire".to_string(),
                    host: format!("PipeWire {}", info.version()),
                    default_output: Some("PipeWire".to_string()),
                    sample_rate: Some(48000),
                    channels: Some(2),
                    pipewire: true,
                })
            }
            Err(_) => Err(crate::BackendError::NoDevice),
        }
    }

    pub fn new(config: PipeWireConfig) -> Result<Self, crate::BackendError> {
        Ok(Self {
            config,
            scheduler: None,
            graph: None,
            running: std::sync::atomic::AtomicBool::new(false),
            status: std::sync::Mutex::new(PipeWireBackendStatus::default()),
        }
    }

    pub fn initialize(&mut self, graph: &moredata_core::Graph) -> Result<(), crate::BackendError> {
        // Create plan and scheduler
        let plan = moredata_scheduler::Plan::from_graph(&self.graph().unwrap_or_else(|| {
            // We need to store the graph first
            let mut g = Graph::new(48000).unwrap();
            g
        }));
        let scheduler = moredata_scheduler::Scheduler::new(1, moredata_scheduler::Plan::from_graph(&self.graph().unwrap())?);
        self.scheduler = Some(scheduler);
        self.graph = Some(Graph::new(48000).unwrap());
        Ok(())
    }

    fn graph(&self) -> Option<&moredata_core::Graph> {
        self.graph.as_ref()
    }
}

impl crate::AudioBackend for PipeWireBackend {
    fn name(&self) -> &'static str {
        "pipewire"
    }

    fn status(&self) -> AudioStatus {
        let s = self.status.lock().unwrap();
        AudioStatus {
            backend: "pipewire".to_string(),
            host: "PipeWire".to_string(),
            default_output: Some("PipeWire".to_string()),
            sample_rate: Some(self.config.sample_rate),
            channels: Some(self.config.channels),
            pipewire: true,
        }
    }
}

impl PipeWireBackend {
    /// Process one block of audio using the M5.4 inline scheduler.
    /// This is called from the PipeWire process callback.
    pub fn process_block(
        &mut self,
        graph: &moredata_core::CompiledGraph,
        state: &mut moredata_runtime::ProcessState,
        frames: usize,
        out: &mut [f32],
    ) -> Result<(), crate::BackendError> {
        if !self.running() {
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

    fn running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Acquire)
    }
}