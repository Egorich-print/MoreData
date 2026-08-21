# Repository guide

Canonical path: `~/ai-workstation/Projects/MoreData/`

```
crates/moredata-core      graph + DSP
crates/moredata-runtime   process wrapper
crates/moredata-audio     backends
crates/moredata-plugin    capabilities
crates/moredata-cli       moredata binary
crates/moredata-tui       optional TUI
docs/research             M0
docs/architecture         M1
docs/adr                  decisions
tests/fixtures            .mdproject JSON
```

Do not add libpd, juce, or plugdata to workspace dependencies.
