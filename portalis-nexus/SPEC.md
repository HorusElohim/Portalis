# Portalis Nexus Specification

Status: M3 friends and presence complete; M2.5 device linking next, then M4

Protocol: `portalis.protocol.v1`

Production target: Linux

Last updated: 2026-08-08

## 1. Purpose

Portalis Nexus is an independent Rust workspace for the new Portalis control
plane. It contains the protobuf contract, portable Rust client, Linux server,
database adapters, and end-to-end tests.

The server makes identity, friends, presence, collection discovery, torrent
metadata, and peer discovery reliable. Photos, videos, and BitTorrent pieces
continue to move directly between peers.

## 2. Goals

- Define all socket messages with Protocol Buffers.
- Build one client crate for macOS, iOS, Android, and Linux.
- Build a multithreaded asynchronous Rust server for Linux.
- Reuse existing Ed25519 device identities and add separate encryption keys.
- Support unique handles, friends, presence, and shared collections.
- Distribute versioned torrent metadata and current peer candidates.
- Store durable state in indexed MongoDB collections.
- Keep active connections, presence, and peer leases in bounded fast tables.
- Enforce backward-compatible protocol evolution in CI.
- Migrate gradually from the current direct-address protocol.
- Reach 100% line and branch coverage for eligible handwritten core code.

## 3. Non-goals

- The control server does not store or relay media payloads.
- The control server does not replace BitTorrent piece exchange.
- Version 1 does not require horizontal server scaling.
- Version 1 does not require a browser client or administration UI.
- Discovery alone does not guarantee reachability through every NAT. Guaranteed
  transfer requires a separate relay or reachable-seed strategy.

## 4. Invariants

1. Media payloads never pass through the control server.
2. Every external message is size-bounded and validated.
3. Durable commands are authenticated, authorized, idempotent, and versioned.
4. Device signing and encryption private keys never leave their devices.
5. Slow clients cannot create unbounded queues or tasks.
6. MongoDB documents never contain unbounded friend, member, or event arrays.
7. Deleted protobuf field numbers and enum values are permanently reserved.
8. The Flutter backend imports only the portable client crate.
9. The legacy protocol remains available behind a fallback flag until cutover.
10. Backend integration changes are recorded in the root `CHANGELOG.md`.

## 5. Repository Layout

```text
Portalis/
├── portalis/                         # Existing Flutter application
│   └── rust/backend/                 # Imports portalis-nexus-client only
└── portalis-nexus/                   # Independent Cargo workspace
    ├── Cargo.toml
    ├── Cargo.lock
    ├── SPEC.md
    ├── README.md
    ├── buf.yaml
    ├── buf.gen.yaml
    ├── proto/
    │   └── portalis/protocol/v1/
    │       ├── common.proto
    │       ├── connection.proto
    │       ├── identity.proto
    │       ├── friends.proto
    │       ├── presence.proto
    │       ├── collections.proto
    │       └── swarm.proto
    ├── crates/
    │   ├── protocol/                 # limits, ids, frame, validate
    │   ├── client/                   # error, protocol, pending, reconnect,
    │   │                             #   config, transport/
    │   └── server-core/              # Pure domain/application logic
    ├── apps/
    │   └── server/                   # config, state, shutdown, health,
    │                                 #   messages, session, socket,
    │                                 #   handlers/ (one per subsystem)
    ├── demo/                         # Runnable server/client examples
    ├── tests/
    │   └── integration/              # Real server/client/MongoDB tests
    └── docker/
        ├── Dockerfile
        └── compose.yaml
```

The existing backend eventually imports:

```toml
portalis-nexus-client = { path = "../../../portalis-nexus/crates/client" }
```

## 6. Crate Boundaries

### `protocol`

- Compiles authoritative `.proto` files with Prost.
- Re-exports generated Rust messages.
- Validates identifiers, lengths, enums, and protocol limits.
- Builds deterministic domain-separated signing payloads.
- Contains no sockets, database drivers, Flutter bindings, or OS adapters.

### `client`

- Maintains one authenticated socket connection.
- Reconnects with exponential backoff and jitter.
- Correlates responses with requests.
- Exposes typed commands and event streams.
- Uses bounded queues and request concurrency.
- Restores subscriptions and refreshes durable state after reconnecting.
- Has no MongoDB, Axum, server-core, or Linux-only dependency.

### `server-core`

  - Defines registration, authentication, and device-linking rules.
- Defines handle allocation and normalization.
- Defines friendship, membership, collection-head, and lease state machines.
- Exposes repository, clock, random-source, and event-sink traits.
- Is fully testable without sockets or MongoDB.

### `server`

