//! Multithreaded realtime scheduler (M5.4 / M5.5).
//!
//! The scheduler executes a compiled graph across N worker threads in
//! parallel partitions (dependency levels). It preserves every invariant
//! of M5.2/M5.3:
//!
//! - Zero heap allocations in the audio path. Work descriptors live on
//!   the audio thread's stack; only an `AtomicPtr` is shared with the
//!   worker pool.
//! - No `Mutex` on the audio thread. Worker threads block on a
//!   `Condvar` while idle; the audio thread never waits on a lock.
//! - The worker set is fixed at scheduler construction. The control
//!   plane may replace the scheduler (it lives outside the audio
//!   thread) but the audio thread holds `&Scheduler` for the lifetime
//!   of a block.

use moredata_core::{CompiledGraph, EventWindow, ProcessState};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

const MAX_PARTITIONS: usize = 256;

/// A single worker. The set is fixed at scheduler construction; the
/// audio thread never resizes it.
pub struct Worker {
    #[allow(dead_code)]
    id: usize,
}

/// Pre-computed partition plan for a specific compiled graph.
pub struct Plan {
    partitions: Vec<Vec<usize>>,
}

impl Plan {
    pub fn from_graph(graph: &CompiledGraph) -> Self {
        let partitions = graph.parallel_partitions();
        Self { partitions }
    }

    pub fn partitions(&self) -> &[Vec<usize>] {
        &self.partitions
    }
}

/// Stack-allocated work descriptor. The audio thread owns one of
/// these per partition and hands a pointer to the workers through
/// an `AtomicPtr`. There is **no** heap allocation: the descriptor
/// sits on the audio thread's stack.
struct Work {
    graph: *const CompiledGraph,
    state: *mut ProcessState,
    partition: *const usize,
    partition_len: usize,
    frames: usize,
    /// Per-partition completion counter; the audio thread spins on
    /// this until it reaches zero.
    done: *const AtomicUsize,
    /// Atomic cursor into `partition`. Workers fetch-add to claim
    /// the next index. Reset to 0 by the audio thread before
    /// publishing.
    cursor: *const AtomicUsize,
}

// SAFETY: the audio thread publishes a `&Work` for the duration of
// a single partition and observes `done` before touching the
// descriptors again.
unsafe impl Send for Work {}

/// Worker pool control block. Workers wait on a Condvar and are
/// woken when the audio thread publishes a non-null work pointer.
struct PoolInner {
    /// Published work pointer; null when idle.
    slot: AtomicPtr<Work>,
    cvar: Condvar,
    /// Lock around `slot` only for the Condvar wait predicate.
    /// Audio thread uses `AtomicPtr` directly; the lock is here
    /// only to satisfy `Condvar::wait`.
    state: Mutex<()>,
    shutdown: AtomicBool,
}

