# RFC-0256: Explicit durable recipient import-round executor

Status: accepted locally for M1.2.32 on 2026-10-21.

## Motivation

M1.2.31 persists exactly what a recipient approved, but deliberately cannot
execute it. After restart, a caller can inspect the signed ordered prefix but
still has to manually reconstruct and invoke the M1.2.30 round. Replanning from
whatever sources happen to be visible would be unsafe: it could silently change
membership, order or the selected transfer.

M1.2.32 adds a narrow foreground adapter around the already accepted M1.2.29
single-transfer importer. It executes only one active durable M1.2.31 intent,
only with caller-supplied live memberships, and only after a separate read-only
execution plan.

## Read-only plan

Planning opens exactly one active, unconsumed intent and matches every supplied
membership to the signed digest, revision and caller order. It verifies the
current coordinator observation, the signed baseline chain member, all pending
candidate bindings, completed receipts and unchanged unplanned sources through
the M1.2.31 observer. Missing, changed or reordered membership fails closed.

The plan is content-addressed over intent/round IDs, exact coordinator plan and
sequence, completed/total source counts, bounded step count and the
coordinator-refresh flag. Planning is read-only: it creates no state, imports
no history, opens no network and starts no task.

## Explicit execution

`apply_next` executes at most one pending source. `apply_bounded` executes at
most the plan budget: one step by default and never more than eight. Imports
are sequential, so at most one import is in flight. Before each import the
executor creates a fresh M1.2.29 plan from the exact current coordinator and
the caller-supplied membership, then compares its complete candidate binding
with the recipient-signed intent. The existing atomic single-transfer importer
performs the actual mutation.

After every source, the executor reopens the active intent, reconstructs the
current coordinator observation and requires progress to advance by exactly one
ordered source with no pending coordinator refresh. Changed or consumed intent,
stale coordinator, candidate drift, non-prefix progress, an unplanned source
change or a fork fails closed.

The known crash window is handled explicitly. If history import committed but
the coordinator refresh did not, a read-only plan reports refresh required.
The next foreground apply reconciles the exact coordinator transition before
attempting another source. A complete intent replays as a zero-import no-op.
Execution never consumes the intent automatically; the separate M1.2.31
recipient-signed consume ceremony remains mandatory.

## Privacy and authority boundary

The executor adds no durable database. Its plan and receipt expose only opaque
IDs, bounded counters, closed dispositions and coordinator sequence. They do
not retain a membership snapshot, message bodies or plaintext.

There is no timer, background loop, spawned task, source discovery, cross-process
orchestration, network operation, active runtime registration, IPC command, CLI
command or desktop control. Connection-ticket, sync and history-transfer wire
formats are unchanged and the public default remains `legacy-only`.

## Verification

Three focused stable-path Windows tests cover read-only planning plus changed
membership rejection; `apply_next` followed by bounded execution over two real
source Devices in signed reverse order; and restart reconciliation after an
import committed before coordinator refresh. The full execution crate passes
34/34 tests. The boundary verifier rejects 10/10 negative mutations and M1
readiness contains 158 closed gates. No product network was opened, no second
host was required, no release build or archive was created, no Git tag was
added and no automatic GitHub build was started.

## Next slice

A later slice may expose this foreground ceremony across an explicit local
process boundary. It must preserve caller-supplied live membership, exact plan
confirmation and bounded execution; mere intent presence must never become
background authority.
