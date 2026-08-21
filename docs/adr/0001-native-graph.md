# ADR-0001: Native graph, not Pd

- Status: accepted
- Date: 2026-08-22

## Decision

MoreData graph is a first-party typed IR (`NodeId`, `PortId`, `Connection`, `Param`).
Pd, PlugData, JUCE, libpd are not dependencies of `moredata-core`.

## Consequences

- Pd patches may be imported later as a lossy translator into this IR
- We reimplement oscillator/gain/mixer instead of wrapping `[osc~]`
- Plugin hosts sit behind `moredata-plugin`, never inside the graph kernel
