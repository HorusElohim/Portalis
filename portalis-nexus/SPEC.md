# Portalis Nexus Specification

Status: M5 swarm discovery complete; the M6 online friend-to-friend sharing
slice is in progress. The portable capsule and handoff codecs, recipient-device
grant response, exact-device routing, and backend Ed25519/X25519 credential
migration are implemented; the backend collection binding and Flutter façade
remain. Share revocation is complete; the remaining M5.5 account-control
commands are not prerequisites for that slice.

Protocol: `portalis.protocol.v1`

Production target: Linux

Last updated: 2026-08-11

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
    │       ├── shares.proto
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

    // M5.5, account control.
    ListDevicesRequest list_devices_request = 20;
    ListDevicesResponse list_devices_response = 21;
    RevokeDevice revoke_device = 22;
    DeviceRevoked device_revoked = 23;
    ChangeHandle change_handle = 24;
    HandleChanged handle_changed = 25;
    DeleteAccount delete_account = 26;
    AccountDeleted account_deleted = 27;

    ResolveHandleRequest resolve_handle_request = 30;
    ResolveHandleResponse resolve_handle_response = 31;
    FriendCommand friend_command = 32;
    FriendEvent friend_event = 33;
    ListFriendsRequest list_friends_request = 34;
    ListFriendsResponse list_friends_response = 35;
    PresenceEvent presence_event = 36;
    BlockUser block_user = 37;          // M5.5
    UnblockUser unblock_user = 38;      // M5.5

    PutKeyEnvelope put_key_envelope = 40;
    KeyEnvelopePut key_envelope_put = 41;
    ListKeyEnvelopesRequest list_key_envelopes_request = 42;
    ListKeyEnvelopesResponse list_key_envelopes_response = 43;
    RevokeShareAccess revoke_share_access = 44;      // M5.5
    ShareAccessRevoked share_access_revoked = 45;    // M5.5

    PublishShare publish_share = 50;
    SharePublished share_published = 51;
    ListSharesRequest list_shares_request = 52;
    ListSharesResponse list_shares_response = 53;
    FetchShareRequest fetch_share_request = 54;
    FetchShareResponse fetch_share_response = 55;
    GrantShareAccess grant_share_access = 56;
    ShareAccessGranted share_access_granted = 57;
    ShareEvent share_event = 58;
    ShareHandoff share_handoff = 59;

    AnnouncePeer announce_peer = 60;
    PeerAnnounced peer_announced = 61;
    LookupPeersRequest lookup_peers_request = 62;
    LookupPeersResponse lookup_peers_response = 63;
    WithdrawPeer withdraw_peer = 64;
  }
}
```

Field allocation is grouped by subsystem, and a group keeps room to grow: 28
and 29 belong to identity, 39 to friends, 46 to 49 to shares and their key
envelopes, 65 upwards to swarm discovery.

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
- Maximum encrypted live handoff: 4 MiB + 1 KiB, including its authenticated
  wrapper and bounded collection name.
- Maximum encrypted manifest capsule: 256 KiB; both this byte limit and the
  manifest entry-count quota apply.
- Maximum handle: 32 UTF-8 bytes after normalization.
- Maximum collection name: 256 UTF-8 bytes.
- Maximum peer candidates returned: 64.
- Maximum pending client requests: 128.
- Maximum queued outbound messages per connection: 256.

Every limit has boundary-minus-one, boundary, and boundary-plus-one tests.

### Rate limits

Numbers, not intentions: a limit without one cannot be implemented or tested.
Exceeding a limit answers `RATE_LIMITED` with `retry_after_ms`; sustained
abuse closes the connection.

Before authentication, keyed by source address:

- 5 concurrent connections.
- 20 messages per second.
- 5 registrations per hour.

After authentication, keyed by user unless stated:

- 100 commands per second per connection, bursting to 200.
- 30 publications per minute per share.
- 60 friend commands per hour.
- 120 key-envelope pushes per minute per device.
- 600 peer announcements and lookups per minute per device.

### Per-user quotas

Bounded frames and queues stop one connection exhausting the server; these
stop one account exhausting the database, which is otherwise unbounded by
design.

- 16 devices per user.
- 2 000 friendships per user.
- 1 000 shares owned per user.
- 512 members per share.
- 4 096 manifest entries per snapshot.

A quota reached answers `INVALID_MESSAGE` naming the limit, because the caller
must remove something rather than retry.

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
lets an existing device add its envelope. Account recovery remains a separate
product decision and never weakens this approval rule.

Revoking a device stops Nexus issuing it new envelopes and answering its
requests. It does not reach the share keys that device already holds, so
revocation means rotating the key of every share it could read and publishing
the next revision sealed only to the devices that remain. Nexus enforces
authorization; only re-keying changes what a revoked device can still open.

### Device identity on Portalis clients

A device holds two independent keypairs: the Ed25519 signing key the Portalis
application already persists, and an X25519 key that only ever receives sealed
share keys. Neither is derived from the other. A client generates and stores
both together in whatever secure storage its platform offers, and a client
that has an Ed25519 identity but no X25519 key generates one before it first
registers or links.

Losing the X25519 private key loses access to every capsule sealed to it. The
device presents a new key and an owner re-seals; there is no recovery path
that avoids someone who already holds the share key, by design.

Nexus derives `DeviceId` as BLAKE3 over the Ed25519 public key. The Portalis
application has until now used that raw public key as its own device
identifier. The two must not be confused: both are 32 bytes and neither is
self-describing. The application adopts the derived identifier, and manifest
entries carry the public key itself (§11), so both are recoverable from one
stored field and neither has to be looked up.

That changes what a manifest entry signs, which makes it a manifest version
rather than a rename. Entries written by earlier builds are verified under the
rule they were signed with, and are rewritten in the current encoding when
their collection is next published.

## 10. Friends and Presence

Friendship uses one canonical edge document with the two binary user IDs sorted
into `user_low` and `user_high`.

```text
NONE -> PENDING -> ACCEPTED
  ^        |          |
  |        v          v
  +----- REJECTED   REMOVED

