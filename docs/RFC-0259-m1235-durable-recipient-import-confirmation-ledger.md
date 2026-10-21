# RFC-0259: durable recipient import confirmation ledger

Status: accepted for M1.2.35; dormant explicit-confirmation persistence and
negative boundary verified.

## Purpose

M1.2.34 can construct an exact ephemeral recipient import plan, but it rejects
all mutation commands. M1.2.35 adds the next deliberately narrower authority:
the current recipient Device may persist evidence that the user explicitly
confirmed one exact plan. A confirmation is evidence of approval only. It does
not execute an import, consume the durable intent, open a network endpoint or
register a runtime command.

The ledger remains in the dormant
`kilogram-peer-history-recovery-runtime` integration crate. Active runtime IPC
stays at version 33/domain v14, and CLI, desktop and server dispatch do not
depend on this new surface.

## Exact confirmation binding

Planning accepts only an M1.2.34 dispatch whose outcome is `Planned` and an
already validated M1.2.33 `Confirm` request. The command must repeat the exact
intent, execution plan, coordinator ID and sequence, ordered membership
references and membership-set digest retained by the ephemeral dispatch.
Substitution is rejected before any state is created.

The request contract already requires `UserConfirmed` and the plan-specific
confirmation phrase. The ledger additionally requires the signing identity to
be the configured owner Device. A caller-selected expiry must be later than
the issue time and no more than 900 seconds away.

Planning is read-only. An absent ledger is treated as an empty ledger without
creating the history-recovery directory or database. The candidate is signed
by the current recipient Device and exact-bound to its caller nonce, sequence
and previous confirmation ID.

## Durable replay ledger

Explicit commit acquires the shared state-directory lock and reopens the exact
ledger state before writing. Records and the nonce index are committed in one
redb transaction with immediate durability. The ledger is bounded to 4096
records and forms one append-only content-addressed chain. Every open verifies:

- owner Account and Device metadata;
- canonical encoding, Device signature and content-addressed record ID;
- the one-to-one nonce index;
- consecutive sequence and previous-ID linkage from the single head;
- absence of orphan records, duplicate references and internal forks.

An exact committed nonce/binding replay returns the original durable record.
The same nonce with another plan fails closed. Two candidates planned from one
head cannot fork it: after one commits, the stale candidate must be replanned.
Expiry is checked again under the state lock. A failure before transaction
commit leaves no record and the same candidate remains safely retryable.

The chain detects inconsistent internal rollback, fork and corruption. Like
the existing M1.2.28 coordinator, it does not claim detection of a whole-file
rollback performed together with all local state; that requires an external
witness or a separately anchored monotonic counter.

## Privacy and authority boundary

The record stores owner IDs, opaque intent/execution/coordinator IDs,
membership-set digest, nonce, sequence linkage and expiry. It stores no
membership snapshot, message body, plaintext, ciphertext history artifact,
filesystem path or transport authority.

The confirmation summary is privacy-safe and remains unusable by itself. No
M1.2.32 apply method is called, no mutation nonce is accepted, and the intent
is not consumed. A future adapter must separately revalidate the live plan,
confirmation, mutation nonce and durable recovery state immediately before
each bounded mutation.

Ticket, sync and history-transfer wire formats are unchanged. Public default
remains `legacy-only`.

## Verification

Eight stable-path tests prove no-create planning, atomic commit, exact replay,
nonce conflict rejection, stale concurrent-plan rejection, expiry and foreign
Device rejection, retry after an injected pre-commit failure, corruption
rejection and exact membership-reference binding. Strict package Clippy passes.

The fail-closed boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 161 closed gates with rustfmt and locked,
offline, workspace-wide strict Clippy. No product network, second host,
release build, archive, tag or automatic GitHub build is used.
