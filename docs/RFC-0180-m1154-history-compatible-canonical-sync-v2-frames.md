# RFC-0180: M1.1.54 history-compatible canonical sync v2 frames

Status: accepted locally, inactive candidate.

## Outcome

Kilogram now has a complete closed frame codec for the allocated successor
identifier `kilogram/m1/sync/cbor/2`. Inventory, diff, event batch, complete,
pause, paused and rejected frames use the typed Event identity and closed Event
carrier introduced in M1.1.53.

This milestone does not register or dial the successor ALPN, advertise it in a
connection ticket and does not change active wire bytes. Active synchronization remains
Postcard v6 over `kilogram/m0/sync/8`.

## Typed history in every frame

A v2 inventory, diff or completion never contains a bare Event ID. Each entry
is the exact nested deterministic-CBOR encoding of the ordered pair
`(event namespace, 32-byte Event ID)`. A v2 event batch contains only the exact
closed carrier encoding for either:

1. a verified legacy Postcard-v6 bridge; or
2. a verified native canonical-CBOR-v1 authorized event.

The carrier decoder selects its one decoder from the explicit namespace. It
does not use format sniffing and does not fall back after a parse error.

Event arrays must be strictly ordered by their derived typed Event keys. Key
arrays are also strictly ordered and duplicate-free. Every nested event must
name the outer frame conversation.

## Authentication and bounds

Inventory and diff use new domain-separated Ed25519 preimages:

- `kilogram:sync-inventory-signature:cbor-v2\0`;
- `kilogram:sync-diff-signature:cbor-v2\0`.

They bind the Device identity, conversation and session binding. Closed client
and server variant registries reject unknown codes. Canonical booleans accept
only unsigned integer zero or one. Inventory size, batch size and every outer
or nested object remain explicitly bounded.

Independent JavaScript vectors cover the signatures and exact outer bytes.
Rust conformance tests cover both carrier variants plus signature, ordering,
conversation, boolean, unknown-variant and trailing-byte rejection.

## Why activation remains blocked

The existing durable canonical session checkpoint still stores untyped Event
IDs and identifies the older `/cbor/1` generation. Reusing it would erase the
namespace distinction on resume. Before `/cbor/2` can be advertised, Kilogram
still needs:

1. a typed durable canonical sync session checkpoint bound to the exact v2
   protocol identifier;
2. an authenticated connection-ticket capability for that identifier;
3. explicit dual-reader, downgrade and rollback tests;
4. sustained fuzzing and fresh hosted/field evidence.

Until those prerequisites are verified, `/cbor/2` remains allocated but not
registered or dialed.
