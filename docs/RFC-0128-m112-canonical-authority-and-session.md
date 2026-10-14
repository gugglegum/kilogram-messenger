# RFC-0128: Canonical authority and session foundation (M1.1.2)

Status: accepted as an inactive cross-language conformance baseline.

## 1. Purpose

M1.1.1 fixed the deterministic-CBOR profile and the first event/sync vectors,
but a peer still could not implement Kilogram account authority without copying
Rust/Postcard layout. M1.1.2 defines the minimum complete authority chain needed
to authenticate one session:

1. Account Root and Device identifiers;
2. a Root-signed Device certificate;
3. Root-signed Device revocations and an authority snapshot; and
4. a Device-signed authorization bound to one transport session.

This is a candidate format only. The active messenger continues to encode these
objects with the exact Postcard versions recorded in
`PROTOCOL-COMPATIBILITY.json`. No existing ALPN, artifact version, signature
domain or decoder changes meaning in this milestone.

## 2. Normative schema sources

`CANONICAL-AUTHORITY.cddl` is the human- and machine-readable CDDL grammar under
RFC 8610. `CANONICAL-AUTHORITY-SCHEMA.json` gives the same contract as an exact
positional registry used by the repository's fail-closed tooling. Both inherit
`kilogram-deterministic-cbor-v1` from RFC-0127 and therefore permit only
preferred unsigned integers, definite byte strings and definite arrays.

The CDDL grammar describes shape and cardinality. The JSON registry additionally
records semantic ordering and linkage rules that plain CDDL does not express:

- capabilities are non-empty and strictly increasing;
- revocations are strictly increasing by unsigned bytewise Device ID;
- every nested authority object names the same Account ID;
- every revocation sequence and the certificate sequence precede the snapshot
  revision; and
- the session certificate is not revoked by the embedded snapshot.

## 3. Cryptographic transcripts

The candidate uses Ed25519 already present in the implemented authority model.
It introduces no cryptographic primitive. Each signature covers the exact CBOR
content array prefixed by one new, NUL-terminated domain:

- `kilogram:device-certificate-signature:cbor-v1\0`;
- `kilogram:device-revocation-signature:cbor-v1\0`;
- `kilogram:account-authority-snapshot-signature:cbor-v1\0`; and
- `kilogram:device-session-authorization-signature:cbor-v1\0`.

The first three signatures are made by the Account Root. The final signature is
made by the Device identified by the embedded certificate. Account ID and Device
ID are the corresponding 32-byte Ed25519 verifying keys.

The distinct domains prevent candidate bytes from being reinterpreted as any
active Postcard transcript. Session binding remains a 32-byte opaque value: its
derivation belongs to the future transport migration, while this schema proves
that an authorization cannot be replayed under a different binding.

## 4. Bounds and canonical sets

A certificate carries one or both known capabilities: `1` (`sign-events`) and
`2` (`sync-history`). An authority snapshot contains at most 32 revocations,
matching the current account-device bound. Exact ordering eliminates alternate
encodings of logical sets and makes the signed transcript stable across
languages.

Unknown capability codes, empty/duplicate/out-of-order capabilities,
over-limit or out-of-order revocations, inconsistent accounts, future
certificate/revocation sequences and revoked session devices fail closed.

## 5. Implementation and verification

The implementation part of this milestone supplies:

- an owned Rust encoder/decoder for every signed object and content transcript;
- deterministic Root/Device golden material and exact nested envelope bytes;
- positive signature/linkage tests and malformed/tampered negative tests;
- an independent JavaScript reconstruction using only Node's maintained
  cryptographic implementation for Ed25519 verification; and
- a PowerShell boundary that checks the CDDL, JSON registry, Rust bindings,
  vectors and independent verifier as a closed set.

The public seeds in the vectors are test material and must never be reused as
production keys. The independent verifier does not implement Ed25519 itself.

## 6. Activation and residual work

M1.1.2 does not activate deterministic CBOR. Activation still requires:

1. schemas for the remaining selected event/ratchet/recovery families;
2. an explicit new protocol version or parallel identifier;
3. bounded dual-reader migration and downgrade/cross-version tests;
4. decoder fuzzing plus a sustained malformed-input corpus;
5. fresh two-host and hosted evidence; and
6. independent security review before any production claim.

The milestone therefore establishes a portable authority/session foundation,
not a production wire migration, global authority freshness proof, recovery
redesign or audit result.
