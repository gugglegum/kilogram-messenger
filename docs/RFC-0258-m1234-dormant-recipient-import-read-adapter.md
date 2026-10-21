# RFC-0258: dormant recipient import read adapter

Status: accepted for M1.2.34; dormant read-only adapter and negative boundary
verified.

## Purpose

M1.2.33 defines a bounded cross-process vocabulary, but deliberately provides
no adapter. M1.2.34 connects only its `Plan` and `Status` requests to the
existing M1.2.32 in-process planner. The adapter remains transport-independent
and is not registered in `RuntimeIpcCommand`, the runtime server, CLI, desktop,
worker or any network path.

The dependency bridge lives in the separate
`kilogram-peer-history-recovery-runtime` integration crate. The offline
`kilogram-peer-history-recovery-execution` production library retains no
dependency on `kilogram-runtime-ipc`; its real signed fixtures compile the
same adapter source only as a test integration surface.

The adapter returns a privacy-safe contract outcome plus, after successful
planning, an ephemeral in-process execution plan. It does not sign a response;
that descriptor-bound transport responsibility remains outside this adapter.
It persists neither the plan nor membership authority.

## Caller-owned membership authority

`Plan` accepts only the ordered opaque references already validated by the
M1.2.33 request. For every reference it synchronously invokes a caller-owned
loader. The loader must produce exactly one live membership together with the
same authority-reference and authenticated-context IDs. Missing, ambiguous,
substituted or digest-mismatched results are rejected before the M1.2.32
planner runs.

Loaded membership snapshots live only in a transient vector for the duration
of the call. They are not copied into the dispatch result, intent database or
execution plan. The recovery database cannot discover or reconstruct the
caller-authorized context.

## Read-only plan and status

`Plan` first exact-matches the requested intent, active/unconsumed state and
current coordinator ID and sequence. It then reloads all memberships, builds a
transient ordered M1.2.32 source list and calls the existing bounded read-only
planner. The returned projection contains only opaque IDs, closed counts and
the explicit confirmation phrase. Planning creates no file and performs no
import.

`Status` has two safe modes:

1. Without an execution plan it reports `ActiveUnplanned` only while the
   durable intent still names the exact original coordinator. It does not load
   membership or guess progress after coordinator movement.
2. With a caller-retained ephemeral plan context it reloads the exact original
   membership references and reruns M1.2.32 planning with the same budget. The
   new plan must content-match the retained plan before status is returned.

A foreign or stale plan is rejected before membership loading. Durable intent
lookup is read-only and bounded; it does not create a database when state is
absent.

## Mutation boundary

`Confirm`, `ApplyNext` and `ApplyBounded` are always rejected by this adapter.
An active intent, successful plan or observed status grants no import
authority. No confirmation, mutation nonce, replay ledger, import or consume
operation exists in M1.2.34.

The active runtime IPC remains version 33 with descriptor domain v14. Ticket,
sync and history-transfer wire formats are unchanged. Public default remains
`legacy-only`.

## Verification

Eight stable-path tests use real signed recovery fixtures and durable stores.
They prove read-only planning, unplanned and exact-known-plan status, mandatory
membership reload, missing/ambiguous/substituted reference rejection, stale
coordinator/plan rejection before loader invocation, and unconditional
mutation-command rejection. Full execution regression passes 42/42.

The fail-closed boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 160 closed gates with rustfmt and locked,
offline, workspace-wide strict Clippy. No product network, second host,
release build, archive, tag or automatic GitHub build is used.
