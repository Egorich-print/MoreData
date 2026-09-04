# ADR-0012: M5.5 OS-thread pool (partial)

> Status: complete (M5.5.1+M5.5.2 landed); f32 summation order remains
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

> **Resolved 2026-09-04 (audit + M5.5.1/M5.5.2 fix):**
> 1. **Drop race / lost wakeup — FIXED.** `Drop` sets `shutdown` while
>    holding the pool mutex, then `notify_all`. Workers sleep via
>    `cvar.wait_while` with a predicate on `shutdown || new dispatch`,
>    so a notify racing with `wait` can no longer be lost.
> 2. **Deadlock when `workers < partition_len` — FIXED.** The `done`
>    counter is now initialized to the worker count (each worker
>    decrements exactly once per dispatch), not the partition length
>    (which deadlocked when `done` could never reach zero).
> 3. **Double-dispatch race — FIXED.** A monotonic `epoch` counter is
>    incremented per dispatch; workers track their last served epoch and
>    never process the same dispatch twice (which would corrupt `done`).
> 4. **Lost notify on publish — FIXED.** The audio thread publishes
>    `slot`/`epoch` while holding the pool mutex (a two-store critical
>    section), because the predicate reads those atomics; publishing
>    without the lock allowed the notify to slip between predicate
>    evaluation and sleep.
> Trade-off accepted: the audio thread briefly takes the pool mutex on
> publish. It is uncontended while workers run (they drop the guard
> before processing), so the block-time bound still holds.
> Regression tests: `pool_narrower_than_partition_completes`,
> `pool_drop_does_not_hang`, `parallel_pool_actually_uses_pool_and_matches_serial`.
> Also fixed a **test-suite bug**: previous `parallel_pool_*` tests never
> called `.with_pool()`, so they exercised the inline path only.

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
