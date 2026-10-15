# RFC-0145: M1.1.19 canonical runtime mailbox envelope

Status: accepted locally as an inactive migration target.

## Motivation

The canonical mailbox capability, transport, gossip and provisioning stages do
not by themselves describe how a client durably associates an encrypted
offline item with its local queue and authenticated event. The active runtime
does this with several Postcard v1 records in `runtime_mailbox.rs`. Leaving
those records outside the migration model would make crash repair, retries and
replication semantically ambiguous even if the blind store protocol were
canonical.

## Contract

`CANONICAL-RUNTIME-MAILBOX.cddl` defines four bounded objects:

- local and peer binding records: a local-Device-signed owner-local binding
  record containing the exact
  canonical provisioning binding;
- a local-Device-signed imported peer record containing the exact owner-signed
  peer offer, with owner and local roles deliberately reversed;
- a mailbox payload that binds one canonical authorized event to the binding,
  source and recipient Account/Device pairs and conversation; and
- a source-Device-signed durable dispatch that binds queue, contact, mailbox,
  deterministic item, event, expiry and recipient metadata.

The item ID is BLAKE3 over queue ID, Event ID and binding ID. A separate event
item ID intentionally omits queue identity for reverse acknowledgement use.
The replication binding commits to the entire signed durable dispatch, so a
provider set cannot silently substitute a different queue/event/binding tuple.

The payload-to-dispatch verifier checks both Accounts, both Devices,
conversation, Event ID and binding ID. Dispatch TTL is limited to 60 seconds
through seven days. Nested provisioning and event objects are decoded and
cryptographically verified, not merely treated as opaque bytes.

## Independence and migration boundary

The independent Node reference reproduces exact CBOR, Ed25519 signatures and
BLAKE3 identifiers using the already independently verified provisioning,
event and mailbox vectors. Rust golden and negative tests decode the same
bytes. Four new fixtures raise the bounded deterministic decoder corpus from
8350 to 9394 rejected mutations. Payload mutations are checked in the context
of the verified signed dispatch; the payload is intentionally AEAD plaintext
and its routing metadata is not falsely presented as independently signed.

This RFC does not activate canonical CBOR. The active Postcard v1 runtime,
domains, payload version and recovery behavior are unchanged. Activation still
requires a new explicit protocol version or parallel identifier, dual readers,
downgrade tests, sustained fuzzing and fresh field evidence.