any state ----------> BLOCKED
```

Commands use compare-and-set filters so concurrent accept, reject, and remove
operations are deterministic and idempotent.

### Blocking

A refusal that can be sent again is not a refusal, and handle resolution makes
every account reachable by anyone who knows its handle. Either side may
therefore block the other from any state.

While an edge is blocked, friend commands from both sides are refused, and
presence is delivered in neither direction. Existing share memberships between
the two are left alone: they belong to the shares' owners to revoke, and
silently dropping someone's access to media is a different decision from
declining to hear from them.

The edge records who blocked, and only that user may clear it, returning the
edge to `NONE`. Blocking is the one transition the other side cannot answer.

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

### Canonical manifest and `SnapshotId`

The manifest lists the media a share contains. Its canonical encoding is the
one definition both the content root and the capsule are built from, so any
client that can compute one can compute the other, and two clients that
disagree about a byte disagree about the share.

```text
manifest := "portalis.manifest.v1\0"
            u32     entry_count
            entry*                        ascending by info_hash

entry    := u8      entry_version = 1
            u8[20]  info_hash             BitTorrent v1, from the torrent engine
            u32     name_len
            u8[]    name                  name_len bytes, UTF-8, NFC-normalized
            u8      has_thumbnail         0 or 1
            u8[32]  thumbnail_hash        present only when has_thumbnail is 1
            u8[32]  author_public_key     Ed25519
            u64     added_at_unix_ns
            u8[64]  signature             over every preceding field of the entry
```

Every integer is little-endian, and every variable-length field carries its
length before its bytes, so no pair of fields can be reinterpreted as a
different pair — the rule the connection payloads in `signing.rs` already
follow.

`SnapshotId` is BLAKE3 over exactly those bytes. The Portalis backend builds
them once from the torrent engine's authoritative info hashes; it never
performs a second file scan, never asks Nexus to reinterpret a torrent, and
never treats protobuf serialisation as the canonical encoding.

An entry carries its author's Ed25519 public key rather than an identifier
derived from it. The key is what the signature verifies against, and every
identifier this system uses is derivable from it, which is what lets one
manifest satisfy both the local collection model and Nexus (§9).

### Capsule format

Nexus stores the capsule and never opens it, so its format is a contract
between clients that the server cannot enforce. It is fixed here for that
reason:

```text
capsule  := u8      capsule_version = 1
            u8[12]  nonce
            u8[]    ciphertext            ChaCha20-Poly1305, tag included
