# RFC-0141: Canonical blind-mailbox capability artifacts (M1.1.15)

Status: implemented as an inactive candidate.

## Outcome

M1.1.15 defines the deterministic-CBOR target for the complete `blind-mailbox-capability`
compatibility surface. The target covers:

- the public read/write capability address and derived opaque mailbox ID;
- the mailbox HPKE context, exact AAD, domain-separated info and opaque sealed
  envelope representation;
- write authorization bound to an item, requested TTL, exact envelope length
  and digest;
- closed list/delete read authorizations, including bounded pagination;
- store-signed bounded-volunteer offers and their bounded admission work;
- store-signed stored and delete receipts plus their exact identifiers.

The Rust implementation strictly decodes and verifies all signed objects and
checks the semantic links between address, envelope, authorization and receipt.
An independent Node implementation produces the CBOR, Ed25519 and BLAKE3
golden vectors without calling Rust or a CBOR package.

## Privacy boundary

The store still receives only unrelated capability keys, opaque mailbox/item
identifiers, ciphertext, sizes and timing. The canonical artifacts contain no
Kilogram Account, Device, conversation or event identifier. Ciphertext bytes
remain opaque to this contract; HPKE cross-language cryptographic conformance
is an explicit activation prerequisite rather than an unsupported claim.

## Deliberate separation

This RFC does not canonicalize or activate:

- `mailbox-provisioning`, which distributes and acknowledges capabilities;
- `blind-mailbox-provider-gossip`, which advertises volunteer providers;
- `blind-mailbox-transport`, which frames requests and responses over Iroh;
- `runtime-mailbox-envelope`, which carries application dispatch state.

Those surfaces retain their own compatibility entries and cannot inherit a
complete status from this RFC.

Provisioning, provider gossip, transport and runtime dispatch therefore remain
independent migration work.

## Migration safety

Every candidate cryptographic operation uses a new `cbor-v1` domain. The
active mailbox remains Postcard v1, with its existing domains and HPKE AAD.
No active ALPN, version, payload, runtime reader or network byte changes.
Activation still requires a new explicit protocol identifier or parallel ALPN,
dual-reader downgrade tests, HPKE cross-language conformance, coverage-guided
fuzzing and fresh field/hosted evidence.

## Verification

The fail-closed boundary pins the schema, object registries, resource limits,
golden vectors, independent reference, active Postcard separation and coverage
accounting. Six new signed decoder fixtures expand the deterministic mutation
corpus from 4,698 to 6,264 cases with a 7,000-case ceiling. The complete M1
readiness inventory grows from 48 to 49 gates.
