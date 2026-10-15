# RFC-0129: Canonical ratchet bootstrap artifacts (M1.1.3)

Status: accepted as an inactive cross-language conformance baseline.

## 1. Purpose

M1.1.3 defines deterministic-CBOR representations for the public artifacts
needed to start asynchronous pairwise sessions: the Device-authenticated
ratchet identity, one-time prekey bundles, a bounded signed prekey pool and an
Account-authority-complete prekey directory.

Secret Double Ratchet state, message keys, skipped-key storage and active
`vodozemac` serialization are deliberately outside this RFC. The active
messenger continues to use the versioned Postcard objects recorded by
`PROTOCOL-COMPATIBILITY.json`.

## 2. Normative sources

`CANONICAL-RATCHET.cddl` defines the RFC 8610 grammar.
`CANONICAL-RATCHET-SCHEMA.json` records exact positional fields, limits,
signature/digest domains, Rust bindings and semantic rules not expressible in
plain CDDL. Both inherit `kilogram-deterministic-cbor-v1` from RFC-0127.

Nested signed artifacts are encoded as byte strings containing their complete
canonical CBOR. Consequently an outer signature authenticates exactly one
nested byte representation, and a decoder must validate both the byte-string
boundary and the nested schema.

## 3. Authentication and identifiers

The Device Ed25519 key signs the ratchet identity, every prekey bundle and the
pool envelope under distinct NUL-terminated `cbor-v1` domains. The prekey-pool
ID is BLAKE3 over its own domain followed by the complete signed pool CBOR.

The ratchet Curve25519 and Ed25519 public keys remain opaque 32-byte values at
this layer. This RFC authenticates their binding to a Kilogram Device ID; it
does not redefine the key conversion or Olm session algorithm.

## 4. Pool invariants

A pool contains between one and 64 bundles. All bundles contain the exact same
signed ratchet identity as the pool, sequences are contiguous in array order,
one-time keys are unique, publication precedes expiry and validity is at most
30 days. Unknown or malformed nested objects fail closed.

## 5. Directory invariants

A directory contains one to 32 Device certificates and exactly one pool for
each certificate. Both arrays are strictly ordered by unsigned bytewise Device
ID. The embedded Root-signed authority snapshot, certificates and pools must:

- name one Account;
- put every certificate sequence before the authority revision;
- exclude every revoked Device; and
- have exact certificate-to-pool Device coverage.

The directory has no additional signing key: its authority is the verified
Root snapshot/certificates plus each Device-signed pool. Distribution freshness
and anti-rollback remain transport/discovery responsibilities.

## 6. Activation boundary

This milestone changes no ALPN, Postcard reader, runtime path or stored object.
Activation requires a new explicit protocol version or parallel identifier,
dual-reader downgrade tests, decoder fuzzing, fresh field evidence and hosted
evidence. M1.1.3 is a portable candidate foundation, not a production wire
migration or a claim about global prekey freshness.
