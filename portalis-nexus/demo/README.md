# Portalis Nexus demo

Runnable examples of the control plane. Nothing here is mocked: it is the real
server, the portable client, and real sockets between them.

## Guided walkthrough

```sh
cargo run -p portalis-nexus-demo
```

Starts a server on an ephemeral port and narrates one story end to end:
greeting, registration, two devices sharing a username, a replayed challenge
being refused, the same device authenticating on a fresh connection, an
unenrolled device being refused, a correlated ping, and a graceful drain.

## Two processes

Start the server:

```sh
cargo run -p portalis-nexus-server
```

Then run the client. The first run registers; every later run authenticates
with the key it saved:

```sh
cargo run -p portalis-nexus-demo --bin client
cargo run -p portalis-nexus-demo --bin client
```

Arguments are `[endpoint] [username]`, and `PORTALIS_NEXUS_DEMO_KEY` chooses
where the device key is written (`demo-device.key` by default).

Identities are held in memory, so restarting the server forgets them. Delete
the key file to start over as a new device.

## What to read

- `src/lib.rs` — `DemoDevice` implements `DeviceSigner`. This is the seam an
  application fills: the client only ever sees the public key and finished
  signatures, never the private key, so a real app can back it with a keychain
  or secure enclave.
- `src/bin/client.rs` — authenticate first, register only when the server does
  not know the device. Note that each connection may sign once, so the attempt
  spends that connection's challenge either way.
- `src/main.rs` — the same calls, with a server embedded in the process.

## Authority

A signature is bound to the server it was meant for. The client derives that
authority from the endpoint it dials, and the server carries its configured
one; they must agree or every signature is refused. The walkthrough binds the
server to its ephemeral address for exactly this reason.
