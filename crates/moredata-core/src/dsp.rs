use crate::graph::NodeKind;
use std::f32::consts::TAU;

pub const MAX_BLOCK: usize = 64;

#[derive(Debug, Clone)]
pub enum NodeState {
    Osc { phase: f32 },
    Gain,
    Mixer,
    Output,
}

impl NodeState {
    pub fn new(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Oscillator => NodeState::Osc { phase: 0.0 },
            NodeKind::Gain => NodeState::Gain,
            NodeKind::Mixer => NodeState::Mixer,
            NodeKind::Output => NodeState::Output,
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
