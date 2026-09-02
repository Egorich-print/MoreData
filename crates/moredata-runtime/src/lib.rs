//! Realtime execution wrapper. No filesystem, network, or logging in process().

pub mod link;

use moredata_core::{CompiledGraph, Diagnostics, EventQueue, ProcessState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Staging capacity for one block's events (bounded dispatch work).
const BLOCK_EVENT_LIMIT: usize = 128;

pub struct Runtime {
    graph: CompiledGraph,
    state: ProcessState,
    events: std::sync::Arc<EventQueue<256>>,
    event_buf: Box<[std::mem::MaybeUninit<Option<moredata_core::Event>>; BLOCK_EVENT_LIMIT]>,
    blocks: AtomicU64,
    frames: AtomicU64,
    xruns: AtomicU64,
    last_block_ns: AtomicU64,
    events_dropped: AtomicU64,
    backend: String,
}

impl Runtime {
    pub fn new(graph: CompiledGraph, state: ProcessState, backend: impl Into<String>) -> Self {
        Self::with_events(graph, state, backend, EventQueue::new())
    }

    pub fn with_events(
        graph: CompiledGraph,
        state: ProcessState,
        backend: impl Into<String>,
        events: EventQueue<256>,
    ) -> Self {
        Self::with_shared_events(graph, state, backend, std::sync::Arc::new(events))
    }

    /// Share the queue with a control-side handle (stress tests, MIDI thread).
    pub fn with_shared_events(
        graph: CompiledGraph,
        state: ProcessState,
        backend: impl Into<String>,
        events: std::sync::Arc<EventQueue<256>>,
    ) -> Self {
        Self {
            graph,
            state,
            events,
            // SAFETY: array of MaybeUninit — never read before written.
            event_buf: Box::new(unsafe {
                std::mem::MaybeUninit::<
                    [std::mem::MaybeUninit<Option<moredata_core::Event>>; BLOCK_EVENT_LIMIT],
                >::uninit()
                .assume_init()
            }),
            blocks: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            last_block_ns: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
            backend: backend.into(),
        }
    }

    pub fn graph(&self) -> &CompiledGraph {
        &self.graph
    }

    pub fn events(&self) -> &EventQueue<256> {
        &self.events
    }

    /// Control-plane entry for parameter patches (applied at next boundary).
    /// Coalescing happens in `ParamSnapshot`; generation guard in the engine.
    pub fn apply_params(&self, snap: &moredata_core::ParamSnapshot) -> bool {
        self.graph.apply_snapshot(snap)
    }

    /// Realtime callback. `out` is mono interleaved frames.
    /// Drain queue → sort by frame → process with dispatch. No allocation.
    pub fn process(&mut self, out: &mut [f32]) {
        let t0 = Instant::now();

        // Stage this block's events into the fixed window.
        let mut window = moredata_core::event::EventWindow::new(unsafe {
            &mut *(self.event_buf.as_mut_ptr()
                as *mut [Option<moredata_core::Event>; BLOCK_EVENT_LIMIT])
        });
        let mut staged = 0usize;
        while staged < BLOCK_EVENT_LIMIT {
            match self.events.pop() {
                Some(ev) => {
                    window.push(ev);
                    staged += 1;
                }
                None => break,
            }
        }
        window.prepare();
        self.events_dropped.store(
            (self.events.dropped() as u64) + (self.events.len() as u64),
            Ordering::Relaxed,
        );

        self.graph
            .process_with_events(&mut self.state, out.len(), out, &mut window);

        let ns = t0.elapsed().as_nanos() as u64;
        self.last_block_ns.store(ns, Ordering::Relaxed);
        self.blocks.fetch_add(1, Ordering::Relaxed);
        self.frames.fetch_add(out.len() as u64, Ordering::Relaxed);
    }

    pub fn record_xrun(&self) {
        self.xruns.fetch_add(1, Ordering::Relaxed);
    }

    pub fn diagnostics(&self) -> Diagnostics {
        let mut d = self.diagnostics_inner();
        d.backend = self.backend.clone();
        d
    }

    fn diagnostics_inner(&self) -> Diagnostics {
        Diagnostics {
            blocks: self.blocks.load(Ordering::Relaxed),
            frames: self.frames.load(Ordering::Relaxed),
            xruns: self.xruns.load(Ordering::Relaxed),
            last_block_ns: self.last_block_ns.load(Ordering::Relaxed),
            sample_rate: self.graph.sample_rate,
            max_block: self.graph.max_block,
            nodes: self.graph.node_count(),
            ..Diagnostics::default()
        }
    }

    pub fn events_pending(&self) -> usize {
        self.events.len()
    }
}
