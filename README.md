# Kilogram

Kilogram is an experimental peer-to-peer messenger with end-to-end encryption.
The protocol is currently at the architecture and transport proof-of-concept
stage. Do not use it for sensitive communication.

The project goals and draft architecture are documented in
[`docs/RFC-0001-core-architecture.md`](docs/RFC-0001-core-architecture.md).

## Current milestone: M0.1.2 local event store

The CLI exchanges a signed text event and a signed acknowledgement over an
authenticated Iroh/QUIC connection. Application-level device identities are
persistent and deliberately separate from ephemeral Iroh transport identities.
Every verified event is also persisted locally before the corresponding send
or acknowledgement. Repeated writes are idempotent and stored corruption is
detected when history is read.

This remains a development prototype. It does **not** yet implement Account
Root Identity, device authorization/revocation, message-level E2EE, encrypted
storage, peer-to-peer history synchronization, seed phrases, or groups. The
local device secret and message bodies are currently stored unencrypted in the
explicitly selected state directory. Do not use it for sensitive
communication.

Start a listener:

```powershell
cargo run -p kilogram-cli -- listen `
  --state-dir .tmp/bob `
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

The connection ticket is public addressing data: it contains the listener's
Iroh address and public application device ID, allowing the connector to bind a
signed acknowledgement to the invited device. It contains neither the Iroh
endpoint secret nor the application device secret. The Postcard event encoding
and development conversation-label derivation are provisional M0 choices, not
the final public wire protocol.

## Development checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```
