# Portalis Nexus

Portalis Nexus is the Rust control plane for Portalis. It provides a portable
client library and a Linux server for identity, friends, presence, collection
metadata, and BitTorrent peer discovery. Media remains peer-to-peer.

The architecture and migration contract live in [`SPEC.md`](SPEC.md).

## Workspace

- `crates/protocol`: protobuf-generated types and validation.
- `crates/client`: portable WebSocket client and deterministic protocol rules.
- `crates/server-core`: transport-independent server rules.
- `apps/server`: Linux-oriented Axum server.
- `proto`: authoritative protobuf schemas.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

If Buf is installed:

```sh
buf lint
buf breaking --against '.git#branch=main,subdir=portalis-nexus'
```

Run the server:

```sh
PORTALIS_NEXUS_LISTEN_ADDR=127.0.0.1:8080 cargo run -p portalis-nexus-server
```

Then request `http://127.0.0.1:8080/health/live` or
`http://127.0.0.1:8080/health/ready`.

The local server also exposes `ws://127.0.0.1:8080/v1/socket`. Clients must
request the `portalis.protobuf.v1` subprotocol, receive a binary protobuf
`ServerHello`, and can then exchange correlated `Ping`/`Pong` envelopes.

Use `NexusClient::connect_with_retry(endpoint, &ReconnectPolicy::default())`
when a caller needs a bounded retry loop. The policy doubles its delay per
failure, spreads it across 80%-120% with randomized jitter, caps the result at
its maximum delay, and returns the final transport error once its configured
maximum attempts are spent.
