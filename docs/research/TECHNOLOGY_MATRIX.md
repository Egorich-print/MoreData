# Technology Matrix — MoreData M0

> Date: 2026-08-22
> Status: design-only except where marked verified

## Core vs adapter vs UI vs control vs hardware

| Layer | What it is | Night-1 status |
|-------|------------|----------------|
| **Core** | Graph IR, DSP nodes, scheduler, buffers | implemented |
| **Adapter** | Audio backends, plugin hosts, Pd importer | cpal host implemented; PipeWire/CLAP design-only |
| **Control plane** | CLI, JSON API, project load/save | CLI implemented |
| **UI** | TUI (engineer), WebUI (human) | TUI prototype; WebUI design-only |
| **Hardware backend** | Daisy / Semechko / Buildroot image | design-only |

## Audio I/O

| Tech | Role | Realtime notes | Host |
|------|------|----------------|------|
| **cpal 0.15** | Cross-platform host I/O | Callback must stay allocation-free. CoreAudio (macOS), WASAPI, ALSA, optional JACK | verified on this Mac |
| **PipeWire** | Linux production graph | SPA buffers, quantum/rate, pw_stream process callback. Requires libpipewire. Not on macOS | design-only |
| **ALSA** | Fallback / Buildroot | Direct mmap possible; cpal already covers ALSA | design-only |
| **CoreAudio** | This machine | via cpal | verified device present |
| **JACK** | Pro Linux/mac | Optional cpal feature; not installed here | blocked (no JACK) |

**PipeWire assumptions (must not leak into core):**

- Server may change quantum and rate at runtime → runtime must accept block-size change on control plane, never realloc in callback
- Ports are SPA buffers, not MoreData ports
- Device disconnect is a backend event, not a graph panic
- MoreData is a *client*, not a PipeWire module

## Plugin formats

| Format | Headless | GUI required | Rust story | Priority |
|--------|----------|--------------|------------|----------|
| Native Rust DSP | yes | no | first-class | 1 |
| **CLAP** | yes | optional | `clack` / `clap-sys`; designed for headless | 2 |
| **LV2** | yes | optional | `lv2` crate; Lilv for scan | 3 |
| **VST3** | often GUI-centric | frequently yes | Steinberg SDK / `vst3-sys`; licensing friction | 4 |
| JUCE | N/A | typical | not a plugin format; hosting JUCE apps is out of scope | optional later |

Scanner must flag: `gui-required`, `allocates-in-process`, `not-headless-safe`.

## Rust audio ecosystem (not forked, may be used as crates)

| Crate | Use |
|-------|-----|
| cpal | host I/O |
| dasp / dasp_sample | sample conversions (optional) |
| hound | WAV read/write for offline renderer |
| fundsp | reference DSP ideas only — not a dependency of core |
| nih-plug | CLAP/VST3 plugin *framework* for *being* a plugin, not hosting |
| rustfft | future FFT nodes |
| ringbuf | SPSC control→rt if needed; prefer lock-free slot |

**Rejected as core:** libpd, plugdata, juce, fundsp-as-engine, fundsp graph.

## Embedded / image

| Tech | Fit |
|------|-----|
| **Buildroot** | Linux image: MoreData + PipeWire + SSH + CLI. Follow BalanSir `buildroot-external` pattern |
| **Armbian** | convenience distro; not reproducible enough for product images |
| **Alpine** | musl, small; good for containers, weaker realtime defaults |
| **Daisy Seed (STM32H7)** | 480 MHz, ~1 MiB SRAM, SAI audio. No Linux, no PipeWire. Static graph / no_std subset |
| **libDaisy / DaisySP** | C++ reference DSP. Port algorithms, do not link |
| **Semechko** | local hardware target name (no existing repo). Treat as Daisy-class MCU + custom I/O |

## Realtime Rust

Rules encoded in runtime:

- No heap in `process()`
- No `Mutex` in process path ( Hibiki anti-pattern )
- No filesystem, network, log flush, HTTP
- Preallocate all node state at graph-commit
- Control messages via wait-free SPSC or triple-buffer params
- `#![cfg(not(feature = "std"))]` path planned for Daisy subset
