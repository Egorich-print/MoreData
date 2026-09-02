use crate::graph::NodeKind;
use std::f32::consts::TAU;

pub const MAX_BLOCK: usize = 64;

#[derive(Debug, Clone)]
pub enum NodeState {
    Osc {
        phase: f32,
    },
    Gain,
    Mixer,
    Output,
    Gate {
        level: f32,
        attack: f32,
        release: f32,
    },
}

impl NodeState {
    pub fn new(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Oscillator => NodeState::Osc { phase: 0.0 },
            NodeKind::Gain => NodeState::Gain,
            NodeKind::Mixer => NodeState::Mixer,
            NodeKind::Output => NodeState::Output,
            NodeKind::Gate => NodeState::Gate {
                level: 0.0,
                attack: 0.005,
                release: 0.08,
            },
        }
    }
}

pub fn process_osc(state: &mut NodeState, freq: f32, amp: f32, sr: f32, out: &mut [f32]) {
    let NodeState::Osc { phase } = state else {
        return;
    };
    let inc = freq / sr;
    for s in out.iter_mut() {
        *s = amp * (*phase * TAU).sin();
        *phase += inc;
        if *phase >= 1.0 {
            *phase -= 1.0;
        }
    }
}

/// Gate with frame-ordered level changes inside the block. `changes` must
/// be sorted by frame ascending (guaranteed by the event dispatcher).
/// The envelope approaches `changes[k].1` from the moment its frame is
/// reached. Realtime-safe: no allocation.
pub fn process_gate_events(
    state: &mut NodeState,
    changes: &[(u16, f32)],
    a_coef: f32,
    r_coef: f32,
    out: &mut [f32],
) {
    let NodeState::Gate { level, .. } = state else {
        return;
    };
    let mut ci = 0usize;
    let mut target = 0.0f32;
    for (i, s) in out.iter_mut().enumerate() {
        while ci < changes.len() && changes[ci].0 <= i as u16 {
            target = changes[ci].1;
            ci += 1;
        }
        let coef = if target >= *level { a_coef } else { r_coef };
        *level += (target - *level) * coef;
        *s *= *level;
    }
}
