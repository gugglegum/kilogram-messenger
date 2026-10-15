# RFC-0151: M1.1.25 canonical history rewrap

Status: accepted candidate specification; not active on the network.

## Outcome

M1.1.25 defines the complete deterministic-CBOR candidate for the separate
`history-rewrap` compatibility surface. It is the cryptographic payload that a
trusted existing Device creates for a newly authorized Device. The already
canonical history-recovery ceremony decides consent, sources, ranges and
resume checkpoints; this RFC specifies the recipient-only encrypted events
that those pages may carry.

This RFC changes no active wire bytes. The runtime continues to use the
Postcard v1 request, bundle and source-signed transfer in
`crates/kilogram-protocol/src/rewrap.rs`. The candidate has no protocol
identifier and cannot be selected by an active peer.

## Authority and request

The manifest embeds one exact current Root-signed Account device list and one
Root-signed conversation membership. Source and recipient must be distinct,
currently authorized Devices in that Account, and that Account must be a
member of the conversation. The roster supplies the recipient HPKE key and
prevents an untrusted caller from substituting an arbitrary encryption key.

The recipient signs an exact conversation, source, recipient, 32-byte session
binding, bounded range and SAS. The SAS is derived from the canonical roster
plus the ordered source and recipient roles. The source cannot move an
otherwise valid request to another session, Device pair or roster.

## Inventory, entries and transfer

The manifest commits the complete, strictly ordered Event ID inventory and one
contiguous range of at most 256 entries. Every entry embeds one fully verified
canonical authorized `RatchetText` event, its inventory index, an HPKE
encapsulated key and ciphertext. A source Device signature covers all of those
fields and the manifest ID.

HPKE uses the project's RFC 9180 base-mode suite: DHKEM(X25519,
HKDF-SHA256), HKDF-SHA256 and ChaCha20Poly1305. The exact AAD is the domain
`kilogram:history-rewrap-entry-aad:cbor-v1\0` followed by canonical CBOR for
the manifest ID, inventory index and Event ID. Consequently a ciphertext
cannot be transplanted between manifests, positions or events. Only the
recipient private key corresponding to the roster certificate can open it.

The final source-signed transfer embeds the exact recipient request and the
complete range bundle. Contextual verification also receives the externally
expected request and session binding; merely presenting a self-consistent
transfer is not sufficient to satisfy a pending request.

## Evidence and bounds

The candidate includes RFC 8610 CDDL, a machine-readable schema, strict Rust
decoders, a deterministic golden vector and an independent Node reference for
CBOR, Ed25519 and BLAKE3. The vector contains a real HPKE ciphertext produced
and opened by `kilogram-crypto`; independent cross-language HPKE seal/open
conformance remains an activation requirement rather than being overstated by
the Node verifier.

The inventory is bounded at 4096 events, a page at 256 entries, each plaintext
at 64 KiB, a bundle at 8 MiB and a signed transfer at 9 MiB. Decoder mutation
tests cover the request, signed entry, bundle and contextual transfer.

## Activation boundary

Complete canonical coverage is not activation. Deployment still requires a
new explicit version or parallel ALPN, a dual reader, downgrade tests,
sustained coverage-guided fuzzing, independent HPKE conformance and fresh
multi-device field evidence. Until then, active Postcard v1 remains
authoritative.
