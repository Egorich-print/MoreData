# ADR-0003: Plugin capability model

- Status: accepted (design)
- Date: 2026-08-22

## Decision

Plugins are classified, not blindly loaded:

```
native | clap | lv2 | vst3
realtime-safe | unknown
headless-safe | gui-required
sandboxable | in-process
```

Night-1 implements native nodes only. Hosts are adapters.

Priority: native → CLAP → LV2 → VST3.
