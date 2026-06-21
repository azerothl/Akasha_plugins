param(
  [string]$Target = "wasm32-unknown-unknown"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$wasmDir = Join-Path $root "plugins\homeassistant\wasm"
$pluginDir = Join-Path $root "plugins\homeassistant"

Write-Host "[homeassistant] Ensuring target $Target is installed..."
rustup target add $Target | Out-Null

Push-Location $wasmDir
try {
  Write-Host "[homeassistant] Building WASM plugin..."
  cargo build --release --target $Target

  $built = Join-Path $wasmDir "target\$Target\release\akasha_homeassistant_plugin.wasm"
  if (-not (Test-Path $built)) {
    throw "Built WASM not found: $built"
  }

  $dest = Join-Path $pluginDir "plugin.wasm"
  Copy-Item $built $dest -Force
  Write-Host "[homeassistant] plugin.wasm updated at $dest"
}
finally {
  Pop-Location
}
