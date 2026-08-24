# Kilogram

Kilogram is an experimental peer-to-peer messenger with end-to-end encryption.
The protocol is currently at the architecture and transport proof-of-concept
stage. Do not use it for sensitive communication.

The project goals and draft architecture are documented in
[`docs/RFC-0001-core-architecture.md`](docs/RFC-0001-core-architecture.md).

## Current milestone: M0.1.5 transport path diagnostics

The CLI exchanges a signed text event and a signed acknowledgement over an
authenticated Iroh/QUIC connection. Application-level device identities are
persistent and deliberately separate from ephemeral Iroh transport identities.
Every verified event is also persisted locally before the corresponding send
or acknowledgement. Repeated writes are idempotent and stored corruption is
detected when history is read.

Two known conversation devices can reconcile bounded batches in both directions
until their histories converge. The inventory is signed by the requesting
application device and bound to the listener's current Iroh Endpoint ID. The
listener signs its diff with the application device key named in the ticket. It
rejects devices that have never authored an event in its local copy of the
conversation.

The transport-independent reconciliation state machine lives in
`kilogram-session`; the Iroh ALPN and typed stream framing live in
`kilogram-transport-iroh`. The CLI only orchestrates these layers. This is the
first concrete transport-replacement boundary, not yet the final transport API.

After a delivery or synchronization exchange, both peers wait up to three
seconds for Iroh relay-to-direct migration and print `transport_path` (`direct`,
`relay`, `custom`, or `unknown`), the selected remote transport address, RTT,
and number of open paths. These development diagnostics make the upcoming
two-host LAN test distinguish a real direct path from a successful relay
fallback.

This remains a development prototype. It does **not** yet implement Account
Root Identity, device authorization/revocation, message-level E2EE, encrypted
storage, account-authorized synchronization, seed phrases, or groups. The local
device secret and message bodies are currently stored unencrypted in the
explicitly selected state directory. Do not use it for sensitive communication.

Create or load Alice's identity first:

```powershell
cargo run -p kilogram-cli -- identity --state-dir .tmp/alice
```

Copy the printed device ID and start Bob's listener with that device explicitly
authorized:

```powershell
cargo run -p kilogram-cli -- listen `
  --state-dir .tmp/bob `
  --allow-device <ALICE_DEVICE_ID> `
  --ticket-file .tmp/listener.ticket
```

In another terminal, connect and send a message:

```powershell
cargo run -p kilogram-cli -- connect `
  --state-dir .tmp/alice `
  --ticket-file .tmp/listener.ticket `
  --message "hello"
```

The listener exits after acknowledging one event. Reusing a state directory
preserves the application device ID and advances its author sequence across
restarts. Do not run two processes against the same state directory: the M0
sequence allocator is intentionally single-process only.

Inspect and cryptographically verify the local history after either process has
exited:

```powershell
cargo run -p kilogram-cli -- history --state-dir .tmp/alice
cargo run -p kilogram-cli -- history --state-dir .tmp/bob
```

Events are stored as immutable content-addressed files beneath
`STATE_DIR/events`. The `history` command prints the current causal frontier;
its file-order output is deterministic but is not yet a chat timeline.

To synchronize missing events, start `listen` again on one device and run:

```powershell
cargo run -p kilogram-cli -- sync `
  --state-dir .tmp/alice `
  --ticket-file .tmp/listener.ticket
```

Each round accepts at most 4,096 inventory IDs and transfers at most 64 events
in each direction. One `sync` invocation automatically continues for up to 64
rounds on the same Iroh connection and finishes with
`sync_more_available=false`. This full-ID inventory is an M0 mechanism, not the
future compact Merkle summary: once a local conversation exceeds 4,096 events,
this development profile must be replaced rather than treated as scalable sync.

The connection ticket is public addressing data: it contains the listener's
Iroh address and public application device ID. The application device signs
this mapping together with the one requester device ID authorized by the
listener, so tampering is detected before a connection or inventory is sent.
Possession of the ticket alone is insufficient to deliver or synchronize
events without the allowed device key. The public ticket therefore exposes
both device IDs as metadata. It contains neither the Iroh endpoint secret nor
an application device secret. The ticket encoding, Postcard event encoding,
and development conversation-label derivation are provisional M0 choices, not
the final public wire protocol.

## Development checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```
