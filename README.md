# Akasha Plugins

Plugins library for Akasha (similar spirit to `Akasha_skills`), focused on **high-level tools** that agents can call and that can render advanced outputs in **Tauri UI** and **TUI**.

---

## Repository structure

```text
Akasha_plugins/
├── plugins/
│   ├── maps/              # outils agent (cartographie, itinéraires)
│   ├── graph/             # outils agent (graphiques)
│   ├── simulation/        # outils agent (scénarios)
│   ├── caldav-channel/    # canal CalDAV → calendrier Akasha (WASM + sidecar)
│   └── matrix-channel/    # canal Matrix → sessions chat Akasha (WASM + sidecar)
│       ├── plugin.json
│       ├── manifest.toml
│       ├── README.md
│       ├── wasm/          # contrat catalogue (hooks, permissions)
│       └── sidecar/       # binaire natif (sync réseau runtime)
├── scripts/
│   └── build_plugins.py
├── plugins.json          # auto-generated catalog
├── index.html            # simple catalog page
└── .github/
    └── workflows/
        ├── sync-plugins.yml
        └── plugins-catalog-validate.yml   # CI: rebuild + validate plugins.json (semver, wasm_sha256 hex)
```

---

## Plugin package format (for Akasha daemon)

Each installable plugin directory must contain:

- `manifest.toml` (or `manifest.json`)
- `plugin.wasm` (WASM binary)

Akasha loads plugins from:

- `${AKASHA_DATA_DIR}/plugins/<plugin-id>/`

Then reload:

- `akasha plugin reload`

List installed:

- `akasha plugin list`

---

## Trust catalog (hash, review, MCP)

See **[TRUST_CATALOG.md](TRUST_CATALOG.md)** for the roadmap on WASM digest, signatures, visible permissions, and MCP/hook alignment with the core daemon.

## Runtime event alignment

For plugin tool integrations targeting current Akasha clients:

- Emit or map lifecycle signals consistently with daemon task events (`tool_call_started`, `tool_call_finished`).
- Keep payloads deterministic and include correlation identifiers when available.
- Design plugin outputs so UIs can render both rich and fallback forms without assuming a single channel.

This improves timeline coherence in Code Studio, Tauri UI, and TUI.

## Catalog format (`plugin.json`)

`plugin.json` is for this repository catalog and website.

Required fields:

- `id`
- `name`
- `version`
- `description`
- `author`
- `category`
- `tags`
- `icon`
- `featured`
- `permissions`
- `entry_tools`
- `ui_views`

---

## Plugin families

### Agent tools (`category`: analytics, navigation, modeling)

- `maps` — itinerary + distance between points
- `graph` — chart rendering (timeseries/scatter/bar)
- `simulation` — scenario simulation and timeseries output

These plugins expose `entry_tools` and `ui_views`. Tauri renders rich views; TUI falls back to text/table.

### Channel bridges (`category`: channels)

- `caldav-channel` — sync CalDAV ↔ calendrier Akasha (`on_schedule_fire` + sidecar `akasha-caldav-sidecar`)
- `matrix-channel` — messages Matrix → `POST /api/message` (hooks canal + sidecar `akasha-matrix-sidecar`)

Channel plugins use **WASM for catalogue/hooks** (via `hook_events` in `plugin.json`) and a **sidecar** for the sync réseau. See each plugin `README.md` for env vars and limitations.

---

## Build catalog

```bash
python scripts/build_plugins.py
```

This regenerates `plugins.json` from all `plugins/*/plugin.json` manifests.

## Build first executable plugin (`maps`)

```bash
# PowerShell
./scripts/build_maps_plugin.ps1

# Bash
./scripts/build_maps_plugin.sh
```

This generates `plugins/maps/plugin.wasm` from `plugins/maps/wasm`.

## Build `graph` plugin

```bash
# PowerShell
./scripts/build_graph_plugin.ps1

# Bash
./scripts/build_graph_plugin.sh
```

This generates `plugins/graph/plugin.wasm` from `plugins/graph/wasm`.

## Build `simulation` plugin

```bash
# PowerShell
./scripts/build_simulation_plugin.ps1

# Bash
./scripts/build_simulation_plugin.sh
```

This generates `plugins/simulation/plugin.wasm` from `plugins/simulation/wasm`.

---

## Test loop with Akasha app

1. Build plugin WASM in this repo (when implementation is ready)
2. Copy plugin folder to `${AKASHA_DATA_DIR}/plugins/<id>`
3. Run:
   - `akasha plugin reload`
   - `akasha plugin list`
4. Call tool from agent workflow (once tool-dispatch plugin routing is enabled in daemon)

Tool call conventions now supported by daemon plugin dispatch:

- `TOOL: maps_distance <from_lat> <from_lon> <to_lat> <to_lon> [mode]`
- `TOOL: maps_route <from_lat> <from_lon> <to_lat> <to_lon> [mode]`
- `TOOL: graph_plot <chart> <y1> <y2> ...`
- `TOOL: graph_stats <json-or-args...>`
- `TOOL: plugin.maps <json-or-args...>`
- `TOOL: plugin.call maps <json-or-args...>`
- `TOOL: plugin.graph <json-or-args...>`
- `TOOL: plugin.call graph <json-or-args...>`
- `TOOL: sim_run <initial> <growth_rate> <noise> <horizon>`
- `TOOL: sim_compare <initial> <growth_rate> <noise> <horizon>`
- `TOOL: plugin.simulation <json-or-args...>`
- `TOOL: plugin.call simulation <json-or-args...>`

---

## Notes

This repo scaffolds plugin **catalog + contracts + packaging layout** first.
Runtime dispatch from agent tool calls to `PluginRegistry::call_tool` is handled in Akasha core and should be enabled there.
