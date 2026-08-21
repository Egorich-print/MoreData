# ADR-0004: JSON project format (night-1)

- Status: accepted
- Date: 2026-08-22

## Decision

Night-1 project files are JSON (`*.mdproject` extension kept as the mission name).
Schema: sample_rate, nodes[], connections[], params{}.

Markdown wrapping can be added without changing the IR.

## Example

```json
{
  "sample_rate": 48000,
  "nodes": [
    {"id": "osc", "kind": "oscillator", "params": {"freq": 440.0, "amp": 0.2}},
    {"id": "out", "kind": "output"}
  ],
  "connections": [{"from": ["osc", "out"], "to": ["out", "in"]}]
}
```
