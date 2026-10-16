# RFC-0178: M1.1.52 canonical sync historical-signature bridge

Status: accepted locally as an inactive activation prerequisite.

## Problem found before activation

The active `AuthorizedEvent`, Device certificate, Account authority snapshot
and ratchet artifacts are signed over Postcard-specific preimages. The native
canonical event candidate uses different deterministic-CBOR preimages and
domains. A generic migration process does not possess every historical
author's private Device and Root keys, so it cannot honestly translate and
re-sign existing history.

Consequently, the allocated `kilogram/m1/sync/cbor/1` frames can represent new
native canonical events but cannot carry the installed base of active
Postcard events. Activating that generation as a replacement would strand
history. This is now an explicit activation blocker rather than an implicit
assumption.

## Exact-byte authenticated bridge

`sync-legacy-event-bridge-v1` is a deterministic-CBOR envelope around the
exact active Postcard `AuthorizedEvent` bytes. It redundantly names the
conversation, active Event ID, author Account and author Device. Decoding is
successful only after all of the following checks:

1. the outer CBOR is canonical, bounded, exact-version and has no trailing
   data;
2. the nested bytes pass the active Postcard decoder and the original event,
   certificate and authority-signature verification;
3. re-encoding the verified active object yields exactly the nested bytes;
4. every redundant identifier equals the value derived from that object.

Conversation-membership authorization is still checked by the receiving
store, exactly as it is after active synchronization today. The envelope does
not mint a new signature, weaken an old signature or require an old private
key. It preserves the evidence that already exists.

## Protocol-generation boundary

The existing `kilogram/m1/sync/cbor/1` schema has a closed event type and a
closed variant registry. This RFC does not silently alter it and does not put
bridge bytes under that identifier. A later stage must allocate an explicit
history-compatible canonical sync generation, integrate this bridge into its
closed frames and authenticate that exact generation in the connection-ticket
capability.

Until then, the runtime remains `legacy-only`: `kilogram/m0/sync/8` is the
only registered and dialed synchronization ALPN, canonical dial remains
disabled, and format sniffing or parse-error fallback remains forbidden.

## Verification and scope

The Rust implementation verifies the nested active signatures and exact
metadata binding. A JavaScript reference independently reproduces the outer
CBOR vector and its size bounds. Tests reject changed conversation, event,
Account or Device IDs, changed nested bytes, non-minimal CBOR and trailing
data.

This milestone changes no active wire bytes, network behavior, stored event,
Event ID, release artifact or user-visible behavior. It prevents an unsafe
activation and supplies the lossless primitive required for the next explicit
sync generation.
