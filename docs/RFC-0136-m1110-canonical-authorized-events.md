# RFC-0136: Canonical authorized events (M1.1.10)

Status: candidate, not active.

## Outcome

M1.1.10 defines the complete deterministic-CBOR representation of every event payload used by the current runtime:
acknowledgements and ratchet-encrypted text.
It also defines the signed event envelope, its BLAKE3 Event ID, and the evidence
needed to authorize an event author. The active network continues to use
Postcard v5; no protocol identifier was assigned and no wire bytes changed.

## Canonical object graph

`CANONICAL-EVENT.cddl` and `CANONICAL-EVENT-SCHEMA.json` define:

1. acknowledgement payload `[1, acknowledged-event-id]`;
2. ratchet text payload containing an exact Root-signed recipient device list,
   an exact signed sender ratchet identity, and one bounded ciphertext per
   recipient;
3. event content with conversation, author, per-author sequence and parents;
4. the Ed25519-signed event envelope;
5. the BLAKE3 Event ID over the domain and exact signed-event bytes; and
6. an authorized-event envelope carrying the author's device certificate and
   matching authority snapshot.

Parent Event IDs and recipient Device IDs are strictly increasing, so duplicate
or permuted sets have no second valid encoding. The recipient set must exactly
match the embedded device list, the sender identity must name the author, and a
ciphertext is limited to 128 KiB. Nested canonical objects are limited to
512 KiB and are decoded with their own strict decoders.

## Authorization and membership

The event signature is checked against the author Device ID. The certificate
must name that device, belong to the snapshot Account, be within the snapshot
sequence, include `SignEvents`, and not be revoked. Conversation membership is deliberately external
to the authorized-event bytes: the verifier accepts an
independently Root-signed membership and requires the same conversation plus
the author's Account. This keeps membership updates out of every message while
still making the complete authorization decision explicit and testable.

## Independent evidence

`CANONICAL-EVENT-VECTORS.json` fixes exact bytes for acknowledgement, ratchet
text, signed and authorized envelopes, signing preimages, signatures and Event
IDs. `scripts/verify-kilogram-canonical-event-reference.mjs` regenerates and
verifies them without importing Rust code. It contains an independent strict
CBOR implementation, Node Ed25519 verification and the repository's independent
BLAKE3 reference.

Both new decoders join the deterministic mutation corpus. The corpus grows from 2,088 to 2,610 malformed
cases and its aggregate ceiling rises from 2,500 to
3,000; per-fixture position grids and the 512 KiB fixture bound are unchanged.

## Activation boundary

This RFC does not activate canonical CBOR. Activation still requires complete
target-surface coverage, an explicit new protocol identifier or parallel ALPN,
dual-reader and downgrade tests, sustained coverage-guided fuzzing, and fresh
field evidence. Until then Postcard v5 remains the only active event format.
