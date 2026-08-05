# Changelog

## Portalis 1.0.1 / backend 0.1.1 — 2026-08-05

### Added

- Added a persisted download-folder selector in Settings, supported on desktop
  and mobile.
- Added startup detection for a stale or mismatched native Rust backend.

### Fixed

- A Dart/Rust bridge mismatch now reports a clear compatibility error instead
  of failing later with a `RangeError` while decoding generated bindings.
- The startup compatibility message now gives the exact Windows release-build
  command needed when Flutter is loading an older native DLL.
- Download-folder changes are treated as restart-required and do not move
  torrents that were already registered.

### Engineering

- Added repository contribution instructions in `AGENT.md`.
- Corrected the Windows build helper to generate bindings from only the
  intended bridged modules.
