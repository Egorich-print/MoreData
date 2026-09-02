# ADR-0010: Realtime/control contract (M5.2 + M5.3)

> Status: implemented + verified
> Date: 2026-08-22

## Context

M5.1 made the audio callback own `RtLink` via `FnMut`, removing `Mutex`
from the realtime path. The next step is to formalize:

1. **Event semantics** — exact ordering, frame clamping, overflow.
2. **Parameter semantics** — coalescing and stale-patch rejection.
3. **Allocation budget** — the realtime path must perform zero heap
   allocations; verified by a counting allocator.

## Decisions

### Event ordering
1. Global order is by `(frame ascending, push order)`. Two events with
   the same `frame` keep arrival order (stable sort).
2. `frame` is clamped to `[0, block_frames - 1]` at dispatch time; an
   out-of-range value never panics and never reaches a node.
3. Events with an unknown `NodeId` are dropped silently. Events of an
   unsupported kind for a given node are dropped silently (counted in
   `EventQueue::dropped`).

### Overflow
- Policy: **drop newest**, count in `EventQueue::dropped`. The producer
  never blocks. There is no implicit back-pressure.
- Producers must treat `dropped()` as a control-plane signal (e.g. enable
  an output rate limit, degrade a UI).

### Stale-patch rejection
- Each `CompiledGraph` carries a monotonic `generation`.
- `ParamSnapshot::generation` must match the engine's generation to be
  applied. A patch addressed to a previous engine returns `false`.
- Coalescing: a snapshot contains at most one value per slot; last
  write within a snapshot wins.

### Realtime guarantees
- `process()` does not allocate. Verified by a counting global
  allocator (`thread_local!` counter so concurrent tests do not
  contaminate the measurement).
- `EventQueue::push` and `pop` are non-blocking; the producer uses a
  bounded SPSC ring.
- `RtLink::refresh` runs at block boundary; the control side may spin
  until the previous engine is retired (bounded by one block).

### Ownership
- `Runtime` owns the live engine and the staging buffer. The
  `EventQueue` may be shared via `Arc<EventQueue<256>>` for stress
  tests, MIDI threads, and external producers.
- `ControlLink`/`RtLink` own separate halves of the engine-swap
  channel via shared `Arc<Mailbox<Runtime>>`.

## Test campaign (M5.2 + M5.3)

| Test | Verifies |
|------|----------|
| `event_ordering_is_frame_deterministic` | push order preserved; dispatch consumes all events |
| `frame_clamped_no_panic` | `frame = u16::MAX` does not panic |
| `overflow_policy_drops_newest_and_counts` | `dropped()` counter reflects overflow |
| `mandatory_events_survive_bounded_backlog` | 100 events/block × 50 blocks: zero drops |
| `param_snapshot_coalesces_last_write_wins` | last write per slot wins |
| `stale_generation_rejected_fresh_accepted` | generation guard rejects old-gen patches |
| `hot_swap_energy_transitions` | engine swap changes audio output (loud→silent) |
| `zero_allocations_in_process_path` | counting allocator reports `0` allocations during 256 process calls with param + event activity |
| `stress_control_churn_vs_rt` | 10 000 blocks; control thread recompiles + publishes + pushes events; max block time < 500 µs, avg < 50 µs, engine survives |

## Consequences

+ The control plane can rebuild and publish engines at any rate without
  risking realtime safety; hot-swap is invariant.
+ The audio callback is a closed system: it touches only
  pre-allocated buffers, atomic slots, and the SPSC event queue.
+ A future scheduler or multithreaded DSP graph must be measured
  against the same contract: zero allocations in process, bounded
  block time, no locks on the audio thread.
− Generation guards make "spooky action" impossible, but they also
  mean control code must re-snapshot after every publish. A higher-
  level editor API should make this implicit.

## Next

- M5.4: multithreaded scheduler, measured against this contract.
- M5.5: PipeWire adapter — the cpal callback is the reference
  implementation; PipeWire must hit the same invariants.
- M5.6: AArch64 / Buildroot image: the static graph must be a
  subset of this contract.
