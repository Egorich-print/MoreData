# Open Questions — MoreData M0

> Date: 2026-08-22

1. **Semechko hardware** — no repo, schematic, or MCU choice in vault. Assume STM32H7-class until specified.
2. **Project file format** — `.mdproject` (mission) vs JSON/RON. Night-1 uses JSON; markdown project is a later serializer.
3. **License of hosted VST3** — Steinberg SDK terms; do not vendor SDK until legal review.
4. **Multi-rate graphs** (oversampling, control-rate) — night-1 is single-rate; oversampling is a node, not a graph feature.
5. **MIDI / CV** — needed for synths; night-1 is audio-only graph plus numeric parameters.
6. **Who owns the audio thread** — backend (cpal/PipeWire) owns it; runtime is a callback. Confirmed.
7. **Graph polyphony / voice stealing** — deferred; mixer+osc is monophonic proof.
8. **Pd importer fidelity** — objects with GUI/data structures cannot map 1:1. Importer is lossy by design.
9. **WebUI on embedded** — static Svelte + Rust HTTP. Node runtime never on device. Design-only tonight.
10. **Exact Daisy board** — Seed vs Patch vs custom. DSP subset is board-agnostic.
