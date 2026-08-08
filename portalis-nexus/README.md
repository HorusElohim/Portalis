# Portalis Nexus

Portalis Nexus is the Rust control plane for Portalis. It provides a portable
client library and a Linux server for identity, friends, presence, collection
metadata, and BitTorrent peer discovery. Media remains peer-to-peer.

The architecture and migration contract live in [`SPEC.md`](SPEC.md).

## Workspace

- `crates/protocol`: protobuf-generated types and validation.
- `crates/client`: portable client-side protocol foundation.
- `crates/server-core`: transport-independent server rules.
- `apps/server`: Linux-oriented Axum server.
- `proto`: authoritative protobuf schemas.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
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
