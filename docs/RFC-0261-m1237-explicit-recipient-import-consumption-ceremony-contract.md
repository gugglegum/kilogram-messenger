# RFC-0261: explicit recipient import-consumption ceremony contract

Status: accepted for M1.2.37; dormant plan/commit vocabulary and signed response
boundary verified.

## Purpose

M1.2.36 deliberately leaves a fully imported M1.2.31 intent in
`CompleteAwaitingSeparateConsume`. Import and intent consumption are different
authorities: finishing a bounded import does not authorize clearing the active
intent. M1.2.37 defines a separate cross-process ceremony with only two stages:
a read-only short-lived plan and an explicit commit.

This version defines canonical data and signature validation only. It is not a
variant of the active `RuntimeIpcCommand`, is not dispatched by the runtime,
does not open the intent store, and cannot consume anything by itself. Active
IPC remains version 33/domain v14 and the public default remains `legacy-only`.

## Completed mutation evidence

The ceremony accepts evidence only from a complete M1.2.36 mutation receipt.
The completed count must equal the total source count. The evidence exact-binds
the intent, execution plan, confirmation and mutation nonce, the specific
completed mutation record ID, a canonical receipt digest, the exact ordered
membership-set digest, the final coordinator plan ID and sequence, and the
imported event count.

The record ID and receipt digest are separate domains. A future adapter must
reload both from the durable mutation ledger and must independently observe the
same active intent and final coordinator. Merely knowing an intent ID, or
presenting an incomplete execution receipt, is insufficient.

## Read-only plan

`Plan` carries the completed evidence, the same ordered opaque membership
references used by execution, a fresh consumption nonce distinct from all
mutation evidence, and a requested lifetime no longer than 300 seconds. The
references must reproduce the evidence's membership-set digest. A future read
adapter will reload their caller-owned live state and issue a content-addressed
plan. The plan repeats the evidence digest, durable mutation references, final
coordinator, nonce and exact expiry and derives an operator phrase of the form
`CONSUME xxxx xxxx xxxx xxxx` from the plan ID.

The contract plan constructor is pure and creates no filesystem state. A plan
response is signed by the current Device named by the authenticated runtime
descriptor and explicitly says that the intent is not consumed.

## Explicit commit

`Commit` repeats the exact completed evidence, ordered membership references
and complete plan, requires the exact confirmation phrase and the
`UserConfirmed` approval enum. A successful future adapter response must bind a
durable consumption ID, the same evidence, plan and nonce, the same final
coordinator, a consumption time inside the plan lifetime, and proof that the
active intent was cleared.

`Consumed` and `AlreadyConsumed` are valid only for the exact commit binding.
A different nonce cannot borrow an earlier consumption result. Stage skip,
descriptor/request substitution, altered evidence or coordinator, implicit
approval, expired result, noncanonical encoding and signature tampering fail
closed.

The response wrapper is Device-signed and exact-binds the request digest,
runtime descriptor and current descriptor Device. It permanently advertises
`explicit_commit_required=true` and `automatic_consumption=false`.

## Privacy and activation boundary

The contract contains opaque IDs, bounded counts, digests, sequence numbers,
ordered opaque authority/context membership references, nonce, timestamps and
a short operator phrase. The references authorize a future caller-owned live
reload; they are not membership snapshots. The contract contains no membership
snapshot, message body, plaintext, ciphertext history artifact, filesystem
path, transport authority or raw runtime token.

No adapter, durable consumption-plan ledger, active runtime registration, CLI,
desktop control, timer, background worker or network authority is added. A
future version must implement read-side live revalidation before a later,
separate mutation adapter may call the already existing intent-store consume
primitive.

## Verification

Eight stable-path tests cover canonical plan/commit round trips, incomplete and
domain-overlapping evidence, evidence/nonce/expiry binding, implicit approval,
descriptor substitution, current-Device signed responses, stage separation and
tampered/oversized encodings. The complete runtime IPC regression passes 43/43
and strict package Clippy passes.

The fail-closed boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 163 closed gates with rustfmt and locked,
offline, workspace-wide strict Clippy. No product network, second host, release
build, archive, tag or automatic GitHub build is used.