- Routes decoded envelopes to a handler module per subsystem, so the socket
  and the session never learn what a command means.
- Runs the Tokio multithread runtime.
- Terminates TLS and WebSocket connections.
- Implements MongoDB repositories.
- Owns connection, presence, and swarm registries.
- Exposes health, readiness, and metrics endpoints.
- Initializes structured tracing and graceful shutdown.

## 7. Transport

Version 1 uses secure WebSockets (`wss://`) with binary protobuf frames. One
WebSocket binary message contains exactly one encoded `Envelope`.

Endpoint:

```text
GET /v1/socket
Upgrade: websocket
Sec-WebSocket-Protocol: portalis.protobuf.v1
```

Reasons:

- Full-duplex commands and events over one connection.
- Native message boundaries without custom TCP framing.
- Mobile, desktop, reverse-proxy, and future browser compatibility.
- Simpler deployment than custom TLS/TCP or an initial QUIC stack.
- No separate gRPC-Web path for a future administration interface.

Raw TCP, gRPC, and QUIC remain possible future transports. The protobuf domain
messages must not depend on WebSocket-specific details.

## 8. Protobuf Contract

### Compatibility rules

- Package name: `portalis.protocol.v1`.
- Every enum starts with `*_UNSPECIFIED = 0`.
- Changes inside `v1` are additive and backward compatible.
- Existing field numbers and enum values are never reused.
- Removed fields and values are marked `reserved`.
- `buf lint` and `buf breaking` run in CI.
- Golden encoded messages verify compatibility with released clients.
- A breaking semantic change creates `portalis.protocol.v2`.

### Envelope shape

```proto
syntax = "proto3";

package portalis.protocol.v1;

message Envelope {
  bytes message_id = 1;       // UUIDv7, exactly 16 bytes
  bytes correlation_id = 2;   // Request message_id for responses
  uint64 timestamp_unix_ns = 3;

  oneof payload {
    ServerHello server_hello = 10;
    RegisterUser register_user = 11;
    AuthenticateDevice authenticate_device = 12;
    Authenticated authenticated = 13;
    Ping ping = 14;
    Pong pong = 15;
    Ack ack = 16;
    ProtocolError protocol_error = 17;
    LinkDevice link_device = 18;
    DeviceLinked device_linked = 19;

    ResolveHandleRequest resolve_handle_request = 30;
    ResolveHandleResponse resolve_handle_response = 31;
    FriendCommand friend_command = 32;
    FriendEvent friend_event = 33;
    ListFriendsRequest list_friends_request = 34;
    ListFriendsResponse list_friends_response = 35;
    PresenceEvent presence_event = 36;

    PublishShareSnapshot publish_share_snapshot = 50;
    ShareSnapshotEvent share_snapshot_event = 51;
    ListSharesRequest list_shares_request = 52;
    ListSharesResponse list_shares_response = 53;
    GetShareSnapshotRequest get_share_snapshot_request = 54;
    GetShareSnapshotResponse get_share_snapshot_response = 55;

    AnnouncePeerLease announce_peer_lease = 70;
    LookupPeersRequest lookup_peers_request = 71;
    LookupPeersResponse lookup_peers_response = 72;
  }
}
```

Field allocation is grouped by subsystem and finalized before implementation.

### Time

Every timestamp on this wire, in the domain, and in storage is **nanoseconds
since the Unix epoch** in a `u64`, named `*_unix_ns`. One unit everywhere means
no conversion seams to get wrong; `u64` nanoseconds run out in 2554 and the
`i64` MongoDB stores them in runs out in 2262.

The single exception is inside `UUIDv7`, whose 48-bit timestamp field is
defined as milliseconds. `user_id_from` converts rather than truncating,
because 48 bits of nanoseconds wrap every three days and would destroy the
time ordering v7 exists for. `NANOS_PER_MILLI` marks every such boundary.

Precision is whatever the host clock offers — microseconds on macOS — so the
unit is a promise about scale, not about resolution.

### Identifiers

- `UserId`: UUIDv7, 16 bytes.
- `ShareId`: UUIDv7, 16 bytes.
- `SnapshotId`: BLAKE3 content root, 32 bytes.
- `MessageId`: UUIDv7, 16 bytes.
- `DeviceId`: BLAKE3-derived Ed25519 public-key identifier, 32 bytes.
- `InfoHashV1`: 20 bytes.
- `ContentHash`: BLAKE3, 32 bytes.

Binary identifiers use protobuf `bytes`, not hexadecimal strings.

### Semantics

- A command's `message_id` is its idempotency key.
- A response sets `correlation_id` to the command's `message_id`.
- Durable writes return `Ack` only after database write concern succeeds.
- Domain failures return typed `ProtocolError` messages.
- Malformed, abusive, or unauthenticated traffic may close the connection.
- Durable state is refreshed after reconnect rather than replaying every
  transient socket event.

