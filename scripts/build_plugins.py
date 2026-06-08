#!/usr/bin/env python3
"""Build plugins.json from repository plugin.json files."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
PLUGINS_DIR = ROOT / "plugins"
OUT_FILE = ROOT / "plugins.json"


def iter_plugin_json_files() -> list[Path]:
    return sorted(PLUGINS_DIR.glob("*/plugin.json"))


def load_plugin_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    data.setdefault("id", path.parent.name)
    data.setdefault("path", str(path.parent.relative_to(ROOT)).replace("\\", "/"))
    wasm = path.parent / "plugin.wasm"
    if wasm.is_file():
        h = hashlib.sha256()
        with wasm.open("rb") as wf:
            for chunk in iter(lambda: wf.read(1024 * 1024), b""):
                h.update(chunk)
        data["wasm_sha256"] = h.hexdigest()
    else:
        data.pop("wasm_sha256", None)
    return data


def main() -> None:
    items: list[dict] = []
    for plugin_json in iter_plugin_json_files():
        items.append(load_plugin_json(plugin_json))

    previous_generated_at: str | None = None
    if OUT_FILE.is_file():
        try:
            previous = json.loads(OUT_FILE.read_text(encoding="utf-8"))
            if previous.get("plugins") == items:
                previous_generated_at = previous.get("generated_at")
        except json.JSONDecodeError:
            previous_generated_at = None

    output = {
        "generated_at": previous_generated_at or datetime.now(timezone.utc).isoformat(),
        "count": len(items),
        "plugins": items,
    }

    with OUT_FILE.open("w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"Wrote {OUT_FILE} with {len(items)} plugin(s)")


if __name__ == "__main__":
    main()
