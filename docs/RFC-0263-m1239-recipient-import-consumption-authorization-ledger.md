# RFC-0263: recipient import-consumption authorization ledger

Status: accepted for M1.2.39; dormant durable authorization verified.

## Purpose

M1.2.38 can reconstruct complete recipient-import evidence and issue an
ephemeral consumption plan, but an in-memory plan is not durable proof that the
current user explicitly approved its destructive transition. M1.2.39 therefore
records one exact Device-signed authorization before any later adapter may
consume the active intent.

This deliberately splits authority from mutation. The authorization grants a
later adapter permission to attempt only the exact named consumption plan; it
does not itself clear the intent, import history, send a runtime response, open
network access or schedule background work. Active IPC remains version 33/domain
v14 and the public default remains `legacy-only`.

## Exact authorization ceremony

Planning accepts only a validated M1.2.37 `Commit` request paired with the exact
ephemeral plan returned by M1.2.38. The complete plan, completed-evidence digest,
membership-set digest, nonce, timestamps and derived `CONSUME` phrase must agree.
The non-default `UserConfirmed` value is mandatory. A `Plan` command, missing
read-adapter plan, changed plan, changed completed evidence, changed membership
set, substituted phrase or foreign Device fails closed.

The ledger reuses the plan's original lifetime rather than extending approval.
An authorization is valid for no more than 300 seconds from the plan timestamp.
Planning is read-only: missing storage remains missing and no directory, lock,
database, table or candidate file is created until explicit commit.

## Durable ledger

Explicit commit uses the shared state-directory lock and immediate redb
durability. `history-recovery/recipient-import-consumption-authorizations-v1.redb`
contains at most 4096 content-addressed Device-signed records, one exact nonce
index and one monotonic previous-ID chain. The owner Account and Device, exact
completed evidence, consumption plan, membership-set digest, sequence, previous
authorization ID and expiry are authenticated by the Device signature. Every
read reconstructs the content-addressed plan from that evidence and rejects an
inconsistent stored binding.

Exact replay returns the original authorization without another record. The
same nonce with another plan fails closed. A stale concurrently planned head,
expired commit, owner mismatch, orphan, fork, corrupt record/head or internal
rollback also fails closed. A failure before the atomic transaction commit is
retryable and cannot leave a visible authorization. As with the earlier local
ledgers, this does not claim detection of a whole-file rollback performed by an
attacker without an external witness.

The ledger stores no membership snapshot, message body, plaintext, ciphertext
history artifact, filesystem path, transport authority or raw runtime token.

## Activation boundary

The API is exported only by the dormant recovery runtime-integration crate.
Neither the CLI nor the Windows desktop application depends on that crate. No
active `RuntimeIpcCommand`, server dispatcher, response signer, automatic
worker, timer or network path is added. Most importantly, no intent consume
method is called in this version; M1.2.40 must separately reconcile the exact
authorization with live state before making the durable mutation.

## Verification

Seven focused stable-path tests cover read-only planning, atomic commit and
restart, exact idempotent replay, non-commit rejection, plan mismatch, foreign
Device, expiry, stale concurrent head, nonce rebinding, injected pre-commit
failure, owner mismatch and corrupt-head detection. The fail-closed boundary
rejects 10/10 semantic mutations. Aggregate accepted-tag readiness contains
165 closed gates with rustfmt and locked, offline strict Clippy.

No product network, second host, release build, archive, tag or automatic
GitHub build is used.
