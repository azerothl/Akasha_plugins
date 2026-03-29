param(
  [string]$Target = "wasm32-unknown-unknown"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$simWasm = Join-Path $root "plugins\simulation\wasm"
$pluginDir = Join-Path $root "plugins\simulation"

Write-Host "[simulation] Ensuring target $Target is installed..."
rustup target add $Target | Out-Null

Push-Location $simWasm
try {
  Write-Host "[simulation] Building WASM plugin..."
  cargo build --release --target $Target

  $built = Join-Path $simWasm "target\$Target\release\akasha_simulation_plugin.wasm"
  if (-not (Test-Path $built)) {
    throw "Built WASM not found: $built"
  }

  $dest = Join-Path $pluginDir "plugin.wasm"
  Copy-Item $built $dest -Force
  Write-Host "[simulation] plugin.wasm updated at $dest"
}
finally {
  Pop-Location
}