```

The plaintext is exactly the canonical manifest above, and the key is the
share's random 32-byte secret. The nonce is
`BLAKE3("portalis.capsule.v1/nonce" || share_id || revision_le || snapshot_id)[..12]`.
Including the snapshot prevents two owner devices that race with different
candidate manifests for one revision from reusing a ChaCha20-Poly1305 nonce.
It remains identical for an exact retry, so a publisher whose acknowledgement
was lost re-encrypts to the same bytes and Nexus recognises the retry rather
than refusing it. Associated data is
`share_id || revision_le || snapshot_id`, so a capsule lifted from one share
or revision fails to open under another.

`capsule_version` is the only field a reader may act on before authenticating
the rest. A version it does not know is a capsule it must not guess at.

### Media confidentiality

Encrypting the capsule keeps manifest entry names and info hashes away from
Nexus. The separately encrypted live handoff keeps the `.torrent` descriptor,
file names, and directory structure away from it. Neither mechanism encrypts
the media: BitTorrent pieces move between peers exactly as stored.

Version 1 therefore rests the confidentiality of a private share on the
secrecy of its info hashes. A torrent created for the Nexus path is private
from birth: its info dictionary contains the `private` flag, and clients use
no DHT, PEX, tracker, or local peer discovery for it. Nexus is its only
discovery path (§12).

An existing public torrent cannot be made private without changing its info
hash, because the `private` flag is inside the hashed info dictionary. The M6
slice therefore publishes newly created private torrents only. Existing
collections keep their current hashes and legacy discovery behavior until a
later migration explicitly replaces their torrents; they are never relabelled
as private while retaining a public-torrent hash.

This is a deliberate and weaker position than encrypting payloads, and it is
stated rather than implied: anyone who learns an info hash can fetch the
pieces, and a former member keeps that knowledge. Payload encryption is the
upgrade path; the capsule carries a version byte so adding a content key is an
additive change rather than a new format.

### Publication

The first authenticated publication creates a share and makes its publisher the
permanent owner. A subsequent update publishes exactly
`current_revision + 1`, names the prior snapshot, and points to the new
snapshot. An identical signed retry is idempotently successful; conflicting
bytes for one revision fail. A peer may seed and fetch a snapshot but cannot
mutate the owner's share.

Nexus stores the bounded capsule and its signature, never plaintext torrent
descriptors, file names, directory structure, or media. It does not include a
BitTorrent library and never parses or validates what a capsule contains — it
holds bytes it cannot read, and returns them to permitted devices so a newly
linked device can synchronise while the original is offline.

The signature is made by the publishing device's Ed25519 key over the
domain-separated tuple `share_id`, revision, optional prior snapshot,
`snapshot_id`, and `BLAKE3(capsule)`. Nexus verifies it against the
authenticated device before storing the publication. A returned snapshot
names the publisher device and carries its signing public key, so a recipient
can derive the device ID and verify the same signature before opening the
capsule.

Opening a capsule also verifies every manifest entry against the
`author_public_key` it carries. Canonical structure without valid authorship
is not an accepted manifest. Names are NFC-normalized before signing; a decoder
rejects a name that is not already NFC so decoding never silently changes
signed bytes.

### Membership

Membership is stored as separate edge documents. Server authorization is
checked before a private share summary, capsule, or live handoff is returned.
The live rendezvous path may also forward an authorized encrypted descriptor
from one online client to another; it is bounded, transient, and is not written
to MongoDB. M6 uses `.torrent` bytes and never a bare magnet on the private
path.

An owner grants and revokes membership. Revoking removes the edge, so Nexus
stops returning summaries, capsules, envelopes, and handoffs to that user. It
does not reach what they already hold. Removing someone therefore means
rotating the share key and publishing the next revision sealed only to the
members who remain — the same rule device revocation follows (§9). Nexus
enforces authorization and cannot enforce forgetting; a specification that
implied otherwise would be lying about what removal buys.

### Collections become shares

A Portalis collection keeps its local identifier when it is published.
Publishing mints a fresh `ShareId` (UUIDv7) and a fresh random 32-byte share
key, and stores both beside the collection. M6.0 does this only for new
collections whose torrents were private from birth; §20 defines the later
choice for existing public info hashes.

The existing invite secret never becomes the share key. It is a join
credential that everyone holding an invite link has already seen, and a share
key must be known only to the devices an owner has sealed it to.

### M6 online friend-to-friend handoff

The first M6 product slice is deliberately narrower than the complete share
model. It proves this one journey:

> A and B are accepted friends and each has one online device. A creates a new
> private collection, shares it with B through Nexus, and B downloads its files
> directly from A.

The flow is:

1. A creates each collection entry as a BitTorrent v1 torrent with
   `private = true`, adds it locally from `.torrent` bytes, and retains those
   exact bytes beside the local entry for handoff.
2. A mints and persists the collection's `ShareId` and random share key, builds
   the canonical manifest, seals it, signs the publication, and publishes
   revision one.
3. A selects B from its accepted friends and grants B access. The successful
   response includes B's bounded list of non-revoked recipient devices, each
   with `DeviceId` and X25519 public key. The per-user device quota bounds this
   disclosure. It is available only to the share owner for a user who has
   access to that share.
4. A seals the share key to each returned device and stores the resulting key
   envelope before sending torrent metadata.
5. For each manifest entry, A sends an encrypted live handoff to B's device.
   Its protocol wrapper carries `share_id`, `recipient_device_id`, `info_hash`,
   and the ciphertext. Nexus forwards it only to that exact authenticated
   device. An offline recipient produces a typed unavailable response; success
   means the payload was queued to that live connection, not that its media
   download completed.
6. B fetches the authorized snapshot and its key envelope, opens the capsule,
   decrypts the handoff, validates the torrent descriptor and info hash,
   creates one local collection bound to the `ShareId`, and adds the torrent
   from bytes with Nexus peer candidates as initial peers. If the live event
   arrives before the envelope refresh completes, B retains that bounded event
   locally and processes it after the key is available.
7. A refreshes its Nexus swarm lease while seeding. B discovers A through
   Nexus only and BitTorrent transfers the media directly between them.

One handoff carries one entry and has this client-to-client format:

```text
handoff   := u8      handoff_version = 1
             u8[12]  random_nonce
             u8[]    ciphertext             ChaCha20-Poly1305, tag included

