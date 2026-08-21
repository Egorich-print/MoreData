use crate::CompiledGraph;
use crate::GraphError;
use crate::compile::CompileOptions;
use crate::graph::{Graph, NodeKind};
use crate::project::Project;

fn sine_graph() -> Graph {
    let mut g = Graph::new(48_000).unwrap();
    let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    g.set_param(osc, "freq", 440.0).unwrap();
    g.set_param(osc, "amp", 0.5).unwrap();
    g.connect(osc, "out", out, "in").unwrap();
    g
}

#[test]
fn construct_and_validate() {
    let g = sine_graph();
    g.validate().unwrap();
    assert_eq!(g.nodes().len(), 2);
}

#[test]
fn duplicate_node() {
    let mut g = Graph::new(48_000).unwrap();
    g.add_node("osc", NodeKind::Oscillator).unwrap();
    let e = g.add_node("osc", NodeKind::Oscillator).unwrap_err();
    assert!(matches!(e, GraphError::DuplicateNode(_)));
}

#[test]
fn invalid_connection_direction() {
    let mut g = Graph::new(48_000).unwrap();
    let a = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let b = g.add_node("gain", NodeKind::Gain).unwrap();
    let e = g.connect(a, "out", b, "out").unwrap_err();
    assert!(matches!(e, GraphError::DirectionMismatch { .. }));
}

#[test]
fn unknown_port() {
    let mut g = Graph::new(48_000).unwrap();
    let a = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let b = g.add_node("out", NodeKind::Output).unwrap();
    let e = g.connect(a, "nope", b, "in").unwrap_err();
    assert!(matches!(e, GraphError::UnknownPort { .. }));
}

#[test]
fn param_range() {
    let mut g = Graph::new(48_000).unwrap();
    let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
    assert!(g.set_param(osc, "freq", 99_000.0).is_err());
    assert!(g.set_param(osc, "wet", 0.1).is_err());
}

#[test]
fn cycle_rejected() {
    let mut g = Graph::new(48_000).unwrap();
    let a = g.add_node("g1", NodeKind::Gain).unwrap();
    let b = g.add_node("g2", NodeKind::Gain).unwrap();
    let _ = g.add_node("out", NodeKind::Output).unwrap();
    g.connect(a, "out", b, "in").unwrap();
    g.connect(b, "out", a, "in").unwrap();
    assert!(matches!(g.validate(), Err(GraphError::Cycle(_))));
}

#[test]
fn no_output() {
    let mut g = Graph::new(48_000).unwrap();
    g.add_node("osc", NodeKind::Oscillator).unwrap();
    assert!(matches!(g.validate(), Err(GraphError::NoOutput)));
}

#[test]
fn process_sine_nonzero() {
    let g = sine_graph();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let mut buf = [0.0f32; 64];
    cg.process(&mut st, 64, &mut buf);
    let energy: f32 = buf.iter().map(|x| x * x).sum();
    assert!(energy > 0.1, "energy={energy}");
    assert!(buf.iter().all(|x| x.abs() <= 0.51));
}

#[test]
fn gain_scales() {
    let mut g = Graph::new(48_000).unwrap();
    let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let gain = g.add_node("gain", NodeKind::Gain).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    g.set_param(osc, "amp", 0.5).unwrap();
    g.set_param(gain, "gain", 0.5).unwrap();
    g.connect(osc, "out", gain, "in").unwrap();
    g.connect(gain, "out", out, "in").unwrap();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let mut buf = [0.0f32; 64];
    cg.process(&mut st, 64, &mut buf);
    assert!(buf.iter().all(|x| x.abs() <= 0.26));
}

#[test]
fn mixer_sums() {
    let mut g = Graph::new(48_000).unwrap();
    let a = g.add_node("a", NodeKind::Oscillator).unwrap();
    let b = g.add_node("b", NodeKind::Oscillator).unwrap();
    let mix = g.add_node("mix", NodeKind::Mixer).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    g.set_param(a, "amp", 0.2).unwrap();
    g.set_param(b, "amp", 0.2).unwrap();
    g.set_param(b, "freq", 880.0).unwrap();
    g.connect(a, "out", mix, "in").unwrap();
    g.connect(b, "out", mix, "in").unwrap();
    g.connect(mix, "out", out, "in").unwrap();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let mut buf = [0.0f32; 64];
    cg.process(&mut st, 64, &mut buf);
    let energy: f32 = buf.iter().map(|x| x * x).sum();
    assert!(energy > 0.05);
}

#[test]
fn project_roundtrip() {
    let g = sine_graph();
    let json = serde_json::to_string_pretty(&g.to_project()).unwrap();
    let p = Project::from_json(&json).unwrap();
    let g2 = p.to_graph().unwrap();
    assert_eq!(g2.nodes().len(), 2);
}

#[test]
fn rt_param_update() {
    let g = sine_graph();
    let osc = g.node_by_name("osc").unwrap().id;
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    cg.set_param(osc, "amp", 0.0).unwrap();
    let mut buf = [0.0f32; 32];
    cg.process(&mut st, 32, &mut buf);
    assert!(buf.iter().all(|x| *x == 0.0));
}

#[test]
fn bad_sample_rate() {
    assert!(Graph::new(0).is_err());
}
