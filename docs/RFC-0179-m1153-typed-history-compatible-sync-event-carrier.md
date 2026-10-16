# RFC-0179: M1.1.53 typed history-compatible sync event carrier

Status: accepted locally as an inactive protocol prerequisite.

## Why an Event ID needs a namespace

The active Postcard event and the native deterministic-CBOR event use
different signing and Event-ID domains. Once one synchronization generation
can carry both formats, a bare 32-byte Event ID is ambiguous: equal bytes do
not mean equal events when they were derived in different domains.

The v2 event identity is therefore the ordered pair `(namespace, event-id)`.
Namespace `1` is the active Postcard-v5 Event-ID domain carried through the
verified legacy bridge. Namespace `2` is the native canonical-CBOR-v1 Event-ID
domain. Sorting, duplicate detection, inventory comparison and future session
evidence must use the complete pair, never the raw ID alone.

## Closed carrier

The v2 event carrier is a closed deterministic-CBOR variant:

1. `legacy-postcard-v6` contains a `sync-legacy-event-bridge-v1`; decoding
   re-verifies the original Postcard event, certificate and authority
   signatures and all redundant bridge bindings.
2. `canonical-cbor-v1` contains an `authorized-event-v1`; decoding verifies
   its canonical event, certificate and authority signatures.

The variant selects exactly one decoder. Unknown variants, a payload encoded
for the other variant, trailing bytes, non-canonical CBOR and objects larger
than eight MiB fail closed. There is no format sniffing or parse-error
fallback.

## Separate protocol generation

The exact successor identifier is `kilogram/m1/sync/cbor/2`. It is allocated
in the transport registry but is not registered or dialed by any endpoint.
The already allocated `/cbor/1` generation is unchanged and remains blocked
for installed history.

Allocation plus a carrier is not a usable synchronization protocol. Before
activation, v2 still needs closed inventory, diff, batch, completion and
session-checkpoint frames using typed keys, plus an authenticated connection
ticket capability for this exact identifier, dual-reader implementation and
downgrade tests.

## Scope

This milestone does not change active wire bytes, connection-ticket bytes,
stored events, runtime behavior, release artifacts or user-visible behavior.
It reserves a collision-free history model and prevents later code from
silently treating two Event-ID domains as one namespace.
