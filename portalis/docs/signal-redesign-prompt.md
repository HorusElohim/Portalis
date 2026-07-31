# Prompt — implement the "Signal" redesign

Paste everything below the line into Claude in VS Code, with the repo open.

---

Implement the **"Signal"** redesign of the Portalis Flutter UI, from the design
doc `Portalis Refresh.dc.html` (Claude Design project
`ab1bf97d-3349-4f7d-8dfb-86f6de7d7ba0`). Work through it screen by screen,
adding widget tests as you go. Do not implement it in one commit.

## The one rule that overrides the mockup

**Never render a value the Rust model cannot produce.** This codebase has been
cleaned of fabricated UI twice already (a hardcoded "2 admins" pill, a
hardcoded avatar initial, a decorative storage cap, four inert settings
toggles, a cover-image placeholder). Do not reintroduce that class of thing.

The mockup contains several invented values. For each, either **derive it from
the model**, or **leave it out** — never hardcode it, and never show a
plausible-looking zero in place of something unknown. The audit is below; treat
it as part of the spec.

## Design system

Replace the violet accent entirely. Update `lib/theme.dart`:

| Token | Value | Use |
|---|---|---|
| `bg` | `#07090A` | app background |
| `surfaceDeep` | `#0B1110` | screen/phone body |
| `surface` | `#121A18` | cards, rows |
| `surfaceRaised` | `#141D1B` | inputs, search |
| `border` | `rgba(255,255,255,.07)` | hairlines |
| `borderStrong` | `rgba(255,255,255,.12)` | outlined controls |
| `text` | `#E6EDEA` | primary text |
| `textDim` | `#8B9A95` | secondary |
| `textFaint` | `#7C8B86` | mono labels |
| **`signal`** | **`#5CE7A3`** | **only ever means "data is moving"** |
| `signalDim` | `#2FA97A` | gradient end |
| `signalWash` | `rgba(92,231,163,.12)` | tinted fills |
| `ember` | `#F0B357` | torrents only |

`signal` is load-bearing: it marks live transfer and nothing else. A static,
idle, or complete thing must not be mint. `ember` marks torrent-sourced items
exclusively, so the two content types stay distinguishable at a glance.

Typography — add to `pubspec.yaml` and bundle the fonts (do not rely on a
network fetch; the app must render offline):

- **Space Grotesk** 600 — headings, numbers, collection names
- **Instrument Sans** 400/600 — body
- **JetBrains Mono** 400/500 — labels, metrics, hashes, addresses

Keep `PageBody` (`lib/widgets/common.dart`) as the max-width wrapper; the
desktop layout below replaces it only on the wide breakpoint.

## Screens

Current files are in `lib/screens/`. Existing state management is
`Collections.instance` and `SettingsService.instance` (both `ChangeNotifier`
singletons) — keep them, this is a presentation change.

### 1. Shell — new bottom navigation (`root_shell.dart`)

Three tabs: **Collections**, **Transfers**, **You**. Today there is no tab bar
(Settings and User hang off header icons), so this is new structure:

- **Collections** — the existing `HomeScreen`.
- **Transfers** — **new screen**. It is *not* in the mockup as a full screen,
  so design it from the model: every collection with `state == 'downloading'`
  or a non-zero `downloadMbps`/`uploadMbps`, with per-collection progress and
  rates. Empty state when nothing is moving.
- **You** — the existing `UserScreen`. Move Settings to a row inside it (the
  mockup reaches Settings from You, not from a header gear).

### 2. Collections / Home (`home_screen.dart`)

- Header: avatar chip (real nickname initial — already wired), "Portalis".
- Title block: "Moving now" + `N collections · M transfers in flight`.
- **Live transfer hero card** — only when something is actually transferring.
  `RECEIVING`/`SENDING` kicker with a pulsing `LiveDot`, collection name, live
  MB/s, progress bar, `downloaded / total`, collaborator avatar stack.
  When nothing is moving, omit the card entirely rather than showing a dormant
  one.
- Filter chips: All / Sharing / Receiving — derive from `state`
  (`seeding` → Sharing, `downloading` → Receiving).
