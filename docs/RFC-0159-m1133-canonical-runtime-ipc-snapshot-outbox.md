# RFC-0159: canonical runtime IPC snapshot-bound outbox projection

Status: implemented as an inactive M1.1.33 candidate.

## Summary

M1.1.33 adds the read-only `outbox-status-page` operation to the
deterministic-CBOR runtime IPC candidate. It exposes bounded delivery state
without allowing a changing durable queue to create silent gaps, duplicates or
counter drift between pages. A content-derived snapshot ID binds the complete
ordered projection, while every continuation carries that same ID and an exact
start index.

This contract does not activate canonical CBOR. The shipping runtime remains
Postcard IPC version 26 with JSON descriptor version 7. No listener, network
protocol, ALPN, endpoint publication, desktop behavior or accepted M1.0.0 tag
changes. No network was executed to define or verify this contract.

## Complete immutable snapshot

The runtime first projects every durable outbox record into a bounded item and
sorts the complete set by strictly increasing Queue ID. It then computes the
snapshot ID as a domain-separated BLAKE3 digest of canonical CBOR containing:

- the profile version;
- all global counters; and
- every projected item, including its state and retry count.

The ID therefore changes when an item, ordering relation, state, retry count or
global counter changes. It is not a timestamp, database generation, random
nonce or hash of only the returned page. At most 4096 queue records and 4096
retry records are committed, with at most 256 contacts.

## State and counter semantics

Each item binds its Queue ID, canonical contact ID, peer Account,
Conversation ID, one of seven delivery states, optional acknowledgement Event
ID and retry-record count. Every non-delivered item contributes to `pending`;
every non-queued item contributes to `materialized`; only `delivered` carries
one nonzero acknowledgement Event ID. Mailbox pending, stored, expired and
failed counters match their exact states, and the sum of item retry counts
equals the global retry counter.

These counters describe the complete snapshot, not merely the current page.
Inconsistent totals, duplicate or unsorted Queue IDs, an acknowledgement on a
non-delivered item, or a delivered item without an acknowledgement fail
closed.

## Exact pagination

An initial request has no cursor and starts at index zero. A continuation
contains the exact snapshot ID and next start index returned by the preceding
page. Page size is between 1 and 128 items. The response repeats the immutable
snapshot ID and global counters, preserves strict Queue ID order and returns no
more than the requested limit.

Continuation arithmetic is exact: `next start = requested start + returned
items`. A non-final page must be nonempty and carry that exact cursor. A final
page reaches the global queue count and carries no cursor. Snapshot
substitution, skipped or repeated indexes, premature empty pages and cursors
past the committed count fail closed.

## State drift and retention

The portable response proves what the descriptor Device signed; it cannot by
itself force a mutable database to retain an old view. An eventual active
adapter must pin or persist the exact immutable snapshot for the bounded cursor
lifetime. If queue state changes, the snapshot is evicted, or the runtime
restarts without that retained view, continuation must fail explicitly as a
stale snapshot. It must never rebuild changed contents under an existing
cursor, even if the requested numeric index still exists.

The contract deliberately does not require indefinite snapshot storage. A
bounded implementation may expire retained snapshots and ask the caller to
restart at page one, provided this is explicit and cannot be confused with a
valid continuation.

## Authentication and bounds

Requests retain the signed loopback descriptor and short-lived bearer model
from RFC-0154. Each response is signed by the exact descriptor Device and binds
the digest of the complete request, including cursor and page limit. Requests
are limited to 64 KiB and signed responses to 128 KiB; request lifetime remains
at most 30 seconds.

Request digest, response signature, response ID and snapshot ID have separate
domains. Unknown variants, noncanonical CBOR, trailing bytes, zero identities,
expired requests, bearer or descriptor substitution, command/result mismatch,
state/count inconsistency and cursor substitution fail closed. Golden vectors
are independently reproduced in JavaScript and request and response objects
enter the bounded decoder mutation corpus.

## Activation boundary

M1.1.33 remains inactive. Activation requires an explicit IPC version
migration, dual-reader and downgrade tests, a transactionally stable active
projection adapter, stale-snapshot behavior across mutation and restart,
sustained fuzzing and fresh desktop/CLI field evidence. This read-only contract
does not change message delivery, mailbox replication or the active durable
queue.
