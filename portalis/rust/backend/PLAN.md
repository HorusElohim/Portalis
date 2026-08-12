# Portalis v3 — Implementation plan

The order in which `SPEC.md` gets built, chosen so nothing is written twice.

Read `SPEC.md` first; this document assumes its vocabulary and refers to its
sections as §n and its decisions as Dn.

## How this plan works

**Every step is atomic in purpose.** One step changes one thing. If a step
needs a sentence with "and also", it is two steps.

**Every step leaves the tree green.** Format, clippy, the full test suite and
the coverage gate all pass at every step. A step is not finished when the code
exists; it is finished when the gates pass and its demo runs.

**Every step deletes something, or says why not.** The `Deletes` line is not
decoration — it is how the codebase gets smaller while gaining capability. A
step with an empty `Deletes` and no explanation is a step that added a second
way to do something.

**Never two implementations of one thing.** No compatibility shims, no
parallel paths, no `_v2` modules. When a step replaces something, the
replacement and the deletion are in the same step. This is the rule that keeps
"no rework" true.

**Every step extends the demo set.**

## The demo set

One `demo` crate, one binary per step, kept forever:

```
demo/src/bin/
├── 01-formats.rs        canonical bytes, printed and verified
├── 02-device-log.rs     enrol, revoke, replay, and six attacks refused
├── 03-revisions.rs      a chain, a rollback, a fork
...
```

Run one with `cargo run -p demo --bin 02-device-log`. Run them all in CI.

They accumulate rather than being replaced, which makes them an executable
changelog and a regression net: if step 7 breaks step 2's demo, that is a real
break and CI says so immediately. Each one prints what it is proving, so a
demo is also the fastest way to understand a subsystem.

## Why this order

Four dependencies force almost all of it:

1. **Objects before transport.** The QUIC server would otherwise be written
   against server-authoritative rules and then rewritten against verifiable
   ones. Build what is being moved before building the thing that moves it.
2. **Objects before storage.** A storage trait written before the objects
   settle changes shape twice.
3. **Domain before bridge.** Projection types mirror the domain; a bridge
   built first gets retyped.
4. **Bridge before Flutter.** Moving Dart files before the bridge shape is
   final moves them twice.

Two orderings are choices rather than constraints, and both are deliberate:

- **Direct peer sharing (step 8) comes before the service (step 10).** It is
  smaller, it proves the P2P claim rather than asserting it, and it makes the
  service an additive step instead of a migration. §22's gate — two devices
  complete a share with no service running — is then met early and stays met.
- **Renaming happens inside the step that rewrites a module**, never as its
  own pass. `capsule` → encrypted manifest and `handoff` → entry payload cost
  nothing when the file is already open, and a rename-only commit is pure
  churn.

---

# Step 0 — Quick wins and the vector harness

**Purpose.** Remove two measured regressions and build the test harness every
format step needs.

| | |
|---|---|
| **Adds** | `tests/vectors.rs` helper: a format's bytes, its hash, and a byte-exact assertion with a readable diff |
| | Parallel history load; debounced history save; piece detail only for visible collections |
| **Deletes** | The serialized `_loadPeerHistoryThenRefresh` chain; the per-tick full re-encode |
| **Demo** | `01-formats.rs` — prints and verifies the manifest bytes that exist today |
| **Gate** | First collection list no longer waits on history; bridge traffic during a transfer measurably lower |
| **Size** | Small |

Independent of everything else, so it can also be skipped if the app's
day-to-day speed is not bothering you. It is first because it is free.

# Step 1 — Canonical formats consolidated in `protocol`

**Purpose.** Put every canonical byte layout in one crate, under its final
name, before anything new is written against it.

| | |
|---|---|
| **Adds** | `protocol/src/format/{manifest,entry}.rs` |
| **Deletes** | `client/src/manifest.rs`, `client/src/capsule.rs`, `client/src/handoff.rs` (1 336 lines moved and reduced) |
| **Demo** | `01-formats.rs` extended: manifest and entry payload, byte-exact |
| **Gate** | Vectors pin every format; `client` no longer defines a canonical byte |
| **Size** | Medium — a move plus a rename, no new logic |

Why here: the service verifies signatures on write (§10), so both sides need
these types, and `client` cannot be a dependency of the service. Doing it now
means every later step imports the final path. Sealing and opening stay in
`client` — the service never gains the ability to decrypt.

Renames land here: capsule → encrypted manifest, handoff → entry payload.

# Step 2 — Device log

**Purpose.** Make D2 real. Nothing else in the trust model works without it.