/// Worker pool. One shared instance, owned by the `Scheduler`.
pub struct WorkerPool {
    inner: Arc<PoolInner>,
    handles: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    pub fn new(count: usize) -> Self {
        let count = count.clamp(1, 16);
        let inner = Arc::new(PoolInner {
            slot: AtomicPtr::new(std::ptr::null_mut()),
            cvar: Condvar::new(),
            state: Mutex::new(()),
            shutdown: AtomicBool::new(false),
        });
        let mut handles = Vec::with_capacity(count);
        for id in 0..count {
            let inner = inner.clone();
            let handle = thread::Builder::new()
                .name(format!("moredata-w{id}"))
                .spawn(move || worker_loop(id, inner))
                .expect("spawn worker");
            handles.push(handle);
        }
        Self { inner, handles }
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::Release);
        // Publish a sentinel non-null pointer so all workers wake
        // from the Condvar and observe `shutdown`.
        let sentinel: *mut Work = std::ptr::dangling_mut::<Work>();
        self.inner.slot.store(sentinel, Ordering::Release);
        self.inner.cvar.notify_all();
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

fn worker_loop(id: usize, inner: Arc<PoolInner>) {
    let _ = id;
    let mut guard = inner.state.lock().unwrap();
    loop {
        if inner.shutdown.load(Ordering::Acquire) {
            return;
        }
        let ptr = inner.slot.load(Ordering::Acquire);
        if ptr.is_null() {
            guard = inner.cvar.wait(guard).unwrap();
            continue;
        }
        // Drop the lock before running the partition so the audio
        // thread can republish a new pointer for the next partition.
        drop(guard);
        if inner.shutdown.load(Ordering::Acquire) {
            return;
        }
        // SAFETY: `ptr` is a stack pointer of the audio thread.
        // The audio thread waits on `done` (an AtomicUsize in
        // its own `done_counters` vector) before reusing the
        // descriptor, so the worker reads a valid value. Each
        // worker takes a different `idx` from the partition so
        // its `&mut` to `state.buffers[idx*mb..]` and
        // `state.states[idx]` does not alias another worker's.
        // We never read the same cell from two threads.
        unsafe {
            let work = &*ptr;
            let part = std::slice::from_raw_parts(work.partition, work.partition_len);
            let graph = &*work.graph;
            let state_ptr = work.state;
            let done = work.done;
            let cursor = work.cursor;
            let frames = work.frames;
            loop {
                let pos = (*cursor).fetch_add(1, Ordering::AcqRel);
                if pos >= part.len() {
                    break;
                }
                let idx = part[pos];
                run_leaf(graph, state_ptr, idx, frames);
            }
            (*done).fetch_sub(1, Ordering::AcqRel);
        }
        guard = inner.state.lock().unwrap();
    }
}

/// Process a single leaf node (oscillator, in the current set of
/// node kinds). Mutates only `state.buffers[idx*mb..]` and
/// `state.states[idx]`, so multiple workers can run this for
/// disjoint `idx` without aliasing.
unsafe fn run_leaf(graph: &CompiledGraph, state_ptr: *mut ProcessState, idx: usize, frames: usize) {
    use moredata_core::dsp;
    // SAFETY: the caller (worker_loop) ensured the pointer is live
    // for the duration of the partition and disjoint from other
    // workers' slices (each worker owns a distinct `idx`).
    let state = unsafe { &mut *state_ptr };
    let sr = graph.sample_rate as f32;
    let mb = state.max_block;
    let node = &graph.nodes[idx];
    match node.kind {
        moredata_core::NodeKind::Oscillator => {
            let freq = node.freq.map(|i| graph.params[i].load()).unwrap_or(440.0);
            let amp = node.amp.map(|i| graph.params[i].load()).unwrap_or(0.2);
            let start = idx * mb;
            let buf = &mut state.buffers[start..start + frames];
            dsp::process_osc(&mut state.states[idx], freq, amp, sr, buf);
        }
        moredata_core::NodeKind::Gate => {
            // Gate with no event this block: pass through (buffer is
            // already 0 from the per-block zeroing; this is a
            // no-op for leaf dispatch).
        }
        _ => {
            // Cross-node mix is performed on the audio thread after
            // the leaf phase; we never reach here.
        }
    }
}

pub struct Scheduler {
    workers: Vec<Worker>,
    plan: Plan,
    /// OS-thread pool. `None` means "inline only" (the M5.4
    /// behaviour). `Some` activates M5.5 parallelism.
    pool: Option<WorkerPool>,
    /// Per-partition completion counters, indexed by partition.
    /// Audio thread resets to `partition_len` and spins until zero.
    done_counters: Vec<AtomicUsize>,
    /// Stack-allocated work descriptors, one per partition. Their
    /// addresses are stable for the lifetime of the scheduler.
    /// `UnsafeCell` is required because the audio thread writes
    /// descriptors that workers read concurrently.
    work_descs: Vec<std::cell::UnsafeCell<Work>>,
    /// Per-partition atomic cursors. Workers fetch-add to claim
    /// node indices. Owned by the scheduler; never reallocated.
    cursors: Vec<AtomicUsize>,
}

impl Scheduler {
    pub fn new(workers: usize, plan: Plan) -> Self {
        let count = workers.clamp(1, 16);
        let workers = (0..count).map(|id| Worker { id }).collect();
        let parts = plan.partitions.len();
        let done_counters: Vec<AtomicUsize> = (0..parts).map(|_| AtomicUsize::new(0)).collect();
        // `Work` descriptors are placeholders; their pointers
        // are filled in at the start of every block. Each
        // descriptor owns one cursor atomic.
        let work_descs = (0..parts)
            .map(|_| {
                std::cell::UnsafeCell::new(Work {
                    graph: std::ptr::null(),
                    state: std::ptr::null_mut(),
                    partition: std::ptr::null(),
                    partition_len: 0,
                    frames: 0,
                    done: std::ptr::null(),
                    cursor: std::ptr::null(),
                })
            })
            .collect();
        // Per-partition atomic cursors; stable addresses, Send/Sync.
        let cursors: Vec<AtomicUsize> = (0..parts).map(|_| AtomicUsize::new(0)).collect();

        Self {
            workers,
            plan,
            pool: None,
            done_counters,
            work_descs,
            cursors,
        }
    }

