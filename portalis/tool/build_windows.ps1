<#
.SYNOPSIS
  Build the Rust backend DLL and run the Flutter Windows app.
.DESCRIPTION
  - Optionally regenerates flutter_rust_bridge bindings if codegen is available
  - Builds the Rust crate at rust/backend as a cdylib (backend.dll)
  - Runs `flutter pub get` then `flutter run -d windows`

  Requires: Rust toolchain, Visual Studio Build Tools C++ workload, Flutter SDK.
.EXAMPLE
  ./tool/build_windows.ps1              # codegen (if available), build Rust, run Flutter (Windows)
.EXAMPLE
  ./tool/build_windows.ps1 -NoCodegen   # skip codegen, build Rust, run Flutter (Windows)
#>

param(
  [switch]$NoCodegen
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Info($m) { Write-Host "[INFO ] $m" -ForegroundColor Cyan }
function Write-Ok($m)   { Write-Host "[ OK  ] $m" -ForegroundColor Green }
function Write-Warn($m) { Write-Host "[WARN ] $m" -ForegroundColor Yellow }
function Write-Err($m)  { Write-Host "[ERROR] $m" -ForegroundColor Red }

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
$repoRoot   = Resolve-Path (Join-Path $scriptRoot '..')
Set-Location $repoRoot

Write-Info "Repo: $repoRoot"

function Test-Cmd($name) { $null -ne (Get-Command $name -ErrorAction SilentlyContinue) }

if (-not (Test-Cmd cargo))   { Write-Err "cargo not found in PATH"; exit 1 }
if (-not (Test-Cmd flutter)) { Write-Err "flutter not found in PATH"; exit 1 }

function Maybe-Codegen {
  if ($NoCodegen) { Write-Info "Skipping flutter_rust_bridge codegen (per flag)."; return }
  if (-not (Test-Cmd flutter_rust_bridge_codegen)) {
    Write-Warn "flutter_rust_bridge_codegen not installed; skipping codegen."
    return
  }
  Write-Info "Regenerating flutter_rust_bridge bindings..."
  flutter_rust_bridge_codegen generate `
    --rust-root "rust/backend" `
    --rust-input crate `
    --dart-output "lib/bridge_generated" `
    --rust-output "rust/backend/src/api.rs"
  Write-Ok "Codegen complete."
}

function Build-Rust {
  Write-Info "Building Rust backend (release)..."
  Push-Location "rust/backend"
  cargo build --release
  Pop-Location
  $dll = Join-Path "rust/backend/target/release" "backend.dll"
  if (Test-Path $dll) { Write-Ok "Built: $dll" }
  else { Write-Warn "backend.dll not found where expected: $dll" }
}

function Run-Flutter {
  Write-Info "Running Flutter app (Windows desktop)..."
  flutter pub get
  flutter run -d windows
}

Maybe-Codegen
Build-Rust
Run-Flutter

Write-Ok "Done."

