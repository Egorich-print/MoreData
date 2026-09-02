# Mission State
```text
Mission: MoreData — Rust-native realtime audio platform
Current phase: M5.5 OS-thread pool (partial) — M5.4 inline fully verified
Current stage: Prototype
Last completed checkpoint: Scheduler with opt-in pool, 38 tests green,
    clippy clean; pool drop has known race documented in ADR-0012
Current git commit: see `git log -1` on mission/moredata
Known blockers: PipeWire absent on macOS; VST3 hosting deferred
Known risks: Pool drop race (M5.5.1 follow-up); f32 summation order
    under parallel execution (M5.5.2)
Next action: M5.6 PipeWire adapter; M5.5.1 pool drop fix; M5.7 AArch64/Buildroot
```
