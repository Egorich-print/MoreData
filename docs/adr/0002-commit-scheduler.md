# ADR-0002: Commit-based scheduler

- Status: accepted
- Date: 2026-08-22

## Decision

The realtime thread never mutates topology. Control plane builds a `Graph`,
`compile()`s it into `CompiledGraph` + preallocated `ProcessState`, then
atomically publishes the generation.

Process order is a static topological sort computed at compile time.

## Consequences

- Adding a node is not realtime
- Parameter tweaks are realtime (atomics)
- No `Mutex<Graph>` in the callback