### Initial limits

- Maximum binary frame: 8 MiB.
- Maximum torrent descriptor: 4 MiB.
- Maximum handle: 32 UTF-8 bytes after normalization.
- Maximum collection name: 256 UTF-8 bytes.
- Maximum peer candidates returned: 64.
- Maximum pending client requests: 128.
- Maximum queued outbound messages per connection: 256.

Every limit has boundary-minus-one, boundary, and boundary-plus-one tests.

## 9. Identity and Authentication

The client reuses Portalis's existing Ed25519 identity for signatures. Each
device also owns an independent X25519 encryption keypair. Both private keys
stay local; the server stores only the authorized public halves.

### Registration

1. Server sends `ServerHello` with protocol range, connection ID, timestamp,
   and a fresh 32-byte challenge.
2. Client sends `RegisterUser` with desired username, Ed25519 signing public
   key, X25519 encryption public key, and a domain-separated signature covering
   the full challenge context.
3. Server verifies signature, challenge age, and one-time use.
4. Server allocates stable `UserId` and unique user handle.
5. Server stores the first authorized device and returns `Authenticated`.

### Authentication

1. Server sends a fresh challenge.
2. Client signs protocol version, server authority, connection ID, challenge,
   and server timestamp.
3. Server verifies signature, device status, expiry, and replay cache.
4. Connection becomes associated with its user and device.

Challenges expire after 60 seconds and can be consumed once.

### Handles

User-facing handles have the form:

```text
<username>#<discriminator>
```

Proposed constraints:

- Username: 3-24 letters, numbers, or underscore.
- Display casing preserved; normalized value stored for lookup.
- Discriminator: five cryptographically random Crockford Base32 characters.
- Unique MongoDB index on `(normalized_username, discriminator)`.
- `UserId` remains immutable when a handle changes.

Allocation retries random discriminators against the unique index. It never
scans for the next available value.

### Device linking and encryption keys

The schema supports multiple devices per user from day one. A device record
therefore contains an Ed25519 signing public key and an X25519 encryption
public key. They are different keys with different purposes; neither key is
derived from the other.

An already authenticated, authorized device links a new device by sending its
two public keys and a domain-separated approval signature. The signed payload
binds the user, approving device, candidate signing key, candidate encryption
key, and operation version. Nexus verifies that signature, rejects a duplicate
candidate signing key, and stores the new device atomically. The new device can
then authenticate normally with its Ed25519 key.

Nexus never receives a share secret in plaintext. A client creates a random
per-share symmetric key, encrypts each snapshot capsule with it, and stores one
X25519-encrypted key envelope per authorized device. Linking a second device
lets an existing device add its envelope; revoking a device prevents new
envelopes and future capsule access. Account recovery remains a separate
product decision and never weakens this approval rule.

## 10. Friends and Presence

Friendship uses one canonical edge document with the two binary user IDs sorted
into `user_low` and `user_high`.

```text
NONE -> PENDING -> ACCEPTED
  ^        |          |
  |        v          v
  +----- REJECTED   REMOVED
```

Commands use compare-and-set filters so concurrent accept, reject, and remove
operations are deterministic and idempotent.

Presence is derived from authenticated connections:

- User online when at least one authorized device is connected.
- Ping every 20 seconds.
- Connection dead after 60 seconds without valid traffic.
- Presence visible only to authorized friends.
- Multiple devices aggregate into one user state.
- Heartbeats are not persisted to MongoDB.

## 11. Shares, Snapshots, and Torrent Handoffs

A **share** is the stable social object. It has a locally generated `ShareId`,
an immutable owner, visibility/access policy, and a monotonically increasing
revision. A **snapshot** is an immutable, content-addressed media manifest:
adding, removing, renaming, or replacing media produces a different
`SnapshotId`.

`SnapshotId` is the BLAKE3 content root of the resolved canonical manifest.
The Portalis backend builds it once from the torrent engine's authoritative
20-byte BitTorrent v1 info hashes and each signed manifest entry, in canonical
info-hash order with length-prefixed fields. It never performs a second file
scan or asks Nexus to reinterpret a torrent. Protobuf serialisation is never
treated as the canonical manifest encoding.

The first authenticated publication creates a share and makes its publisher the
permanent owner. A subsequent update publishes exactly
`current_revision + 1`, names the prior snapshot, and points to the new
snapshot. An identical signed retry is idempotently successful; conflicting
bytes for one revision fail. A peer may seed and fetch a snapshot but cannot
mutate the owner's share.

