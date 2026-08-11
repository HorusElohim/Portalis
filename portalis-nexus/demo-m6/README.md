# Nexus M6 two-process demo

This demo keeps the real Nexus server and the client story in separate
processes. The client executable opens two live devices concurrently and
walks through the M6 control-plane path: identity, friendship and presence,
canonical capsule publication, recipient-device discovery, sealed key
delivery, encrypted `.torrent` handoff, and Nexus swarm lookup.

From `portalis-nexus`, start the prototype server in one terminal:

```sh
cargo run -p portalis-nexus-m6-demo --bin nexus-demo-server
```

Run the client story in another terminal:

```sh
cargo run -p portalis-nexus-m6-demo --bin nexus-demo-client
```

The server uses in-memory state and starts empty each time. Set both
`PORTALIS_NEXUS_M6_ADDRESS` and the client's `PORTALIS_NEXUS_M6_ENDPOINT` when
using a different address. The endpoint authority must match the server's
listen address because device signatures are bound to it.

Both demo processes log connection lifecycle and each command/response at
`debug` without logging opaque payload bytes. The client's ten narrated steps
remain on stdout while structured logs go to stderr. If your shell already
defines a quieter global filter, override it explicitly:

```sh
RUST_LOG=nexus_demo_server=info,portalis_nexus_server=debug \
  cargo run -p portalis-nexus-m6-demo --bin nexus-demo-server

RUST_LOG=portalis_nexus_client=debug \
  cargo run -p portalis-nexus-m6-demo --bin nexus-demo-client
```

The descriptor is a small, fixed private-torrent-shaped fixture. This demo
proves the Nexus connections and cryptographic envelopes; the Portalis backend
integration is responsible for constructing and validating production torrent
bytes before handing them to the portable client.
