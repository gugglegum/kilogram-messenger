# Kilogram

Kilogram is an experimental peer-to-peer messenger with end-to-end encryption.
The protocol is currently at the architecture and transport proof-of-concept
stage. Do not use it for sensitive communication.

The project goals and draft architecture are documented in
[`docs/RFC-0001-core-architecture.md`](docs/RFC-0001-core-architecture.md).
The current two-network Windows procedure is in
[`docs/M0.3-CROSS-NETWORK-TEST-RU.md`](docs/M0.3-CROSS-NETWORK-TEST-RU.md).

## Current milestone: M0.3 cross-network route verification

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
and number of open paths. These development diagnostics made the two-host LAN
test distinguish a real direct path from a successful relay fallback.

The listener now signs one of three application route policies into ticket v2:

- `auto` accepts Iroh's selected direct or relay path;
- `direct-only` permits relay-assisted connection establishment and NAT traversal,
  but withholds Kilogram protocol frames until a direct IP path is selected;
- `relay-only` disables all IP transports at both endpoints, so the ticket and
  the established connection contain only a relay path.

Connection establishment, route selection, stream opening, and framed wire I/O
have bounded timeouts with operation-specific diagnostics. A local process smoke
verified delivery and reconnect/sync after listener restart in both forced modes.
The public relay test selected `euc1-1.relay.n0.iroh.link`; the cross-network
two-host hole-punching test is still pending.

The file event store canonicalizes its root before deriving content-addressed
event paths. On Windows this produces verbatim absolute paths and avoids the
legacy 260-character limit even when `LongPathsEnabled=1` is insufficient for a
specific atomic-file operation. A Windows regression test and a full release
recovery smoke cover event paths longer than 260 characters.

M0.2 is complete on two physical Windows PCs. A signed text event and its
signed acknowledgement travelled over a direct LAN path with approximately
1 ms RTT. A second client store containing only Alice's device key recovered
both events from Bob in one sync round, verified them on read, and reconstructed
the acknowledgement as the single causal frontier.

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
  --ticket-file .tmp/listener.ticket `
  --route-policy auto
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
Iroh address, public application device ID, and route policy. The application
device signs this mapping together with the one requester device ID authorized
by the listener, so tampering is detected before a connection or inventory is sent.
Possession of the ticket alone is insufficient to deliver or synchronize
events without the allowed device key. The public ticket therefore exposes
both device IDs as metadata. It contains neither the Iroh endpoint secret nor
an application device secret. The ticket encoding, Postcard event encoding,
and development conversation-label derivation are provisional M0 choices, not
the final public wire protocol. Ticket v2 intentionally does not decode old v1
tickets; restart the listener to generate a ticket matching this build.

## Development checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```
