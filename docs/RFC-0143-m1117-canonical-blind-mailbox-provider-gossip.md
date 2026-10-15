# RFC-0143: Canonical blind-mailbox provider gossip (M1.1.17)

## Status

Accepted locally as an inactive migration target. It does not change active
Postcard bytes, protocol negotiation, provider selection, or field behavior.

## Decision

M1.1.17 marks complete `blind-mailbox-provider-gossip` in the canonical coverage
inventory. Deterministic CBOR now defines the exact portable artifacts and
cryptographic preimages for:

- a provider offer ID over the exact canonical signed storage offer;
- the common selection transcript containing a fresh salt, store key,
  authenticated transport identity and offer ID;
- separate normal-selection and gossip-selection ranks;
- bounded gossip entries and short-lived request/reply frames;
- a frame ID and exact `reply_to` binding.

The canonical domains use `cbor-v1`; they intentionally do not reuse the active
Postcard domain strings. Activation therefore requires an explicit new version
and dual-reader transition.

## Bounds and validation

A frame contains at most eight offers, is valid for at most 60 seconds and is
at most 20,480 bytes. Each relayed offer is at most 2,048 bytes, crosses at most
two authenticated peer edges, is currently signature-valid and may be at most
900 seconds old. Store keys within a frame are unique.

The deterministic selector orders by the domain-separated BLAKE3 rank and then
by store key, while accepting at most one offer for each authenticated transport
identity. Gossip selection additionally filters out low-work, stale, oversized
or hop-exhausted offers. The default admission threshold remains eight leading
zero bits.

The gossip frame remains intentionally unsigned, matching the existing design:
it travels inside an authenticated peer session. A caller that persists or
correlates a frame uses the exact frame ID; the strict ID-aware decoder rejects
any byte mutation against that expected ID.

## Privacy boundary

Authenticated observer tags, positive path-colocation evidence, raw IP
addresses and relay origins remain installation-local. They affect local policy
but never enter the canonical offer, selection transcript or gossip frame.
This is important: canonicalization must not turn local anti-correlation hints
into globally linkable network metadata.

## Evidence

The schema, CDDL, golden vectors and Rust codec are checked by a bounded
PowerShell gate. A separate Node.js implementation reconstructs every CBOR
byte, verifies the nested Ed25519 storage-offer signature and independently
computes BLAKE3 offer, selection and frame IDs. Rust tests cover exact vectors,
selection determinism, transport deduplication, validity, duplicate store keys
and reply binding.

## Compatibility and non-goals

The active provider registry continues to use Postcard and the existing v1
domains. No active runtime imports this module. This RFC does not activate a
new gossip version, prove Sybil resistance, publish local path-domain evidence,
or change volunteer traffic policy. Those remain separate concerns.
