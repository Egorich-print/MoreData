# ADR-0007: Hardware model — shared DSP, split runtime

- Status: accepted (design)
- Date: 2026-08-22

## Decision

```
Common Graph/DSP model
        │
        ├── Linux target (std, PipeWire/cpal, dynamic commit)
        └── Embedded target (no_std subset, static graph, SAI/I2S)
```

Daisy / Semechko do not run the Linux runtime. They consume the same node
algorithms compiled with a static schedule.

See `docs/architecture/DAISY_SEMECHKO.md`.