Nexus stores a bounded encrypted snapshot capsule and its signature, never
plaintext torrent descriptors, file names, directory structure, or media. The
capsule can carry a magnet link or `.torrent` descriptor encrypted by a random
per-share secret. Nexus does not include a BitTorrent library and never parses
or validates the encrypted torrent payload. It authorizes and returns capsules
to permitted devices so a user's newly linked device can synchronise while the
original device is offline.

Membership is stored as separate edge documents. Server authorization is
checked before a private share summary, capsule, or live handoff is returned.
The live rendezvous path may also forward an authorized, encrypted magnet or
`.torrent` from one online client to another; it is bounded, transient, and is
not written to MongoDB.

## 12. Swarm Discovery

An authenticated seeder announces a short-lived peer lease containing:

- Info hash.
- BitTorrent listen port.
- Address-family and transport capabilities.
- Requested lease duration.

The server combines the validated advertised BitTorrent port with the source IP
observed directly by the socket layer. Behind a reverse proxy, forwarding
headers are accepted only from explicitly configured trusted proxy networks.
It never trusts an arbitrary client-supplied public IP.

Initial policy:

- Lease duration: 90 seconds.
- Refresh interval while seeding: 30 seconds.
- Disconnect removes leases immediately when possible.
- Expiration remains the correctness mechanism.

Lookup returns bounded randomized candidates, preferring compatible transport,
recent leases, and diverse network prefixes. The client merges and de-duplicates
server, direct, tracker, and DHT candidates before handing them to librqbit.

The Portalis server is deterministic discovery, not guaranteed reachability.
Symmetric NAT and restrictive firewalls still require a relay, reachable seed,
or a compatible hole-punching transport.

## 13. Concurrency and Fast Tables

The server uses Tokio's multithread runtime. Worker count defaults to available
parallelism and remains configurable for container limits.

Each socket owns:

- One bounded read/decode loop.
- One bounded `mpsc` queue and writer loop.
- One cancellation token.
- One request-concurrency semaphore.
- Authentication and rate-limit state.

No lock guard is held across `.await`. Blocking work uses a bounded blocking
pool. Unbounded task creation and unbounded `spawn_blocking` are prohibited.

Version 1 uses sharded concurrent maps for:

- `DeviceId -> ConnectionHandle`.
- `UserId -> active device count`.
- `InfoHash -> peer lease set`.
- Short-lived `MessageId -> idempotency result` cache.

Lease and challenge expiration uses deadline scheduling, not full-table scans.
Generation tokens ensure stale timeout events cannot delete renewed entries.

These maps are ephemeral accelerators. MongoDB remains authoritative for users,
devices, friendships, memberships, and collection heads. Interfaces must permit
a later distributed connection registry and event bus without changing the
protobuf contract.

## 14. MongoDB Model

Production and integration tests use a replica set so transactions and change
streams are available.

### Durable collections

`users`

- `_id`, username, normalized username, discriminator, status, timestamps,
  schema version.
- Unique `(normalized_username, discriminator)` index.

`devices`

- Device ID, user ID, Ed25519 signing public key, X25519 encryption public key,
  creation/last-authentication/revocation times.
- Unique device ID and `(user_id, revoked_at)` indexes.

`friendships`

- Canonical user pair, requester, state, version, timestamps.
- Unique `(user_low, user_high)` index.
- State/listing indexes from each side of the edge.

`shares`

- Share ID, immutable owner, visibility policy, current revision/snapshot,
  timestamps, schema version.

`share_memberships`

- Share ID, user ID, role, version, timestamps, revocation state.
- Unique `(share_id, user_id)` and user-listing indexes.

`share_snapshots`

- Share ID, revision, prior/current snapshot IDs, encrypted capsule hash/content,
  publisher, signature, timestamp.
- Unique `(share_id, revision)` index.

`share_key_envelopes`

- Share ID, recipient device ID, encrypted share-key envelope, envelope
  algorithm/version, creation/revocation timestamps.
- Unique `(share_id, recipient_device_id)` index.

`command_receipts`

- Actor device, message ID, durable result, expiration.
- Unique `(actor_device_id, message_id)` and TTL indexes.

### Modeling rules

- Friendships and memberships are edge documents, never growing arrays.
- Snapshot history is append-only; the current share revision is updated with a transaction or
  safe compare-and-set.
- Mutable documents carry optimistic-concurrency versions.
- Security-sensitive multi-document transitions use transactions.
- Production uses majority write concern for identity and sharing changes.
- Migrations are explicit, resumable, idempotent, and fixture-tested.
- Presence and peer heartbeats do not create MongoDB writes.

## 15. Security

