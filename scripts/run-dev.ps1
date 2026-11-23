$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (Test-Path "$env:USERPROFILE\.cargo\bin") {
  $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
}
if (Test-Path "$env:USERPROFILE\.xwin\llvm\bin") {
  $env:Path = "$env:USERPROFILE\.xwin\llvm\bin;$env:Path"
}

Write-Host "Building SubPrism Liquid Glass UI..." -ForegroundColor Cyan
Push-Location "$root\ui"
if (Get-Command bun -ErrorAction SilentlyContinue) {
  bun run build
} else {
  npm run build
}
Pop-Location

Write-Host "Launching SubPrism (Release)..." -ForegroundColor Green
cargo run --release -p subprism
