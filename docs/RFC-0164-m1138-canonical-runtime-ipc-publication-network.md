# RFC-0164: M1.1.38 canonical runtime IPC publication network boundary

Status: implemented as an inactive M1.1.38 candidate.

## Purpose

M1.1.38 defines deterministic-CBOR local requests for the two existing runtime
intentions `PublishOwnTicket` and `RefreshContactTicket`. The boundary binds the
exact current publication state before any network or durable mutation and
returns a Device-signed description of the completed effect.

This stage does not activate canonical CBOR. The active desktop and CLI runtime
continues to use Postcard IPC version 26 and JSON descriptor version 7.

## Publish request

`publish-own-ticket` commits:

- the exact current publication state: contact, peer Account, durable peer-scope
  digest, channel epoch, public write capability and accepted publication head;
- the exact recipient Device-list digest and a count from one through 32;
- a canonical root-only HTTPS service origin and a TTL from 30 through 3600
  seconds; and
- the signed descriptor ID, bearer token, request ID and bounded lifetime used
  by every canonical runtime IPC mutation.

The runtime must exact-compare this state under the same durable lock used for
commit. It creates the signed ticket, recipient HPKE slots and opaque envelope
inside the trusted runtime. The caller never supplies a successor head.

The result advances the publication generation exactly once while preserving
the contact, peer, scope, epoch and capability. It carries the new publication
ID, ticket digest, recipient directory summary, opaque envelope length and
digest, and the canonical write authorization over that exact envelope. The
result reports a completed HTTPS exchange but remains `pending-peer-observation`.
A peer Device observation is separate evidence; HTTP success is not proof of remote durability or eventual peer delivery.

## Refresh request

`refresh-contact-ticket` commits the exact contact, peer publisher Device,
channel, latest local observation head and latest accepted publication head.
The runtime fetches the opaque envelope, verifies store authorization and the
publisher signature, decrypts a local HPKE slot, validates freshness and then
atomically installs the ticket before returning success.

The result contains the exact Device-signed publication observation. Its local
Account and Device must equal the runtime descriptor, its observation
generation advances exactly once, and its publication generation strictly
advances the supplied high-water mark. Publisher Account, publisher Device,
channel, predecessor and publication identity substitutions fail closed.

## Secret boundary

Neither command nor result carries:

- ticket plaintext;
- HPKE ciphertext or recipient slots;
- the publication write secret or Device secret;
- the plaintext durable peer scope; or
- a local filesystem path.

The opaque envelope fixture in the golden vector is deliberately not an HPKE
conformance claim. It exists only to independently reproduce the length,
digest, write authorization and outer IPC signatures. Independent HPKE vectors
remain required before activation.

## Idempotency and crash boundary

The request ID is the durable idempotency key. A future active adapter must
exact-compare the request bytes and current state, perform the network exchange,
commit the resulting head and cache the exact signed response under one
recoverable operation. An exact retry may return `already-committed`; reuse of
the request ID for different bytes must fail before another network or local
effect.

For refresh, success is forbidden until the ticket and observation are in one
atomic durable transaction. For publish, success means the exact HTTPS upload
completed and the local publication head is committed; it does not claim that
the store will retain the object after that response.

## Canonical profile and evidence

The request and response have 128 KiB limits. Service origins are exact UTF-8
canonical `https://host/` URLs without credentials, path, query or fragment.
The response is signed by the descriptor Device and binds the complete request
digest. Strict decoders reject non-minimal CBOR, unknown variants, trailing
bytes, stale bearer contexts and mismatched nested signed artifacts.

Evidence includes:

- Rust round-trip, vector and substitution tests;
- independent Node.js Ed25519, BLAKE3, KDF and CBOR reproduction for publish
  and refresh;
- bounded decoder mutation fixtures for both requests and responses; and
- a boundary guard proving the active Postcard runtime is unchanged.

No network was executed by this stage.

## Deferred work

Root-authorized publication-conflict request and resolution commands are not
included. Activation also still requires an explicit IPC version migration,
dual readers, sustained fuzzing and fresh desktop/CLI field evidence.