- TLS mandatory outside local tests; prefer TLS 1.3.
- Authenticate before domain commands and authorize every resource operation.
- Domain-separate all signed payloads.
- Reject replayed and expired challenges.
- Bound frames, queues, concurrent requests, and database operation times.
- Apply per-IP limits before authentication and per-device/user limits after.
- Redact credentials, challenges, collection secrets, and descriptor content
  from logs.
- Run the Linux container as non-root with a read-only root filesystem.
- Run dependency, license, container, and secret scans in CI.

## 16. Observability

The server initializes structured `tracing` at process start and carries safe
connection, message, and correlation identifiers through each operation.

Endpoints:

- `/health/live`: process is alive.
- `/health/ready`: MongoDB and startup state are ready.
- `/metrics`: Prometheus-compatible metrics under deployment policy.

Metrics include connections, authentication outcomes, commands, protocol
errors, queue saturation, MongoDB latency, active users, leases, lookup latency,
reconnects, and connection lifetime. OpenTelemetry export is configurable.

## 17. Linux Deployment

Linux is authoritative for production and CI.

- Multi-stage Docker build with pinned toolchain and dependencies.
- `linux/amd64` and `linux/arm64` images.
- Numeric non-root user and read-only root filesystem.
- Graceful `SIGTERM` handling and bounded socket drain.
- MongoDB replica set for production and integration tests.
- Backup, restore drill, certificate rotation, and monitoring before launch.
- Keep macOS server compilation working where practical for local development.

Version 1 deploys as one server process. Horizontal scaling is introduced only
after measured need and requires shared connection routing/presence/event-bus
adapters, not protocol changes.

## 18. Testing and Coverage

Coverage is a gate, not the complete definition of correctness.

### Coverage policy

- 100% line and branch coverage for eligible handwritten `protocol`,
  `server-core`, and deterministic client state-machine code.
- Generated protobuf code is excluded.
- Bootstrap/platform adapters and genuinely unreachable defensive branches need
  explicit documented exclusions.
- Coverage cannot decrease in pull requests.
- `cargo llvm-cov` produces CI reports.

### Test layers

1. Schema: Buf lint/breaking and released golden binary fixtures.
2. Domain: identities, handles, friendships, authorization, versions, leases.
3. Property: state-machine invariants and stale-expiration races.
4. Client: correlation, timeout, cancellation, reconnect, bounded queues.
5. Transport: handshake, limits, malformed frames, slow peers, shutdown.
6. MongoDB: replica-set transactions, indexes, TTL, races, migrations.
7. End-to-end: MongoDB, server, and two real Rust clients.
8. Fuzzing: envelope decoding, validators, and signing payloads.
9. Mutation: pure protocol/domain/application code.
10. Load/soak: connection ramp, presence fan-out, hot swarms, reconnect loops.

Unit tests inject clocks, randomness, repositories, and event sinks. They do not
sleep or access the network. Real-time behavior is isolated to bounded
integration tests.

## 19. Implementation Milestones

### M0: Workspace and quality gates

- Cargo workspace and crate skeletons.
- Protobuf/Buf generation.
- Format, lint, audit, test, coverage, and Linux build CI.
- Empty server with health endpoints and Docker image.

Gate: CI rejects a deliberate protobuf breaking change.

### M1: Connection lifecycle

- Envelope, hello, ping/pong, ack, and error messages.
- WSS server and portable client supervisor.
- Backpressure, timeout, reconnect, tracing, and shutdown.

Gate: multiple clients reconnect after forced server restart without leaks.

Status: complete.

The `/v1/socket` endpoint enforces the protobuf subprotocol and 8 MiB frame
limit, sends a validated `ServerHello`, and replies to binary `Ping` envelopes
with correlated `Pong`s. Both peers split each socket into a read loop and a
single writer task joined by one `MAX_OUTBOUND_QUEUE` channel, so a peer that
stops reading loses its connection instead of growing memory.

The client is a supervised handle that owns no socket. `PendingRequests`
correlates responses by `correlation_id` within `MAX_PENDING_REQUESTS`, so
commands are concurrent and independent; a timeout, a dropped connection, and a
delivered response each remove their waiter exactly once. Envelopes that answer
no request become events. One supervisor task rebuilds the connection under
`ReconnectPolicy`, so callers never reconnect by hand.

`Shutdown` signals every live server socket and waits for them to close within
`GRACEFUL_DRAIN_TIMEOUT`, because upgraded WebSocket connections outlive the
HTTP serve loop that Axum's graceful shutdown tracks. Each socket carries a
`connection_id` tracing span.

