use crate::dsp::{self, MAX_BLOCK, NodeState};
use crate::error::GraphError;
use crate::event;
use crate::graph::{Graph, NodeId, NodeKind};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Process-wide monotonic generation for compiled engines.
fn next_generation() -> u64 {
    static GEN: AtomicU64 = AtomicU64::new(1);
    GEN.fetch_add(1, Ordering::Relaxed)
}

/// Max gates per compiled graph (fixed dispatch table).
pub(crate) const MAX_GATES: usize = 8;
/// Max level changes per gate per block (bounded work guarantee).
pub(crate) const MAX_GATE_EVENTS: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct CompileOptions {
    pub max_block: usize,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            max_block: MAX_BLOCK,
        }
    }
}

#[derive(Debug)]
pub struct ParamSlot {
    pub bits: AtomicU32,
}

impl ParamSlot {
    pub fn new(v: f32) -> Self {
        Self {
            bits: AtomicU32::new(v.to_bits()),
        }
    }

    pub fn store(&self, v: f32) {
        self.bits.store(v.to_bits(), Ordering::Relaxed);
    }

    pub fn load(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }
}

/// One parameter value addressed by slot index. The RT side applies a
/// snapshot only if `generation` matches the compiled graph's generation,
/// so a late patch from the previous engine can never leak into a new one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamValue {
    pub slot: usize,
    pub value: f32,
}

/// Coalesced parameter state for one block boundary. Fixed capacity;
/// control side builds it without allocation and ships it via mailbox.
#[derive(Debug, Clone)]
pub struct ParamSnapshot {
    /// Must match `CompiledGraph::generation()` to be applied.
    pub generation: u64,
    pub values: Vec<ParamValue>,
}