| | |
|---|---|
| **Adds** | `protocol/src/format/devicelog.rs`: `LogEntry`, signing payload, `DeviceLog::replay()`, `verify()` |
| **Deletes** | Nothing — this is genuinely new |
| **Demo** | `02-device-log.rs`: enrol two devices, revoke one, replay; then six attacks, each refused with its reason |
| **Gate** | Rules enforced: self-signed root only at sequence 1 · author enrolled and unrevoked *at that point* · chain hash · no sequence gaps · no double enrolment · no revoking the unknown. Attacks refused: forged signature, author revoked earlier, reordered entries, truncated log, second root, stale log replayed over a newer one |
| **Size** | Medium |

Pure data and cryptography: no async, no store, no network. This is why it
comes before everything that has those.

# Step 3 — Revision chain

**Purpose.** A collection becomes a verifiable history rather than a server
opinion.

| | |
|---|---|
| **Adds** | `protocol/src/format/revision.rs`; `client/src/verify.rs`: chain verification, fork and rollback detection, highest-seen persistence |
| **Deletes** | `server-core/src/share.rs` publication rules (1 168 lines) — ordering is now the chain's job, not the server's |
| **Demo** | `03-revisions.rs`: publish three revisions; refuse a rollback; detect a fork and report which was kept |
| **Gate** | Every §22 chain attack detected with a distinct typed reason |
| **Size** | Large |

The server keeps a compare-and-set as an optimisation (D3), but it stops being
correctness, which is what allows the deletion.

# Step 4 — Content keys and sealing

**Purpose.** Close the loop between the device log and encryption: a key is
sealed only to devices a verified log authorizes.

| | |
|---|---|
| **Adds** | `client/src/keys.rs`: content key generation, seal to a verified device set, open, rotation on member removal |
| **Deletes** | `server-core/src/envelopes.rs` authorization rules (723 lines) — the owner decides, verifiably |
| **Demo** | `04-sealing.rs`: seal to two devices; a third cannot open; revoke one, rotate, and the revoked device cannot open the next revision |
| **Gate** | A key never reaches a device outside the verified log, including when the log offered is stale or forged |
| **Size** | Medium |

Steps 2–4 together are V1. At the end the verifiable core exists and is
provably hostile-service-resistant, with no transport written yet.

# Step 5 — Event bus and supervisor

**Purpose.** Give components a way to talk that is not direct calls, before
there are many of them.

| | |
|---|---|
| **Adds** | `backend/src/core/events.rs` (§11), `core/supervisor.rs`: task ownership, startup order, bounded shutdown |
| **Deletes** | Ad-hoc task spawning; whatever booleans currently stand in for lifecycle |
| **Demo** | `05-lifecycle.rs`: a core that starts, emits lifecycle events, survives a panicking component, and stops with no leaked task |
| **Gate** | No detached task exists; durable events are never dropped; progress events coalesce; a panicking component degrades rather than killing the process |
| **Size** | Medium |

Here rather than later because every subsequent component plugs into it. Any
later and each one gets written with direct calls and then rewired.

# Step 6 — Local store

**Purpose.** One authoritative place for the device's own truth.

| | |
|---|---|
| **Adds** | `backend/src/store/`: §13's client tables, migrations, schema version |
| **Deletes** | `collab_store` JSON persistence; the Dart-side `samples` history (D8) |
| **Demo** | `06-persistence.rs`: create a collection, add media, restart the core, everything returns including transfer history |
| **Gate** | A store from a newer version refuses to open; migration fixtures from the previous release pass |
| **Size** | Large |

# Step 7 — Collection workflows

**Purpose.** Wire steps 1–6 into the operations a person actually performs,
still with no network.

| | |
|---|---|
| **Adds** | `backend/src/collections/{model,publish,receive,members}.rs` |
| **Deletes** | Legacy collection creation and manifest paths in `src/collections.rs` |
| **Demo** | `07-collections.rs`: create, add media, publish revisions, add and remove a member with re-keying — entirely in-process |
| **Gate** | Two in-process cores exchange objects by hand and both verify; no network code involved |
| **Size** | Large |

# Step 8 — Direct peer sharing over QUIC

**Purpose.** Prove the product claim: two devices on one network share with no
service in existence.

| | |
|---|---|
| **Adds** | `client/src/session.rs`: peer connections, `Security` reported at handshake (§15), object exchange |
| **Deletes** | `client/src/{transport,pending,reconnect}.rs` — the WebSocket client and its correlation registry |
| **Demo** | `08-two-peers.rs`: two cores on one machine, one shares a collection, the other verifies and downloads. **No service process exists** |
| **Gate** | §22's headline gate met · security level correct for direct and relayed · an unknown peer is refused |
| **Size** | Large |