    /// Enable the OS-thread pool. Idempotent. The pool is shut down
    /// deterministically when the scheduler is dropped.
    pub fn with_pool(mut self) -> Self {
        let n = self.workers.len();
        if self.pool.is_none() {
            self.pool = Some(WorkerPool::new(n));
        }
        self
    }

    pub fn workers(&self) -> &[Worker] {
        &self.workers
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Inline (M5.4) execution: partitions run on the calling thread.
    pub fn run_block(
        &self,
        graph: &CompiledGraph,
        state: &mut ProcessState,
        frames: usize,
        out: &mut [f32],
        events: &mut EventWindow<'_>,
    ) {
        let frames = frames.min(graph.max_block).min(out.len());
        if frames == 0 {
            return;
        }

        let mb = state.max_block;
        for buf in state.buffers.chunks_mut(mb).take(state.node_count) {
            buf[..frames].fill(0.0);
        }

        for (pi, part) in self.plan.partitions.iter().enumerate() {
            if pi >= MAX_PARTITIONS {
                break;
            }
            graph.process_partition(state, part, frames, events);
        }

        let start = graph.output_index * state.max_block;
        out[..frames].copy_from_slice(&state.buffers[start..start + frames]);
    }

    /// Parallel (M5.5) execution: the OS-thread pool consumes each
    /// partition. The audio thread publishes work via `AtomicPtr` and
    /// spins on a completion counter. **No** heap allocation, **no**
    /// audio-thread lock.
    ///
    /// Each worker touches only its **own** node index — this avoids
    /// `&mut ProcessState` aliasing across threads. Cross-node mix
    /// (mixer, output) is performed by the audio thread after the
    /// partition completes.
    pub fn run_block_parallel(
        &self,
        graph: &CompiledGraph,
        state: &mut ProcessState,
        frames: usize,
        out: &mut [f32],
        _events: &mut EventWindow<'_>,
    ) {
        let Some(pool) = self.pool.as_ref() else {
            self.run_block(graph, state, frames, out, &mut EventWindow::empty());
            return;
        };
        let frames = frames.min(graph.max_block).min(out.len());
        if frames == 0 {
            return;
        }

        // Zero all node buffers for this block. This is the only place
        // we touch every buffer; partition execution only writes.
        let mb = state.max_block;
        let nc = state.node_count;
        for buf in state.buffers.chunks_mut(mb).take(nc) {
            buf[..frames].fill(0.0);
        }

        for (pi, part) in self.plan.partitions.iter().enumerate() {
            if pi >= MAX_PARTITIONS {
                break;
            }
            let len = part.len();
            if len <= 1 || pool.is_empty() {
                graph.process_partition(state, part, frames, &mut EventWindow::empty());
                continue;
            }
            // Parallel leaf-only partition: each worker calls
            // `process_leaf_node`, which mutates only the worker's
            // own `state.buffers[idx*mb..]` and `state.states[idx]`.
            let done = &self.done_counters[pi];
            done.store(len, Ordering::Release);
            // Reset cursor to 0; the audio thread is the only
            // place that touches it before notifying workers.
            self.cursors[pi].store(0, Ordering::Release);
            let desc = self.work_descs[pi].get();
            unsafe {
                (*desc).graph = graph as *const _;
                (*desc).state = state as *mut _;
                (*desc).partition = part.as_ptr();
                (*desc).partition_len = len;
                (*desc).frames = frames;
                (*desc).done = done as *const _;
                (*desc).cursor = &self.cursors[pi] as *const AtomicUsize;
            }
            pool.inner.slot.store(desc, Ordering::Release);
            pool.inner.cvar.notify_all();
            while done.load(Ordering::Acquire) > 0 {
                std::hint::spin_loop();
            }
            pool.inner
                .slot
                .store(std::ptr::null_mut(), Ordering::Release);
            // After leaf nodes complete, perform cross-node mix
            // (mixers, outputs) inline on the audio thread.
            for &idx in part {
                if graph.nodes[idx].kind != moredata_core::NodeKind::Oscillator {
                    graph.process_partition(state, &[idx], frames, &mut EventWindow::empty());
                }
            }
        }

        let start = graph.output_index * mb;
        out[..frames].copy_from_slice(&state.buffers[start..start + frames]);
    }
}
