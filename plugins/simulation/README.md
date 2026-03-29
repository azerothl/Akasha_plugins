# simulation plugin

High-level simulation plugin for Akasha.

## Entry tools

- `sim_run`
- `sim_compare`
- `plugin.simulation`
- `plugin.call simulation ...`

## Build `plugin.wasm`

From repository root:

- Windows PowerShell: `./scripts/build_simulation_plugin.ps1`
- Bash: `./scripts/build_simulation_plugin.sh`

This compiles `plugins/simulation/wasm` and copies `plugin.wasm` into `plugins/simulation/`.

## Input contract (example)

```json
{
  "tool": "sim_run",
  "model": "generic_growth",
  "params": {
    "initial": 100,
    "growth_rate": 0.02,
    "noise": 0.03,
    "horizon": 120
  },
  "seed": 42
}
```

## Output contract (example)

```json
{
  "ok": true,
  "view": "timeseries",
  "summary": "Simulation 'generic_growth' finished",
  "series": [
    { "name": "value", "points": [[0, 100], [1, 101.3], [2, 99.8]] }
  ],
  "metrics": {
    "avg": 102.4,
    "p95": 115.2,
    "final": 109.8
  }
}
```

## Compare mode (example)

```json
{
  "tool": "sim_compare",
  "model": "generic_growth",
  "params": { "initial": 100, "growth_rate": 0.02, "noise": 0.03, "horizon": 60 },
  "compare": { "growth_rate": 0.03 }
}
```

Returns `view=table` with base/alternative metrics and a `delta` block.

## Tauri / TUI rendering

- Tauri: chart panel for series + metrics cards.
- TUI: compact table and metric summary.

> `plugin.wasm` is produced by plugin implementation build pipeline.
