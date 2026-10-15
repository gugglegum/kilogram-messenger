# RFC-0144: M1.1.18 canonical mailbox provisioning

Status: accepted locally as an inactive migration target, 2026-10-15.

## Result

This stage records complete `mailbox-provisioning` coverage under the canonical
coverage definition. It specifies deterministic CBOR for the complete recipient-bound
mailbox lifecycle while changing no active Postcard wire bytes.

The target includes:

- a mode-tagged provisioning core and binding ID;
- the Device-signed owner-local binding, carried only as encrypted local state;
- the Device-signed peer offer, HPKE-sealed to one exact recipient Device;
- purpose-specific HPKE info and exact canonical header AAD;
- a bounded, ordered and distinct volunteer replica set plus commitment ID;
- signed activate, rotate and revoke generations linked by exact update IDs;
- the recipient ACK bound to one authenticated session and one exact update.

## Legacy HTTPS and exact-volunteer modes

The source has Postcard v1 legacy HTTPS bindings and Postcard v2 service-free
exact-volunteer bindings. The candidate does not reproduce that implicit split.
Both translate into one closed family with an explicit mode:

- `legacy-https` requires exactly one normalized HTTPS service descriptor;
- `exact-volunteer` forbids a service descriptor and requires a replica set on
  every activation.

The legacy mode exists for explicit migration, not as the desired architecture.
It must not silently become the default or be inferred from missing data.

## Cryptographic binding

The binding ID hashes the exact canonical provisioning core. Therefore mode,
identities, scope, mailbox address, optional legacy service and creation time
cannot be substituted without changing the ID.

Local and peer plaintexts have different signature domains. Their opaque HPKE
envelopes have different purpose tags and HPKE info strings. The canonical
header itself is the AAD. A local ciphertext therefore cannot be replayed as a
peer offer, and an offer cannot be retargeted to another binding ID.

The codec verifies that carried read/write Ed25519 seeds derive the public keys
inside the canonical mailbox address. A peer receives only the write seed.

## Ordered lifecycle and recipient ACK

Generation one is an activation without a predecessor. Later generations name
the exact previous update ID. Rotation must change the binding ID; revocation
must retain the active predecessor's binding ID. Creation time cannot regress.

An ACK is signed by the exact recipient Device and commits to session binding,
update ID, both Account and Device pairs, generation, binding ID and revocation
state. It is not a generic delivery receipt.

## Bounds and fail-closed behavior

The canonical decoder enforces the existing operational bounds: URL at most
2048 bytes, sealed artifact at most 16 KiB, update at most 32 KiB, ACK at most
4 KiB, two through eight ordered store keys, and offer validity from 60 seconds
through 30 days. Unknown modes, purposes and action tags fail closed.

Golden vectors are regenerated independently in Node.js using Node crypto for
Ed25519 and the independent repository BLAKE3 implementation. Four signed
fixtures extend the deterministic mutation corpus to 8350/9000 cases.

## Activation boundary

This RFC does not activate canonical CBOR. Active code still reads and writes
Postcard v1/v2 with its existing domains. Activation requires an explicit
protocol identifier, translation tests for both source versions,
cross-language HPKE seal/open conformance, rollback/downgrade migration tests,
sustained fuzzing and security review.
