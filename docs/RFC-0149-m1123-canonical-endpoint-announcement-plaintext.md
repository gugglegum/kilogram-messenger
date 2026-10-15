# RFC-0149: M1.1.23 canonical endpoint-announcement plaintext

Status: accepted locally as an inactive migration candidate.

## Outcome

M1.1.23 defines deterministic-CBOR representations for the signed plaintext
bundle used to announce contact endpoints and for the signed evidence that a
Device accepted an observation made by another Device. This advances the
`endpoint-announcement` surface from not-started to partial. It does not
activate canonical CBOR and changes no active wire bytes.

The active Postcard v4 bundle, HPKE v1 envelope and acknowledgement v2 remain
the runtime contract. A future activation still requires a recipient-only HPKE
envelope, a session-bound recipient acknowledgement, an explicit protocol
identifier, dual-reader downgrade tests, sustained fuzzing and fresh field
evidence.

## Trust and authorization

Every bundle embeds an exact current Root-signed device list. Verification
checks that source and recipient are distinct, active members of that list;
the bundle cannot authorize its own signing Device. The list is evidence of
the account roster, not an authority update.

An accepted observation retains the source observation and its signature, then
adds an accepting-Device signature over the exact local account, accepting
Device, source Device, source bundle ID and import time. This provides a
portable chain of who observed and who accepted without treating either party
as Root.

## Contact and publication state

Contacts are strictly ordered by peer Account ID and derived conversation ID.
Each contact contains one to four Device-ID-ordered endpoint candidates and
exactly one primary. A candidate carries a closed route policy, write key,
bounded ticket and one closed state:

1. no publication evidence;
2. a direct signed publication observation;
3. a signed accepted observation;
4. a signed canonical publication-conflict proof;
5. a Root-signed canonical conflict resolution.

Nested publication, conflict and resolution objects are verified according to
their own canonical schemas. Direct and accepted observations must match the
candidate channel and peer. A conflict detector must belong to the embedded
roster. A resolution must match the exact authority revision, peer, replacement
write key, route policy and ticket digest.

## Bounds and identifiers

The bundle is limited to 16 MiB, 256 contacts, four endpoints per contact,
4 KiB conversation labels and 8 MiB tickets. Its validity interval is positive
and at most one hour; verification permits at most five minutes of future clock
skew. Bundle and accepted-observation IDs are BLAKE3 domain-prefixed hashes of
the exact signed canonical object. Signatures use distinct Ed25519 domains.

## Evidence and non-activation

The checked artifacts include CDDL, machine-readable schema, deterministic
vectors, a strict Rust decoder, an independent JavaScript CBOR/Ed25519/BLAKE3
reference and bounded decoder mutations. They run offline and open no sockets.

This is intentionally only partial `endpoint-announcement` coverage. The
recipient-only HPKE envelope and session-bound recipient acknowledgement are
not represented by this RFC. Therefore the candidate does not activate, does
not replace the active Postcard/HPKE runtime and needs no laptop or external
field run at this stage.
