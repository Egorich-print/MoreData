# ADR-0008: CLI/JSON is the agent API

- Status: accepted
- Date: 2026-08-22

## Decision

Primary control surface is `moredata` CLI with `--json`.

```
moredata status --json
moredata audio status --json
moredata graph validate <file>
moredata render <file> -o out.wav
moredata diagnostics --json
```

TUI and future WebUI consume the same types (`moredata_core::report`).
No DSP calls from UI crates.
