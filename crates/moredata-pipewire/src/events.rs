//! PipeWire event handling.

use moredata_core::event::{Event, EventKind, EventWindow, EventQueue};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// PipeWire event types mapped to MoreData event kinds.
#[derive(Debug, Clone)]
pub enum PipeWireEvent {
    /// Buffer size changed (quantum change)
    BufferSizeChanged { frames: u32 },
    /// Sample rate changed
    SampleRateChanged { rate: u32 },
    /// Device disconnected
    DeviceDisconnected,
    /// Device connected
    DeviceConnected,
    /// Latency changed
    LatencyChanged { nsec: u32 },
    /// XRun (buffer underrun/overrun)
    XRun,
}

/// Event loop for processing PipeWire events on the control plane.
/// The audio thread never processes these events directly.
pub struct PipeWireEventLoop {
    queue: EventQueue<64>,
    running: std::sync::atomic::AtomicBool,
}

impl PipeWireEventLoop {
    pub fn new() -> Self {
        Self {
            queue: EventQueue::new(),
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn start(&self) {
        self.running.store(true, std::sync::atomic::Ordering::Release);
    }

    pub fn stop(&self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn push_event(&self, event: crate::PipeWireEvent) {
        let ev = match event {
            crate::PipeWireEvent::BufferSizeChanged { frames } => {
                crate::Event::new(crate::EventKind::Trigger, crate::graph::NodeId(0))
            }
            crate::PipeWireEvent::SampleRateChanged { rate } => {
                crate::Event::new(crate::EventKind::Trigger, crate::graph::NodeId(0))
            }
            crate::PipeWireEvent::DeviceDisconnected => {
                crate::Event::new(crate::EventKind::Trigger, crate::graph::NodeId(0))
            }
            crate::PipeWireEvent::DeviceConnected => {
                crate::Event::new(crate::EventKind::Trigger, crate::graph::NodeId(0))
            }
            crate::PipeWireEvent::LatencyChanged { nsec } => {
                crate::Event::new(crate::EventKind::Trigger, crate::graph::NodeId(0))
            }
            crate::PipeWireEvent::XRun => {
                crate::Event::new(crate::EventKind::Trigger, crate::graph::NodeId(0))
            }
        };
        self.queue.push(ev);
    }

    pub fn drain_to_window(&self, window: &mut moredata_core::event::EventWindow<'_>) {
        let mut staged = 0;
        while let Some(ev) = self.queue.pop() {
            if !window.push(ev) {
                break;
            }
            staged += 1;
        }
    }
}

impl Default for PipeWireEventLoop {
    fn default() -> Self {
        Self::new()
    }
}