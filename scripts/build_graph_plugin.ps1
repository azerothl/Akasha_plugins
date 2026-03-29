param(
  [string]$Target = "wasm32-unknown-unknown"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$graphWasm = Join-Path $root "plugins\graph\wasm"
$pluginDir = Join-Path $root "plugins\graph"

Write-Host "[graph] Ensuring target $Target is installed..."
rustup target add $Target | Out-Null

Push-Location $graphWasm
try {
  Write-Host "[graph] Building WASM plugin..."
  cargo build --release --target $Target

  $built = Join-Path $graphWasm "target\$Target\release\akasha_graph_plugin.wasm"
  if (-not (Test-Path $built)) {
    throw "Built WASM not found: $built"
  }

  $dest = Join-Path $pluginDir "plugin.wasm"
  Copy-Item $built $dest -Force
  Write-Host "[graph] plugin.wasm updated at $dest"
}
finally {
  Pop-Location
}
