# Akasha Plugins

Plugins library for Akasha (similar spirit to `Akasha_skills`), focused on **high-level tools** that agents can call and that can render advanced outputs in **Tauri UI** and **TUI**.

---

## Repository structure

```text
Akasha_plugins/
├── plugins/
│   ├── maps/
│   │   ├── plugin.json
│   │   ├── manifest.toml
│   │   └── README.md
│   ├── graph/
│   │   ├── plugin.json
│   │   ├── manifest.toml
│   │   └── README.md
│   └── simulation/
│       ├── plugin.json
│       ├── manifest.toml
│       └── README.md
├── scripts/
│   └── build_plugins.py
├── plugins.json          # auto-generated catalog
├── index.html            # simple catalog page
└── .github/
    └── workflows/
        └── sync-plugins.yml
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

## Initial plugin roadmap

- `maps`: itinerary + distance between points
- `graph`: chart rendering (timeseries/scatter/bar)
- `simulation`: scenario simulation and timeseries output

All 3 plugins define output contracts that Tauri can render richly and TUI can render as text/table fallback.

---

## Build catalog

```bash
python scripts/build_plugins.py
```

This regenerates `plugins.json` from all `plugins/*/plugin.json`.

## Build first executable plugin (`maps`)

```bash
# PowerShell
./scripts/build_maps_plugin.ps1

# Bash
./scripts/build_maps_plugin.sh
```

This generates `plugins/maps/plugin.wasm` from `plugins/maps/wasm`.

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
- `TOOL: plugin.maps <json-or-args...>`
- `TOOL: plugin.call maps <json-or-args...>`

---

## Notes

This repo scaffolds plugin **catalog + contracts + packaging layout** first.
Runtime dispatch from agent tool calls to `PluginRegistry::call_tool` is handled in Akasha core and should be enabled there.
