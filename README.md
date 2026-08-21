# MoreData

Rust-native realtime audio platform for embedded synthesizers.

Not a Pure Data / PlugData / JUCE / libpd wrapper.

```
Graph → DSP → Realtime runtime → backend (cpal / wav / PipeWire later) → device
```

## Requirements

- Rust 1.98, edition 2024 (`rust-toolchain.toml`)

## Quick start

```bash
cargo test --workspace
cargo run -p moredata-cli -- status --json
cargo run -p moredata-cli -- render tests/fixtures/sine.mdproject -o /tmp/sine.wav --seconds 0.2 --json
cargo run -p moredata-cli -- play tests/fixtures/sine.mdproject --seconds 1 --json
```

## Crates

| Crate | Role |
|-------|------|
| `moredata-core` | Graph IR + DSP |
| `moredata-runtime` | Scheduler / process metrics |
| `moredata-audio` | Backends (cpal, wav, null) |
| `moredata-plugin` | Capability model |
| `moredata-cli` | Agent control plane (`--json`) |
| `moredata-tui` | Optional diagnostics |

## Docs

- `docs/MISSION_STATE.md` — live ledger
- `docs/research/` — M0 discovery
- `docs/architecture/` — model
- `docs/adr/` — decisions

## License

MIT OR Apache-2.0
