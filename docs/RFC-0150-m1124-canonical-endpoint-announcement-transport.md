# RFC-0150: M1.1.24 canonical endpoint-announcement transport

Status: accepted candidate specification; not active on the network.

## Outcome

M1.1.24 completes the canonical `endpoint-announcement` surface started by
RFC-0149. The signed plaintext bundle is now wrapped in a recipient-only HPKE
envelope whose exact canonical header is authenticated as AAD. A recipient then
returns an Ed25519-signed, session-bound acknowledgement covering the exact
bundle and the result of applying it.

This RFC changes no active wire bytes. The running implementation remains
Postcard v4, HPKE envelope v1 and acknowledgement v2. The deterministic-CBOR
transport has no assigned protocol identifier and therefore cannot be selected
by an active peer.

## Envelope

The header commits to the Account ID, source Device ID, recipient Device ID and
canonical bundle ID. Its exact CBOR bytes are the HPKE associated data. The
domain-separated HPKE `info` value is
`kilogram:endpoint-announcement-envelope:cbor-v1\0`.

The ciphersuite remains the project's RFC 9180 base-mode suite:
DHKEM(X25519, HKDF-SHA256), HKDF-SHA256 and ChaCha20Poly1305. Encryption uses
the recipient encryption key certified by the exact current Root-signed roster
inside the plaintext bundle. After opening, the recipient revalidates the
complete signed bundle, its validity interval, header fields, bundle ID and the
private identity's correspondence to that certified key.

Changing the account, either Device ID, bundle ID, encapsulated key or
ciphertext therefore fails closed. Relays and mailbox providers see only the
bounded opaque envelope and cannot read the endpoint announcements.

## Acknowledgement

The recipient Device signs a canonical acknowledgement under
`kilogram:endpoint-announcement-acknowledgement:cbor-v1\0`. It covers:

- an external 32-byte session binding;
- bundle, source and recipient identities;
- the exact authority revision;
- derived totals for contacts, endpoints, observation evidence, conflicts and
  resolutions;
- bounded counts for newly applied contacts, endpoints, publication bindings,
  observations, conflicts and resolutions.

Totals are derived from the verified bundle rather than accepted from the
caller. Added counts can never exceed their corresponding totals. Replaying an
otherwise valid acknowledgement in another session is rejected.

## Evidence and bounds

The candidate includes RFC 8610 CDDL, a machine-readable schema, an independent
Node reference, a real HPKE golden envelope opened by the Rust implementation,
deterministic acknowledgement vectors and strict Rust round-trip tests. The
bounded mutation corpus includes both the encrypted envelope with contextual
HPKE opening and the acknowledgement with contextual session verification.

The maximum plaintext is 16 MiB, HPKE ciphertext is plaintext plus the 16-byte
AEAD tag, the whole envelope has 512 bytes of bounded overhead, and the signed
acknowledgement is at most 4 KiB.

## Activation boundary

Complete canonical coverage does not activate canonical CBOR. Activation still
requires an explicit protocol identifier or parallel ALPN, dual-reader and
downgrade tests, sustained coverage-guided fuzzing, cross-language HPKE
seal/open conformance, and fresh field evidence. Until those gates are met,
the active Postcard runtime remains authoritative.
