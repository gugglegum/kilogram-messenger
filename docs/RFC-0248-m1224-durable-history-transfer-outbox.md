# RFC-0248: M1.2.24 durable history-transfer outbox

## Status

Accepted locally on 2026-10-20. This is a dormant local persistence boundary,
not an active background sender. Runtime IPC remains postcard v33 with descriptor
domain v14, and the public default remains `legacy-only`.

## Problem

M1.2.23 can retry an exact request after a lost acknowledgement, but an in-memory
caller could generate a different nonce or Endpoint binding after restart. That
would create multiple valid request identities for one transfer and make it
unclear which recipient receipt closed the operation.

## Decision

Before any network attempt, the source creates the canonical M1.2.23 request and
persists its exact signed bytes in a dedicated redb outbox. A transfer head maps
the M1.2.22 transfer ID to exactly one request ID. The request independently
binds owner Account, source and recipient Devices, both Iroh Endpoint IDs,
nonzero session nonce, transfer ID and complete encrypted envelope.

Request and head are committed in one `Durability::Immediate` transaction. A
restart opens the store read-only or writable, revalidates metadata and every
signature/binding, and returns the same canonical bytes. Re-preparing the exact
request is idempotent; a different nonce, Endpoint or request for the same
transfer is rejected. Orphan request/receipt rows, duplicate heads, ID mismatch,
foreign owner and corruption fail closed. The store is capped at 16,384 intents.

The source persists the exact canonical recipient-Device-signed response in a
separate immediate transaction. It is accepted only after revalidation against
the durable request, both endpoints, transfer and recipient Device. Exact receipt
replay is idempotent; a different receipt after acknowledgement is rejected.
Injected failures before either commit leave `absent` or `pending`, never an
ambiguous acknowledged state.

## Authority and privacy boundary

Read-only `open_existing` never creates a database. Status exposes opaque request
and transfer IDs, an Account-scoped recipient token and only
`absent`/`pending`/`acknowledged`; it exposes no message body, plaintext, raw
identity, filesystem path or network coordinate. Persisted envelopes contain
encrypted projections and signed metadata, not readable message bodies.

The outbox does not itself open a connection, schedule work, retry in the
background, import history or register runtime/IPC/desktop authority. The new
ALPN remains absent from the active runtime. Whole-database rollback cannot be
detected without an external monotonic witness and is not claimed.

## Verification

Four stable-path tests cover exact request survival across close/read-only
restart, acknowledged receipt plus exact replay, competing request/receipt
rejection, and rollback of injected request and receipt failures before commit.
Together with the four M1.2.23 transport tests, the crate passes 8/8. The
dedicated boundary rejects 10/10 negative mutations. Aggregate accepted-tag
readiness contains 150 closed gates with rustfmt and workspace-wide
locked/offline strict Clippy.

No product network operation, second host, release build, archive, Git tag or
automatic GitHub build is used for this stage.

## Next

M1.2.25 should add an explicitly invoked executor that loads one pending intent,
uses its exact bytes for an M1.2.23 session, and commits the signed receipt with
a bounded retry policy. Background scheduling and multi-source recovery remain
later, separately gated stages.
