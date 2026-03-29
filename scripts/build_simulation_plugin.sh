#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-wasm32-unknown-unknown}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIM_WASM_DIR="$ROOT_DIR/plugins/simulation/wasm"
PLUGIN_DIR="$ROOT_DIR/plugins/simulation"

printf "[simulation] Ensuring target %s is installed...\n" "$TARGET"
rustup target add "$TARGET" >/dev/null

pushd "$SIM_WASM_DIR" >/dev/null
printf "[simulation] Building WASM plugin...\n"
cargo build --release --target "$TARGET"

BUILT="$SIM_WASM_DIR/target/$TARGET/release/akasha_simulation_plugin.wasm"
if [[ ! -f "$BUILT" ]]; then
  echo "Built WASM not found: $BUILT" >&2
  exit 1
fi

cp "$BUILT" "$PLUGIN_DIR/plugin.wasm"
printf "[simulation] plugin.wasm updated at %s/plugin.wasm\n" "$PLUGIN_DIR"
popd >/dev/null
