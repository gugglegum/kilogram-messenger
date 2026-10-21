# RFC-0260: explicit bounded recipient import mutation adapter

Status: accepted for M1.2.36; dormant foreground mutation adapter, durable
nonce replay and crash reconciliation verified.

## Purpose

M1.2.35 records that the current recipient Device explicitly approved one
exact import plan, but deliberately cannot import anything. M1.2.36 adds the
smallest mutation boundary above it: a caller may explicitly submit only
`ApplyNext` or `ApplyBounded`, and the dormant integration adapter invokes the
already accepted M1.2.32 foreground executor only after every authority input
has been revalidated.

This remains a library API. It is not registered in the runtime server, CLI or
desktop application, has no timer or background worker, and opens no network.
Runtime IPC stays at version 33/domain v14 and public default remains
`legacy-only`.

## Exact authority checks

The first attempt must name an existing Device-signed M1.2.35 confirmation and
repeat its exact intent, execution plan, coordinator ID and sequence,
membership-set digest and confirmation ID. The configured owner Account and
Device must match. Every membership reference is synchronously reloaded by a
caller-owned authority loader and must reproduce both opaque reference IDs and
the signed membership digest.

The adapter then asks the M1.2.32 planner for a fresh live plan. Before durable
preparation, that plan ID, active intent and coordinator must exactly match the
confirmed request. A new operation is rejected when the confirmation is
expired. `ApplyNext` authorizes one step; `ApplyBounded` authorizes no more than
its explicit budget, with the existing hard maximum of eight. Imports remain
sequential and only one mutation nonce may be in flight.

## Durable nonce and response rule

Before the first import, the adapter appends a Device-signed `Prepared` record
to the separate
`history-recovery/recipient-import-mutations-v1.redb` ledger. After execution,
it appends a `Completed` record containing the bounded privacy-safe IPC receipt
before returning success. Both writes use the shared state-directory lock and
redb immediate durability.

The ledger permits at most 4096 mutations and 8192 records. All records form
one canonical, content-addressed, Device-signed sequence with an exact nonce
index. Each open verifies metadata, signatures, record IDs, sequence and
previous-ID linkage, phase transitions, the nonce index and the receipt's
binding and counts. An exact completed nonce replay returns the same persisted
receipt without reloading membership or importing again. Reusing that nonce
for any other binding, preparing a parallel mutation, or observing a fork,
orphan, internal rollback or corrupt head fails closed.

As with the confirmation ledger, this local chain does not claim detection of
a whole-database rollback performed together with every local witness.

## Crash reconciliation

The M1.2.32 importer can durably commit an import before its coordinator
projection is refreshed. A retry with an existing `Prepared` nonce therefore
does not demand that live progress still equal the original count. Instead it
requires the same active intent and exact live memberships, then permits only a
monotonic prefix no larger than the original bounded budget.

If the current plan reports the import-before-coordinator-refresh window, the
new executor helper repairs only the coordinator projection and returns a
fresh plan; it imports no additional source. Already durable steps are rebuilt
from the signed intent metadata, reported as `AlreadyImported`, and their event
count must exactly equal the live durable progress delta. Only the remaining
authorized steps may then execute. No additional import is started after the
confirmation expires, although an already completed durable receipt may still
be replayed and an already imported step may be reconciled/finalized.

The intent is never consumed automatically. Final consumption remains a
separate future signed ceremony.

## Privacy boundary

The mutation ledger stores owner IDs, opaque intent/execution/confirmation and
coordinator IDs, the membership-set digest, mutation nonce, sequence linkage,
bounded counts and privacy-safe step receipts. It stores no membership
snapshot, message body, plaintext, ciphertext history artifact, filesystem
path or transport authority.

## Verification

Three stable-path ledger tests cover prepare/complete/replay, parallel and
rebound nonce rejection, foreign Device rejection and corrupt-head failure. A
real signed-fixture test performs an actual foreground import, persists the
receipt, proves exact replay after confirmation expiry without reloading
membership, rejects a rebound nonce and proves the intent remains unconsumed.
A separate real fixture reproduces import-before-coordinator-refresh and proves
that reconciliation imports no duplicate.

The fail-closed boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 162 closed gates with rustfmt and locked,
offline, workspace-wide strict Clippy. No product network, second host,
release build, archive, tag or automatic GitHub build is used.
