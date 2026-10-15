# RFC-0146: M1.1.20 canonical ticket-publication capability

Status: accepted locally as an inactive migration target.

## Motivation

The endpoint-announcement surface embeds an opaque ticket-store write
capability. Canonicalizing the announcement first would leave a nested Postcard
artifact and a header-specific signature preimage inside an otherwise portable
object. M1.1.20 therefore closes the smaller dependency first.

## Contract

`CANONICAL-TICKET-PUBLICATION.cddl` defines a self-authenticating capability
and a signed write authorization. The capability contains an Ed25519 public
write key and the BLAKE3 channel ID derived from that exact key. The
authorization binds the exact canonical capability, positive generation,
bounded body length and raw BLAKE3 body digest. Verification against the body
recomputes both length and digest.

The private Device secret and peer-specific opaque scope are KDF inputs and are
never serialized. Epoch zero deliberately preserves the existing BLAKE3
derive-key context and therefore the legacy pseudonymous write key. Positive epoch
derivation uses the existing rotated context over `secret || scope || 0 ||
epoch.be64`. The serialized migration target uses a new CBOR-specific Ed25519
signature domain so old header authorization cannot be reinterpreted as the
new envelope.

This is complete `ticket-publication-capability` coverage: both derivation
branches, channel self-authentication, canonical bytes, signing preimage,
signature, length and body binding have strict Rust verification, golden
vectors and an independent Node reference. The reference implements BLAKE3
hash and derive-key directly and uses Node's Ed25519 implementation.

## Migration boundary

This RFC does not activate canonical CBOR. The active store remains Postcard
plus lowercase hexadecimal HTTP headers and the legacy raw signing domain.
Activation requires a new explicit authorization version, dual readers,
cross-version downgrade/replay tests, sustained fuzzing and fresh field
evidence. Endpoint announcements may depend on this schema in the next stage,
but cannot silently replace the active capability.

Two new decoder fixtures raise the deterministic mutation corpus from 9394 to
9800 rejected cases while retaining the 10000-case bound.
