# WebUI architecture

> Status: design-only
> Date: 2026-08-22

Human-facing. Not on the realtime path.

```
Svelte (static assets)
    → Rust HTTP/WebSocket (control plane)
        → Graph commit / params
```

Desktop wrapper: Tauri (same pattern as Hibiki), optional.

Production embedded: **no Node runtime**. Prebuilt static files served by Rust.

Night-1 does not ship a WebUI.