Every wait is bounded: the handshake, each request, connection teardown, and
server draining. Integration suites exercise these against real sockets,
including peers that never answer, refuse the subprotocol, advertise an
unsupported version, push unsolicited envelopes, or close mid-request.

### M2: Identity

- Registration and challenge authentication.
- User/device MongoDB repositories and indexes.
- Unique handle allocation and device revocation model.

Gate: valid existing Portalis identities authenticate; replay, wrong-key,
expired, and revoked-device tests fail safely.

Current slice: the identity contract and its cryptography. `identity.proto`
adds `RegisterUser`, `AuthenticateDevice`, and `Authenticated` at the reserved
envelope numbers. Signed payloads are domain-separated by operation and built
from length-prefixed fields, so a signature covers exactly one operation, on
one connection, against one server, for one challenge; concatenation ambiguity
cannot reinterpret a username. `derive_device_id` turns an existing Portalis
Ed25519 public key into a stable `DeviceId` with BLAKE3, so a device keeps its
keypair and needs no new secret. `Handle` carries display casing beside the
normalized form used for uniqueness, and discriminators render from injected
entropy over Crockford Base32.

Note: the Flutter app currently treats the raw Ed25519 public key as its own
device identifier, so Nexus labels the same key differently. The derivation is
deterministic, making the mapping trivial, but the two identifiers should be
reconciled before M6 integration.

The registration and authentication rules now sit in `IdentityService` over
repository, clock, and random-source traits, so they are provable without a
database. `IssuedChallenge` spends a challenge once and expires it after
`CHALLENGE_LIFETIME_MS`; a wrong guess still spends the attempt, so a
connection cannot keep probing. Registration verifies the signature before it
writes anything, refuses a device that is already enrolled, and allocates a
handle by retrying random discriminators against the unique index rather than
scanning for a free one. Authentication rejects unknown and revoked devices and
records the time of each success.

A user and its first device are written through one `insert_registration` port
call rather than two, because a user whose device is missing holds a handle it
can never authenticate with. Expressing the pair as one operation is what lets
the MongoDB adapter wrap it in a transaction, and lets handle allocation retry
the whole write on a collision without stranding a device.

Registration and authentication now run end to end over a real socket. Each
connection owns a `Session` holding the one challenge it was greeted with;
spending it binds the connection to an identity. The client signs through a
`DeviceSigner` the caller implements, so key material never enters the client
crate. Both peers name the server the same way: the client derives the
authority from the endpoint it dialled and the server carries its configured
one, so a signature only verifies against the deployment it was meant for.

Failures come back as typed `ProtocolError` codes the caller can act on:
unauthenticated for a bad signature, unknown device, or spent challenge;
unauthorized for a revoked device; invalid message for a rejected username.
Storage failures report a generic internal error, keeping database detail in
the logs rather than on the wire.

Status: complete.

Storage is now durable. `MongoStore` implements the same three ports the
in-memory adapter does, with unique indexes on `(normalized_username,
discriminator)`, on device ID, and on the friendship edge; a transaction around
`insert_registration`, so a handle collision leaves no device behind; and a
version filter on every friendship write, so a stale writer loses rather than
overwrites. `NexusStore` chooses the backend at startup from
`PORTALIS_NEXUS_MONGODB_URI`, and every layer above the ports is unchanged —
which is what the port design was for. The server process refuses to start
without that variable or when MongoDB cannot prepare its indexes. In-memory
storage remains an explicit test and development adapter, never a production
fallback.

Duplicate-key failures are read as lost races rather than outages: a unique
index rejecting a write is the store working. Everything else is an outage,
including a server that goes away mid-write.

`tests/mongo.rs` runs against a real replica set in Docker, covering the
registration transaction and its rollback, index idempotence, compare-and-set,
a device enrolled twice, a stopped server, a standalone server that cannot open
a transaction, and malformed connection strings. Two tests carry the gate that
only durable storage can meet: an identity registered by one process
authenticates from a second one holding nothing but the database, and a revoked
device stays revoked across the same restart. Docker's absence skips these;
Docker present but failing is a failure, never a silent pass.

Per section 18, `mongo/mod.rs` is excluded from the coverage gate as a platform
adapter: what remains after those tests is driver-internal error propagation
that cannot be triggered deterministically. Its decisions live in
`mongo/documents.rs` and `store.rs`, which stay measured at 100%.

### M2.5: Device linking and encrypted share access

Status: complete.

- Register separate X25519 encryption public keys beside Ed25519 signing keys.
- Link a device only through an existing authorized device's signed approval.
- Persist per-device encrypted share-key envelopes without ever receiving a
  share secret in plaintext.

Gate: a second approved device authenticates, receives only its encrypted
envelope, decrypts the same share capsule locally, and a revoked device cannot
receive a replacement envelope.