plaintext := u32     collection_name_len
             u8[]    collection_name         UTF-8, NFC-normalized
             u8[20]  info_hash
             u32     torrent_len
             u8[]    torrent_bytes
```

The share key encrypts the plaintext. Associated data is
`share_id || recipient_device_id || info_hash`. The sender draws a fresh nonce
for every attempt. The receiver rejects a descriptor over the protocol limit,
a descriptor whose info dictionary is not private, or a descriptor whose
computed v1 info hash differs from the handoff and manifest. Receiving the
same `share_id + info_hash` again is idempotent.

The handoff uses `.torrent` bytes rather than asking librqbit to resolve a bare
magnet. A magnet resolver does not know that a torrent is private until after
it obtains metadata and may disclose the info hash to DHT first. Adding the
validated descriptor as bytes gives the engine the private flag before it
starts discovery.

The collection name is included because the canonical manifest describes its
entries, not the collection object. A keeps its existing local collection ID;
B generates its own local ID. Both persist the same `ShareId`, which is the
deduplication key for the Nexus path. The legacy invite secret remains a
legacy-path credential and is not required by this flow.

This slice intentionally defers offline handoff storage, share invitations and
acceptance, collaborative member publication, linked-device key repair,
cryptographic erasure after revocation, migration of existing public torrents,
and relayed media transport. A and B must be online at the same time, and the
transfer remains subject to ordinary BitTorrent reachability.

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

A private share is the exception: its torrents carry the BitTorrent `private`
flag, so DHT, PEX, trackers, and local discovery are not used for them and
Nexus is the only source of candidates (§11). A receiver adds a private
torrent from validated descriptor bytes, never through a bare magnet resolver.
Its confidentiality rests on the secrecy of its info hashes, and announcing
one to a public network spends that secrecy permanently.

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

- Share ID, user ID, granted timestamp.
- Unique `(share_id, user_id)` and user-listing indexes.
- No role: the owner is on the share record and everyone else may read, which
  is every distinction the rules currently draw. A role column with one value
  is a source of truth waiting to disagree with the one beside it.
- No revocation state: revoking removes the edge. A share's history lives in
  `share_snapshots`, and keeping tombstones here would only record who was
  once allowed to read media they may still hold (§11).

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
- Apply the per-address limits of §8 before authentication and the per-user
  ones after, and enforce the per-user quotas that bound durable growth.
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

Status: complete.

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

The completed path stores mutable share heads, append-only snapshots, and
membership edges in memory or MongoDB. Snapshot insertion and head movement
share one transaction, with the head revision in the write filter; a lost CAS
aborts without leaving an orphan revision. Lists, fetches, live events,
per-device key envelopes, and transient encrypted handoffs all check the same
membership source of truth. Unauthorized and missing share IDs both answer
`NOT_FOUND`, preventing private-state probing. Capsules and handoffs are
bounded metadata and Nexus never parses or persists their plaintext.

Gate met: a granted device receives and opens its own sealed share key, fetches
the latest encrypted capsule, outsiders cannot discover it, and both the pure
rules and durable transaction refuse stale revisions.

### M5: Swarm discovery

Status: complete.

- Announce, refresh, lookup, and expire peer leases.
- Fast swarm table and candidate diversity.
- Client merge of server, direct, tracker, and DHT candidates.

Gate: two clients discover current endpoints and expired peers disappear.

The server binds every announcement to the remote IP observed by the TCP
socket and accepts only its listen port, family, transports, and lease request
from the client. Leases refresh by device, are removed on disconnect, and are
checked for expiration on every lookup. Results are bounded, prefer compatible
transport and recent leases, and take one peer per IPv4 /24 or IPv6 /64 before
filling remaining slots. The client merges direct, Nexus, tracker, and DHT
candidates by endpoint with deterministic source preference.

Gate met: two authenticated socket clients discover each other's current
source-bound endpoints; disconnect and time-based expiry both remove leases.

### M5.5: Account and membership control

Status: partial. `RevokeShareAccess` is complete. The other commands remain
required before a production account UI, but do not block the bounded M6
online-sharing slice below.

Every command here is one a user-facing client must offer and cannot build
today: the protocol can add a member but not remove one, mark a device revoked
but never say so, and refuse a friend request that can be sent again a second
later. A production account UI would otherwise ship buttons that have no
messages behind them; the M6.0 development slice exposes none of those
unfinished controls.

- `RevokeShareAccess`: an owner removes a member (§11).
- `ListDevices` and `RevokeDevice`: a user sees their devices and signs one
  out. A device may revoke itself; only an authorized device may revoke
  another. Revoking closes that device's live connections.
- `BlockUser` and `UnblockUser` (§10).
- `ChangeHandle`: a new username under the existing `UserId`, allocated by the
  same rules as registration.
- `DeleteAccount`: removes users, devices, friendships, memberships, and key
  envelopes; leaves published snapshots owned by nobody and unreachable.

Gate: each command's effect survives a restart, a revoked device's live
connections close, a blocked user cannot re-request, and a deleted account
frees its handle.

### M6: Flutter integration

M6 is delivered in two slices. The first proves the product path without also
solving offline delivery, account administration, or migration.

#### M6.0: one online friend receives one new private collection

Protocol and portable-client work, in order:

1. Change capsule nonce derivation to include `snapshot_id` and add fixed
   cross-platform test vectors.
2. Verify manifest-entry signatures and NFC on construction and capsule open.
3. Define the publication signing payload. Add publisher device identity to a
   returned snapshot and make Nexus verify the signature before storage.
4. Extend `ShareAccessGranted` with the granted user's bounded non-revoked
   recipient-device IDs and X25519 public keys. Add the entry info hash to the
   outer `ShareHandoff` message so the receiver can construct its authenticated
   context before decryption.
5. Implement the versioned encrypted torrent-handoff codec from §11 in
   `portalis-nexus-client`.
6. Route a handoff to the exact requested device and return a typed unavailable
   result when that device has no live connection.
7. Align the documented torrent-descriptor and encrypted-handoff limits with
   the protocol constants, keeping one complete handoff below the 8 MiB frame
   limit.

Portalis backend work, in order:

1. Add only `portalis-nexus-client` as the Nexus dependency and implement its
   signer with the existing Ed25519 identity. Generate the independent X25519
   key before first registration.
2. Add an optional local Nexus binding beside a collection: `ShareId`, share
   key, acknowledged revision and snapshot, plus the exact private `.torrent`
   bytes for each entry. It is one source of truth for retries and handoffs.
3. Add a Nexus-only creation path that builds torrents with `private = true`
   from birth. Do not convert existing collection torrents in this slice.
4. Publish revision one, grant the selected accepted friend, seal the share key
   to the returned device keys, and send one handoff per manifest entry.
5. On B, consume the key envelope and handoff, validate both the descriptor and
   manifest, create or find the local collection by `ShareId`, and add the
   torrent from bytes.
6. Announce A's lease and give B only Nexus lookup results as initial peers for
   that private torrent. No DHT, PEX, tracker, local discovery, or bare-magnet
   resolution is allowed on this path.
7. Expose the smallest Flutter Rust Bridge façade needed to select a friend,
   start the share, observe waiting/failed/receiving state, and report typed
   errors. Protobuf and sockets remain outside Dart.
8. Put the entire path behind the runtime feature flag; disabled behavior is
   byte-for-byte the legacy behavior.

Automated gate:

- Two real backend instances and one real Nexus server execute the complete
  flow from new files on A to verified downloaded bytes on B.
- The received `.torrent` is private, its computed info hash matches the
  manifest, and the test observes no DHT or tracker discovery for it.
- Repeating publication, grant, envelope push, handoff, and receive is
  idempotent.
- A handoff to an offline or wrong device fails without creating a collection
  on B or reporting success on A.
- Legacy behavior remains unchanged when the feature is disabled.

#### M6.1: portability and account completeness

- Complete the remaining M5.5 device and account controls.
- Move both private keys behind the platform secure-storage adapter.
- Add linked-device key repair and the broader FRB façade.
- Pass macOS, iOS, Android, and Linux builds; a capsule and handoff written on
  one platform open on another.
- Decide which deferred §11 capabilities enter the next product slice based on
  observed use, rather than implementing them speculatively.

M6.0 is a development-gated integration slice, not a production security
release. M6.1 secure storage is required before enabling Nexus for an external
beta.

When Nexus is unreachable the client keeps working against its local state and
its existing discovery paths. A pending M6.0 share remains visible locally and
retries when both users are online; it does not silently fall back to public
discovery. Publications wait rather than fail, since a publication is a
revision of durable local state and not a request that can be abandoned;
everything else refreshes on reconnect (§8).

### M7: Dual-protocol migration

- Run server discovery in shadow mode.
- Dual-publish where safe without duplicate imports.
- Prefer server lookup with legacy/direct/tracker/DHT fallback only where the
  collection's public/private policy permits it.
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
3. Authenticate using the existing device identity, and generate the X25519
   key beside it (§9).
4. Deliver M6.0 for newly created Nexus-private collections only. Mint a
   `ShareId` and share key and bind them to the existing local collection ID.
5. Leave every existing collection on the legacy path during M6.0. Do not
   claim that an existing public torrent became private without changing its
   info hash.
6. Measure the bounded flow before choosing an existing-collection migration.
   A migrated torrent either remains explicitly legacy-public with its current
   hash, or is rebuilt as private with a new hash.
7. Preserve legacy manifest entries under their original verification rule.
   Only the device holding an entry author's private key can re-sign that
   authorship in the current encoding; another device must retain the legacy
   signature or create an explicit owner re-attestation rather than pretending
   to be the original author.
8. When dual publication begins, use persisted `ShareId` bindings to prevent
   duplicate collection creation and media import across paths.
9. Read server state first only for Nexus-bound collections and retain the
   matching legacy fallback policy for legacy-public ones. Private torrents
   never fall back to DHT, trackers, or local discovery.
10. Stage rollout by cohort and keep rollback as runtime configuration.
11. Remove legacy address encoding only after the support window expires.

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
7. Whether media payloads are encrypted before seeding, which §11 defers by
   resting v1 on info-hash secrecy. The answer decides whether a former member
   can still fetch pieces they hold an info hash for.
8. Retention of a deleted account's published snapshots, which §19's M5.5
   leaves owned by nobody rather than removing them from every member's
   history.

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
