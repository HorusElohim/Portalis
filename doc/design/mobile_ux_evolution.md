# Portalis v2: product, UX, and core architecture brief

## Release boundary

`Portalis 1.0.3 / backend 0.1.2` is the current released baseline. Do not
rewrite its changelog, alter its bridge expectation, or silently change its
persistent collection semantics.

V2 starts from that baseline in a dedicated development line:

- A UI-only v2 iteration may continue to read backend `0.1.2`; it must keep
  the frontend compatibility expectation at `0.1.2`.
- Per-collection lifecycle commands require a new Rust/Flutter contract. Ship
  that as a paired release: frontend `2.0.0` and backend `0.2.0`, regenerate
  Flutter-Rust Bridge bindings, and rebuild both together.
- Existing persisted collections are migration input, never disposable test
  data. Back up and test them before any operation that changes their meaning.

## What is wrong with the current Home

The current `Home` screen makes one controller responsible for two different
products: a library of collections and a live transfer dashboard. It then
shows the fastest transfer as a large `LiveTransferCard` and again as a
`CollectionRow` in the same list. This is the duplicate visible on mobile.

The screen also permanently stacks identity, hero transfer, smart input, two
creation controls, an aggregate transfer line, filters, and a list. Each item
is individually defensible; together they leave no hierarchy.

V2 must not rearrange that stack. It replaces it with a single purpose:

> **Home is the collection library. A live transfer is a collection in that
> library, not a second destination or a second card.**

## V2 mobile product model

### Normal state

```text
compact app bar: Portalis mark + connection health + profile entry
smart input: paste a magnet/invite or search collections
collections list: one row/card per collection
contextual add button: opens Share / Join / Add torrent actions
bottom navigation
```

There is no permanent hero transfer, permanent aggregate transfer card, or
permanent filter-chip row.

- If transfers are active, show a small live count/rate indicator in the app
  bar. Tapping it applies an **Active** filter; it is not a second transfer
  view.
- Search owns filtering. A filter button appears only when it is useful and
  opens a compact sheet for All, Active, Shared, and Torrents.
- The add button opens one explicit action sheet. The empty state can promote
  its primary action, but normal library state does not need a wide “New
  share” button above the list.
- The Portalis mark is stable app identity, not decoration repeated in every
  surface.

### Collection row

Each collection has one adaptive row/card, ordered by activity and then recent
change. It contains only information needed to decide whether to open it:

```text
type/status mark      collection name                 overflow menu
state label           42%                             (only valid commands)
progress bar
1.2 / 2.8 GB · 3m left · 8 peers · ↓ 4.2 MB/s
```

- A determinate transfer always shows a percentage and a progress bar.
- Unknown metadata uses a clear “Connecting” or “Getting metadata” treatment,
  never a fabricated percentage.
- Finished, paused, stopped, pending, and error states use explicit labels;
  they do not imitate live activity.
- Long torrent names truncate once, preserving the state and percentage.
- Details, media, collaborators, and full transfer facts belong on the
  collection detail screen, not the library row.

### Empty state

Only when the library is empty, show the Portalis mark with its energy effect,
one sentence explaining device-to-device sharing, and a primary “Share files”
action. Join and torrent actions remain available from the add sheet.

## Visual direction

The visual system should feel precise, dark, and alive only when reality is
alive.

- Remove the full-card gold/green bloom. Active rows receive a thin tinted
  edge, a low-opacity surface tint, and a short progress pulse.
- Preserve glow for the Portalis mark, live status indicator, and meaningful
  event transitions. It must not compete with text.
- Use typography, spacing, and progress as the main hierarchy; colour is a
  supporting signal.
- Keep mint for Portalis/live peer state and warm amber for torrent activity,
  but use both sparingly and never as the only state signal.
- Respect reduced motion and maintain contrast in dark conditions.

## Notifications are an overlay, not a Home section

The event system does not consume permanent vertical space in the mobile
library. It is an application-level overlay anchored at the top centre below
safe-area/window chrome.

```text
Portalis energy mark  Event title                         dismiss
                       short, actionable event detail
```

Requirements:

- One visible event at a time; a bounded queue/history handles the rest.
- The mark pulses only on event entry, success, real activity, or attention.
- Events are typed: activity, success, information, warning, and error.
- Long operations update one event instead of emitting polling noise.
- Warnings and errors remain actionable; routine success/events dismiss after
  an accessible delay.
