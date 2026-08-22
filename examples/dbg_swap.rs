fn main() {
    let mut g = moredata_core::Graph::new(48_000).unwrap();
    let osc = g.add_node("osc", moredata_core::NodeKind::Oscillator).unwrap();
    let out = g.add_node("out", moredata_core::NodeKind::Output).unwrap();
    g.set_param(osc, "freq", 440.0).unwrap();
    g.set_param(osc, "amp", 0.5).unwrap();
    g.connect(osc, "out", out, "in").unwrap();
    let loud = {
        let (cg, st) = moredata_core::CompiledGraph::compile(&g, Default::default()).unwrap();
        moredata_runtime::Runtime::new(cg, st, "t")
    };
    g.set_param(osc, "amp", 0.0).unwrap();
    let silent = {
        let (cg, st) = moredata_core::CompiledGraph::compile(&g, Default::default()).unwrap();
        moredata_runtime::Runtime::new(cg, st, "t")
    };
    let (ctrl, mut rt) = moredata_runtime::link::channel(loud);
    let mut buf = [0.0f32; 64];
    rt.process(&mut buf);
    println!("loud energy: {}", buf.iter().map(|x: &f32| x*x).sum::<f32>());
    ctrl.publish(silent);
    rt.process(&mut buf);
    println!("silent energy: {}", buf.iter().map(|x: &f32| x*x).sum::<f32>());
    println!("first samples after swap: {:?}", &buf[..8]);
}