Nexus never holds a share key. `seal` and `open` are an X25519 exchange to the
recipient device's encryption key with ChaCha20-Poly1305 over it, and the
share and recipient device are authenticated alongside the ciphertext, so an
envelope cannot be transplanted onto another share or another device. A
low-order ephemeral key is refused on both sides: sealing names it, opening
reports only that the envelope did not open.

`EnvelopeService` decides one thing — whether the sender may address the
recipient it named. The recipient is re-read from storage rather than trusted
from whatever the sender last knew, so a device revoked after the sender last
checked still refuses a replacement envelope, and a device belonging to
another user is refused outright. An oversized ciphertext or a malformed
ephemeral key is rejected before anything is stored.

Fetching is scoped by the connection rather than the request: `list` takes the
device from the authenticated session, so there is nothing to authorize beyond
having authenticated, and no way to ask for someone else's envelopes.
Listing is a keyset page ordered by share ID, carrying the cursor the next
request resumes from, so a device with more shares than one page learns where
to continue rather than silently stopping.

Storage keeps one row per share and recipient device under a unique index, so
a rotated key replaces its predecessor rather than piling up beside it. The
durable adapter upserts under that index and retries a lost insert race once,
because reporting an outage there would drop a rotated key while telling the
caller it was stored.

Gate met: a linked device decrypts a share key Nexus never saw, an envelope
reaches only the device it names, a revoked device is refused a replacement,
and an envelope addressed to another user's device is refused — over real
sockets, with the same storage behaviour proven against a real MongoDB
replica set.

### M3: Friends and presence

Status: complete.

- Resolve handles and manage friend-state transitions.
- Track multi-device presence and send friend-only events.

Gate: two clients become friends and observe deterministic online/offline state.

Current slice: the friendship contract and its state machine. `friends.proto`
and `presence.proto` add handle resolution, friend commands and events, friend
listing, and presence at the reserved envelope numbers.

`FriendshipEdge` sorts its two user IDs, so naming the edge does not depend on
who asked first and one unique index keeps a single row per friendship.
`apply` decides what an action does without touching storage: only the side
that did not ask may accept or reject, either side may remove an accepted
friendship, the asker may cancel one still pending, and a refusal or removal
can be asked again. A request sent back while one is pending is taken as
accepting it. Repeating any command reports `Unchanged` rather than failing or
bumping the version, and every move carries the version its write must match,
which is what makes concurrent commands deterministic.

`FriendService` applies those rules over storage and time. A command reads the
edge, applies the action, and writes under the version it read; losing that
race means another side wrote first, so it re-reads and re-applies up to
`COMMAND_ATTEMPTS` times before reporting contention. Handle resolution
normalizes both halves of `<username>#<discriminator>` before the indexed
lookup, and listing carries the peer behind each edge along with who asked.

`UserDirectory` is split out of `IdentityRepository`: friends and presence read
users but never enrol or revoke devices, so they do not depend on that surface.

`PresenceRegistry` derives who is online from live connections rather than
storing it: a user is online while at least one device is connected, and only
the transitions are reported, so callers fan out one event per real change
rather than one per device. Coming back clears the last-seen time.

Resolving handles, acting on friendships, and listing them are served over the
socket, refused for a connection that has not authenticated, and answered with
typed codes: invalid message for a rejected action, rate limited when a
contended edge exhausts its retries, and a generic internal error that keeps
storage detail off the wire. Two clients become friends end to end.

Presence is fanned out to accepted friends only. Every event is addressed to a
specific friend's live connections rather than broadcast, so a stranger and a
pending friendship both see nothing. A connection that authenticates is told
where its friends already stand, because otherwise a client would see nothing
until someone's state changed. Reading friends is best-effort: a store outage
shares nothing rather than failing the command that triggered it, and clients
refresh on reconnect.

Gate met: two clients become friends, observe each other going offline and
coming back, and a second device leaving does not report its user away.

### M4: Encrypted shares and snapshots

- Share/member/snapshot repositories and per-device key envelopes.
- Signed snapshot publication and authorized listing/events/fetches.
- Encrypted torrent-capsule limit and transient client-to-client handoff.

Gate: an authorized linked device decrypts the latest capsule; an unauthorized
device cannot discover private state; share revisions never regress.

Current slice: the publication rules, over no storage and no clock. `publish`
decides what a publication does to a share without performing it, which is
what lets the write carry the revision it read and lose safely to a concurrent
publisher.

A share that does not exist starts at revision one and has nothing to follow,
and creating it fixes its owner permanently: a peer may seed and fetch a share
without being able to move it. An update publishes exactly one revision past
what is stored and names the snapshot the share is actually on, so a
publication built against a revision another device already replaced is
refused rather than silently overwriting it. Revisions never regress and never
skip.

