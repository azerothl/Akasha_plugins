# Testing plugins with Akasha (Tauri + TUI)

This repository stores plugin packages and contracts.

## 1) Build a plugin package

Each plugin folder must include:

- `manifest.toml`
- `plugin.wasm`

Current folders (`maps`, `graph`, `simulation`) already include manifests and contracts.
You only need to produce `plugin.wasm` for each plugin implementation.

Examples:

- `./scripts/build_maps_plugin.ps1`
- `./scripts/build_graph_plugin.ps1`
- `./scripts/build_simulation_plugin.ps1`

## 2) Install into Akasha data dir

Use CLI install command with a plugin folder path:

- `akasha plugin install C:\path\to\Akasha_plugins\plugins\maps`
- `akasha plugin install C:\path\to\Akasha_plugins\plugins\graph`
- `akasha plugin install C:\path\to\Akasha_plugins\plugins\simulation`

Then:

- `akasha plugin reload`
- `akasha plugin list`

## 3) Verify in daemon API

Check installed plugins:

- `GET /api/plugins`

## 4) Test from app

- Tauri UI: call tool from an agent flow and render output by `view` type (`map`, `graph`, `timeseries`, `table`, `summary`).
- TUI: display text fallback summary and tabular metrics.

## 5) Recommended rollout

1. Implement + validate `maps` first
2. Implement + validate `graph`
3. Add `simulation`

## Important integration note

For agent tool calls to reach plugins, Akasha daemon must route unknown tool calls to:

- `PluginRegistry::call_tool(plugin_id, input_json)`

If this routing is not enabled yet, plugins can still be installed/listed but not invoked by agent tool syntax.
