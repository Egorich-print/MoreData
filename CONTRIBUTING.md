# Contributing

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Realtime path (`CompiledGraph::process`) must not allocate, lock, or log.
Control plane changes belong in CLI / TUI / future WebUI, never inside DSP nodes.

Commit style: `feat(graph): …`, `docs: …`, `chore: …` (see mission §43).
