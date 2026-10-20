# RFC-0253: M1.2.29 explicit recipient recovery import plan/apply

## Status

Accepted locally on 2026-10-20. This is a dormant, caller-invoked bridge from
the M1.2.28 durable recipient coordinator to the already verified M1.2.21/
M1.2.22 import path. It is not registered by the active runtime, CLI or desktop
application. Runtime IPC remains postcard v33 with descriptor domain v14, no
wire format changes, and the public default remains `legacy-only`.

## Problem

The coordinator can prove which approved source has staged work ahead of its
recipient-signed import receipt chain, but calling the low-level importer with
an arbitrary transfer ID would move the selection decision outside that proof.
Selecting the newest staged transfer is also wrong when a linear backlog has
several pages: generation zero must be imported before generation one.

## Read-only exact plan

`RecipientHistoryRecoveryImporter::plan` requires one explicit source ordinal
from an already durably applied coordinator plan. It reopens and verifies the
current coordinator chain, requires its exact observations still to match the
verified inbox and receipt stores, and refuses a coordinator plan which still
needs apply. Planning is read-only and creates no state.

The next transfer ordinal equals the coordinator's imported chunk count. Thus
a three-generation staged backlog selects generations zero, one and two in
that order across three separate calls; no generation can be skipped. The plan
binds the coordinator plan and opaque source key, exact selected transfer and
execution record, current inbox head, expected import-receipt head, monotonic
counts/ranges, and a digest plus revision of the verified conversation
membership. It contains no message body or plaintext and grants no network or
background authority.

## Explicit one-transfer apply

`RecipientHistoryRecoveryImporter::apply` imports exactly one transfer. It
checks the same recipient Device and membership digest, acquires the shared
state-directory lock, and under that one lock revalidates the durable
coordinator, exact inbox head and selected backlog generation before invoking
the existing atomic importer. A newer staged head, changed membership, changed
coordinator or substituted transfer therefore fails closed before import.

The existing importer still performs the full source signature, Account,
recipient, consent selection, membership, event, projection, checkpoint and
receipt-chain checks. It publishes authorized events, encrypted projections,
checkpoints and the recipient-Device-signed receipt through its existing
crash-consistent transaction. This stage does not duplicate or weaken that
authority boundary.

After durable import, apply builds and applies one fresh coordinator plan. The
two durable stores cannot share one transaction, so import necessarily commits
before coordinator refresh. That window is explicitly resumable: retrying the
same import plan verifies the exact already-imported artifacts, returns
`AlreadyImported`, and then advances the coordinator without duplicating work.
A refresh failure is reported as post-import rather than pretending the import
rolled back. Whole-database rollback still cannot be detected without an
external witness.

## Authority boundary

There is no batch loop, source discovery, timer, task spawn, network endpoint,
runtime IPC command or desktop control. The caller chooses one source ordinal
and invokes one apply. Reports expose only the ordinal, bounded counts,
coordinator sequence and closed outcomes; they omit identities, network
coordinates, message bodies and plaintext.

## Verification

Four focused stable-path tests prove three-generation backlog order and
read-only planning; rejection of an unapplied coordinator and changed
membership; rejection of a newer inbox head; and restart after import commit
but before coordinator refresh. The full execution crate passes 22/22 tests.
The dedicated boundary rejects 10/10 negative mutations, and accepted-tag
readiness contains 155 closed gates with rustfmt and workspace-wide locked/
offline strict Clippy.

No product network, relay or second host is required because this stage adds no
transport. No release build, archive, Git tag or automatic GitHub build is
created.

## Next

M1.2.30 may add a bounded explicit multi-source recipient import round over
these one-transfer plans. It must preserve deterministic fairness, one import
in flight, finite work budgets and restart-safe outcomes without becoming a
background service or active runtime/desktop command.