- Collection rows: thumbnail, name, `N items · size`, and a trailing badge —
  mint `%` while downloading, outlined `SHARING` when seeding, ember `%` for
  `CollectionKind.torrent`.
- FAB (+) bottom-right above the nav bar → the share flow.

### 3. First run / empty (`home_screen.dart` empty state)

Concentric pulsing rings behind a mint upload glyph; "Send anything, straight
to a friend"; primary **Share something**; two secondary cards **Join with a
key** (mint) and **Add a torrent** (ember); footer
`NO ACCOUNT · NOTHING LEAVES THIS DEVICE UNASKED`.

Keep the existing distinction between *empty* and *backend failed* — there is
already a `_CollectionsError` state and a test asserting the two differ. Do not
collapse them.

### 4. Create & share (`share_screen.dart`)

`STEP 1 OF 2` header, large inline-editable collection name with a blinking
caret, three source chips (Photos / Files / Folder), `N items` + total size,
3-column grid of picked items with per-item remove, the privacy note, and
**Create & get link**. Calls `Collections.instance.createWithMedia`.

### 5. Join (`join_collection_screen.dart`)

"Got an invite?", monospace invite-key field with mint focus ring, a validation
line under it, **Paste** / **Scan QR** buttons, **Join collection**, and
"You'll appear as *nickname*" (already real).

