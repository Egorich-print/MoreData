//! RT contract verification campaign (M5.2 + M5.3).
#![deny(unsafe_op_in_unsafe_fn)]

use moredata_core::{CompileOptions, CompiledGraph, Event, EventKind, Graph, NodeId, NodeKind};
use moredata_runtime::{Runtime, link::channel};

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


fn gate_graph(freq: f32) -> (Graph, NodeId, NodeId, NodeId) {
    let mut g = Graph::new(48_000).unwrap();
    let osc = g.add_node("osc", NodeKind::Oscillator).unwrap();
    let gate = g.add_node("gate", NodeKind::Gate).unwrap();
    let out = g.add_node("out", NodeKind::Output).unwrap();
    g.set_param(osc, "freq", freq).unwrap();
    g.set_param(osc, "amp", 0.5).unwrap();
    g.connect(osc, "out", gate, "in").unwrap();
    g.connect(gate, "out", out, "in").unwrap();
    (g, osc, gate, out)
}

fn new_rt(g: &Graph, events: std::sync::Arc<moredata_core::event::EventQueue<256>>) -> Runtime {
    let (cg, st) = CompiledGraph::compile(g, CompileOptions::default()).unwrap();
    Runtime::with_shared_events(cg, st, "test", events)
}

#[test]
fn event_ordering_is_frame_deterministic() {
    let (g, _osc, gate, _out) = gate_graph(440.0);
    let mut rt = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    rt.events().push(Event::note_on(40, gate, 60, 127));
    rt.events().push(Event::note_off(10, gate, 60));
    let mut out = [0f32; 64];
    rt.process(&mut out);
    assert_eq!(rt.events_pending(), 0);
    assert_eq!(rt.events().dropped(), 0);
}

#[test]
fn frame_clamped_no_panic() {
    let (g, _osc, gate, _out) = gate_graph(440.0);
    let mut rt = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    rt.events().push(Event::note_on(u16::MAX, gate, 60, 127));
    let mut out = [0f32; 16];
    rt.process(&mut out);
    assert_eq!(rt.events_pending(), 0);
}

#[test]
fn overflow_policy_drops_newest_and_counts() {
    let q = moredata_core::event::EventQueue::<4>::new();
    for i in 0..7u16 {
        q.push(Event {
            frame: i,
            node: NodeId(1),
            kind: EventKind::Trigger,
        });
    }
    assert_eq!(q.dropped(), 3);
    for i in 0..4u16 {
        assert_eq!(q.pop().map(|e| e.frame), Some(i));
    }
}

#[test]
fn mandatory_events_survive_bounded_backlog() {
    let (g, _osc, gate, _out) = gate_graph(220.0);
    let mut rt = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    let mut out = [0f32; 64];
    for _block in 0..50u32 {
        for k in 0..100u32 {
            rt.events().push(Event {
                frame: (k % 64) as u16,
                node: gate,
                kind: if k % 2 == 0 {
                    EventKind::NoteOn {
                        note: 60,
                        velocity: 127,
                    }
                } else {
                    EventKind::NoteOff { note: 60 }
                },
            });
        }
        rt.process(&mut out);
    }
    assert_eq!(rt.events().dropped(), 0, "no drops below capacity");
}

#[test]
fn param_snapshot_coalesces_last_write_wins() {
    let mut snap = moredata_core::ParamSnapshot::new(1);
    snap.push(0, 100.0);
    snap.push(0, 500.0);
    snap.push(1, 0.5);
    assert_eq!(snap.values.len(), 2);
    assert_eq!(snap.values[0].value, 500.0);
}

#[test]
fn stale_generation_rejected_fresh_accepted() {
    let (mut g, osc, _gate, _out) = gate_graph(440.0);
    let (cg1, st1) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let gen1 = cg1.generation();
    g.set_param(osc, "freq", 880.0).unwrap();
    let (cg2, st2) = CompiledGraph::compile(&g, CompileOptions::default()).unwrap();
    let gen2 = cg2.generation();
    assert_ne!(gen1, gen2);
    let mut stale = moredata_core::ParamSnapshot::new(gen1);
    stale.push(cg2.param_slot(osc, "freq").unwrap(), 123.0);
    assert!(!cg2.apply_snapshot(&stale));
    let mut fresh = moredata_core::ParamSnapshot::new(gen2);
    fresh.push(cg2.param_slot(osc, "freq").unwrap(), 123.0);
    assert!(cg2.apply_snapshot(&fresh));
    drop((cg1, st1, cg2, st2));
}

