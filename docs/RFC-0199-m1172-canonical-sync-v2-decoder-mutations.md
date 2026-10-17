# RFC-0199: M1.1.72 bounded canonical sync v2 decoder mutations

Status: accepted locally; deterministic regression corpus verified, transport inactive.

## Purpose

The historical M1.1.5 decoder corpus remains an immutable 31,851-mutation
record for canonical v1 surfaces. It predates the history-compatible canonical
sync v2 family, so treating it as evidence for `/cbor/2` would be incorrect.
M1.1.72 adds a separate, reproducible corpus for every public sync v2 decoder
without rewriting the historical result.

## Closed fixture inventory

`CANONICAL-SYNC-V2-DECODER-MUTATIONS.json` names 20 fixtures from the existing
history, frame and initiator-session vectors plus a new deterministic responder
checkpoint vector. The inventory covers both typed event-key namespaces, both
verified carrier variants, inventory, event batch, diff, all control objects,
all client and server frame variants, and both initiator and responder signed
session checkpoints.

`CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json` closes the previous responder
checkpoint vector gap. A fixed Ed25519 seed derives the exact Device ID,
content, signing preimage, signature, signed CBOR and BLAKE3 checkpoint ID. The
Rust test reconstructs these values rather than merely decoding an opaque
fixture.

## Mutation policy

Every fixture receives a bounded strict-prefix truncation grid, trailing data,
a non-minimal top-level version, an indefinite top-level array and a reserved
CBOR head. Every decoder also receives an oversized declared byte string.

The bounded low-bit-flip grid applies only to authenticated envelopes and
carriers. An unsigned event key, conversation identifier, frontier or control
reason can legitimately become a different valid semantic value after a bit
flip; requiring its parser to reject that value would be a false security
claim. Those objects therefore use the structural-only policy. Signed objects
use authenticated-bit-flips because any changed authenticated byte must fail
verification.

The exact corpus is 2,863 mutations across 20 fixtures, capped at 128 positions
per grid, 8 MiB per fixture and 4,000 total mutations. It uses no randomness
and performs no network access.

## What this does not prove

This is a deterministic bounded regression corpus. It has no compiler
instrumentation and is not coverage-guided fuzzing. It does not claim path or
edge coverage, does not run indefinitely, and does not close the sustained
coverage-guided fuzzing gate. That gate and fresh hosted/physical-device
evidence remain pending.

## Deliberate non-activation

The stage changes test and evidence surfaces only. It does not register or dial
`kilogram/m1/sync/cbor/2`, authorize canonical transport, change active Postcard
wire behavior, perform a network test, produce a release build or archive, or
replace a stable executable.
