# RFC-0254: M1.2.30 bounded explicit multi-source recipient import round

## Status

Accepted locally on 2026-10-20. This is a dormant, caller-invoked orchestration
layer over the M1.2.29 one-transfer recipient importer. It is not registered by
the active runtime, CLI or desktop application. Runtime IPC remains postcard
v33 with descriptor domain v14, no network or transfer wire changes, and the
public default remains `legacy-only`.

## Problem

M1.2.29 deliberately imports exactly one staged generation from one explicit
source. A recovery recipient may have several approved Devices with pending
history, but repeatedly draining the first source would let it monopolize
local work. A naive loop would also import early inputs before discovering an
invalid late membership or source ordinal.

## Bounded read-only plan

`RecipientHistoryRecoveryImportRound::plan` accepts an ordered slice of
caller-supplied source ordinals and their exact signed conversation membership
snapshots. The same ordinal cannot appear twice. The planner requires one
exact durably applied coordinator snapshot and builds one M1.2.29 import plan
for every input before any mutation. Therefore an invalid late source,
membership, inbox head or receipt predecessor leaves every earlier source
unimported.

The default budget is four sources and 128 events; hard limits are eight
sources and 256 events. Every source contributes at most one transfer to a
round, even if it has a longer staged backlog. Planning is read-only and
creates no state, opens no network endpoint and imports no history.

## Sequential explicit apply

Apply first rechecks the complete ordered input set and all candidates. It then
executes exactly one import at a time in caller order through the existing
M1.2.29 plan/apply boundary. After each durable import and coordinator refresh,
the next source is replanned from the fresh exact coordinator snapshot and its
transfer/execution/membership binding must still equal the original round
candidate. There is no automatic next round: another backlog generation needs
a new explicit plan and apply.

## Failure and restart

Completed work must be an exact prefix of caller order. Reapplying the same
ephemeral round plan verifies every recipient-signed receipt and imported
artifact in that prefix, reports it as `AlreadyImported`, and continues with
the first pending source. If a process stopped after an import committed but
before its coordinator refresh, apply first proves that exact one-generation
delta, explicitly reconciles the coordinator, and then continues.

The round is intentionally not one cross-source transaction. A concurrent
change after the initial preflight can stop a later step after an earlier
verified import committed; restart retains and revalidates that prefix. An
unplanned source change, non-prefix progress or more than one imported
generation for a selected source fails closed. Whole-database rollback still
cannot be detected without an external witness.

## Authority and privacy boundary

Reports contain caller ordinals, bounded counts and closed step dispositions.
They contain no Account/Device identity, path, network coordinate, message body
or plaintext. The module adds no source discovery, timer, task spawn,
background worker, network access, runtime IPC command or desktop control.

## Verification

Five focused stable-path tests cover a two-generation backlog limited to one
generation per round; rejection of late invalid inputs and event-budget
overflow before mutation; changed apply membership; restart from an
import-before-refresh window; and two real source Devices executed in reverse
caller order `[1, 0]` with exact idempotent replay. The full execution crate
passes 27/27 tests. The dedicated boundary rejects 10/10 negative mutations,
and accepted-tag readiness contains 156 closed gates with rustfmt and
workspace-wide locked/offline strict Clippy.

No product network or second host is required because this stage adds no
transport. No release build, archive, Git tag or automatic GitHub build is
created.

## Next

M1.2.31 may persist an explicit recipient import-round intent for safe
cross-process operator handoff. It must not serialize secret membership data,
discover sources, create an automatic loop or grant active runtime/desktop
authority.
