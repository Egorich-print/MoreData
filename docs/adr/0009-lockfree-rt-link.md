# ADR-0009: Lock-free realtime/control engine handoff

> Status: implemented
> Date: 2026-08-22
> Supersedes the `Mutex<Runtime>` in cpal callback (night-1 stopgap)

## Context

The cpal audio callback needs access to the processing engine, while the
control plane may want to replace it (graph edit → recompile → swap) or
retire it. The first prototype parked `Runtime` behind `Arc<Mutex<..>>` and
used `try_lock()` in the callback.

Problems with that model:

- unbounded worst case if the control thread holds the lock during a commit
- priority-inversion risk on Linux without PI mutexes
- the RT thread can silently skip blocks (`try_lock` failure = silence)

## Decision

A dedicated SPSC tri-state mailbox pair (`moredata_runtime::link`):

```
ControlLink                    RtLink (audio callback)
    │ publish(Runtime)  ──▶  inbox ──▶ refresh() at block start
    │ ◀──  retire mailbox ── old Runtime after swap
```

States per mailbox: EMPTY / FULL / EXCLUSIVE. `push` and `pop` are a single
CAS each; both sides are non-blocking. Only the control side may spin
(bounded by one block), never the audio thread.

Rules:

1. The audio callback owns `RtLink` exclusively via `FnMut` — no lock, no
   allocation, no syscall in `process`.
2. Engine swaps happen only at block boundaries (`RtLink::refresh`).
3. The retired engine returns to the control plane through the retire slot;
   dropping it there guarantees no RT use-after-free.
4. Parameter smoothing remains atomic-float slots inside `CompiledGraph`
   (unchanged); this ADR covers whole-engine replacement only.

## Consequences

+ No locks on the realtime path; hot-swap verified by test
  (`engine_hot_swap_without_mutex`).
+ Same primitive will back PipeWire's stream (device reconnect = engine
  swap) and future multi-graph routing.
− One engine in flight per direction; a second publish before retire
  spins briefly on the control side. Acceptable: commits are rare.
− `UnsafeCell` + manual state machine — correctness argument lives here
  and in the unit tests; keep them exhaustive.

## Alternatives rejected

- `Mutex` with `try_lock` (status quo): unbounded silence under load.
- `crossbeam` channel: allocation on overflow paths, heavier than needed.
- Double-buffered `ArcSwap`: still allocates per commit; harder to bound.