The mockup's validation line reads `VALID KEY · ANA'S MACBOOK · 128 PHOTOS`.
We can only decode the collection **name** and **address count** from an invite
code before joining — there is no device name or item count in it. Show what
parses (`VALID KEY · <name> · N ADDRESSES`) and nothing more. The existing
screen already parses this; keep that logic.

Drop **RECENTLY NEARBY** — see the audit.

### 6. Add torrent (`add_torrent_screen.dart`)

Ember accent throughout. Magnet field, Paste / .torrent file buttons, save-to
row (from `torrent::output_dir`), **Start download**.

Drop the pre-add preview card (name / size / SEEDS / PEERS / ETA) — see the
audit.

### 7. Settings (`settings_screen.dart`)

Restructure the existing screen into progressive disclosure:

- Search field — client-side filter over the rows.
- **Health card** — see the audit for exactly which of its three claims are
  real.
- **SPEED** — upload and download limit (live-applied, already wired).
- **SHARING** — friendly toggles over real `EngineSettings` fields:
  "Keep sharing after restart" → `persistSession`; "Find friends on this
  Wi-Fi" → **omit**, see audit.
- **Network & engine** disclosure row → pushes the full advanced screen that
  the current `settings_screen.dart` already implements (ports, DHT, proxy,
  trackers, blocklist, timeouts, performance). Keep every one of those; they
  are all real. Preserve the restart-required banner.

### 8. You (`user_screen.dart`)

Avatar, name, `THIS DEVICE · <short device id>`, **Change name** pill, 2×2
stat cards, **Your address** with copy, identity note.

See the audit for which stats are real.

### 9. Desktop — three panes

At `width >= 1000`, replace the mobile layout with:

- **Sidebar (236px)**: identity chip, "New share" primary, nav
  (Collections / Transfers / People / Settings) with counts.
- **Centre**: title, filter chips, collection rows with inline progress.
- **Inspector (314px)**: selected collection — name, `SHARED WITH n · KEY …`,
  **Copy invite key** + QR button, peer list, footer note.

Below that width, the mobile layout with bottom nav. Use a `LayoutBuilder`;
do not branch on `Platform`, since a narrow desktop window should get the
mobile layout.

Drop the sidebar sparkline and the cover-image drop zone — see the audit.

---

## Model audit — what is real, what must be cut

Backed by the model today (use freely):

- Per collection: `name`, `kind`, `inviteCode`, `collaborators`
  (`deviceId`/`name`/`isAdmin`), `media`, `entries`, `progress`, `totalBytes`,
  `downloadedBytes`, `uploadedBytes`, `downloadMbps`, `uploadMbps`,
  `livePeers`, `pendingMedia`, `state`
- Device: `nickname`, `deviceId`, sync address (`Collections.syncAddress()`)
- Engine: every `EngineSettings` field, `storageUsageBytes`

**Cut or change these — the mockup invents them:**

| Mockup element | Why | What to do |
|---|---|---|
| `2 DEVICES NEARBY` | No device discovery exists. Phase 3 (DHT rendezvous) is unstarted. | Cut. |
| **RECENTLY NEARBY** card ("Ana's MacBook · SAME WI-FI") | Same. | Cut the whole card. |
| Torrent preview: file name, size, `1 FILE`, `VERIFIED HASH`, `128 SEEDS`, `14 PEERS`, `~4m ETA` | We never parse a magnet before adding it, and `TorrentInfo` has only `livePeers` — no seed/peer split, no ETA. | Cut the preview. Show these *after* adding, using real fields, on the collection screen. |
| `SENT 18.4 GB` / `RECEIVED 42.1 GB` | `uploadedBytes`/`downloadedBytes` are per-collection and per-session; there is no lifetime counter. | Either label them honestly as this session's totals, or cut. Do not imply lifetime. |
| `PEOPLE 7` | Derivable: distinct `collaborators.deviceId` across all collections. | Keep — compute it. |
| Health card `PORT OPEN` | Not verifiable. We know the bound port, not whether it is reachable. | Cut, or restate as `PORT <n>` from `bt_listen_port`. |
| Health card `DHT ON` | Real — from `EngineSettings.disableDht`. | Keep. |
| Health card `14 PEERS` | Real — sum of `livePeers`. | Keep. |
| "Everything is healthy" | Asserts a check we do not perform. | Only claim it from conditions actually evaluated, or use a neutral summary. |
| "Find friends on this Wi-Fi" toggle | No mDNS/LAN discovery exists. | Cut. |
| **Back up identity** | No export exists in `device.rs`. | Cut, or implement the export first — do not add a dead button. |
| Cover image / "COVER IMAGE — DROP HERE" | Not modeled. This exact placeholder was deliberately removed. | Cut. |
| Sidebar "THIS SESSION" sparkline | No rate history is retained. | Cut, or add a bounded in-memory ring buffer of samples first. |
| Inspector per-peer rates ("Ana ↓ 28.1") | No peer-identity ↔ rate mapping exists. Collaborators come from the manifest; rates are per-torrent. | Show collaborator names without per-peer rates. |

If you think one of these is actually derivable, say so and show the derivation
before implementing it.

## Tests

The suite is `test/widget_test.dart` (10 tests, all passing). Extend it; do not
rewrite it. `RustLib` is uninitialized under test, so backend calls throw —
that is the existing convention and several tests rely on it.

Add at least:

1. **Theme** — `AppColors.signal` is the mint value; no widget in
   `lib/` references the retired violet (a grep-style assertion over the token
   set is fine).
2. **Bottom nav** — three tabs render; tapping each swaps the body; the
   selected tab is the only one using `text` colour.
3. **Home, live** — a `Collection` with `downloadMbps > 0` renders the hero
   card with its name and rate.
4. **Home, idle** — a collection with no movement renders **no** hero card.
   (This is the "signal only means moving" rule, asserted.)
5. **Filter chips** — Sharing shows only `seeding`, Receiving only
   `downloading`.
6. **Torrent row** — a `CollectionKind.torrent` collection uses ember, a shared
   one uses mint.
7. **Empty vs failed** — keep the existing assertion that these differ.
8. **Transfers tab** — lists only moving collections; empty state otherwise.
9. **You** — stat cards read from the model; "People" equals the count of
   distinct collaborator device ids across collections.
10. **Desktop breakpoint** — at 1200×800 the three-pane layout renders and the
    bottom nav does not; at 390×844 the reverse.
11. **Settings** — the advanced engine rows are reachable behind "Network &
    engine", and the restart banner still appears for a construction-time
    field.

Use `tester.binding.setSurfaceSize` for breakpoint tests and construct
`Collection`/`MediaItem` directly (they have `const` constructors) rather than
mocking the bridge.

## Working agreement

- Commit per screen, not one big commit. Run `flutter analyze` and
  `flutter test` before each; both must be clean.
- Match the surrounding comment style: explain *why*, especially where you
  depart from the mockup.
- Where you cut something from the audit, leave a brief comment saying what was
  cut and what would have to exist to bring it back.
- Do not commit unless asked. When staging, review `git status` before
  committing — do not `git add -A` blind.
