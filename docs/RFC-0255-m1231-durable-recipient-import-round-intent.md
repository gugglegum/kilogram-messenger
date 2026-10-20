# RFC-0255: Durable explicit recipient import-round intent

Status: accepted locally for M1.2.31 on 2026-10-20.

## Motivation

M1.2.30 can preflight and execute one bounded, caller-ordered recipient import
round, but its round plan is deliberately ephemeral. A process restart between
the user's confirmation and the last source import therefore leaves no durable
record of exactly what the recipient approved. Reconstructing a new plan from
whatever happens to be visible after restart would quietly widen authority and
could reorder sources or accept a changed membership.

M1.2.31 adds a narrow local handoff record. It persists only the explicitly
confirmed intent, observes its progress, and records explicit completion. It
does not execute an import, open a connection, discover a source, or start a
worker.

## Contract

Planning is read-only and accepts only an exact, unstarted M1.2.30 round over
the current durably applied coordinator plan. It creates no directory or
database. The candidate intent is signed by the recipient Device and is
content-addressed. It binds:

- owner Account and recipient Device;
- round ID, baseline coordinator plan ID and sequence;
- every coordinator source baseline, including opaque inbox/import heads and
  bounded counters;
- caller-ordered source ordinals and every M1.2.29 import plan ID;
- exact transfer/execution/head/range candidate bindings;
- membership digest and revision, plus source/event budgets.

The commit timestamp is signed and audited, but is deliberately ignored when
deciding whether a freshly reconstructed candidate is the same intent. This
makes an exact restart replay idempotent without permitting any semantic field
to change.

## Durable store

An explicit commit creates a separate
`history-recovery/recipient-import-round-intents-v1.redb`. Records use canonical
Postcard encoding, immediate redb durability, the shared state-directory lock,
and symlink rejection. The database is bounded to 4096 intents, each no larger
than 65536 bytes, and permits exactly one active unconsumed intent. Consumed
records remain available as signed audit history.

The store contains opaque identifiers, counters, ranges, membership digest and
revision. It does not contain a membership snapshot, message bodies, plaintext,
or ciphertext history artifacts. It provides no whole-database rollback claim
without an external witness.

## Observation and completion

Read-only inspection reports `not-started`, `partial`, or `complete`. Completed
sources must form an exact prefix of the signed caller order. Every completed
receipt and imported artifact is reverified; sources outside the round must
remain exactly at their signed baseline. The baseline coordinator record must
still be a verified member of the durable coordinator chain.

The observer deliberately recognizes the crash window in which the atomic
history import has committed but its coordinator refresh has not. Such a round
can be reported complete with `coordinator_refresh_required=true`, but explicit
consume fails closed until the coordinator has been refreshed. Successful
consume adds a second recipient-Device signature binding the intent to the final
coordinator plan, sequence, completed source count and imported event count.

Competing active rounds, same-round forks, foreign recipient signatures,
changed memberships, stale pre-commit state, non-prefix progress, changed
unplanned sources and corrupted/non-canonical records all fail closed. Exact
commit and exact consume replay are idempotent.

## Authority boundary

M1.2.31 does not execute the M1.2.30 round. There is no automatic history
import, cross-process executor, source discovery, timer, background loop,
spawned task, network authority, runtime IPC command, CLI command, or desktop
control. The public default remains `legacy-only`; connection-ticket, sync and
history-transfer wire formats are unchanged.

## Verification

Four focused stable-path Windows tests cover read-only planning and exact
commit replay; foreign, competing and stale rejection; a real two-source
partial prefix across store reopen followed by signed consumption; and the
import-before-coordinator-refresh crash window. The full execution crate passes
31/31 tests. The boundary verifier rejects 10/10 negative mutations and M1
readiness contains 157 closed gates. No product network was opened, no release
build or archive was created, and no Git tag was added.

## Next slice

The next bounded slice may add an explicit durable executor adapter which loads
one active intent, reconstructs the already signed source bindings from
caller-supplied live memberships, and invokes the existing M1.2.30 round one
step at a time. It must remain a foreground action and must not infer authority
from the mere presence of an intent.
