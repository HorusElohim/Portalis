# Portalis Nexus

Portalis Nexus is the Rust control plane for Portalis. It provides a portable
client library and a Linux server for identity, friends, presence, collection
metadata, and BitTorrent peer discovery. Media remains peer-to-peer.

The architecture and migration contract live in [`SPEC.md`](SPEC.md).

## Workspace

- `crates/protocol`: `limits`, `ids`, `frame`, `validate`.
- `crates/client`: `error`, `protocol`, `pending`, `reconnect`, `config`, and
  the `transport` socket actor.
- `crates/server-core`: transport-independent server rules.
- `apps/server`: `config`, `state`, `shutdown`, `health`, `messages`, `socket`.
- `proto`: authoritative protobuf schemas.
- `demo`: runnable examples — see [`demo/README.md`](demo/README.md).
- `demo-m6`: standalone server and concurrent-client M6 walkthrough — see
  [`demo-m6/README.md`](demo-m6/README.md).

Deterministic rules live in their own modules and are covered by tests; the
socket plumbing they drive (`apps/server/src/socket.rs`, `crates/client/src/
transport/`) is excluded from the coverage gate as a platform adapter.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

If Buf is installed:

```sh
buf lint
# `.git` lives in the repository root, one level above this workspace.
buf breaking --against '../.git#subdir=portalis-nexus,ref=main'
```

Until the schema is released on `main`, compare against the previous commit
instead: `buf breaking --against '../.git#subdir=portalis-nexus,ref=HEAD~1'`.

See it work end to end, server and clients in one process:

```sh
cargo run -p portalis-nexus-demo
```

Or run the M6 server and client story as two separate processes:

```sh
cargo run -p portalis-nexus-m6-demo --bin nexus-demo-server
# In another terminal:
cargo run -p portalis-nexus-m6-demo --bin nexus-demo-client
```

Run the server:

```sh
PORTALIS_NEXUS_LISTEN_ADDR=127.0.0.1:8080 cargo run -p portalis-nexus-server
```

Then request `http://127.0.0.1:8080/health/live` or
`http://127.0.0.1:8080/health/ready`.

Every Nexus executable installs a `tracing` subscriber. The production server
emits JSON; demos use compact text on stderr and keep their narrated steps on
stdout. Client and server transport logs carry connection IDs, message IDs,
and protocol variant names only — never encrypted payloads or private
metadata. Set `RUST_LOG` to tune them, for example:

```sh
RUST_LOG=portalis_nexus_client=debug,portalis_nexus_server=debug \
  cargo run -p portalis-nexus-demo
```

The server process requires MongoDB and refuses to start without
`PORTALIS_NEXUS_MONGODB_URI`; identity and friendship state must survive every
restart. In-memory storage remains an explicit test and development adapter,
not a production fallback. Start a local server with:

```sh
PORTALIS_NEXUS_MONGODB_URI=mongodb://127.0.0.1:27017/?directConnection=true \
  cargo run -p portalis-nexus-server
```

Registration writes a user and its first device in one transaction, so the
server needs a replica set rather than a standalone. `PORTALIS_NEXUS_DATABASE`
names the database and defaults to `portalis_nexus`. `docker/compose.yaml`
brings up both the server and a single-node replica set already configured for
this.

The MongoDB tests start their own replica set through Docker and are skipped
when Docker is unavailable. To run them against a server you already have:

```sh
PORTALIS_NEXUS_TEST_MONGODB_URI=mongodb://127.0.0.1:27017/?directConnection=true \
  cargo test -p portalis-nexus-server --test mongo
```

The local server also exposes `ws://127.0.0.1:8080/v1/socket`. Clients must
request the `portalis.protobuf.v1` subprotocol, receive a binary protobuf
`ServerHello`, and can then exchange correlated `Ping`/`Pong` envelopes.

## Client

`NexusClient` is a supervised handle that owns no socket:

```rust
let client = NexusClient::connect("wss://nexus.example/v1/socket").await?;
let pong = client.ping(42).await?;      // correlated, timed out, concurrent
let events = client.events();           // server-initiated envelopes
client.shutdown().await;                // closes the socket and stops retrying
```

`connect` makes one handshake attempt, so a misconfigured endpoint fails
immediately. `connect_with_config` retries the first attempt too, and sets the
request timeout:

```rust
let config = ClientConfig {
    reconnect: ReconnectPolicy::new(initial, maximum, attempts)?,
    request_timeout: Duration::from_secs(5),
};
let client = NexusClient::connect_with_config(endpoint, &config).await?;
```

A supervisor task rebuilds the connection whenever it ends, so callers do not
reconnect by hand. `ReconnectPolicy` doubles its delay per failure, spreads it
across 80%-120% with randomized jitter, and caps the result at its maximum
delay. In-flight requests fail as soon as their connection drops rather than
waiting for the timeout.

Transport behaviour is covered by integration suites against real sockets:
`tests/connection.rs`, `tests/reconnect.rs`, and `tests/events.rs`, with shared
servers in `tests/common/`. Alongside the real server they run peers that never
answer, skip the subprotocol, advertise an unsupported version, push
unsolicited envelopes, or close on command.

## Backpressure and shutdown

Both peers split each socket into a read loop and one writer task joined by a
queue of at most `MAX_OUTBOUND_QUEUE` messages. Filling that queue disconnects
the peer, so a client that stops reading cannot grow server memory. At most
`MAX_PENDING_REQUESTS` commands may await a response at once.

On `SIGTERM` the server finishes its HTTP serve loop and then calls
`Shutdown::drain`, which asks every live socket to send a close frame and waits
up to `GRACEFUL_DRAIN_TIMEOUT` for them to finish.
