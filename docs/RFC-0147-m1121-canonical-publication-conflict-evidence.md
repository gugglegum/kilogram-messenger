# RFC-0147: M1.1.21 canonical publication-conflict evidence

Status: accepted locally as an inactive, partial migration target.

## Motivation

The active publication-conflict ceremony is a large Postcard/base64url surface:
publication and observation chains lead to a conflict proof, which then leads to
a Device request, Root resolution, response and portable claim. Migrating that
entire trust boundary in one opaque change would make independent review and
fail-closed testing harder. M1.1.21 closes the evidence half first.

## Evidence contract

`CANONICAL-PUBLICATION-CONFLICT.cddl` defines three signed deterministic-CBOR
objects:

- a publisher-Device signed ticket publication with a hash-linked generation,
  bounded lifetime, exact ticket digest and exact UTF-8 ticket bytes;
- a local-Device signed observation with its own hash-linked sequence and the
  publication high-water mark; and
- a detector-Device signed conflict proof embedding two exact signed
  observations.

Publication generation one has no predecessor. Every successor names the exact
BLAKE3 ID of the previous signed publication, preserves all identities and does
not move publication time backwards. The observation chain applies the same
rules to its own sequence and additionally prevents publication-generation
rollback.

A proof is accepted only when both observations name the same local Account,
channel, publisher Account, publisher Device and publication generation, but
their publication IDs or ticket digests differ. The two observations are
strictly ordered by their canonical observation IDs, making the proof and
evidence IDs independent of arrival order. The evidence ID uses explicit
length-delimited inputs.

Golden vectors cover two conflicting publications, observations from two local
Devices and one detector proof. Rust tests and an independent Node
Ed25519/BLAKE3 reference reproduce every canonical byte string, signature and
identifier. Three new strict-decoder fixtures raise the deterministic bounded
mutation corpus from 9800 to 10583 cases under a 12000-case ceiling.

## Honest partial coverage

This is partial `publication-conflict` coverage, not a claim that the complete
ceremony is canonical. The Device-signed resolution request, Root-signed
resolution, resolution response and portable verification claim remain on the
active Postcard/base64url contract and are the next stage.

M1.1.21 does not activate canonical CBOR. Active Postcard/base64url readers,
writers, URI prefix, signature domains and compatibility registry remain
unchanged. Activation still requires a new explicit protocol identifier,
complete surface coverage, dual-reader migration, downgrade/replay tests,
sustained fuzzing and fresh field evidence.
