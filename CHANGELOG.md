# Changelog

## Unreleased

### Added

<!-- New user-visible features go here before the next release. -->

- Added truthful per-media torrent piece activity. Verified byte ranges now
  appear at their real relative positions around media previews, while live
  worker markers appear only for peers the engine has actually assigned to
  in-flight pieces; sequential downloads remain sequential instead of being
  visually spread across unrelated files.

- Completed Nexus M4 encrypted shares and snapshots. Private share discovery,
  fetches, live events, key delivery, and transient handoffs now require
  membership; encrypted capsules are bounded and immutable; publication uses
  transactional snapshot history and compare-and-swap heads so revisions
  cannot regress. The portable client exposes the complete flow.

- Completed Nexus M5 swarm discovery. Authenticated seeders announce bounded
  short-lived leases using the IP observed by the socket, lookups prefer
  compatible recent peers across diverse network prefixes, and disconnects
  or expiration remove stale endpoints. Client candidates from direct,
  Nexus, tracker, and DHT discovery now merge without duplicates.

- Added Nexus M2.5 encrypted share-key delivery between a user's approved
  devices. Nexus stores only per-device X25519 envelopes, never plaintext
  share keys or media metadata.

### Changed

<!-- Behaviour and UX changes go here before the next release. -->

- Moved every Nexus timestamp to nanoseconds since the Unix epoch, on the
  wire, in the domain, and in storage. `Envelope.sent_at_unix_ms` is now
  `timestamp_unix_ns`, and `server_time_unix_ms` and `last_seen_unix_ms`
  follow the same `*_unix_ns` naming. Field numbers are unchanged, so this
  breaks the protocol without breaking binary decoding: a client built
  against the old schema still reads the field and interprets nanoseconds as
  milliseconds. Nexus is not deployed, so no compatibility window applies.
  `UUIDv7` keeps milliseconds internally, since its 48-bit timestamp is
  defined in them and nanoseconds would wrap it every three days.

### Fixed

<!-- Bug fixes and regressions go here. -->

- Replaced the ambiguous transfer-history sparkline with a labeled speed
  chart: download and upload now state whether a value is current or a peak,
  the vertical scale has units, the session duration and endpoints are
  explicit, and completed transfers no longer present `0.0 MB/s` as though it
  explained the historical line. Expanded collection cards now let progress,
  peers, remaining time, and history sit directly in the collection without a
  nested panel or empty-chart frame. Completed histories label their final
  timestamp as `END`, including older records without an explicit completion
  time. Expanded collection identity, progress, current/peak speed, and actions
  now share one responsive header above the graph instead of occupying separate
  vertical sections. Its action dock uses compact icon controls for lifecycle
  and destructive commands plus short labeled collection actions, keeping the
  full command set on one line at desktop widths. The progress track now closes
  the transfer visualization beneath the graph instead of separating its
  header from the chart. Transfer plots now use a logarithmic vertical scale
  so simultaneous upload and download remain visible despite large speed
  differences, and either direction keeps the temporal graph on screen.
  Collection actions no longer expose `Forget` or a files-only deletion;
  one `Delete` confirmation now offers `Cancel`, `Delete`, and `Delete with
  files`, with both destructive choices removing the collection. Media
  previews now stay tiny on wide collection cards instead of stretching with
  the four-column layout.
- Shortened the empty Home welcome to `Send anything to anybody`; all welcome
  copy now lingers briefly, then softly fades and collapses, replaying whenever
  Home is entered again while the logo remains available.
- Peer observations now show only their compact age (`4s`, `2m`, `1h`); active
  torrent peers retain their live ember treatment while remembered peers that
  are no longer connected receive a quieter, stable identity color of their
  own.

- Fixed the Settings efficiency benchmark reporting `0 ms`. The run is
  sub-millisecond, and whole milliseconds rounded it away, which reads as a
  broken benchmark rather than a fast one. It now measures in nanoseconds
  like the rest of Portalis — `Stopwatch` ticks that finely, and it was
  `Duration` discarding the precision — and scales the label to the run. The
  card also shows the cost of one operation, and a run too fast for the clock
  no longer reports an infinite rate.

- Fixed a Nexus key-envelope write that could drop a rotated share key while
  reporting a storage outage: two devices pushing for the same share and
  recipient at once made the unique index reject the loser, which is a lost
  race to retry rather than a failure to report.

- Hardened Nexus key envelopes against low-order X25519 keys and record
  transplanting, and bounded retrieval with deterministic cursor pagination.

### Engineering

<!-- Tests, tooling, refactors, and maintenance notes go here. -->

