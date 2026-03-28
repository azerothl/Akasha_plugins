# simulation plugin

High-level simulation plugin for Akasha.

## Entry tools

- `sim_run`
- `sim_compare`

## Input contract (example)

```json
{
  "tool": "sim_run",
  "model": "queue",
  "params": {
    "arrival_rate": 20,
    "service_rate": 25,
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
  "summary": "Simulation finished",
  "series": [
    { "name": "queue_length", "points": [[0, 2], [1, 3], [2, 1]] }
  ],
  "metrics": {
    "avg_queue": 2.1,
    "p95_queue": 5
  }
}
```

## Tauri / TUI rendering

- Tauri: chart panel for series + metrics cards.
- TUI: compact table and metric summary.

> `plugin.wasm` is produced by plugin implementation build pipeline.
