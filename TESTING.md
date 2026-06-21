# Testing plugins with Akasha (Tauri + TUI)

This repository stores plugin packages and contracts.

## 1) Build a plugin package

Each plugin folder must include:

- `manifest.toml`
- `plugin.wasm` (build artifact generated locally; not tracked in git)

Current folders (`maps`, `graph`, `simulation`, `caldav-channel`, `matrix-channel`, `homeassistant`) already include manifests and contracts.
You only need to produce `plugin.wasm` for each plugin implementation.

Channel plugins (`caldav-channel`, `matrix-channel`) also require their **sidecar** binary running beside the daemon — see each plugin `README.md`.

Examples:

- `./scripts/build_maps_plugin.ps1`
- `./scripts/build_graph_plugin.ps1`
- `./scripts/build_simulation_plugin.ps1`
- `./scripts/build_homeassistant_plugin.ps1`

Home Assistant sidecar (WebSocket → webhooks):

```bash
cd plugins/homeassistant/sidecar && cargo build --release
# env: HA_BASE_URL, HA_ACCESS_TOKEN, AKASHA_AUTOMATION_WEBHOOK_SECRET, AKASHA_DAEMON_URL
./target/release/akasha-homeassistant-sidecar
```

## 2) Install into Akasha data dir

Use CLI install command with a plugin folder path:

- `akasha plugin install C:\path\to\Akasha_plugins\plugins\maps`
- `akasha plugin install C:\path\to\Akasha_plugins\plugins\graph`
- `akasha plugin install C:\path\to\Akasha_plugins\plugins\simulation`
- `akasha plugin install C:\path\to\Akasha_plugins\plugins\homeassistant`

Enable Home Assistant connector in Akasha settings (`HA_BASE_URL`, vault `ha_access_token`) before testing `ha_*` tools.

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
