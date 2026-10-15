# RFC-0163: Canonical runtime IPC publication-channel rotation

Status: implemented as an inactive M1.1.37 candidate.

## Summary

M1.1.37 defines a deterministic-CBOR local request for rotating one contact's
ticket-publication channel and a Device-signed response describing the exact
local transition. The caller supplies the exact expected current public state
and the next epoch. The trusted runtime derives the successor capability from
its local Device secret; the successor capability is never caller supplied.

This candidate does not activate canonical CBOR. The shipped runtime continues
to use Postcard IPC version 26 and JSON descriptor version 7.

## Exact current-state anchor

The request commits all public state that must still be current:

- canonical contact and peer Account identifiers;
- the digest of the runtime's durable peer scope;
- the current channel epoch and exact current public capability; and
- the latest accepted publication identifier and generation.

The next epoch must be exactly `current + 1`. The active adapter must compare
the complete expected state under one state lock. Any changed contact, peer,
scope digest, capability, epoch, or publication head fails closed before key
derivation or state mutation.

## Runtime-internal derivation and secret separation

The GUI supplies neither the successor write key nor key-derivation input. The
runtime loads the exact durable peer scope and Device secret, verifies that the
scope digest matches the request, re-derives and compares the exact current
capability, and applies the existing canonical ticket-publication KDF for the
requested next epoch. The derivation helper also validates the signed runtime
descriptor, bearer token and request lifetime before returning a transition.
It erases temporary secret material after use and persists only the state
required by the active adapter.

The response exposes the successor public write key, its self-authenticating
channel identifier, and transition metadata. It contains no Device secret,
Account root secret, plaintext peer scope, private write key, bearer token, or
other reusable local secret. A portable verifier can verify the Device
signature and public transition, but cannot prove the KDF was run from the
secret it is intentionally not given. The Device signature is the runtime's
attestation that internal derivation occurred.

## Atomicity and idempotency

The request ID is the durable idempotency key. A future adapter must perform
the exact-current-state comparison, successor derivation, durable state
replacement, and signed-response recording as one atomic transaction. An exact
retry may return `already-committed` with the byte-identical transition; reuse
of the request ID for different bytes must fail before mutation.

The rotation ID commits the complete request digest, successor public
capability, and next epoch. The signed response repeats the descriptor and
request identifiers, request digest, responder Device, previous public state,
and successor public state. Context substitution therefore fails closed.

## Deliberately separate network work

This slice performs no network publication and no peer delivery. A successful
local transition is explicitly `awaiting-first-publication` and
`pending-peer-delivery`, and it requires no runtime restart. Publishing the
successor, transporting an exact locator to the peer, accepting recipient
acknowledgement, and retry scheduling belong to a later network adapter slice.

Root-authorized conflict resolution is not included. If durable local state,
remote observations, or concurrent Devices disagree, the adapter must retain
the evidence and enter the already specified publication-conflict ceremony; it
must not choose a winner locally.

## Resource bounds

The request and response are each limited to 64 KiB. The private peer scope
used by the runtime is limited to 4 KiB before hashing or derivation. Nested
public capabilities are independently decoded and re-encoded canonically.
Unknown variants, noncanonical integers, trailing bytes, oversized objects,
expired requests, and contextual substitutions fail closed.

## Activation and compatibility

This RFC specifies candidate bytes and verification semantics only. It changes
no active runtime, desktop, CLI, storage, transport, publication, or conflict
behavior. Activation requires an adapter that proves runtime-internal
derivation, atomic compare-and-commit, exact retry replay, crash recovery, and
the subsequent network publication and recipient-delivery boundaries. The
normal IPC migration, dual-reader, downgrade, fuzzing, and field-evidence
requirements also remain.

No network was executed to verify this candidate. The golden rotation vector,
independent Node.js implementation, fail-closed boundary tests, Rust unit
tests, and bounded decoder mutation corpus cover the new local objects.
