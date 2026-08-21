# Daisy / Semechko targets

> Status: design-only
> Date: 2026-08-22

## Common subset

Portable: oscillator, gain, mixer, biquad, envelope — `f32` process, no heap,
fixed max block (e.g. 48 or 64 frames).

Not portable: dynamic graph commit, plugin scan, JSON CLI, PipeWire, TUI.

## Embedded limitations

| Constraint | Implication |
|------------|-------------|
| STM32H7 ~1 MiB SRAM | static `ProcessState`, no Vec growth |
| No MMU OS | no threads beyond audio IRQ + main |
| SAI/I2S block | backend replaces cpal |
| Flash | graph compiled into firmware or loaded as blob |

## Static graph compilation

Control plane (host) compiles `Graph` → `CompiledGraph` → codegen or packed blob.
Device runs `process()` only.

Semechko: unspecified locally; treat as Daisy-class until schematic exists.
Do not port Linux runtime onto the MCU.
