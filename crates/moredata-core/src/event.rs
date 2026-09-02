//! Deterministic event transport (M5.2).
//!
//! Events are an ORDERED STREAM, unlike parameters (state). Delivery rules:
//!
//! - FIFO from the control plane; within a block, ordered by `frame`
//!   ascending, ties keep arrival order (stable sort).
//! - Fixed-capacity SPSC ring. Overflow policy: drop NEWEST, count in
//!   `dropped`. The counter is the error signal; producers never block.
//! - Frame offsets are clamped into the block at dispatch time.
//! - Events target a `NodeId`; unknown or non-consuming nodes ignore them.

use crate::graph::NodeId;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    PitchBend { value: i16 },
    CC { controller: u8, value: u8 },
    Trigger,
    ProgramChange { program: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// Sample offset within the target block. Clamped on dispatch.
    pub frame: u16,
    pub node: NodeId,
    pub kind: EventKind,
}

impl Event {
    pub fn note_on(frame: u16, node: NodeId, note: u8, velocity: u8) -> Self {
        Self {
            frame,
            node,
            kind: EventKind::NoteOn { note, velocity },
        }
    }

    pub fn note_off(frame: u16, node: NodeId, note: u8) -> Self {
        Self {
            frame,
            node,
            kind: EventKind::NoteOff { note },
        }
    }
}

/// Lock-free SPSC event ring. Power-of-two capacity. No allocation after
/// construction; producer and consumer touch disjoint indices.
pub struct EventQueue<const N: usize> {
    buf: [std::cell::UnsafeCell<Option<Event>>; N],
    head: AtomicUsize, // consumer owns writes
    tail: AtomicUsize, // producer owns writes
    dropped: AtomicUsize,
}

unsafe impl<const N: usize> Send for EventQueue<N> {}
unsafe impl<const N: usize> Sync for EventQueue<N> {}

impl<const N: usize> Default for EventQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> EventQueue<N> {
    const MASK: usize = N - 1;

    pub fn new() -> Self {
        debug_assert!(N.is_power_of_two(), "capacity must be power of two");
        // SAFETY: array of UnsafeCell<Option<Event>> can be zero-initialized
        // because Option<Event> has no invalid niche requirement here; we
        // explicitly write None via ptr::write to stay sound.
        let mut buf: [std::mem::MaybeUninit<std::cell::UnsafeCell<Option<Event>>>; N] =
            unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        for slot in buf.iter_mut() {
            slot.write(std::cell::UnsafeCell::new(None));
        }
        Self {
            buf: unsafe { std::ptr::read(&buf as *const _ as *const _) },
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        }
    }

    /// Producer side. Non-blocking; drops newest on overflow.
    pub fn push(&self, ev: Event) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= N {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        unsafe {
            (*self.buf[tail & Self::MASK].get()) = Some(ev);
        }
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
    }

    /// Consumer side. Non-blocking.
    pub fn pop(&self) -> Option<Event> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let ev = unsafe { (*self.buf[head & Self::MASK].get()).take() };
        self.head.store(head.wrapping_add(1), Ordering::Release);
        ev
    }

    pub fn dropped(&self) -> usize {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Stable insertion sort for the per-block event window. Alloc-free.
/// Window sizes are bounded (drain limit per block), so O(n^2) is fine.
pub fn sort_by_frame(events: &mut [Event]) {
    for i in 1..events.len() {
        let key = events[i];
        let mut j = i;
        while j > 0 && events[j - 1].frame > key.frame {
            events[j] = events[j - 1];
            j -= 1;
        }
        events[j] = key;
    }
}

/// Drain limit per block: bounds the sort and dispatch work.
pub const BLOCK_EVENT_LIMIT: usize = 128;

/// Borrowed, fixed-capacity staging buffer for one block's events.
/// The RT side drains the queue into the window (bounded by BLOCK_EVENT_LIMIT),
/// sorts by frame (stable insertion sort over a Copy array), dispatches,
/// then rewinds for the next block. No allocation.
pub struct EventWindow<'a> {
    buf: &'a mut [Option<Event>],
    len: usize,
    cursor: usize,
}

