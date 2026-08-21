# PipeWire adapter

> Status: design-only (no PipeWire on this macOS host)

```
MoreData Graph
  → Runtime.process
    → PipeWire pw_stream process
      → SPA buffer
        → device
```

## Boundary

- Core never includes `pipewire` crate
- `moredata-audio` will gain `src/pipewire.rs` behind `feature = "pipewire"`
- Quantum/rate changes → control-plane reconfigure, then swap `CompiledGraph`
- Disconnect → `record_xrun` + backend event, graph stays loaded

## Assumptions we will not bake into IR

- PipeWire graph is not MoreData graph
- Port names are SPA, mapped at adapter
- Latency is backend metadata, not a node param (except explicit delay nodes)
