# RFC-0206: M1.1.82 durable canonical sync v2 activation authority

Status: accepted locally; durable local authority verified, active transport
unchanged.

## Purpose

M1.1.81 defined six independent transport rights but deliberately provided no
durable source of authority. M1.1.82 adds that source without changing the
ordinary runtime default. A state directory with no activation metadata is
still exactly `legacy-only`, epoch zero, normal disposition.

Canonical activation cannot be obtained from an environment variable, a
startup convenience flag, a malformed file, or a guessed state name. The only
explicit authority is a Device-signed append-only chain operated through
`canonical-sync-policy-status`, `canonical-sync-policy-plan`, and
`canonical-sync-policy-apply`.

## Signed append-only chain

Each version-one record binds:

- the exact local Device ID;
- a migration epoch that advances by exactly one;
- the exact previous signed record ID;
- migration state and activation disposition;
- one bounded actor (`explicit-local` or `release-policy`);
- an optional exact canonical selection/progress evidence ID;
- a non-empty, trimmed, control-free reason of at most 256 UTF-8 bytes;
- an Ed25519 Device signature.

The record ID is a domain-separated BLAKE3 digest of the entire canonical
signed record. Records are stored no-clobber under the existing runtime
append-only root and participate in the existing crash transaction and
vault-primary typed delta. Decode verifies fixed lengths, canonical absent-ID
encoding, the expected Device, signature, filename, chain and transition.

The first explicit transition is `legacy-only -> advertise-cbor-v2` at
epoch one. Forward and pre-evidence rollback transitions move exactly one
state. Once canonical evidence is bound it cannot change or disappear, and
rollback is forbidden. Evidence observation and entry/exit from emergency
containment are same-state, one-epoch records.

## Plan and apply

`status` verifies the complete chain and signed high-water before returning a
policy. Absence of both is the safe implicit default. Any malformed, forked,
gapped, wrong-Device or mismatched authority fails closed.

`plan` is read-only. It deterministically signs the one permitted successor
and prints its exact record ID. `apply` recomputes the candidate from current
verified state and requires the operator to provide that exact ID. A stale
plan, changed reason, changed actor, changed evidence binding or changed chain
head is rejected before writing.

This split is intentional: the state-changing command cannot silently select
whatever transition happens to be current when it runs.

## Crash boundary and high-water

The append-only record commits first through `StateTransaction` and the
vault-primary write path. A small Device-signed high-water marker then binds
the exact epoch and head record ID and is atomically replaced. Therefore a
crash cannot publish a high-water that points to an uncommitted record.

A crash after the record commit but before high-water publication leaves the
authority unusable. Repeating `apply` repairs it only when all supplied inputs
recreate the byte-exact already stored record and its ID matches the earlier
plan. Arbitrary recovery, best-effort truncation and automatic advancement are
not allowed.

A retained newer high-water detects removal or rollback of the chain tail.
The chain detects tamper, replay, fork and gaps. As with the existing local
vault generation, a coordinated rollback of the complete state directory
together with an old matching high-water cannot be detected without an
independent current Device, external service, TPM or other monotonic witness.
This RFC does not claim otherwise.

## Deliberate non-activation

M1.1.82 does not add `/cbor/2` to an ordinary endpoint, publish a capability,
accept or initiate a canonical session, execute a network test, change
Postcard v6, create a release, ZIP or tag, or change the public default.

M1.1.83 must consume only this verified authority when building the endpoint
ALPN list and dispatch policy. Until that integration is complete, a signed
opt-in record has no transport effect.
