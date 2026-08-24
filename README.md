# Kilogram

Kilogram is an experimental peer-to-peer messenger with end-to-end encryption.
The protocol is currently at the architecture and transport proof-of-concept
stage. Do not use it for sensitive communication.

The project goals and draft architecture are documented in
[`docs/RFC-0001-core-architecture.md`](docs/RFC-0001-core-architecture.md).

## Current milestone: M0 transport smoke test

The first CLI exchanges one UTF-8 message over an authenticated Iroh/QUIC
connection. It does **not** yet implement Kilogram identities, message-level
E2EE, history, seed phrases, or groups.

Start a listener:

```powershell
cargo run -p kilogram-cli -- listen --ticket-file .tmp/listener.ticket
```

In another terminal, connect and send a message:

```powershell
cargo run -p kilogram-cli -- connect `
  --ticket-file .tmp/listener.ticket `
  --message "hello"
```

The listener exits after acknowledging one message. The connection ticket is
public addressing data; it does not contain the endpoint's secret key.

## Development checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```
