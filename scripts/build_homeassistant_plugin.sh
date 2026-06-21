#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-wasm32-unknown-unknown}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$ROOT/plugins/homeassistant/wasm"
PLUGIN_DIR="$ROOT/plugins/homeassistant"

echo "[homeassistant] Ensuring target $TARGET is installed..."
rustup target add "$TARGET" >/dev/null

pushd "$WASM_DIR" >/dev/null
echo "[homeassistant] Building WASM plugin..."
cargo build --release --target "$TARGET"
BUILT="$WASM_DIR/target/$TARGET/release/akasha_homeassistant_plugin.wasm"
if [[ ! -f "$BUILT" ]]; then
  echo "Built WASM not found: $BUILT" >&2
  exit 1
fi
cp "$BUILT" "$PLUGIN_DIR/plugin.wasm"
echo "[homeassistant] plugin.wasm updated at $PLUGIN_DIR/plugin.wasm"
popd >/dev/null
