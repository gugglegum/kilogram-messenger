# RFC-0154: Canonical local runtime IPC core

Status: implemented as an inactive M1.1.28 candidate.

## 1. Scope

This stage starts deterministic-CBOR coverage of the local runtime IPC trust
boundary. It covers a signed loopback descriptor, a short-lived
bearer-authenticated request, and a Device-signed response for `Ping`,
`WaitForChange`, `Shutdown`, and bounded errors. The larger contact, mailbox,
publication, automation, recovery, and messaging command families remain
explicit gaps, so the aggregate `runtime-ipc` surface is partial.

The candidate does not activate a reader or writer. The running product still
uses exact Postcard IPC version 26 and its JSON descriptor version 7. No active
wire byte, executable name, listener, firewall rule, or runtime behavior is
changed by this RFC.

## 2. Signed loopback descriptor

The descriptor names exactly IPv4 `127.0.0.1` or IPv6 `::1`, a nonzero port,
a random 32-byte bearer token, Account ID, Device ID, server-instance ID, and
a bounded validity interval. The named Device signs the deterministic-CBOR
content. Its descriptor ID is a separate domain-separated BLAKE3 digest over
the complete signed envelope.

The signature prevents an unrelated local process from replacing the
descriptor with its own port and token. Loopback-only validation prevents this
local control capability from becoming a remote API by field substitution.
The candidate still assumes that descriptor file permissions protect the
bearer token from other OS users.

## 3. Fresh correlated request

Every core request commits to the exact descriptor ID, a nonzero caller-made
request ID, the descriptor bearer token, issuance and expiry times, and one
command. Its positive lifetime is at most 30 seconds and cannot outlive the
descriptor. The contextual verifier checks both objects at the supplied local
time with at most 30 seconds of forward clock skew.

`WaitForChange` additionally carries the last observed revision and a timeout
between 1 and 120,000 milliseconds. Unknown variants, zero identifiers,
invalid lifetimes, noncanonical CBOR, trailing data, and contextual
substitutions fail closed.

## 4. Device-signed exact response

The response content repeats the descriptor ID and request ID, commits to a
domain-separated digest of the complete canonical request, names the
responding Device, and carries the result. The descriptor Device signs that
content. This closes the stale-descriptor port-reuse race: possession of an
old port alone is not sufficient to impersonate a runtime after the original
listener exits.

Result semantics are checked against the request. `Pong` repeats the exact
Account and Device from the descriptor. `ChangeState` is accepted only for
`WaitForChange`; `changed=true` requires a greater revision, while
`changed=false` requires the exact requested revision. `ShutdownAccepted` is accepted only for
`Shutdown`. A nonzero-code, nonempty, valid UTF-8 error of at most 4 KiB can
answer any command.

## 5. Bounds and activation

Descriptors are limited to 2 KiB and requests and responses to 64 KiB. A
descriptor lasts at most 24 hours. Independent JavaScript constructs the CBOR,
Ed25519 signatures, and BLAKE3 identifiers without calling Rust. Three new
context-aware mutation fixtures exercise descriptor, request, and response
decoders.

This is intentionally not full runtime IPC coverage and does not activate canonical CBOR.
Activation requires completion of every remaining command and
result, an explicit IPC version migration, dual readers with downgrade tests,
sustained coverage-guided fuzzing, and fresh desktop and CLI field evidence.