- Closed the Nexus M4 and M5 coverage gap. Both new handlers shipped without
  test modules, so the share and swarm commands are now covered at the socket
  boundary: every command refusing an unauthenticated connection, malformed
  identifiers, private shares answering the same way whether or not they
  exist, members hearing a publication, handoffs reaching only devices that
  may already read the share, and each domain refusal reaching the wire as a
  typed code. Also covers the publication retry loop — losing a
  compare-and-set to an identical publication, and exhausting the retries —
  the in-memory store's refusal to move a head that moved or rewrite a
  published revision, the swarm's one-peer-per-network pass filling a
  response on its own, and the storage failures inside `fetch` and `grant`.

- Removed an unreachable `unreachable!` from share publication by deciding the
  write's precondition and the identical-retry answer in one match, so the
  fourth case no longer exists to be impossible.

- Closed the Nexus M2.5 coverage gap, restoring the 100% line and function
  gate. Covers the durable key-envelope store against a real replica set
  (rotation replacing its predecessor, listings scoped to the device they
  name, and the page boundary where MongoDB's ordering of binary values has
  to agree with the page type), the key-envelope handler, the client's
  envelope builders and page parsing, and opening an envelope that names a
  low-order ephemeral key.

- Serialized Docker-backed Nexus MongoDB integration tests and bounded every
  Docker readiness, connection, and shutdown wait so an unhealthy daemon
  reports a clear failure instead of hanging the suite.

- Started Portalis Nexus as an isolated Rust workspace for the new protobuf
  control-plane protocol and Linux discovery server, with portable client and
  server-core crates, health endpoints, container packaging, and dedicated CI.
- Fixed the Nexus coverage gate to use stable `cargo-llvm-cov` line, function,
  and region thresholds instead of unsupported branch-threshold flags.
- Added the first Nexus WebSocket transport slice: bounded protobuf frames,
  subprotocol negotiation, validated server hello, and correlated ping/pong
  with a real client/server integration test.
- Added bounded exponential reconnect with randomized jitter to the portable
  Nexus client, verified by two clients reconnecting after a forced restart.
- Gave each Nexus socket a bounded outbound queue and a dedicated writer task,
  so a peer that stops reading is disconnected instead of growing server memory.
- Added graceful Nexus socket draining, so shutdown signals every live
  connection and waits for it to close within a bounded timeout.
- Completed the Nexus connection lifecycle: the client is now a supervised
  handle that correlates concurrent requests, times them out, reconnects on its
  own, and exposes server-initiated envelopes as an event stream.
- Split the Nexus crates into focused modules, each covered by its own tests.
- Added the Nexus identity contract: registration and device-authentication
  messages, domain-separated signing payloads that bind a signature to one
  operation, connection, server, and challenge, BLAKE3 device identifiers
  derived from existing Ed25519 keys, and user handle rules.
- Added the Nexus registration and device-authentication rules over injected
  storage, time, and randomness: single-use expiring challenges, handle
  allocation that retries random discriminators against the unique index, and
  device revocation, writing each user and its first device atomically.
- Wired Nexus registration and device authentication end to end over the socket,
  with per-connection challenge state, typed refusal codes, and client commands
  that sign through a caller-supplied device signer. Storage remains in-memory
  until the durable adapter lands.
- Added a Nexus demo: a narrated walkthrough of the whole identity flow and a
  two-process client that persists its device key. Running it found a bug where
  a connection stamped its hello and its challenge from two separate clock
  readings, so every signature failed whenever the two straddled a millisecond.
- Added the Nexus friendship contract and state machine: one canonical edge per
  pair, idempotent commands, and versioned transitions for deterministic
  concurrent accepts, rejects, and removals, plus the friend service over it:
  handle resolution, friend listing, and commands that re-read and re-apply
  when another side writes first.
- Separated the Nexus server's layers: the socket moves bytes, the session
  holds who a connection is, and a handler module per subsystem decides what a
  command means. Adding a subsystem no longer changes transport code, and the
  protobuf build discovers schemas instead of listing them.
- Added Nexus presence aggregation across a user's devices, and served handle
  resolution, friend commands, and friend listing over the socket, verified by
  two clients becoming friends end to end.
- Completed Nexus friends and presence: events reach accepted friends only, a
  connection learns where its friends stand on arrival, and only the last
  device leaving reports a user away.
- Added transport integration suites for connection, reconnect, and event
  behaviour, which caught three client defects: a shutdown requested before the
  supervisor first ran was never observed, tearing down a peer-closed socket
  panicked the supervisor and stranded in-flight requests, and a handshake
  against a peer that never greeted could stall forever.
- Added durable MongoDB-backed Nexus identity and friendship storage with
  transactional registration, unique indexes, optimistic friendship writes,
  and real replica-set integration coverage. The server now refuses to start
  without a configured and reachable MongoDB instance.

## Portalis 1.0.8 / backend 0.1.6 — 2026-08-07

### Added

<!-- New user-visible features go here before the next release. -->

- Added native, bounded HEIC/HEIF preview decoding for iOS, macOS, and
  Android without changing or duplicating the original media file.

### Changed

<!-- Behaviour and UX changes go here before the next release. -->

