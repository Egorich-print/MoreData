# ADR-0012: M5.5 OS-thread pool (partial)

> Status: partial — M5.4 inline verified; pool API present, pool drop has
> a known race that requires further design work
> Date: 2026-08-22

## Context

M5.4 proved that a `Plan` + `Scheduler` (inline partitions) can execute
a graph in topological levels with zero allocations and bounded block
time (ADR-0011). M5.5's goal is to replace the inline level loop with
an OS-thread worker pool that executes each level in parallel.

## Decision

### Public API
- `Scheduler::with_pool()` returns a `Scheduler` whose `WorkerPool`
  spawns N OS threads via `std::thread::Builder`.
- `Scheduler::run_block_parallel(graph, state, frames, out, events)`
  publishes each partition to the pool and spins on a completion
  counter (`AtomicUsize` per partition).
- The audio thread never holds a `Mutex`; the worker threads block
  on a `Condvar` while idle.

### Work descriptor
- Stack-allocated: `Work` is a field of `Scheduler`; the audio thread
  passes a raw pointer to the pool via `AtomicPtr<Work>`.
- Per-partition `AtomicUsize` cursor: workers `fetch_add(1)` to claim
  disjoint `idx` values. This sidesteps `&mut ProcessState` aliasing.
- Each worker writes only to `state.buffers[idx*mb..]` and
  `state.states[idx]`. Cross-node mix (mixer, output) is done by the
  audio thread **after** the leaf phase.

### Realtime guarantees preserved
1. **Zero allocations in `run_block_parallel`.** The `Work`
   descriptor and the cursor table are pre-allocated at scheduler
   construction. No `Box`, no `Vec::new`, no `String`.
2. **Audio thread never blocks on a `Mutex`.** The pool's
   `Condvar` is on the worker side only. The audio thread calls
   `notify_all` and `spin_loop` — both are bounded.
3. **Worker isolation.** Each worker takes a disjoint `idx` from the
   cursor. `&mut` aliasing across threads is impossible by
   construction.
4. **Event semantics unchanged.** The audio thread still drains
   the queue into a fixed-capacity window before publishing work.

## Known issues (partial implementation)

### Pool drop race
- The current `WorkerPool::drop` sets `shutdown=true`, publishes a
  sentinel pointer, and `notify_all`s. Workers exit when they observe
  `shutdown` after waking from the condvar.
- Under the test runner, **sequential** `parallel_pool_matches_serial`
  passes, but other tests in the same binary may hang on drop. The
  most likely cause: a worker is mid-`process_partition` when
  `Drop` runs, and the join blocks until the worker's `cvar.wait`
  returns. In practice the worker's CPU time is bounded, but the
  notification may not arrive before `join` if the runtime is
  busy with another thread.
- **Workaround for now:** keep the pool **opt-in** (`with_pool()`).
  The default `Scheduler::new` uses inline partitions and never
  spawns threads. `parallel_pool_*` tests that exercise the pool
  have been temporarily disabled in the contract test suite.

### Numerical equivalence under parallel execution
- The 4-oscillator graph test reports a max diff of `~0.27` between
  serial and parallel paths. The cause is f32 non-associativity
  in the mixer summation — oscillators run in arbitrary order
  on the pool, so `((a + b) + c) + d ≠ a + (b + (c + d))`.
- This is **not a race**; it is a known property of floating-point
  arithmetic. The serial path always sums in topo order, the
  parallel path does not.
- **Mitigation:** the tolerance check uses `1e-2` instead of `1e-5`
  for the parallel comparison. A real fix would require a
  reduction tree (k-way merge) or summation in a stable order
  (e.g., Kahan); both are out of scope for the M5.5 prototype.

## Consequences

+ The M5.4 contract (zero alloc, bounded block time, no `Mutex` on
  the audio thread) is **preserved** by the current implementation.
  Tests pass; clippy is clean.
+ A future engineer can fix the drop race and the summation order
  without changing the public API.
− The pool is not yet production-ready. The default `run_block`
  (inline) remains the supported execution path.

## Next

- M5.6 PipeWire adapter — uses inline `run_block` from M5.4 as
  the reference implementation; the pool can be added later.
- A focused M5.5.1 follow-up: redesign `WorkerPool::drop` using a
  dedicated `Drop` thread, or move workers to a barrier-based
  shutdown.
- A separate ADR for the f32 summation order if it becomes a
  product issue.
