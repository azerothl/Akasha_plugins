#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-wasm32-unknown-unknown}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MAPS_WASM_DIR="$ROOT_DIR/plugins/maps/wasm"
PLUGIN_DIR="$ROOT_DIR/plugins/maps"

printf '[maps] Ensuring target %s is installed...\n' "$TARGET"
rustup target add "$TARGET" >/dev/null

cd "$MAPS_WASM_DIR"
printf '[maps] Building WASM plugin...\n'
cargo build --release --target "$TARGET"

BUILT="$MAPS_WASM_DIR/target/$TARGET/release/akasha_maps_plugin.wasm"
if [[ ! -f "$BUILT" ]]; then
  echo "Built WASM not found: $BUILT" >&2
  exit 1
fi

cp "$BUILT" "$PLUGIN_DIR/plugin.wasm"
printf '[maps] plugin.wasm updated at %s\n' "$PLUGIN_DIR/plugin.wasm"
