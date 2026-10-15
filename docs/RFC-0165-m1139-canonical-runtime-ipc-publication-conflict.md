# RFC-0165: M1.1.39 canonical runtime IPC publication-conflict boundary

Status: implemented as an inactive M1.1.39 candidate.

## Purpose

M1.1.39 defines deterministic-CBOR local requests for creating and applying a
Root-authorized publication-conflict resolution ceremony. The IPC exact-binds
the retained conflict, current authority and quarantined publication channel,
but deliberately does not transport the sensitive portable artifacts.

This stage does not activate canonical CBOR. The active desktop and CLI runtime
continues to use Postcard IPC version 26 and JSON descriptor version 7.
No network was executed and no artifact broker was activated by this stage.

## Exact current state

Both commands commit the authority revision and Device-list digest, conflict
proof and evidence identifiers, peer Account and Device, old publication
channel and epoch, and a content-addressed reference to the current endpoint
ticket. The apply command additionally commits the exact compact public claim
and content-addressed request artifact of the pending ceremony.

A future adapter must compare this state while holding the same durable lock
used for commit. A mismatch in authority, proof, peer, channel, ticket or
pending request fails before staging, signing or mutation.

## Create request

`create-publication-conflict-request` is valid only when no ceremony is
pending. The caller supplies the bounded byte length and domain-separated
digest of a replacement ticket that is already staged in trusted storage. The
runtime verifies that artifact together with the retained proof, creates the
portable Device-signed request internally and durably stages it before success.

The signed result exposes only the compact public request claim, requester
Device, proof identity, request and replacement-ticket artifact references,
and the honest state `awaiting-offline-authorization`. It does not claim that
the Account Root has approved the change.

## Apply response

`apply-publication-conflict-response` requires the exact pending request claim
and a bounded content-addressed reference to a separately staged Root response.
The runtime independently verifies the Root signature against the exact current
Account authority. The response claim must match every public field of the
pending request ceremony.

Success is emitted only after one atomic transaction replaces the peer endpoint
ticket, retains the Root resolution, releases the matching quarantine and
preserves the original conflict proof. The signed result explicitly reports
that sibling propagation is still pending; local application is not proof of
remote convergence.

## Secret and artifact boundary

The portable Device request and Root response include publication write keys
and replacement-ticket plaintext, so they remain inside a trusted
content-addressed staging area. The local IPC carries only sizes, dedicated
domain-separated digests and compact public verification claims. It forbids:

- the Account Root secret;
- publication write keys or other reusable write capability material;
- replacement-ticket plaintext;
- portable request or Root response plaintext; and
- local filesystem paths.

The digest proves exact artifact selection, not trustworthy persistence. An
eventual artifact broker must independently enforce bounded content-addressed
storage, access control, atomic promotion and crash recovery before activation.

## Idempotency and crash boundary

The request ID is the durable idempotency key. The runtime must exact-compare
the complete request bytes and current state, stage or atomically commit the
effect, and cache the exact signed response under one recoverable operation. An
exact retry may return `already-committed`; reuse of the request ID for
different bytes fails before another artifact or state effect.

Creation success requires the portable request artifact to be durable before
the result is returned. Apply success requires Root verification and atomic
endpoint, resolution, quarantine and retained-proof state before the result is
returned. Neither response requires a runtime restart.

## Canonical profile and evidence

Requests and responses have 128 KiB limits. Request and Root response artifacts
are bounded by the existing 9 MiB portable conflict limits; replacement tickets
retain the existing 8 MiB limit. Unknown variants, non-minimal CBOR, trailing
data, stale descriptors, bearer substitutions, artifact substitutions and
context substitutions fail closed.

Evidence includes:

- strict Rust encoders, decoders, contextual verifiers and round-trip tests;
- deterministic golden create/apply requests and Device-signed responses;
- an independent Node reference for CBOR, Ed25519, BLAKE3 identifiers and
  content-addressed artifact digests;
- bounded mutation fixtures for both requests and both responses; and
- a fail-closed boundary that proves the active Postcard runtime is unchanged.

This is a protocol and local-trust-boundary result, not a completed adapter.
Real Root-device transport, trusted artifact staging, durable transactions and
sibling propagation remain activation work.
