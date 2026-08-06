# Changelog

## Unreleased

### Added

<!-- New user-visible features go here before the next release. -->

### Changed

<!-- User-visible behavior, UX, or architecture changes go here. -->

### Fixed

<!-- Bug fixes and regressions go here. -->

### Engineering

<!-- Tests, tooling, refactors, and maintenance notes go here. -->

## Portalis 1.0.5 / backend 0.1.4 — 2026-08-06

### Added

- Added live native import progress for collection creation and media batches,
  covering bounded copying, torrent hashing, seed startup, and visible failures.
- Added durable Rust import descriptors so an interrupted large share resumes
  from the same staging batch after Portalis restarts instead of becoming an
  empty collection with lost progress state.

### Changed

- Unified compact and wide window navigation under one adaptive shell state,
  preserving open collections and add flows while resizing on Windows.
- Tightened collection previews around a fuller first media row, grouped
  Invite and Add media with lifecycle actions, removed manual Sync from the
  collection view, and added a compact media-details layout with refresh.
- Moved collection publication to Rust-owned native file paths: Flutter now
  sends lightweight descriptors while Rust validates, copies, hashes, and
  seeds files without loading their contents into Dart or the FFI bridge.
- Made each published batch an immutable Portalis-owned snapshot in the
  configured download folder, preventing later source edits or deletion from
  corrupting pieces already announced to the swarm.
- Preserved selected media byte-for-byte, including HEIC originals, instead of
  rewriting files in the frontend before sharing.

### Fixed

- Fixed narrow mobile collection peer chips overflowing and prevented the
  Settings efficiency benchmark from leaving pending test timers.
- Removed the one-shot byte payload that could overflow the native bridge when
  creating a large share; share size is now bounded only by filesystem,
  platform, and BitTorrent implementation constraints.
- Made atomic state replacement work when the destination JSON already exists
  on Windows, including repeated import-progress persistence.

### Engineering

- Removed the frontend HEIC conversion dependency and obsolete payload-limit
  tests now that ingestion has one Rust-owned path.
- Regenerated Flutter-Rust Bridge bindings around `SourceFile` and native
  `ImportInfo`, and made the Windows helper discover Cargo-installed codegen
  from Git Bash.
- Added coverage for large persisted import descriptors, portable source-name
  validation, active-versus-failed polling, atomic state replacement, and the
  unified responsive navigation contract.

## Portalis 1.0.4 / backend 0.1.3 — 2026-08-06

### Added

- Added real collection lifecycle controls for restart, pause, stop, remove,
  and deleting local files with confirmation.
- Added a shared live transfer panel with prominent progress, rates, peers,
  ETA, and download/upload activity graph.
- Added persistent anonymous torrent-peer history with last-seen timestamps
  and explicit forget actions in People and collection details.
- Added persistent transfer history so collection download graphs and
  completion dates survive application restarts.
- Added first-frame video thumbnails and unified cross-platform video playback
  in the media viewer.
- Added a dedicated User destination for device identity and a local efficiency
  benchmark that refreshes whenever Settings is entered.

### Changed

- Made Stop non-destructive: it now halts transfer activity while retaining the
  collection and torrent for later restart.
- Exposed the complete collection command strip directly on every collection
  card; desktop expansion now focuses on the detailed preview without repeating
  those controls.
- Removed duplicate titles from expanded collection cards, compacted their
  command buttons, and increased spacing around filters and collection rows.
- Reworked transfer previews around polling-backed download/upload history from
  transfer start to current or completion, with date-time labels and a lighter
  energy treatment.
- Replaced the separate New share and Add torrent controls with one command
  input; Enter on an empty input opens the add flow and `.torrent` selection is
  restricted to torrent files.
- Unified collection detail presentation across mobile and desktop, removed the
  redundant information toggle, and replaced the desktop sidebar with a top
  identity and event rail.
- Moved transient notifications to the energized top event rail.
- Separated User and Settings across mobile and desktop navigation so identity
  stays distinct from engine configuration.
- Unified named collaborators and anonymous torrent peers into one collection
  view peer section, retaining last-seen details and forget actions.
- Enabled inline preview attempts for MKV and AVI movies, with external-player
  fallback when the platform codec is unavailable.
- Made media details visible by default and replaced the Windows-only playback
  path with a broader cross-platform codec backend.
- Expanded the media viewer preview to use most of the available viewport while
  keeping transfer facts and details accessible below it.
- Made the Portalis logo a persistent Home button in the desktop header.
- Aligned the Flutter-Rust Bridge toolchain and generated bindings to 2.12.0.

## Portalis 1.0.3 / backend 0.1.2 — 2026-08-05

### Changed

- Reorganized the Flutter frontend around explicit Collections and Media
  feature boundaries, with route-level screens and reusable presentation
  components colocated with their feature.
- Made Media independently reusable: file metadata, format capabilities, HEIC
  conversion, thumbnails, and the in-app viewer now live under
  `features/media`.
- Moved the remaining cross-feature visual primitives into `design/`; removed
  the transitional `ui/` implementation layer.

### Engineering

- Kept collection transfer, collaboration, and persistence state in the
  Collections feature while exposing `MediaPreview` as the extension point for
  future in-app image, text, audio, document, and richer video previews.
- Updated tests and imports to use feature-level public paths.

## Portalis 1.0.2 / backend 0.1.2 — 2026-08-05

### Fixed

- State files that cannot be read are now reported as errors instead of being
  mistaken for a first launch and overwritten.
- Settings accepts both normal and typographic dashes in the listen-port range.

### Changed

- Consolidated Flutter state into feature controllers, repositories, and pure
  domain models, with one application composition root.
- Confined Flutter-Rust Bridge DTOs to bootstrap and feature data adapters.
- Split Home and Settings into focused presentation components while keeping
  the same adaptive desktop and compact layouts.
- Renamed the backend sync scheduler around its actual responsibility:
  reconciliation.

### Engineering

- Added focused tests for listen-port parsing, settings copies, and vault read
  failures.
- Removed obsolete model, UI-barrel, and singleton-service compatibility code.

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
