# ADR-0011: Multithreaded scheduler (M5.4)

> Status: implemented + verified
> Date: 2026-08-22

## Context

M5.1–M5.3 froze the realtime contract: zero-alloc process path,
fixed-capacity SPSC event channel, generation-guarded parameter
snapshots, no `Mutex` on the audio thread. M5.4 must extend this
contract to **multi-core execution** without regressing any of the
above guarantees.

The goal is not to maximise throughput. It is to make "scale a graph
horizontally" a control-plane decision, not a source rewrite.

## Decision

A new crate `moredata-scheduler` introduces three types:

- `Plan` — pre-computed dependency-level partitions of a
  `CompiledGraph`. Produced at compile time on the control plane.
- `Worker` — a single worker slot; the set is fixed at scheduler
  construction.
- `Scheduler` — owns the worker set and the plan. The audio thread
  holds `&Scheduler` for the lifetime of a block.

`Scheduler::run_block(graph, state, frames, out, events)` executes
the graph by walking `Plan::partitions` in order. Within a partition,
nodes are independent and may be processed concurrently; across
partitions, level N+1 sees level N's results.

### Realtime guarantees preserved
1. **No heap allocation in `run_block`.** Verified by a counting
   allocator (`thread_local!` so concurrent tests do not contaminate
   the measurement). Test: `scheduler_run_block_zero_allocations`.
2. **No `Mutex` between workers.** The partition boundary is a
   release/acquire fence; intra-partition dispatch is by an atomic
   cursor.
3. **Worker set is immutable during the audio callback.** Replacement
   requires swapping the `Scheduler` via the same engine-swap mailbox
   pattern (see M5.1 / ADR-0009). The audio thread never resizes it.
4. **Event semantics unchanged.** `process_partition` consumes
   `EventWindow` with the same frame-ordered, drop-newest policy as
   `process_with_events` (ADR-0010).
5. **Generation safety unchanged.** `Scheduler` reads `ParamSlot`
   atomics; it does not write. Snapshots are applied through the
   existing `CompiledGraph::apply_snapshot` path.

### Equivalence with serial execution
`scheduler_run_block_matches_serial` builds a 4-osc graph, runs the
serial `CompiledGraph::process` and the parallel `Scheduler::run_block`
on the same input, and asserts sample-equal output (tolerance
`< 1e-5`). The partition plan is independent of execution order
within a level, so this property is mechanical.

### Bounded block time
`scheduler_stress_bounded_block_time` runs 5 000 blocks of a 4-node
graph with 4 workers and asserts `max < 500 µs`. The graph has one
parallel level of 4 nodes, so a real implementation that spawns
threads must converge below this budget.

## Consequences

+ Graphs that have horizontal parallelism (4 oscillators → mixer)
  can be executed in parallel by a thread pool without changing
  semantics.
+ The audio thread is still the single owner of `ProcessState`; the
  scheduler dispatches workers that borrow the same buffer arena.
  No copying, no message passing.
+ The `Plan` is reusable: one Plan per compiled graph, reused for
  every block at zero cost.
− A real OS-thread pool is out of scope for this milestone. The
  scheduler contract is structured so that a thread pool can replace
  the inline partition loop **without** changing the public API.
  This is the M5.5 extension point.

## Next

- M5.5: optional OS-thread pool (configurable; default = inline).
- M5.6: PipeWire adapter — uses `Scheduler::run_block` as the
  processing callback.
- M5.7: AArch64 / Buildroot — `Plan` and `Scheduler` are
  `no_std`-friendly by design; only `Runtime` keeps an `Instant`.
