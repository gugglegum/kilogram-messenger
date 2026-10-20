# RFC-0247: M1.2.23 authenticated Iroh history-transfer session

## Status

Accepted locally on 2026-10-20. This is a separately testable transport crate,
not an active runtime protocol. Runtime IPC remains postcard v33 with descriptor
domain v14, the ordinary runtime does not advertise the new ALPN, and the public
default remains `legacy-only`.

## Problem

M1.2.22 produced a bounded source-Device-signed envelope and a durable recipient
inbox, but it did not define how two authenticated Iroh endpoints could transfer
that exact envelope. Reusing sync or mailbox framing would mix authorities and
would make endpoint substitution, partial framing and ambiguous acknowledgement
harder to reject.

## Decision

The new `kilogram-peer-history-recovery-transport` crate owns the dedicated ALPN
`kilogram/m1/history-recovery/postcard/1`. One canonical postcard request on one
bidirectional stream binds the authenticated source Endpoint ID, exact recipient
Endpoint ID, a nonzero 32-byte session nonce, the content-addressed transfer ID
and the complete M1.2.22 envelope. The envelope source Device signs the complete
request. A recipient compares the signed Endpoint IDs with the authenticated
Iroh connection before any durable mutation.

The request is bounded to 9 MiB plus 4 KiB of framing. The response is bounded
to 4 KiB, and every open/read/write/stopped operation has a 180-second deadline.
Empty, oversized, non-canonical, trailing, truncated, wrong-version, zero-nonce,
foreign-endpoint, foreign-Device and transfer-substitution inputs fail closed.
No fallback ALPN or alternate decoder is attempted.

After complete verification, the recipient stages the exact envelope through
the M1.2.22 append-only inbox. Only then does the recipient Device sign a
response binding the request ID, transfer ID, both Endpoint IDs, recipient
Device ID and `staged` or `already-staged` disposition. If the first response is
lost, an exact replay is idempotent and reports `already-staged`; it neither
duplicates the inbox entry nor grants import authority.

## Authority and privacy boundary

The new crate is intentionally absent from the CLI and Windows desktop manifests.
The active runtime neither advertises nor accepts its ALPN. The offline execution
crate still has no Iroh or Tokio dependency, so filesystem execution and network
transport remain separate authority domains. The recipient inbox still grants
neither network nor import authority; the existing recipient-Device-signed import
receipt chain controls consumption order.

The wire carries encrypted history projections and signed metadata from the
M1.2.22 envelope, never plaintext message bodies. This stage does not create a
durable source outbox, background scheduler, automatic resume, multi-source
selection, runtime IPC command or desktop control.

## Verification

Four stable-path tests cover signed request/response binding, a real local Iroh
loopback that stages once and acknowledges an exact retry, authenticated Endpoint
substitution before staging, and zero-nonce/trailing-byte rejection. A pre-existing
Iroh bidirectional loopback test is the control for the host transport. Windows
network harnesses are compiled without execution and launched only through
`scripts/run-cargo-tests-stable.ps1`, so the executable path is stable.

The dedicated boundary rejects 10/10 negative mutations. Aggregate accepted-tag
readiness contains 149 closed gates with rustfmt and workspace-wide locked/offline
strict Clippy. The loopback uses only `127.0.0.1`; no product Internet/relay
operation, second host, release build, archive, Git tag or automatic GitHub build
is used. Whole-database rollback detection without an external monotonic witness
is not claimed.

## Next

M1.2.24 should persist an exact source transfer intent and receipt so a crash or
lost acknowledgement can resume the same session safely. Runtime activation,
automatic scheduling and multi-source recovery remain later, separately gated
stages.
