# ADR-0006: PipeWire is a backend, not the graph

- Status: accepted
- Date: 2026-08-22

## Decision

`AudioBackend` trait:

```
open(config) → stream
process callback → runtime.process(buf)
close()
```

PipeWire (Linux) and cpal (host) both implement it. Graph types never mention SPA
or pw_stream.

This Mac has no PipeWire; night-1 ships `CpalBackend`, `NullBackend`, `WavBackend`.
