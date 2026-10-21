# RFC-0262: recipient import-consumption read adapter

Status: accepted for M1.2.38; dormant read-only planning adapter verified.

## Purpose

M1.2.37 defines a signed two-stage contract for separately consuming a fully
imported recipient recovery intent. M1.2.38 connects only its `Plan` request to
the durable state that already exists after M1.2.36. The adapter issues an
ephemeral, content-addressed consumption plan only after it independently
reconstructs and revalidates all completed evidence.

`Commit` is rejected before any mutation ledger, intent store or membership
loader is opened. The adapter is not registered in `RuntimeIpcCommand`, has no
CLI or desktop entry point, and does not persist a plan or consume an intent.
Active IPC remains version 33/domain v14 and the public default remains
`legacy-only`.

## Exact completed-mutation reload

The M1.2.36 mutation ledger now exposes a read-only lookup by the exact mutation
nonce. Missing storage remains missing: inspection never creates a directory,
database, table, lock or replacement record. The lookup distinguishes absent,
prepared and completed state. Only completed state projects the exact signed
record ID, membership-set digest and execution receipt.

The adapter reconstructs canonical M1.2.37 completed evidence from those three
durable values and compares the entire result with the request. An absent or
prepared mutation, changed record ID, changed receipt, changed membership-set
digest or any other rebound evidence fails closed. A request cannot nominate a
different completed import merely because its final counts look equivalent.

## Live intent and membership revalidation

Before loading membership, the adapter exact-checks the current coordinator ID
and sequence against the completed receipt. It then reopens the exact signed
intent by ID, requires that it is still the one active unconsumed intent, and
reloads every opaque membership reference through the caller-owned loader.
Missing, ambiguous, reordered, substituted or digest-mismatched membership is
rejected.

The existing read-only progress observer must report `Complete`, no pending
coordinator refresh, exact completed/total source counts, exact imported event
count and the same final coordinator ID and sequence. A consumed intent, stale
coordinator, incomplete durable prefix or changed live membership cannot obtain
a new plan.

Loaded membership snapshots remain transient. The outcome contains only opaque
IDs, closed counts, a bounded expiry and the derived operator phrase; it contains
no membership snapshot, message body, plaintext, ciphertext history artifact,
filesystem path, transport authority or raw runtime token.

## Mutation and activation boundary

Successful dispatch returns an ephemeral in-process
`RuntimeIpcRecipientImportConsumptionPlanSummary`. It writes no consumption
plan, clears no intent, opens no network and schedules no background work.
The existing intent-store consume primitive is deliberately absent from this
adapter. Durable plan/commit replay and the actual explicit consume transition
belong to a later mutation adapter.

No active runtime command, signed runtime dispatch, CLI, desktop control or
automatic consumption is added. This version therefore proves the read side of
the ceremony without broadening product authority.

## Verification

Four stable-path real signed-fixture tests cover an exact successful plan,
commit rejection before reload, changed evidence/membership/coordinator, and
missing-ledger/consumed-intent rejection. A mutation-ledger unit test separately
proves no-create absent inspection plus prepared/completed projection.

The fail-closed boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 164 closed gates with rustfmt and locked,
offline strict Clippy. No product network, second host, release build, archive,
tag or automatic GitHub build is used.
