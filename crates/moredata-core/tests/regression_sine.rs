use moredata_core::{CompileOptions, CompiledGraph, Project};

const PROJECT: &str = include_str!("../../../tests/fixtures/sine.mdproject");

#[test]
fn fixture_renders_stable_peak() {
    let g = Project::from_json(PROJECT).unwrap().to_graph().unwrap();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let mut peak = 0.0f32;
    let mut buf = [0.0f32; 64];
    for _ in 0..100 {
        cg.process(&mut st, 64, &mut buf);
        for s in buf {
            peak = peak.max(s.abs());
        }
    }
    assert!(peak > 0.2 && peak < 0.26, "peak={peak}");
}
