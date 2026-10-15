# RFC-0142: Canonical blind-mailbox transport (M1.1.16)

Status: candidate, not active.

## Decision

M1.1.16 completes the `blind-mailbox-transport` compatibility surface with a
deterministic-CBOR target for the full put/list/delete request and response
protocol. The new objects compose the capability address, encrypted envelope,
authorizations, and receipts already fixed by RFC-0141 instead of duplicating
their cryptographic rules.

Under the coverage definition, this is a complete `blind-mailbox-transport`
surface: every operation object, digest preimage, closed outcome, strict
decoder, golden vector, and independent reference is present.

The peer forwarding envelope carries a closed operation code and the exact
operation-specific request. A domain-separated BLAKE3 request digest is then
included in every peer response. Successful responses must match the operation,
request capability, expected store key, item, receipt, and page state. Rejected
responses use a closed four-value registry: rate limited, transfer budget
exhausted, invalid capability, or store unavailable.

## Page and resource rules

List pages contain no more than eight strictly ordered items. Every item embeds
an opaque canonical envelope and a signed stored receipt; item ID and expiry
must agree across both. `more_available` is true exactly when a non-empty page
has a `next_after_item_id`, and that cursor equals the last returned item.

The candidate retains the active implementation's 4 KiB accounting allowance
and bounded request/response sizes. Nested capability objects remain subject to
their tighter RFC-0141 limits.

## Evidence

- `CANONICAL-MAILBOX-TRANSPORT.cddl` and the JSON schema define every request,
  response, peer outcome, terminal status, and rejection variant.
- Rust strict decoders revalidate nested signatures and all semantic links.
- An independent Node implementation regenerates exact frames, Ed25519 delete
  authorization, and request digest bytes using the independent BLAKE3 code.
- Golden vectors cover all put/list/delete peer requests, all successful peer
  responses, all terminal response variants, and representative failures.
- The bounded mutation corpus exercises every peer request through its strict
  decoder (7045 deterministic cases in total).

## Non-activation boundary

The active mailbox remains Postcard encoded on the existing
`kilogram/m0/blind-mailbox/1` ALPN. No active mailbox request, response, ALPN,
or runtime dispatch path imports the candidate module. Activation requires a
new explicit version or parallel ALPN, downgrade tests, fuzzing, and fresh
field evidence.

Provider gossip, mailbox provisioning, and runtime mailbox envelopes remain
separate compatibility surfaces.