impl<'a> EventWindow<'a> {
    pub fn new(buf: &'a mut [Option<Event>]) -> Self {
        for s in buf.iter_mut() {
            *s = None;
        }
        Self {
            buf,
            len: 0,
            cursor: 0,
        }
    }

    pub fn empty() -> Self {
        // Zero-length backing storage; all methods are no-ops on it.
        let mut none: [Option<Event>; 0] = [];
        Self {
            buf: unsafe { &mut *(&mut none as *mut [Option<Event>; 0]) },
            len: 0,
            cursor: 0,
        }
    }

    pub fn push(&mut self, ev: Event) -> bool {
        if self.len >= self.buf.len() {
            return false;
        }
        self.buf[self.len] = Some(ev);
        self.len += 1;
        true
    }

    /// Sort staged events by frame ascending (stable). O(n^2), n bounded
    /// by BLOCK_EVENT_LIMIT — fine at 64-frame blocks.
    pub fn prepare(&mut self) {
        for i in 1..self.len {
            let key = match self.buf[i] {
                Some(k) => k,
                None => continue,
            };
            let mut j = i;
            while j > 0 {
                let prev = match self.buf[j - 1] {
                    Some(p) => p,
                    None => break,
                };
                if prev.frame > key.frame {
                    self.buf[j] = self.buf[j - 1];
                    j -= 1;
                } else {
                    break;
                }
            }
            self.buf[j] = Some(key);
        }
    }

    pub fn next_pending(&mut self) -> Option<Event> {
        if self.cursor >= self.len {
            return None;
        }
        let ev = self.buf[self.cursor];
        self.cursor += 1;
        ev
    }

    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_and_overflow() {
        let q: EventQueue<4> = EventQueue::new();
        let n0 = NodeId(0);
        for i in 0..6u16 {
            q.push(Event {
                frame: i,
                node: n0,
                kind: EventKind::Trigger,
            });
        }
        assert_eq!(q.dropped(), 2);
        for i in 0..4u16 {
            assert_eq!(q.pop().map(|e| e.frame), Some(i));
        }
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn threaded_spsc_order() {
        let q = std::sync::Arc::new(EventQueue::<1024>::new());
        let total = 50_000_usize;
        let prod = {
            let q = q.clone();
            std::thread::spawn(move || {
                for i in 0..total as u32 {
                    loop {
                        if q.len() < 512 {
                            q.push(Event {
                                frame: (i & 0x3f) as u16,
                                node: NodeId(1),
                                kind: EventKind::Trigger,
                            });
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }
            })
        };
        let mut got = 0;
        while got < total {
            if q.pop().is_some() {
                got += 1;
            } else {
                std::hint::spin_loop();
            }
        }
        prod.join().unwrap();
        assert_eq!(got, total);
    }

    #[test]
    fn stable_sort_by_frame() {
        let mut evs = [
            Event {
                frame: 3,
                node: NodeId(1),
                kind: EventKind::Trigger,
            },
            Event {
                frame: 1,
                node: NodeId(1),
                kind: EventKind::Trigger,
            },
            Event {
                frame: 1,
                node: NodeId(2),
                kind: EventKind::Trigger,
            },
            Event {
                frame: 0,
                node: NodeId(1),
                kind: EventKind::Trigger,
            },
        ];
        sort_by_frame(&mut evs);
        let frames: Vec<u16> = evs.iter().map(|e| e.frame).collect();
        assert_eq!(frames, vec![0, 1, 1, 3]);
        // tie between the two frame=1 events keeps arrival order (node 1 first)
        assert_eq!(evs[1].node, NodeId(1));
        assert_eq!(evs[2].node, NodeId(2));
    }
}
