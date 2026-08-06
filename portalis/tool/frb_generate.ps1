<#
.SYNOPSIS
  Regenerate the Flutter-Rust bridge without building or launching the app.
.DESCRIPTION
  Run this from PowerShell after changing a bridged Rust DTO or API. The
  relative paths are intentional: they avoid the Windows prefix-resolution
  error produced by mixing absolute and slash-normalised paths.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($null -eq (Get-Command flutter_rust_bridge_codegen -ErrorAction SilentlyContinue)) {
  throw 'flutter_rust_bridge_codegen is not installed. Run: cargo install flutter_rust_bridge_codegen'
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $repoRoot

flutter_rust_bridge_codegen generate `
  --rust-root 'rust/backend' `
  --rust-input 'crate::bridge,crate::torrent,crate::device,crate::collections,crate::settings' `
  --dart-output 'lib/bridge_generated' `
  --rust-output 'rust/backend/src/api.rs' `
  --no-add-mod-to-lib

Write-Host 'Flutter-Rust bindings regenerated.' -ForegroundColor Green