The WebSocket client dies here rather than earlier because this is the step
that replaces it. Deleting it sooner would leave the demos and tests with
nothing to talk to.

# Step 9 — Bridge

**Purpose.** One stream down, commands up; the interface stops asking.

| | |
|---|---|
| **Adds** | `backend/src/projection/{state,build,emit}.rs` and `api.rs`: §16's five calls, §17's types, §18's tiers |
| **Deletes** | Every polling entry point on the Rust side |
| **Demo** | `09-projection.rs`: drives a core through a share and prints each emitted projection, with byte counts per tier |
| **Gate** | Idle emits nothing · progress coalesces to ≤4 Hz · detail arrives only when subscribed · a command is acknowledged in under 100 ms with the network down |
| **Size** | Large |

# Step 10 — Flutter

**Purpose.** One subscription, no derived state, one structure.

| | |
|---|---|
| **Adds** | `lib/bridge/portalis.dart`; the §19 tree |
| **Deletes** | `Timer.periodic` polling; `lib/screens/` as a parallel tree; SharedPreferences history stores; every Dart-side derivation |
| **Demo** | The app itself, plus a `10-headless.dart` script printing the stream |
| **Gate** | §21's budget measured and met on a real device |
| **Size** | Large |

# Step 11 — The service

**Purpose.** Reach devices that are not on your network, and deliver to
devices that are asleep.

| | |
|---|---|
| **Adds** | `storage` crate: `Repositories` trait, embedded and MongoDB engines (D5) · QUIC listener · mailbox · directory · rendezvous |
| **Deletes** | `apps/server` axum and WebSocket plumbing · `server-core` remnants · the Mongo-only assumptions and the replica-set requirement |
| **Demo** | `11-service.rs`: two cores that cannot see each other, one offline, exchanging through the service — and the same run against both storage engines |
| **Gate** | One binary, engine chosen by configuration · both engines pass one suite · step 8's demo still passes unchanged |
| **Size** | Large |

Step 8's demo passing unchanged is the important part: it proves the service
stayed optional.

# Step 12 — Completeness

**Purpose.** The commands a real person needs and cannot currently send.

| | |
|---|---|
| **Adds** | Fingerprint verification UI · device listing and revocation · blocking · handle change · account deletion · secure storage for both private keys |
| **Deletes** | `collab_sync`, address-bearing invites, rendezvous keys, and their configuration — the one-way migration (D1, invariant 10) |
| **Demo** | `12-account.rs`: link a second device, revoke it, confirm re-keying, block a contact, change a handle, delete the account |
| **Gate** | No callable legacy network path remains anywhere in the tree |
| **Size** | Large |

# Step 13 — Hardening

**Purpose.** Earn the right to give it to someone else.

| | |
|---|---|
| **Adds** | Fuzz targets on every decoder · load and soak · network-change and restart suites · backup and restore drill |
| **Deletes** | Any exclusion in the coverage gate that turned out to be laziness |
| **Demo** | `13-adversarial.rs`: a deliberately hostile service, running every attack in §22 against a real core |
| **Gate** | Four platforms build · the first externally supported protocol is pinned · compatibility gates begin |
| **Size** | Medium |

---

## Rework avoided, and where it would have come from

| Trap | Avoided by |
|---|---|
| Writing the QUIC server against server-authoritative rules, then rewriting | Steps 2–4 before step 8 |
| A storage trait shaped before the objects settled | Step 11 after steps 1–7 |
| Projection types retyped when the domain changed | Step 9 after step 7 |
| Dart files moved twice | Step 10 after step 9 |
| A rename-only pass touching every file | Renames inside steps 1 and 3 |
| Two transports live at once | WebSocket deleted in step 8, the step that replaces it |
| Two persistence layers | `collab_store` deleted in step 6, the step that replaces it |

## Keeping it small

The line count should fall through steps 1–8 despite gaining capability.
Roughly 3 200 lines are marked for deletion in those steps against maybe 2 000
added, because verification replaces enforcement and one honest object
replaces several negotiated ones.

Three habits hold that:

- A step that cannot name what it deletes has probably added a parallel path.
- A concept that needs explaining twice in `SPEC.md` is one concept too many —
  the §2 vocabulary is the budget, and additions to it need a decision entry.
- Demos are the documentation. If a subsystem needs a prose explanation that
  its demo does not give, simplify the subsystem before writing the prose.

## Suggested checkpoints

Steps 4, 8 and 10 are the three points where it is worth stopping to look:
after 4 the trust model is real and testable, after 8 the product claim is
proven, after 10 the app is the one described in `SPEC.md`. Each is a natural
place to reconsider the remaining order with what has been learned.
