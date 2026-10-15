# RFC-0155: Canonical local runtime IPC read model

Status: implemented as an inactive M1.1.29 candidate.

## 1. Scope

This stage extends the inactive deterministic-CBOR local runtime IPC candidate
with the read-only state needed by a desktop client: a paginated conversation
list and snapshot-bound history pages. It reuses the signed loopback descriptor
from RFC-0154, but gives read requests and signed read responses separate
domain separators so their transcripts cannot be substituted for core control
messages.

The running product still uses exact Postcard IPC version 26 and JSON
descriptor version 7. This RFC does not activate canonical CBOR, add a
listener, change an executable, or exercise the network.

## 2. Bounded paginated conversation list

The candidate replaces an unbounded all-at-once conversation-list response
with pages of at most 64 summaries. The continuation key is the exact UTF-8
conversation label plus peer Account ID, matching the deterministic order
already used by the runtime. A returned continuation key must equal the last
summary in a full page, and the first summary of the next page must compare
strictly after the requested key.

Each summary binds its label-derived Conversation ID, peer Account and primary
Device, route policy, exact endpoint health counts, at most four deterministically
ordered endpoint candidates, message count, and optional latest-message
preview. Exactly one endpoint is primary. Usable candidates require an
authority revision and publication channel; quarantined candidates require a
complete signed-conflict projection. UTF-8 text, paths, diagnostics, and
previews are explicitly bounded.

## 3. Snapshot-bound history page

A history request identifies a signed contact by its exact UTF-8 conversation
label and carries an optional cursor plus a limit of at most 100 messages. The
response always exposes the nonzero immutable snapshot ID, even for the final
page. This closes the ambiguity in the current projection where a terminal
page has no cursor and therefore does not expose which snapshot it came from.

The label-derived Conversation ID, request cursor, response snapshot, returned
messages, and next cursor are checked together. For a continued page, the next
cursor's `before_index` plus the returned message count must equal the requested
page end. A terminal page must consume the complete remaining prefix. Event IDs
within a page are distinct, plaintext bodies are valid nonempty UTF-8, and the
aggregate body budget is at most 192 KiB.

## 4. Exact response authentication

Every read request is short-lived, repeats the signed descriptor's bearer
token, and commits to the exact descriptor ID. Every read response is signed
by the Device named by that descriptor and repeats the descriptor ID, request
ID, and a domain-separated digest of the complete request. Command/result
pairing, pagination arithmetic, ordering, counts, and nested bounds are all
verified before a response is accepted.

Independent JavaScript constructs both request/response pairs, Ed25519
signatures, BLAKE3 identifiers, nested options, summaries, endpoint states,
messages, and cursors without invoking Rust.

## 5. Activation and remaining surface

The read response is capped at 256 KiB. The new paginated list is deliberately
not wire-compatible with the active unpaged `ConversationList`; activation
therefore needs an explicit GUI/runtime migration rather than silent decoding
of both meanings under one version.

Runtime IPC coverage remains partial. Contact mutation and own-device
directory operations, message queue and outbox status, mailbox provisioning,
volunteer storage, ticket publication and conflicts, automation, recovery,
and their result projections remain outside this stage. Activation still
requires completion of those families, dual-reader downgrade tests, sustained
coverage-guided fuzzing, and fresh desktop and CLI field evidence.
