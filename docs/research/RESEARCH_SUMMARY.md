# Research Summary — MoreData M0

> Date: 2026-08-22
> Status: verified (environment) / design-only (external ecosystems)
> Identity: GitHub `Egorich-print`, local author Egor Korostelev

## Mission

Build **MoreData**: a Rust-native realtime audio platform for embedded synthesizers.
Not a fork of Pure Data, PlugData, JUCE, or libpd.

## Environment (verified)

| Item | Value |
|------|-------|
| Host | Darwin 25.5.0, ARM64 (Apple T8142) |
| rustc | 1.98.0 (88d9e12ae 2026-08-18) |
| cargo | 1.98.0 (797e8a9bc 2026-08-05) |
| Default audio | Built-in speakers/mic, 48 kHz, stereo out |
| PipeWire / JACK | not installed |
| GitHub | `Egorich-print` (no orgs) |
| Existing `Egorich-print/MoreData` | does not exist |

## Knowledge layer (required)

Read from `Obsidian Vault/Knowledge/System`:

- Three layers: Execution (`~/ai-workstation/Projects/`), Knowledge (Obsidian), Showcase (`Documents/Проекты/`)
- New software project lives in `~/ai-workstation/Projects/MoreData/`
- Registry entry + Obsidian project note + showcase README
- Hardware siblings: Polivoks, Theremin, Aelita (analog synths) — MoreData is the **digital graph/runtime**, not a PCB
- Hibiki is a Tauri music *player* (cpal + Mutex in callback). MoreData must not copy that realtime model

## Naming (verified)

| Check | Result |
|-------|--------|
| crates.io / lib.rs `moredata` | no crate |
| GitHub `Egorich-print/MoreData` | free |
| GitHub search `MoreData` | unrelated: Swift Core Data helpers, Obsidian CSV plugin, wrangling notebooks |
| Trademark / audio engine collision | none found in audio/DSP space |

Name **MoreData** is usable. Crate names: `moredata-*` (hyphenated, crates.io convention).

## Related local projects

| Project | Relation |
|---------|----------|
| Hibiki | Playback engine, not a graph synth. Shares cpal as host I/O only |
| Polivoks / Aelita / Theremin | Hardware synths. Future MoreData targets, not dependencies |
| Vivanta | OS; no audio stack to reuse |
| BalanSir | Workspace/CI/docs conventions to follow |

## Technology conclusions

See `TECHNOLOGY_MATRIX.md` and `ARCHITECTURE_RISKS.md`.

Core decision: **native Rust graph + DSP**. Pd/JUCE/libpd are importers or optional adapters, never the kernel.

Host-night vertical slice on this machine:

```
Graph → Runtime → cpal/CoreAudio → device
```

PipeWire is the Linux production backend (adapter), not available on this Mac.
