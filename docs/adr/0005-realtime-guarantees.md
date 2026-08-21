# ADR-0005: Realtime guarantees

- Status: accepted
- Date: 2026-08-22

## Decision

`CompiledGraph::process` and all `DspNode::process` implementations:

- take `&mut ProcessCtx` with pre-borrowed buffers
- must not allocate, lock, syscall, log, or format
- read parameters via `AtomicU32::load(Relaxed)`
- write only into provided output slices

Violations are compile-time where possible (`no_std` DSP later) and test-time
(allocation counter feature, golden WAV).
