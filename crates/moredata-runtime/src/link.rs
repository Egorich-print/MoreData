//! Lock-free realtime/control handoff (M5.1).
//!
//! Single-producer/single-consumer mailbox with three states
//! (EMPTY / FULL / EXCLUSIVE). Producers never block the audio thread:
//! `push` is non-blocking, `pop` is non-blocking. The control plane may
//! spin-wait briefly (bounded by one audio block) — the RT thread never does.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, Ordering};

const EMPTY: u8 = 0;
const FULL: u8 = 1;
const EXCLUSIVE: u8 = 2;

pub struct Mailbox<T> {
    state: AtomicU8,
    value: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send> Sync for Mailbox<T> {}
unsafe impl<T: Send> Send for Mailbox<T> {}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Mailbox<T> {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(EMPTY),
            value: UnsafeCell::new(None),
        }
    }

    /// Producer side. Never blocks. Returns the value back if occupied.
    pub fn push(&self, v: T) -> Result<(), T> {
        match self
            .state
            .compare_exchange(EMPTY, EXCLUSIVE, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                unsafe { *self.value.get() = Some(v) };
                self.state.store(FULL, Ordering::Release);
                Ok(())
            }
            Err(_) => Err(v),
        }
    }

    /// Consumer side. Never blocks.
    pub fn pop(&self) -> Option<T> {
        match self
            .state
            .compare_exchange(FULL, EXCLUSIVE, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                let v = unsafe { (*self.value.get()).take() };
                self.state.store(EMPTY, Ordering::Release);
                v
            }
            Err(_) => None,
        }
    }
}

/// Control-plane half: publishes engines, collects retired ones.
pub struct ControlLink {
    inbox: std::sync::Arc<Mailbox<Runtime>>,
    retire: std::sync::Arc<Mailbox<Runtime>>,
}

/// Realtime-thread half: owns the active engine. All methods are
/// allocation-free and non-blocking.
pub struct RtLink {
    inbox: std::sync::Arc<Mailbox<Runtime>>,
    retire: std::sync::Arc<Mailbox<Runtime>>,
    current: Option<Runtime>,
}

pub fn channel(initial: Runtime) -> (ControlLink, RtLink) {
    let inbox = std::sync::Arc::new(Mailbox::new());
    let retire = std::sync::Arc::new(Mailbox::new());
    (
        ControlLink {
            inbox: inbox.clone(),
            retire: retire.clone(),
        },
        RtLink {
            inbox,
            retire,
            current: Some(initial),
        },
    )
}

impl ControlLink {
    /// Swap the running engine. Spins at most until the RT thread retires
    /// the previous engine (bounded by one audio block).
    pub fn publish(&self, rt: Runtime) {
        let mut rt = rt;
        loop {
            match self.inbox.push(rt) {
                Ok(()) => break,
                Err(back) => {
                    rt = back;
                    std::hint::spin_loop();
                    std::thread::yield_now();
                }
            }
        }
        if let Some(old) = self.retire.pop() {
            drop(old);
        }
    }

    /// Take a retired engine from the RT thread, if any.
    pub fn poll_retired(&self) -> Option<Runtime> {
        self.retire.pop()
    }
}

impl RtLink {
    /// Drain pending engine swaps. Called at block boundaries.
    fn refresh(&mut self) {
        if let Some(next) = self.inbox.pop()
            && let Some(old) = self.current.replace(next)
        {
            drop(self.retire.push(old));
        }
    }

    /// Realtime callback: refresh once, then process. Silence if no engine.
    pub fn process(&mut self, out: &mut [f32]) {
        self.refresh();
        match &mut self.current {
            Some(rt) => rt.process(out),
            None => out.fill(0.0),
        }
    }

    pub fn has_engine(&self) -> bool {
        self.current.is_some()
    }
}

use crate::Runtime;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_roundtrip() {
        let m: Mailbox<u64> = Mailbox::new();
        assert_eq!(m.pop(), None);
        m.push(7).unwrap();
        assert!(m.push(8).is_err());
        assert_eq!(m.pop(), Some(7));
        assert_eq!(m.pop(), None);
    }

    #[test]
    fn mailbox_spsc_threaded() {
        let m = std::sync::Arc::new(Mailbox::new());
        let n = 10_000_u64;
        let producer = {
            let m = m.clone();
            std::thread::spawn(move || {
                for i in 0..n {
                    let mut v = i;
                    loop {
                        match m.push(v) {
                            Ok(()) => break,
                            Err(back) => v = back,
                        }
                        std::hint::spin_loop();
                    }
                }
            })
        };
        let mut got = 0_u64;
        while got < n {
            if let Some(_v) = m.pop() {
                got += 1;
            }
            std::hint::spin_loop();
        }
        producer.join().unwrap();
        assert_eq!(got, n);
    }

    #[test]
    fn engine_hot_swap_without_mutex() {
        let mut g = moredata_core::Graph::new(48_000).unwrap();
        let osc = g
            .add_node("osc", moredata_core::NodeKind::Oscillator)
            .unwrap();
        let out = g.add_node("out", moredata_core::NodeKind::Output).unwrap();
        g.set_param(osc, "freq", 440.0).unwrap();
        g.set_param(osc, "amp", 0.5).unwrap();
        g.connect(osc, "out", out, "in").unwrap();

        let loud = {
            let (cg, st) = moredata_core::CompiledGraph::compile(&g, Default::default()).unwrap();
            Runtime::new(cg, st, "test")
        };
        g.set_param(osc, "amp", 0.0).unwrap();
        let silent = {
            let (cg, st) = moredata_core::CompiledGraph::compile(&g, Default::default()).unwrap();
            Runtime::new(cg, st, "test")
        };

        let (ctrl, mut rt) = channel(loud);
        let mut buf = [0.0f32; 64];

        rt.process(&mut buf);
        let energy_loud: f32 = buf.iter().map(|x| x * x).sum();
        assert!(energy_loud > 0.1);

        ctrl.publish(silent);
        rt.process(&mut buf);
        let energy_silent: f32 = buf.iter().map(|x| x * x).sum();
        assert_eq!(energy_silent, 0.0);

        assert!(ctrl.poll_retired().is_some());
        assert!(rt.has_engine());
    }
}