- It works identically on compact and wide layouts, respects reduced motion,
  and announces meaningful changes to assistive technology.

Replace `showToast` through a compatibility adapter, then delete it when every
caller uses the event API. Do not run two notification systems in parallel.

## Core v2 architecture

### Collections

Collections remain the Rust-owned, persisted unit of sharing and transfer.
The Flutter feature projects them but does not invent lifecycle state.

```text
Rust collections domain
  -> CollectionSnapshot + CollectionStatus + CollectionCapability[]
  -> Flutter-Rust Bridge DTOs
  -> CollectionsRepository
  -> CollectionsController
  -> library row, detail, and command menu
```

- Replace display-string-only state with a closed status enum in the bridge.
- Have Rust return supported capabilities with each snapshot, or expose one
  authoritative command query. Flutter must not derive unsafe commands from
  loosely related flags.
- Model command execution as a typed operation with a clear result/error,
  refresh the snapshot after completion, and emit one application event.
- Keep mobile and desktop command presentation thin: a bottom sheet on compact
  layouts and context menu on wide layouts render the same command list.

Required v2 collection commands:

| Command | Rust responsibility | Safety |
| --- | --- | --- |
| Pause / Resume | Stop/start the selected collection’s torrent work without changing library membership. | Immediate and idempotent. |
| Stop | Stop active work while retaining resumable collection state. | Confirm when it interrupts transfer. |
| Restart | Recreate/recheck selected work only when backend supports it. | Explain whether verified progress is retained. |
| Remove from Portalis | Forget collection/torrent registration. | Retain local files; explicit confirmation. |
| Remove downloaded files | Delete only verified paths owned by that collection. | Separate destructive confirmation listing the target. |

The current backend only provides collection fetch/delete and global engine
activity. Therefore none of Pause, Resume, Stop, Restart, or Remove files may
appear as functional v2 controls until this contract exists.

### Media

Media owns `MediaItem`, format capability, conversion, thumbnailing, and
preview rendering. A collection supplies collaboration and transfer context;
it does not own the preview implementation.

Build previews as small renderers selected by media capability:

```text
MediaPreview
  image renderer
  video renderer
  text/subtitle renderer
  audio renderer
  external fallback
```

The collection detail opens the Media viewer with a snapshot/context. Media
never mutates collection transfer state directly.

### Notifications

Create a focused `features/notifications` feature:

```text
domain: AppEvent, EventSeverity, EventId
application: NotificationsController (queue, replacement, dismissal)
presentation: EventOverlay and Portalis energy mark
```

`AppControllers` composes it once. Feature controllers publish events through
the narrow event API; widgets do not create global overlays themselves.

### Design and navigation

`design/` stays limited to cross-feature visual primitives. It must not know
about collections, media, commands, or notifications. `screens/` composes the
adaptive shell and routes; feature presentation owns feature widgets.

## Delivery sequence

1. **Baseline:** preserve `1.0.3 / 0.1.2`, add regression tests for one row
   per collection and current persistence behavior.
2. **Library UX:** remove `LiveTransferCard`, simplify mobile Home to app bar,
   smart input, list, and contextual add action. Add percentage/progress and
   lighter live treatment. This can remain compatible with backend `0.1.2`.
3. **Event overlay:** introduce Notifications as an app feature and migrate
   existing toasts through its adapter. This can remain compatible with
   backend `0.1.2`.
4. **Backend contract:** implement typed per-collection status, capabilities,
   command execution, and safe file-deletion semantics in Rust. Regenerate FRB
   bindings and release frontend `2.0.0` with backend `0.2.0`.
5. **Command UX:** render the capability-driven command menu and destructive
   confirmations on both compact and wide layouts.
6. **Media previews:** add renderers one capability at a time with a safe
   external fallback.

## Acceptance criteria

- One named collection appears once in the library, regardless of activity.
- Mobile Home has no permanent hero transfer or aggregate transfer card.
- Every known-size transfer presents progress percentage, progress bar, and
  concise supporting facts.
- Live visual energy is restrained and tied to actual engine state.
- Notifications overlay the app rather than changing Home layout.
- No lifecycle command is visible unless Rust currently supports it.
- Removing library state and deleting files remain distinct operations.
- `1.0.3 / 0.1.2` remains a reproducible baseline; bridge-changing v2 work is
  versioned, generated, tested, and released as a paired frontend/backend
  contract.
