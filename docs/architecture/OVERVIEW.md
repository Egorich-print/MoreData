# MoreData Architecture Overview

> Status: implemented (core/runtime/cli/render) / design-only (PipeWire, Daisy, WebUI, CLAP host)

```
                     MoreData
                        │
        ┌───────────────┼────────────────┐
        │               │                │
       Graph          Runtime          Control
        │               │                │
       IR            Scheduler          API
        │               │                │
       DSP           Backends          CLI/TUI
                                        WebUI
```

## What is the kernel

`moredata-core` is the kernel:

- typed graph (nodes, ports, connections, parameters)
- DSP nodes (oscillator, gain, mixer, output)
- validation
- audio buffers owned by the runtime, borrowed by nodes

Pure Data can disappear completely: **yes, architecturally.** Nothing in core imports Pd types.

## Planes

| Plane | Thread | Allowed |
|-------|--------|---------|
| Realtime audio | backend callback | process graph, atomic param read, no alloc |
| Control | CLI / future server | graph edit, commit, device enum, logs, FS, JSON |

Control never calls into DSP except via `GraphCommit` (swap) and parameter slots.

## Crate map

```
moredata-core      graph + DSP          no I/O
moredata-runtime   scheduler, commit    no FS/net
moredata-audio     AudioBackend trait   cpal host, null, wav
moredata-plugin    capability model     native only tonight
moredata-cli       control plane        JSON
moredata-tui       diagnostic UI        talks to same API types
```

## Data ownership

- `Graph` is edited on the control plane
- `CompiledGraph` is immutable for a generation; RT holds `Arc<CompiledGraph>`
- Node state lives in `NodeState` preallocated at compile
- Parameters: `AtomicU32` bits (f32 to_bits) written by control, read by RT
- Audio buffers: one scratch arena sized to `max_block * channels * nodes`

## Error model

- Graph construction: `GraphError` (recoverable, never panic)
- Process path: infallible. Invalid graphs cannot be committed
- Backend: `BackendError` on open/underrun report; process itself does not return Result

## Logging / telemetry

- RT: counters only (`blocks`, `xruns`, `last_block_ns`)
- Control: `tracing` on CLI
- Agent interface: `moredata … --json`
