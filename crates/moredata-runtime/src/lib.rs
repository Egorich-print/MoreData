//! Realtime execution wrapper. No filesystem, network, or logging in process().

pub mod link;

use moredata_core::{CompiledGraph, Diagnostics, ProcessState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct Runtime {
    graph: CompiledGraph,
    state: ProcessState,
    blocks: AtomicU64,
    frames: AtomicU64,
    xruns: AtomicU64,
    last_block_ns: AtomicU64,
    backend: String,
}

impl Runtime {
    pub fn new(graph: CompiledGraph, state: ProcessState, backend: impl Into<String>) -> Self {
        Self {
            graph,
            state,
            blocks: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            last_block_ns: AtomicU64::new(0),
            backend: backend.into(),
        }
    }

    pub fn graph(&self) -> &CompiledGraph {
        &self.graph
    }

    /// Realtime callback. `out` is mono interleaved frames.
    pub fn process(&mut self, out: &mut [f32]) {
        let t0 = Instant::now();
        self.graph.process(&mut self.state, out.len(), out);
        let ns = t0.elapsed().as_nanos() as u64;
        self.last_block_ns.store(ns, Ordering::Relaxed);
        self.blocks.fetch_add(1, Ordering::Relaxed);
        self.frames.fetch_add(out.len() as u64, Ordering::Relaxed);
    }

    pub fn record_xrun(&self) {
        self.xruns.fetch_add(1, Ordering::Relaxed);
    }

    pub fn diagnostics(&self) -> Diagnostics {
        Diagnostics {
            blocks: self.blocks.load(Ordering::Relaxed),
            frames: self.frames.load(Ordering::Relaxed),
            xruns: self.xruns.load(Ordering::Relaxed),
            last_block_ns: self.last_block_ns.load(Ordering::Relaxed),
            sample_rate: self.graph.sample_rate,
            max_block: self.graph.max_block,
            nodes: self.graph.node_count(),
            backend: self.backend.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moredata_core::{CompileOptions, Graph, NodeKind};

    #[test]
    fn runtime_counts_blocks() {
        let mut g = Graph::new(48_000).unwrap();
        let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
        let out = g.add_node("out", NodeKind::Output).unwrap();
        g.connect(osc, "out", out, "in").unwrap();
        let (cg, st) =
            moredata_core::CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
        let mut rt = Runtime::new(cg, st, "null");
        let mut buf = [0.0f32; 16];
        rt.process(&mut buf);
        let d = rt.diagnostics();
        assert_eq!(d.blocks, 1);
        assert_eq!(d.frames, 16);
        assert_eq!(d.backend, "null");
    }
}
