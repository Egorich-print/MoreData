# Mission State

```text
Mission: MoreData — Rust-native realtime audio platform
Current phase: M5.1 (lock-free RT link) implemented; next M5.2 deterministic param/event transport
Current stage: Prototype
Last completed checkpoint: M5.1 hot-swap engine without Mutex, 19 tests green
Current git commit: see `git log -1` on mission/moredata
Known blockers: PipeWire absent on macOS; Semechko hardware unspecified; VST3 hosting deferred
Known risks: Mailbox is single-slot (second publish spins on control side until retire); PipeWire absent on macOS; VST3 hosting deferred
Next action: M5.2 deterministic parameter/event transport; then M5.3 allocation audit (alloc-scoped test)
```
