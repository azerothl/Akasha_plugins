# graph plugin

High-level graph plugin for Akasha.

## Entry tools

- `graph_plot`
- `graph_stats`

## Input contract (example)

```json
{
  "tool": "graph_plot",
  "chart": "line",
  "x": ["2026-03-01", "2026-03-02"],
  "series": [
    { "name": "latency", "y": [120, 95] }
  ]
}
```

## Output contract (example)

```json
{
  "ok": true,
  "view": "graph",
  "library": "plotly",
  "figure": {
    "data": [{ "type": "scatter", "mode": "lines", "name": "latency", "x": ["2026-03-01"], "y": [120] }],
    "layout": { "title": "Latency trend" }
  },
  "summary": "Graph generated"
}
```

## Tauri / TUI rendering

- Tauri: native graph panel from `figure` payload.
- TUI: textual stats + mini table fallback.

> `plugin.wasm` is produced by plugin implementation build pipeline.
