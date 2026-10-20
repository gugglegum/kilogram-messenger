# RFC-0252: M1.2.28 durable recipient recovery coordinator

## Status

Accepted locally on 2026-10-20. This is a dormant recipient-side durability
primitive. It is not registered by the active runtime, CLI or desktop
application. Runtime IPC remains postcard v33 with descriptor domain v14, no
wire format changes, and the public default remains `legacy-only`.

## Problem

M1.2.27 can fairly attempt several already durable source requests, while the
recipient inbox and import store independently validate their own append-only
chains. After restart, however, a future operator still needs one bounded view
that says which approved sources are awaiting transfer, staged for import,
partly imported or complete. Inferring that state from filenames or mutable UI
memory would permit stale-head apply, accidental regression and false success.

The coordinator must not solve this by becoming a recovery daemon. Merely
looking at progress must not create a database, open a network connection or
import any history.

## Approved source boundary

The caller supplies one to eight approved sources. Each source is constructed
from an existing durable owner-approved consent record and one exact selection;
the source signature, Account, source Device, replacement Device, plan ID and
range are revalidated before it becomes coordinator input. All sources must
belong to one Account and recipient Device, and source Devices must be distinct.

The durable database never stores raw Device IDs. A domain-separated digest of
the Account, source and recipient Devices, consent record, approved plan and
selection index becomes an opaque source key. Consent IDs, plan IDs, transfer
heads and recipient-signed import receipt heads are already content-addressed
opaque IDs.

## Read-only plan

`RecipientHistoryRecoveryCoordinator::plan` first verifies the complete staged
transfer and recipient-signed import receipt chains through their existing
stores. It then reconciles their exact heads and monotonic counts/ranges with
the approved source. Import without a staged transfer, a range mismatch, an
import ahead of its inbox or regression against a durable coordinator head
fails closed.

Planning is read-only and creates no state. An absent coordinator database
remains absent. The returned object contains a content-addressed plan ID, a
monotonic sequence, source ordinals, closed progress states and bounded counts.
It contains no message bodies or plaintext and grants no network, import or
background authority.

## Explicit exact-head-bound apply

`RecipientHistoryRecoveryCoordinator::apply` is the only writer. It acquires
the existing state-directory lock, requires the inbox, import store and
coordinator to share the exact state root, and re-runs the entire observation.
If any transfer or receipt head changed after planning, the stale plan is
rejected. It never invokes transfer, network or import code.

The apply writes every selected source head, one append-only canonical plan
record and one chain head in a single redb transaction with immediate
durability. A failure before commit leaves no plan progress. Exact replay is
idempotent; a different plan must extend the one monotonic content-addressed
chain. Forks, skipped sequence, noncanonical bytes, orphan/substituted heads and
owner metadata mismatch fail closed on every open.

Per-source progress can only advance. Equal generation with a different exact
head is a fork, decreasing counts/ranges is a regression, and a completed
source cannot become pending again under the same approved consent. A genuinely
new recovery requires a new owner-approved consent and therefore a new opaque
source key.

The coordinator database stores only opaque content-addressed IDs and monotonic
state. Transfer ciphertext and import artifacts remain in the existing stores.
Message bodies and plaintext are never copied into it. Like the other local
databases, it cannot prove whole-database rollback without an external witness;
no such claim is made.

## Verification

Four focused tests use the stable Windows harness path. They prove read-only
planning plus exact replay; real staged-transfer and recipient-signed import
progression; stale-plan rejection plus pre-commit rollback; completed-source
regression and duplicate-source rejection. The full execution crate passes
18/18 tests. The dedicated boundary rejects 10/10 negative mutations, and
accepted-tag readiness contains 154 closed gates with rustfmt and workspace-
wide locked/offline strict Clippy.

No product network, relay or second host was required because this stage adds
no transport. No release build, archive, Git tag or automatic GitHub build was
created.

## Next

M1.2.29 should add an explicit recipient import plan/apply operation that can
consume one exact staged head chosen by this coordinator while keeping planning
read-only and preserving the existing membership, checkpoint and recipient-
signature checks. It must not create an automatic importer or runtime command.