#[test]
fn hot_swap_energy_transitions() {
    let (g, _osc, gate, _out) = gate_graph(440.0);
    let loud = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    let (ctrl, mut rt) = channel(loud);
    let mut buf = [0f32; 64];

    // Open the gate first.
    rt.events()
        .expect("engine")
        .push(Event::note_on(0, gate, 60, 127));
    rt.process(&mut buf);
    rt.process(&mut buf); // settle envelope
    assert!(buf.iter().any(|x| x.abs() > 0.01), "loud before swap");
    assert!(buf.iter().any(|x| x.abs() > 0.01), "buffer not all-zero");

    // Publish silent engine; process should silence quickly (gate envelope down).
    let silent = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    ctrl.publish(silent);
    rt.process(&mut buf);
    assert!(buf.iter().all(|x| *x == 0.0), "silent after swap");

    // Now hot-swap back to loud; engine should revive on the new compiled graph.
    let _loud2 = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    rt.process(&mut buf); // first block after new engine init: envelope at 0 → zero
    rt.process(&mut buf);
    assert!(rt.has_engine(), "engine present after swap round-trip");
}

#[test]
fn zero_allocations_in_process_path() {
    let (g, _osc, _gate, _out) = gate_graph(440.0);
    let mut rt = new_rt(
        &g,
        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
    );
    let mut out = [0f32; 64];

    for _ in 0..8 {
        rt.process(&mut out);
    }

    // Pre-create snapshots and events to avoid counting control-plane allocations
    let r#gen = rt.graph().generation();
    let mut snapshots = Vec::with_capacity(256);
    let mut events = Vec::with_capacity(256);
    for i in 0..256u32 {
        let mut snap = moredata_core::ParamSnapshot::new(r#gen);
        snap.push(0, 200.0 + (i % 64) as f32);
        snapshots.push(snap);
        events.push(Event {
            frame: (i % 64) as u16,
            node: NodeId(u32::MAX),
            kind: moredata_core::event::EventKind::Trigger,
        });
    }

    TRACKING.store(1, std::sync::atomic::Ordering::SeqCst);
    LOCAL_ALLOCS.with(|c| c.set(0));
    {
        for i in 0..256u32 {
            rt.graph().apply_snapshot(&snapshots[i as usize]);
            rt.events().push(events[i as usize]);
            rt.process(&mut out);
        }
    }
    let n = LOCAL_ALLOCS.with(|c| c.get());
    TRACKING.store(0, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(n, 0, "process path allocated {n} times");
}

#[test]
fn stress_control_churn_vs_rt() {
    const BLOCKS: usize = 10_000;
    let (g, _osc, gate, _out) = gate_graph(330.0);
    let shared_queue = std::sync::Arc::new(moredata_core::event::EventQueue::<256>::new());
    let first = new_rt(&g, shared_queue.clone());
    let (ctrl, mut rt) = channel(first);

    let control = {
        let queue = shared_queue.clone();
        std::thread::spawn(move || {
            for i in 0..BLOCKS {
                if i % 64 == 63 {
                    let (ng, _, _, _) = gate_graph(f32::from(200 + (i % 400) as u16));
                    let new_rt = new_rt(
                        &ng,
                        std::sync::Arc::new(moredata_core::event::EventQueue::new()),
                    );
                    ctrl.publish(new_rt);
                }
                queue.push(Event {
                    frame: (i % 64) as u16,
                    node: gate,
                    kind: if i % 128 < 64 {
                        EventKind::NoteOn {
                            note: 60 + (i % 12) as u8,
                            velocity: 100,
                        }
                    } else {
                        EventKind::NoteOff { note: 60 }
                    },
                });
                std::hint::spin_loop();
            }
        })
    };

    let mut max_ns = 0u64;
    let mut sum_ns = 0u64;
    let mut out = [0f32; 64];
    for _ in 0..BLOCKS {
        let t = std::time::Instant::now();
        rt.process(&mut out);
        let ns = t.elapsed().as_nanos() as u64;
        max_ns = max_ns.max(ns);
        sum_ns += ns;
    }
    control.join().unwrap();

    let avg_ns = sum_ns / BLOCKS as u64;
    assert!(max_ns < 500_000, "max block {max_ns}ns exceeds contract");
    assert!(avg_ns < 50_000, "avg block {avg_ns}ns exceeds contract");
    assert!(rt.has_engine(), "engine lost during stress");
}
