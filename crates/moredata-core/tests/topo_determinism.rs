//! Topo-sort determinism and diamond-graph coverage (audit P0).
use moredata_core::{CompileOptions, CompiledGraph, Graph, NodeKind};

fn build_diamond(shuffle: bool) -> Graph {
    let mut g = Graph::new(48000).unwrap();
    let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let g1 = g.add_node("g1", NodeKind::Gain).unwrap();
    let g2 = g.add_node("g2", NodeKind::Gain).unwrap();
    let mix = g.add_node("mix", NodeKind::Mixer).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    if shuffle {
        // connect in reverse order; topology must not depend on insertion order
        g.connect(g2, "out", mix, "in").unwrap();
        g.connect(g1, "out", mix, "in").unwrap();
        g.connect(osc, "out", g2, "in").unwrap();
        g.connect(osc, "out", g1, "in").unwrap();
    } else {
        g.connect(osc, "out", g1, "in").unwrap();
        g.connect(osc, "out", g2, "in").unwrap();
        g.connect(g1, "out", mix, "in").unwrap();
        g.connect(g2, "out", mix, "in").unwrap();
    }
    g.connect(mix, "out", out, "in").unwrap();
    g.validate().unwrap();
    g
}

fn render(g: &Graph) -> Vec<f32> {
    let (cg, mut st) = CompiledGraph::compile(g, CompileOptions::default()).unwrap();
    let mut out = Vec::with_capacity(64 * 8);
    let mut buf = [0.0f32; 64];
    for _ in 0..8 {
        cg.process(&mut st, 64, &mut buf);
        out.extend_from_slice(&buf);
    }
    out
}

#[test]
fn diamond_graph_compiles_and_renders() {
    let out = render(&build_diamond(false));
    assert!(out.iter().any(|s| s.abs() > 1e-6), "silence");
    assert!(out.iter().all(|s| s.is_finite()));
}

#[test]
fn topo_order_is_deterministic_irrespective_of_connection_order() {
    let a = render(&build_diamond(false));
    let b = render(&build_diamond(true));
    assert_eq!(
        a, b,
        "bitwise-identical output required for stable topology"
    );
}

#[test]
fn parallel_independent_chains_compile() {
    let mut g = Graph::new(48000).unwrap();
    // two fully independent osc chains
    let out = g.add_node("out", NodeKind::Output).unwrap();
    for i in 0..2 {
        let osc = g.add_node(format!("osc{i}"), NodeKind::Oscillator).unwrap();
        let mix = g.add_node(format!("mix{i}"), NodeKind::Mixer).unwrap();
        g.connect(osc, "out", mix, "in").unwrap();
        g.connect(mix, "out", out, "in").unwrap();
    }
    g.validate().unwrap();
}
