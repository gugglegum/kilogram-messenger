# RFC-0162: Canonical runtime IPC mailbox-capability mutations

Status: implemented as an inactive M1.1.36 candidate.

## Summary

M1.1.36 defines deterministic-CBOR requests and device-signed responses for
creating, rotating, and revoking a local mailbox-capability chain. The
candidate joins the read-only status projection from M1.1.35 to the exact
Device-signed volunteer-provider selection from M1.1.34, without giving an IPC
caller access to mailbox secrets or local binding records.

This candidate does not activate canonical CBOR. The shipped runtime continues
to use Postcard IPC version 26 and JSON descriptor version 7.

## Mutation target

Every command binds one exact target:

- the local contact identifier;
- recipient account and device identifiers;
- the digest of the recipient's current signed account device list; and
- the mailbox scope.

The recipient-device-list digest is a state anchor. An adapter must refuse a
mutation if its current contact or authority state no longer matches that
anchor. This prevents a delayed GUI request from granting a capability to a
device roster that has since changed.

## Create, rotate, and revoke

A create command has no predecessor and creates generation 1. It carries the
exact Device-signed provider-selection response and a bounded requested
validity interval.

A rotate command carries both the exact currently active, recipient-acknowledged
capability head and a new exact Device-signed provider-selection response. The
new update must be generation `previous + 1` and must name the exact previous
update ID.

A revoke command also carries the exact active, recipient-acknowledged head,
but it carries neither a provider selection nor an expiry. Its result is a
revocation at generation `previous + 1`, linked to that exact predecessor.

Create and rotate accept only a selection of two through eight current,
strictly store-key-ordered volunteer offers with distinct transport
identities. The activated capability uses exactly those provider store keys,
and its expiry is both the requested interval and no later than every selected
offer's expiry. No implicit provider substitution is permitted.

## Atomicity and idempotency

The request ID is the idempotency key. A future runtime adapter must perform
the current-state comparison, signed update creation, durable head replacement,
and durable response recording as one atomic transaction. If the exact request
was already committed, it returns `already-committed` with the byte-identical
signed transition. It must never create a second generation for the same
request ID.

If the locally observed head, recipient roster digest, provider registry, or
offer validity differs from the request, the adapter fails closed before
writing anything. The canonical object verifier proves internal cryptographic
and chain consistency; comparison with the adapter's current durable state is
an activation-time responsibility.

Every successful transition remains pending recipient acknowledgement. The
local runtime must not treat a newly created or rotated capability as a fully
agreed remote delivery route until the recipient's signed acknowledgement has
been received and verified.

## Secret separation

Requests and responses contain signed protocol evidence, public identifiers,
provider offers, and the recipient-sealed peer offer already present in a
capability update. They never contain a local mailbox binding or mailbox read secret.
They also never contain a mailbox write secret, decrypted peer offer, account root
secret, or device signing secret. Key generation, sealing, and durable secret
storage stay inside the trusted runtime adapter.

The sealed peer offer is intentionally opaque at this boundary.
Independent HPKE interoperability for producing and opening that ciphertext remains an
activation prerequisite; this stage does not claim it.

## Response authentication

The response is signed by the exact device named by the loopback runtime
descriptor and binds the exact request digest. It returns the signed capability
update, the selected-provider response ID and expiry when applicable, and the
fact that recipient acknowledgement is still pending. Substituting another
request, descriptor, predecessor, provider selection, update, or transition
outcome is rejected.

## Resource bounds

The fixed resource limits are:

- request: 512 KiB;
- response: 128 KiB;
- selected providers: two through eight;
- requested validity: 60 seconds through 30 days.

All CBOR is definite-length, canonical, and bounded before allocation.

## Activation and compatibility

This RFC defines protocol bytes and verification semantics only. It does not
change the active runtime, desktop client, CLI, registry, mailbox storage, or
network behavior. Activation still requires an atomic durable adapter, local
state conflict tests, independent HPKE interoperation, completion of the
remaining runtime IPC surface, an explicit IPC version migration, dual-reader
and downgrade tests, sustained fuzzing, and fresh field evidence.

No network was executed to verify this candidate. Golden create/rotate/revoke
vectors, an independent Node.js verifier, fail-closed boundary tests, Rust unit
tests, and the bounded decoder mutation corpus cover the new objects.
