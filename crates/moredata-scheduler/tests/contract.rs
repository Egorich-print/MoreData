//! M5.4 scheduler contract tests.
//!
//! 1. `parallel_partitions` produces a valid partition plan.
//! 2. Serial and parallel process produce identical output.
//! 3. Scheduler `run_block` performs zero heap allocations.
#![deny(unsafe_op_in_unsafe_fn)]

use moredata_core::{CompileOptions, CompiledGraph, Event, EventKind, Graph, NodeId, NodeKind};
use moredata_scheduler::{Plan, Scheduler};

static TRACKING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
thread_local! {
    static LOCAL_ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct CountAlloc;

unsafe impl std::alloc::GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if TRACKING.load(std::sync::atomic::Ordering::Relaxed) > 0 {
            LOCAL_ALLOCS.with(|c| c.set(c.get() + 1));
        }
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountAlloc = CountAlloc;

fn make_wide_graph() -> Graph {
    let mut g = Graph::new(48_000).unwrap();
    // 4 independent oscillators mixed through a single mixer → out.
    let mut prev = Vec::new();
    for i in 0..4 {
        let id = g
            .add_node(format!("osc{i}"), NodeKind::Oscillator)
            .unwrap();
        g.set_param(id, "freq", 220.0 + (i as f32) * 110.0).unwrap();
        g.set_param(id, "amp", 0.1).unwrap();
        prev.push(id);
    }
    let mix = g.add_node("mix", NodeKind::Mixer).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    for id in prev {
        g.connect(id, "out", mix, "in").unwrap();
    }
    g.connect(mix, "out", out, "in").unwrap();
    g
}

#[test]
fn partitions_are_level_correct() {
    let g = make_wide_graph();
    let (cg, _st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let plan = Plan::from_graph(&cg);
    let parts = plan.partitions();
    // 4 oscillators share no deps, so the first partition has 4 nodes;
    // the mixer is at level 2; the output is at level 3.
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].len(), 4);
    assert_eq!(parts[1].len(), 1);
    assert_eq!(parts[2].len(), 1);
}

#[test]
fn scheduler_run_block_matches_serial() {
    let g = make_wide_graph();
    let (cg_a, mut st_a) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let (cg_b, mut st_b) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let plan = Plan::from_graph(&cg_a);
    let sched = Scheduler::new(4, plan);

    let mut buf_serial = [0f32; 64];
    let mut buf_sched = [0f32; 64];
    let mut window = moredata_core::event::EventWindow::empty();

    cg_a.process(&mut st_a, 64, &mut buf_serial);
    sched.run_block(&cg_b, &mut st_b, 64, &mut buf_sched, &mut window);

    for (a, b) in buf_serial.iter().zip(buf_sched.iter()) {
        let diff = (a - b).abs();
        assert!(diff < 1e-5, "serial={a} sched={b} diff={diff}");
    }
}

#[test]
fn scheduler_process_with_events_dispatches_gates() {
    let mut g = Graph::new(48_000).unwrap();
    let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let gate = g.add_node("gate", NodeKind::Gate).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    g.set_param(osc, "amp", 0.5).unwrap();
    g.connect(osc, "out", gate, "in").unwrap();
    g.connect(gate, "out", out, "in").unwrap();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let plan = Plan::from_graph(&cg);
    let sched = Scheduler::new(2, plan);

    let mut buf = [0f32; 64];
    let mut window = moredata_core::event::EventWindow::empty();
    window.push(Event::note_on(0, gate, 60, 127));
    window.prepare();

    sched.run_block(&cg, &mut st, 64, &mut buf, &mut window);
    // Envelope should now have non-zero output (after attack settle).
    sched.run_block(&cg, &mut st, 64, &mut buf, &mut window);
    let energy: f32 = buf.iter().map(|x| x * x).sum();
    assert!(energy > 0.1, "scheduler gate did not open, energy={energy}");
}

#[test]
fn scheduler_run_block_zero_allocations() {
    let g = make_wide_graph();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let plan = Plan::from_graph(&cg);
    let sched = Scheduler::new(4, plan);

    let mut buf = [0f32; 64];
    let mut window = moredata_core::event::EventWindow::empty();

    for _ in 0..8 {
        sched.run_block(&cg, &mut st, 64, &mut buf, &mut window);
    }

    TRACKING.store(1, std::sync::atomic::Ordering::SeqCst);
    LOCAL_ALLOCS.with(|c| c.set(0));
    for _ in 0..256 {
        sched.run_block(&cg, &mut st, 64, &mut buf, &mut window);
    }
    let n = LOCAL_ALLOCS.with(|c| c.get());
    TRACKING.store(0, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(n, 0, "scheduler allocated {n} times in process path");
}

#[test]
fn scheduler_stress_bounded_block_time() {
    let g = make_wide_graph();
    let (cg, mut st) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let plan = Plan::from_graph(&cg);
    let sched = Scheduler::new(4, plan);

    let mut buf = [0f32; 64];
    let mut window = moredata_core::event::EventWindow::empty();
    let mut max_ns = 0u64;
    for _ in 0..5_000 {
        let t = std::time::Instant::now();
        sched.run_block(&cg, &mut st, 64, &mut buf, &mut window);
        max_ns = max_ns.max(t.elapsed().as_nanos() as u64);
    }
    // 4-node graph, 4 workers: budget is the same as serial.
    assert!(max_ns < 500_000, "scheduler block spike {max_ns}ns");
    let _ = (EventKind::Trigger, NodeId(0));
}
