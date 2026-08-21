# Mission Report — MoreData

> Date: 2026-08-22
> Author identity: GitHub `Egorich-print` / Egor Korostelev
> Host: macOS ARM64, rustc 1.98.0, edition 2024

## Executive Summary

MoreData is a **new** Rust-native realtime audio platform. It is not a Pd/PlugData/JUCE/libpd wrapper.

Night-1 vertical slice (implemented + tested):

```
JSON project → Graph IR → CompiledGraph → Runtime.process → WAV / cpal
```

Pure Data can disappear completely: **yes, architecturally.** Core has zero Pd types.

## Claim ledger

| Claim | State | Evidence |
|-------|-------|----------|
| Native graph (Node/Port/Connection/Param) | implemented, tested | `crates/moredata-core`, 13 unit tests |
| Oscillator / Gain / Mixer / Output | implemented, tested | DSP + graph tests |
| Realtime commit model, no Mutex in graph process | implemented | `CompiledGraph::process`, atomic params |
| Offline renderer | implemented, tested | `moredata render` → 2400 frames WAV |
| CLI `--json` | implemented, verified | `status`, `audio status`, `graph validate`, `render` |
| TUI diagnostics | prototype | `moredata-tui`, control-plane types only |
| cpal/CoreAudio backend | implemented, verified | probe: CoreAudio, 48 kHz, stereo |
| PipeWire backend | design-only | no PipeWire on host; ADR-0006 |
| CLAP/LV2/VST3 hosts | design-only | capability model only |
| Buildroot image | design-only | `docs/architecture/BUILDROOT.md` |
| Daisy / Semechko | design-only | `docs/architecture/DAISY_SEMECHKO.md` |
| WebUI | design-only | `docs/architecture/WEBUI.md` |
| Pd importer | not started | intentionally deferred |

## What was built

- Cargo workspace, Rust 1.98 / edition 2024, CI, dual license
- `moredata-core` graph kernel
- `moredata-runtime` process + counters
- `moredata-audio` cpal / wav / null
- `moredata-plugin` native catalog + capability flags
- `moredata` CLI
- `moredata-tui`
- Research, ADRs, hardware/target docs

## Measurements (this host)

- `cargo test --workspace`: 16 tests passed
- `cargo clippy -- -D warnings`: clean
- Render 0.05 s @ 48 kHz → 2400 frames, 16-bit mono WAV
- Default device: MacBook Pro speakers, CoreAudio, 48 kHz, 2 ch
- Block size: 64 frames (compile-time max)

## Gaps / next

1. PipeWire adapter on Linux CI runner
2. Drop `Mutex` around Runtime in cpal `'static` callback (lock-free slot)
3. MIDI / CV ports
4. Static graph codegen for Daisy
5. CLAP scanner

## Architectural principle

MoreData stays a standalone engine. Legacy software may be imported; it must not become the kernel.
