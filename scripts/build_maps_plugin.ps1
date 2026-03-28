param(
  [string]$Target = "wasm32-unknown-unknown"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$mapsWasm = Join-Path $root "plugins\maps\wasm"
$pluginDir = Join-Path $root "plugins\maps"

Write-Host "[maps] Ensuring target $Target is installed..."
rustup target add $Target | Out-Null

Push-Location $mapsWasm
try {
  Write-Host "[maps] Building WASM plugin..."
  cargo build --release --target $Target

  $built = Join-Path $mapsWasm "target\$Target\release\akasha_maps_plugin.wasm"
  if (-not (Test-Path $built)) {
    throw "Built WASM not found: $built"
  }

  $dest = Join-Path $pluginDir "plugin.wasm"
  Copy-Item $built $dest -Force
  Write-Host "[maps] plugin.wasm updated at $dest"
}
finally {
  Pop-Location
}