impl ParamSnapshot {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            values: Vec::new(),
        }
    }

    pub fn push(&mut self, slot: usize, value: f32) {
        // Coalesce: last write per slot wins.
        if let Some(existing) = self.values.iter_mut().find(|v| v.slot == slot) {
            existing.value = value;
        } else {
            self.values.push(ParamValue { slot, value });
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    src: usize,
    dst: usize,
}

#[derive(Debug)]
struct CompiledNode {
    kind: NodeKind,
    freq: Option<usize>,
    amp: Option<usize>,
    gain: Option<usize>,
}

/// Immutable schedule + atomic param table. Safe to share with the audio thread.
pub struct CompiledGraph {
    pub sample_rate: u32,
    pub max_block: usize,
    generation: u64,
    nodes: Vec<CompiledNode>,
    order: Vec<usize>,
    edges: Vec<Edge>,
    params: Vec<ParamSlot>,
    output_index: usize,
    param_index: Vec<(NodeId, &'static str, usize)>,
    gate_nodes: Vec<(NodeId, usize)>,
}

impl CompiledGraph {
    /// Monotonic engine identity for parameter snapshot validation.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Preallocated scratch + node state. One owner: the audio thread (or renderer).
pub struct ProcessState {
    states: Vec<NodeState>,
    buffers: Vec<f32>,
    node_count: usize,
    max_block: usize,
}

impl CompiledGraph {
    pub fn compile(
        graph: &Graph,
        opts: CompileOptions,
    ) -> Result<(Self, ProcessState), GraphError> {
        graph.validate()?;
        let max_block = opts.max_block.max(1);
        let topo = graph.topo()?;
        let mut id_to_idx: Vec<(u32, usize)> = Vec::new();
        let mut nodes = Vec::new();
        let mut params = Vec::new();
        let mut param_index = Vec::new();
        let mut gate_nodes: Vec<(NodeId, usize)> = Vec::new();
        let mut output_index = None;

        for (i, node) in graph.nodes().iter().enumerate() {
            id_to_idx.push((node.id.0, i));
            let mut freq = None;
            let mut amp = None;
            let mut gain = None;
            match node.kind {
                NodeKind::Oscillator => {
                    freq = Some(push_param(
                        &mut params,
                        &mut param_index,
                        node.id,
                        "freq",
                        *node.params.get("freq").unwrap_or(&440.0),
                    ));
                    amp = Some(push_param(
                        &mut params,
                        &mut param_index,
                        node.id,
                        "amp",
                        *node.params.get("amp").unwrap_or(&0.2),
                    ));
                }
                NodeKind::Gain => {
                    gain = Some(push_param(
                        &mut params,
                        &mut param_index,
                        node.id,
                        "gain",
                        *node.params.get("gain").unwrap_or(&1.0),
                    ));
                }
                NodeKind::Gate => {
                    // Gate consumes events; (node id, state slot) for dispatch.
                    gate_nodes.push((node.id, i));
                }
                NodeKind::Output => output_index = Some(i),
                NodeKind::Mixer => {}
            }
            nodes.push(CompiledNode {
                kind: node.kind,
                freq,
                amp,
                gain,
            });
        }

        let output_index = output_index.ok_or(GraphError::NoOutput)?;

        let mut edges = Vec::new();
        for c in graph.connections() {
            let src = lookup(&id_to_idx, c.from_node.0)?;
            let dst = lookup(&id_to_idx, c.to_node.0)?;
            edges.push(Edge { src, dst });
        }

        let order: Vec<usize> = topo
            .iter()
            .map(|id| lookup(&id_to_idx, id.0))
            .collect::<Result<_, _>>()?;

        let compiled = Self {
            sample_rate: graph.sample_rate,
            max_block,
            generation: next_generation(),
            nodes,
            order,
            edges,
            params,
            output_index,
            param_index,
            gate_nodes,
        };
        let state = ProcessState {
            states: graph
                .nodes()
                .iter()
                .map(|n| NodeState::new(n.kind))
                .collect(),
            buffers: vec![0.0; graph.nodes().len() * max_block],
            node_count: graph.nodes().len(),
            max_block,
        };
        Ok((compiled, state))
    }

    pub fn set_param(&self, id: NodeId, name: &str, value: f32) -> Result<(), GraphError> {
        let slot = self
            .param_index
            .iter()
            .find(|(nid, n, _)| *nid == id && *n == name)
            .map(|(_, _, i)| *i)
            .ok_or_else(|| GraphError::UnknownParam {
                node: format!("{}", id.0),
                param: name.into(),
            })?;
        self.params[slot].store(value);
        Ok(())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Apply a coalesced parameter snapshot at a block boundary.
    /// Realtime-safe: no allocation. Snapshot is ignored unless its
    /// generation matches this engine (stale-patch guard).
    pub fn apply_snapshot(&self, snap: &ParamSnapshot) -> bool {
        if snap.generation != self.generation {
            return false;
        }
        for v in &snap.values {
            if let Some(slot) = self.params.get(v.slot) {
                slot.store(v.value);
            }
        }
        true
    }

    pub fn param_slot(&self, id: NodeId, name: &str) -> Option<usize> {
        self.param_index
            .iter()
            .find(|(nid, n, _)| *nid == id && *n == name)
            .map(|(_, _, i)| *i)
    }

    /// Realtime-safe: no allocation. `frames` must be <= max_block.
    /// Events are consumed frame-ordered; each targets one gate node.
    pub fn process_with_events(
        &self,
        state: &mut ProcessState,
        frames: usize,
        out: &mut [f32],
        events: &mut event::EventWindow<'_>,
    ) {
        let frames = frames.min(self.max_block).min(out.len());
        if frames == 0 {
            return;
        }
        let sr = self.sample_rate as f32;
        let nc = state.node_count;
        let mb = state.max_block;

        for buf in state.buffers.chunks_mut(mb).take(nc) {
            buf[..frames].fill(0.0);
        }

        // Collect per-gate target changes for this block:
        // changes[slot] = list of (frame, level) in event order.
        // Fixed capacity: MAX_GATES gates x MAX_GATE_EVENTS changes each.
        let mut changes = [[(0u16, 0.0f32); MAX_GATE_EVENTS]; MAX_GATES];
        let mut n_changes = [0usize; MAX_GATES];
        while let Some(ev) = events.next_pending() {
            let Some(slot) = self
                .gate_nodes
                .iter()
                .position(|g| g.0 == ev.node)
                .filter(|slot| *slot < MAX_GATES)
            else {
                continue;
            };
            if n_changes[slot] >= MAX_GATE_EVENTS {
                continue;
            }
            use crate::event::EventKind;
            let level = match ev.kind {
                EventKind::NoteOn { velocity, .. } => (velocity as f32 / 127.0).clamp(0.0, 1.0),
                EventKind::NoteOff { .. } => 0.0,
                _ => continue,
            };
            let frame = (ev.frame as usize).min(frames.saturating_sub(1)) as u16;
            changes[slot][n_changes[slot]] = (frame, level);
            n_changes[slot] += 1;
        }
        events.rewind();

        for &idx in &self.order {
            let node = &self.nodes[idx];
            match node.kind {
                NodeKind::Oscillator => {
                    let freq = node.freq.map(|i| self.params[i].load()).unwrap_or(440.0);
                    let amp = node.amp.map(|i| self.params[i].load()).unwrap_or(0.2);
                    let start = idx * mb;
                    let buf = &mut state.buffers[start..start + frames];
                    dsp::process_osc(&mut state.states[idx], freq, amp, sr, buf);
                }
                NodeKind::Gain => {
                    mix_inputs(self, state, idx, frames);
                    let g = node.gain.map(|i| self.params[i].load()).unwrap_or(1.0);
                    let start = idx * mb;
                    let slice = &mut state.buffers[start..start + frames];
                    for s in slice.iter_mut() {
                        *s *= g;
                    }
                }
                NodeKind::Gate => {
                    mix_inputs(self, state, idx, frames);
                    let start = idx * mb;
                    let slice = &mut state.buffers[start..start + frames];
                    if let Some(slot) = self
                        .gate_nodes
                        .iter()
                        .position(|g| g.1 == idx)
                        .filter(|slot| *slot < MAX_GATES)
                    {
                        let NodeState::Gate {
                            attack, release, ..
                        } = &state.states[idx]
                        else {
                            unreachable!("gate slot must hold Gate state")
                        };
                        let a_coef = 1.0 - (-1.0 / (attack.max(0.0005) * sr)).exp();
                        let r_coef = 1.0 - (-1.0 / (release.max(0.0005) * sr)).exp();
                        dsp::process_gate_events(
                            &mut state.states[idx],
                            &changes[slot][..n_changes[slot]],
                            a_coef,
                            r_coef,
                            slice,
                        );
                    }
                }
                NodeKind::Mixer | NodeKind::Output => {
                    mix_inputs(self, state, idx, frames);
                }
            }
        }

        let start = self.output_index * mb;
        out[..frames].copy_from_slice(&state.buffers[start..start + frames]);
    }

    /// Compatibility wrapper without events.
    pub fn process(&self, state: &mut ProcessState, frames: usize, out: &mut [f32]) {
        let mut no_events = event::EventWindow::empty();
        self.process_with_events(state, frames, out, &mut no_events);
    }
}

fn mix_inputs(g: &CompiledGraph, state: &mut ProcessState, dst: usize, frames: usize) {
    let mb = state.max_block;
    let dst_start = dst * mb;
    for e in &g.edges {
        if e.dst != dst {
            continue;
        }
        let src_start = e.src * mb;
        for i in 0..frames {
            state.buffers[dst_start + i] += state.buffers[src_start + i];
        }
    }
}

fn push_param(
    params: &mut Vec<ParamSlot>,
    index: &mut Vec<(NodeId, &'static str, usize)>,
    id: NodeId,
    name: &'static str,
    v: f32,
) -> usize {
    let i = params.len();
    params.push(ParamSlot::new(v));
    index.push((id, name, i));
    i
}

fn lookup(map: &[(u32, usize)], id: u32) -> Result<usize, GraphError> {
    map.iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
        .ok_or_else(|| GraphError::UnknownNode(format!("{id}")))
}
