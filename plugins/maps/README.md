# maps plugin

High-level map plugin for Akasha.

## Entry tools

- `maps_geocode`
- `maps_distance`
- `maps_route`
- `plugin.maps`
- `plugin.call maps ...`

## Build `plugin.wasm`

From repository root:

- Windows PowerShell: `./scripts/build_maps_plugin.ps1`
- Bash: `./scripts/build_maps_plugin.sh`

This compiles `plugins/maps/wasm` and copies `plugin.wasm` into `plugins/maps/`.

## Input contract (supported)

The plugin accepts daemon payload format:

```json
{
  "tool": "maps_route",
  "plugin_id": "maps",
  "action": "route",
  "args": ["45.698", "0.328", "49.009", "2.547", "train"]
}
```

Or explicit JSON payload through `plugin.call`:

```json
{
  "action": "distance",
  "from": { "lat": 45.698, "lon": 0.328 },
  "to": { "lat": 49.009, "lon": 2.547 },
  "mode": "car"
}
```

Geocoding only (city name to coordinates):

```json
{
  "action": "geocode",
  "query": "Meulan-en-Yvelines"
}
```

City names are also supported directly:

```json
{
  "action": "route",
  "from_text": "Chateaubernard",
  "to_text": "Meulan-en-Yvelines",
  "mode": "car"
}
```

Or CLI-style args:

```json
{
  "tool": "maps_distance",
  "args": ["Chateaubernard", "Meulan-en-Yvelines", "car"]
}
```

Or geocode with tool name:

```json
{
  "tool": "maps_geocode",
  "args": ["Chateaubernard"]
}
```

## Output contract (example)

```json
{
  "ok": true,
  "view": "map",
  "summary": "Estimated route: 420.1 km, 180 min (car)",
  "distance_m": 420100.0,
  "duration_s": 10800.0,
  "mode": "car",
  "resolved_from": "chateaubernard",
  "resolved_to": "meulan-en-yvelines",
  "geometry": {
    "type": "LineString",
    "coordinates": [[0.328, 45.698], [2.547, 49.009]]
  },
  "steps": [
    { "instruction": "Go to destination", "distance_m": 420100.0 }
  ]
}
```

Geocode output example:

```json
{
  "ok": true,
  "view": "table",
  "query": "Chateaubernard",
  "resolved_name": "chateaubernard",
  "lat": 45.6663,
  "lon": -0.3341,
  "confidence": 0.9
}
```

## Tauri / TUI rendering

- Tauri: render `view=map` with polyline and markers.
- TUI: render summary + distance + duration + step list.