Republishing the stored revision succeeds only when every byte matches,
because a publisher whose answer was lost retries the same bytes and must not
be stranded; different bytes for a published revision are refused, since a
revision is immutable once written. Nexus compares capsule bytes to recognise
that retry and otherwise treats them as opaque.

### M5: Swarm discovery

- Announce, refresh, lookup, and expire peer leases.
- Fast swarm table and candidate diversity.
- Client merge of server, direct, tracker, and DHT candidates.

Gate: two clients discover current endpoints and expired peers disappear.

### M6: Flutter integration

- Add only `portalis-nexus-client` to the existing backend.
- Add Flutter Rust Bridge façade for connection, identity, friends, presence,
  collections, and errors.
- Keep protobuf and sockets out of Dart.
- Add runtime feature flag.

Gate: macOS, iOS, Android, and Linux builds pass with legacy behavior unchanged
when disabled.

### M7: Dual-protocol migration

- Run server discovery in shadow mode.
- Dual-publish where safe without duplicate imports.
- Prefer server lookup with legacy/direct/tracker/DHT fallback.
- Compare success, latency, and transfer-start metrics.

Gate: staged beta meets agreed reliability and latency objectives, with
configuration-only rollback.

### M8: Stable cutover

- Enable server protocol by default.
- Deprecate address-bearing invites after compatibility window.
- Complete security, load, backup, and restore reviews.
- Remove legacy code only after supported clients no longer depend on it.

## 20. Migration Sequence

1. Build and test Portalis Nexus without changing current app behavior.
2. Import the stable client façade into the Flutter Rust backend.
3. Authenticate using the existing device identity.
4. Register existing collections without changing local IDs or info hashes.
5. Publish heads through both paths during migration.
6. Read server state first and retain current fallbacks.
7. Prevent duplicate collection creation and media import across paths.
8. Measure both paths and stage rollout by cohort.
9. Keep rollback as runtime configuration.
10. Remove legacy address encoding only after the support window expires.

## 21. Initial Acceptance Objectives

- P95 authenticated command latency below 150 ms in-region.
- P95 in-memory peer lookup below 10 ms.
- Presence transition delivered within 5 seconds on healthy connections.
- No unbounded queue, task set, or collection under hostile input.
- Graceful shutdown within 30 seconds.
- Durable social and collection state survives restart.
- Peer leases self-heal within one refresh period after restart.

Discovery success and transfer-start success are measured separately because a
correct peer address may still be unreachable through NAT.

## 22. Minimal Administration UI

The first server exposes health and metrics only. A later UI may display server
and protocol versions, connection/presence counts, MongoDB and migration health,
peer-lease metrics, and rate-limit/authentication summaries.

Administration uses separate authorization and never exposes private keys,
challenges, collection secrets, or plaintext private descriptors.

## 23. Decisions Required Before Production

1. Account recovery and lost-device policy.
2. Hosting region, DNS, certificates, backups, and retention.
3. Relay or reachable-seed strategy for guaranteed transfer connectivity.
4. Collection-head history and command-receipt retention.
5. Measured trigger and technology for horizontal scaling.
6. Final load targets based on expected users, friends, collections, and peers.

## 24. Initial Technology Set

Exact versions are pinned during scaffolding.

- Tokio multithread runtime.
- Axum WebSocket/HTTP server.
- Tokio-compatible Rustls WebSocket client.
- Prost protobuf generation.
- Buf schema linting and breaking-change checks.
- Existing Ed25519 Dalek and BLAKE3 identity primitives.
- Official MongoDB Rust driver.
- DashMap or benchmarked equivalent for sharded tables.
- Tokio delay queue or equivalent for deadline expiration.
- UUIDv7 identifiers.
- `tracing`, Prometheus, and OpenTelemetry-compatible instrumentation.
- Property tests, testcontainers, fuzzing, mutation tests, and `cargo llvm-cov`.

Dependencies are selected only when they preserve portability, bounded resource
use, protocol stability, and testability.

## 25. References

- [Protocol Buffers language guide](https://protobuf.dev/programming-guides/proto3/)
- [Buf breaking-change detection](https://buf.build/docs/breaking/)
- [Tokio runtime documentation](https://docs.rs/tokio/latest/tokio/runtime/)
- [Axum WebSocket module](https://docs.rs/axum/latest/axum/extract/ws/)
- [MongoDB Rust driver](https://www.mongodb.com/docs/drivers/rust/current/)
- [MongoDB transactions](https://www.mongodb.com/docs/manual/core/transactions/)
