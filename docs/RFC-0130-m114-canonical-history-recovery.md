# RFC-0130: Canonical history recovery checkpoints (M1.1.4)

Status: accepted as an inactive cross-language conformance baseline.

## 1. Purpose

M1.1.4 defines a deterministic-CBOR checkpoint chain for bounded history
recovery. It makes consent scope, the source and recipient Devices, the
inventory claim and resumable progress portable across implementations without
changing the active Postcard recovery protocol.

## 2. Normative representation

`CANONICAL-RECOVERY.cddl` defines the RFC 8610 grammar and
`CANONICAL-RECOVERY-SCHEMA.json` fixes the positional objects, domains, limits
and semantic successor rules. Both inherit the deliberately narrow
`kilogram-deterministic-cbor-v1` profile from RFC-0127.

CBOR optionals are closed arrays: `[]` means absent and a fixed-length array
means present. Null, maps, indefinite arrays and alternate encodings are not
accepted. Page size is bounded to 256 events.

## 3. Authority and identity

The recipient Device signs each checkpoint under a dedicated NUL-terminated
domain. The signature therefore records consent to the exact Account,
conversation, source Device, recipient Device, SAS, approved range, page size,
inventory and progress position.

The recovery ID hashes only immutable session fields. The checkpoint ID hashes
the complete signed checkpoint. A successor must name the exact predecessor,
strictly advance progress, preserve every identity field and preserve the first
inventory claim once introduced. A checkpoint fork remains detectable rather
than being resolved by last-writer-wins.

## 4. Initial and resumed states

The initial checkpoint has no inventory claim, no predecessor and points to the
approved range start. A successor introduces the source inventory claim and a
predecessor ID, then advances by at least one event. The inventory event count
must cover the next position. Progress may never exceed the approved end.

`CANONICAL-RECOVERY-VECTORS.json` contains public test material for an initial
checkpoint and one successor, including exact CBOR, Ed25519 preimages,
signatures, checkpoint IDs and their shared recovery ID.

## 5. Activation boundary

This module is inactive: no existing ALPN, Postcard reader, runtime state or
database record changes. Activation requires an explicit new protocol version
or parallel identifier, dual-reader downgrade tests, malformed-input coverage,
fresh field evidence and hosted evidence. It does not claim availability,
honest inventory contents or recovery after both authorized peers disappear.