- Moved every Nexus timestamp to nanoseconds since the Unix epoch, on the
  wire, in the domain, and in storage. `Envelope.sent_at_unix_ms` is now
  `timestamp_unix_ns`, and `server_time_unix_ms` and `last_seen_unix_ms`
  follow the same `*_unix_ns` naming. Field numbers are unchanged, so this
  breaks the protocol without breaking binary decoding: a client built
  against the old schema still reads the field and interprets nanoseconds as
  milliseconds. Nexus is not deployed, so no compatibility window applies.
  `UUIDv7` keeps milliseconds internally, since its 48-bit timestamp is
  defined in them and nanoseconds would wrap it every three days.

- Restyled shared collection rows with the same visual energy as torrent rows,
  including an accented preview tile and a clear shared-state fallback.

## Portalis 1.0.7 / backend 0.1.6 — 2026-08-07

### Added

<!-- New user-visible features go here before the next release. -->

- Added a Future theme — an electric cyan/violet palette drawn from the new
  app icon — as an alternative to the original mint-green Nature theme.
  Switch between them from Settings → Appearance; the choice persists across
  restarts.
- Added iOS Files open-in-place sharing: selected sources retain security-
  scoped, read-only access across restarts while Rust seeds the original file.
- Added no-copy iOS Photos & Videos selection backed by Rust-owned linked
  gallery sources.

### Changed

<!-- Behaviour and UX changes go here before the next release. -->

- Moved every Nexus timestamp to nanoseconds since the Unix epoch, on the
  wire, in the domain, and in storage. `Envelope.sent_at_unix_ms` is now
  `timestamp_unix_ns`, and `server_time_unix_ms` and `last_seen_unix_ms`
  follow the same `*_unix_ns` naming. Field numbers are unchanged, so this
  breaks the protocol without breaking binary decoding: a client built
  against the old schema still reads the field and interprets nanoseconds as
  milliseconds. Nexus is not deployed, so no compatibility window applies.
  `UUIDv7` keeps milliseconds internally, since its 48-bit timestamp is
  defined in them and nanoseconds would wrap it every three days.

- Removed the misleading iOS Photos import promise. iOS now offers Files as
  the no-copy source and never stages picker content in the app cache.
- Made collection synchronization automatically request and retry BitTorrent
  fetches when new manifest entries arrive, using direct peer hints when
  available.

### Fixed

<!-- Bug fixes and regressions go here. -->

- Silenced repetitive "peer stats unavailable" logging for torrents that
  simply aren't live yet (checking, queued, or peerless) — it was firing on
  every 500ms poll instead of only when a live torrent's peer stats
  genuinely failed.
- Silenced repetitive background collection, sync-address, and identity
  restoration logs.
- Fixed near-complete transfers displaying 100% before their final bytes, and
  stopped anonymous peer cards from repeating collection-level speed and size
  as per-peer data.
- Reduced live transfer lag by coalescing overlapping refreshes and moving
  history persistence off the snapshot update path.
- Kept the internal hard-link layout for multi-file shares in Portalis' private
  state directory so source files no longer appear as copied files in Downloads.

### Engineering

<!-- Tests, tooling, refactors, and maintenance notes go here. -->

- Added CocoaPods integration for the media plugins on iOS and macOS, and
  updated the iOS Flutter bridge and Files picker for current Flutter/Xcode
  APIs so unsigned release builds compile on both Apple platforms.
- Added an opt-in two-process integration test that verifies encoded invites,
  manifest synchronization, automatic fetching, torrent hashes, and file
  contents.

## Portalis 1.0.6 / backend 0.1.5 — 2026-08-06

### Added

- Added pooled collaborator transfer summaries to People cards, including
  current rate, shared collection count, and total shared bytes.

### Changed

- Removed the redundant empty-search action and centered Share files button;
  the Portalis logo is now the single share affordance and remains below the
  collection content.
- Made filesystem publication one-copy: a single file seeds from its original
  path, while multi-file layouts use hard links and refuse a hidden copy when
  linking is unavailable.
- Stopped automatic mobile gallery imports because Photos/MediaStore imports
  create a duplicate while the original must remain available for seeding.
- Temporarily refuse mobile path-picker sharing and add-media flows: picker
  cache paths cannot satisfy Portalis' no-copy contract before the native URI
  storage adapter exists.

### Fixed

- Prevented a hidden fallback copy when a desktop multi-file share cannot use
  hard links, keeping the one-copy storage guarantee intact.

### Engineering

- Documented canonical media storage, including the Android MediaStore adapter
  boundary and the one-copy iOS Files-library policy.
- Introduced Rust's canonical content-location boundary, which rejects an
  unsupported `content://` URI instead of converting it into a copied cache
  file.
- Added the Android application-context bootstrap required for Rust-owned
  MediaStore storage; it transfers no media bytes through Flutter.

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
