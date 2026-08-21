# Buildroot integration

> Status: design-only
> Date: 2026-08-22

## Profiles

| Profile | Contents |
|---------|----------|
| minimal | moredatad-equivalent: graph + runtime + null/alsa |
| headless | + PipeWire + CLI + SSH |
| production | headless + watchdog + read-only root |
| development | + TUI + debug symbols |

WebUI is **not** required on device.

## Package sketch

```
package/moredata/
  moredata.mk
  Config.in
```

Depends: host rustc or prebuilt aarch64 musl binary, alsa-lib or pipewire.

Follow BalanSir `buildroot-external` layout when an image is actually built.
