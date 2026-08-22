# Architecture Risks — MoreData M0

> Date: 2026-08-22

| ID | Risk | Impact | Mitigation | Status |
|----|------|--------|------------|--------|
| R1 | Accidental Pd/JUCE coupling | Mission failure | No libpd/JUCE in core Cargo.toml; Pd is importer only | accepted |
| R2 | Mutex in audio callback (Hibiki pattern) | xruns, priority inversion | **Resolved (M5.1)**: SPSC tri-state mailbox, callback owns RtLink via FnMut; see ADR-0009. Residual: single-slot mailbox spins control side if publish outpaces retire | resolved |
| R3 | PipeWire not on macOS host | Cannot verify Linux backend tonight | cpal/CoreAudio as host I/O; PipeWire adapter documented | accepted |
| R4 | VST3 GUI-required plugins | Headless devices unusable | Capability scanner; CLAP first | accepted |
| R5 | Dynamic graph in callback | allocation, topology races | Commit model: edit on control plane, swap generation | accepted |
| R6 | Daisy SRAM (~1 MiB) | Linux runtime will not fit | Separate embedded profile: static graph, no_std DSP subset | design-only |
| R7 | Sample-rate / block-size change | realloc in RT | Reconfigure on control thread; RT uses pre-sized max block | accepted |
| R8 | Name collision "MoreData" | discoverability | Unrelated Swift/Obsidian projects exist; audio niche is free | accepted |
| R9 | rustc 1.98 / edition 2024 | some crates lag | Pin toolchain; avoid nightly-only features | verified |
| R10 | Plugin crash in-process | device lockup | Isolate later; night-1 native nodes only | documented |
| R11 | Semechko hardware unspecified | cannot size buffers | Treat as Daisy-class until schematic exists | documented |
| R12 | Trademark later | rename cost | crates are `moredata-*`; rename surface is crate prefix | accepted |
