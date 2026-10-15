# RFC-0158: canonical runtime IPC durable message insertion

Status: implemented as an inactive M1.1.32 candidate.

## Summary

M1.1.32 adds `queue-message` to the deterministic-CBOR runtime IPC candidate.
It covers only the authority and idempotence boundary that turns one local
plaintext message request into one durable encrypted-at-rest queue record.
Snapshot-bound outbox inspection is deliberately a separate read-only stage.

This contract does not activate canonical CBOR. The shipping runtime remains
Postcard IPC version 26 with JSON descriptor version 7. No listener, network
protocol, ALPN, endpoint publication, desktop behavior or accepted M1.0.0 tag
changes.

## Request ID is the queue ID

The caller supplies one cryptographically random nonzero 32-byte request ID.
That exact value is also the durable queue ID. Retrying the byte-equivalent
request therefore addresses the same record and returns `inserted=false`
instead of creating a duplicate. Reusing the ID with a different conversation,
peer or body must fail closed.

The signed response commits to the digest of the complete request, so changing
any request field cannot be paired with an earlier acknowledgement. The
`inserted` bit reports only whether this exact durable record was newly created;
it is not a delivery receipt.

## Contact and membership authority

The request names a bounded UTF-8 conversation label and an exact peer Account.
The peer must differ from the local Account in the signed descriptor. A success
result repeats the peer, derives the label-bound Conversation ID and derives the
canonical contact ID from local Account, peer Account and Conversation ID.

An eventual active adapter must, under the same state lock and transaction as
queue insertion, verify that the exact contact is enrolled and that the current
conversation membership authorizes both Accounts. The portable CBOR object does
not embed the full membership because that authority is already durable local
state; a signed response cannot make a missing contact or stale membership
valid.

## Plaintext and durability boundary

The local IPC request necessarily contains plaintext. It is accepted only over
the signed loopback descriptor and short-lived bearer model from RFC-0154. The
message is nonempty valid UTF-8 and limited to 65536 bytes. The adapter must
seal and sign the durable queue record before returning success and must never
persist plaintext as an IPC evidence artifact or log field.

Success binds the exact queue, contact, conversation and peer identities. It
does not claim materialization, network transmission, mailbox storage or
delivery. Those later states belong to the outbox projection and existing
canonical event/mailbox contracts.

## Bounds and failure behavior

Queue requests are limited to 128 KiB, signed responses to 64 KiB, labels to
4096 bytes and plaintext to 65536 bytes. The request lifetime is at most 30
seconds and contained by the signed descriptor lifetime. Queue requests and
responses use distinct digest, signature and object-ID domains.

Unknown variants, noncanonical CBOR, trailing bytes, invalid UTF-8, zero IDs,
same-account peers, expired requests, bearer or descriptor substitution,
command/result mismatch, derived-ID mismatch and response substitution fail
closed. Golden vectors are independently reproduced in JavaScript and both
request and response enter the bounded decoder mutation corpus.

## Activation boundary

M1.1.32 remains inactive. Activation requires an explicit IPC version
migration, dual-reader and downgrade tests, exact-content conflict tests against
the durable store, crash/restart idempotence evidence, bounded paginated outbox
inspection, sustained fuzzing and fresh desktop/CLI field evidence.
No network was executed to define or verify this contract.
