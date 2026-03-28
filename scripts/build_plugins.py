#!/usr/bin/env python3
"""Build plugins.json from plugins/*/plugin.json files."""

from __future__ import annotations

import json
from pathlib import Path
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
PLUGINS_DIR = ROOT / "plugins"
OUT_FILE = ROOT / "plugins.json"


def load_plugin_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    data.setdefault("id", path.parent.name)
    data.setdefault("path", str(path.parent.relative_to(ROOT)).replace("\\", "/"))
    return data


def main() -> None:
    items: list[dict] = []
    for plugin_json in sorted(PLUGINS_DIR.glob("*/plugin.json")):
        items.append(load_plugin_json(plugin_json))

    output = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "count": len(items),
        "plugins": items,
    }

    with OUT_FILE.open("w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"Wrote {OUT_FILE} with {len(items)} plugin(s)")


if __name__ == "__main__":
    main()
