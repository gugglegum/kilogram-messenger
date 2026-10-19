# RFC-0243: Durable old-history recovery consent

Status: accepted locally for M1.2.19.

## Purpose

M1.2.18 created a short-lived, source-Device-signed old-history recovery plan,
but deliberately wrote no durable state. M1.2.19 adds a separate owner-bound
redb that records one exact consent before any later history rewrap or transfer
may begin. It does not turn peer re-enrollment acceptance into history consent
and it does not execute recovery.

The database is separate from the peer re-enrollment acceptance, runtime
transition and capability stores. Its metadata binds it to one Account and the
source Device that owns the readable history. Reopening it under another owner
fails closed.

## First durable commit

Before the first write, the store re-verifies the source signature and the
complete live context already defined by RFC-0242: the exact acceptance
receipt, consumed fresh-start transition, replacement Device, current Account
authority, conversation memberships, body-free inventory, selected ranges and
existing checkpoints. The plan must still be within its 15-minute validity
window.

One immediate-durability transaction inserts both an immutable signed record
and its single head keyed by the exact transition ID. The record includes the
canonical signed-plan bytes and independently binds their digest, plan ID,
receipt, transition, authority, source claim, inventory, recipient, range
count and timestamps. Its content-addressed record ID binds the whole encoded
record.

A test-only injected failure before commit leaves neither record nor head and
an exact retry remains possible.

## Replay and restart

An exact replay of the already committed plan is idempotent, including after
the plan's original expiry. Expiry prevents a new first commit; it does not
erase or invalidate evidence that was durably accepted while live.

A different plan for the same transition is a competing consent and is
rejected. Startup and every load verify owner metadata, canonical encoding,
both signatures, content IDs, record/head agreement and a one-to-one mapping
between records and heads. Missing records, orphan records, duplicate heads,
forks, foreign owners, changed bytes and partial metadata all fail closed. An
existing database is never initialized or repaired implicitly.

This local database cannot prove that its entire directory was restored to an
older byte-consistent snapshot. Detecting whole-database rollback requires an
external monotonic witness and is explicitly not claimed here.

## Bounded read-only observation

The no-create read-only handle exposes only absent or committed status. The
committed projection contains artifact IDs, scoped opaque Device tokens,
selection count, commit time and whether the original plan is now expired. It
contains no message bodies, raw Account or Device identity, filesystem path,
ticket, endpoint or other network coordinate. `execution_authorized` is always
false at this stage.

The database is capped at 4,096 records. A record is at most 96 KiB and a head
at most 1 KiB.

## Deliberate non-execution boundary

M1.2.19 does not read plaintext, create a history-rewrap bundle, transfer
ciphertext, advance a recovery checkpoint, select multiple sources, open a
network endpoint, add an active IPC command or add desktop controls. Those are
later and independently gated stages.

The active IPC remains Postcard v33 with descriptor domain v14. Connection
ticket and sync wire formats are unchanged and the public default remains
`legacy-only`.

## Verification

Six stable-path tests cover atomic commit/reopen, privacy-safe observation,
exact replay after expiry, competing-plan rejection, live-context/expiry/owner
rechecks, pre-commit failure and corruption/orphan detection. On Windows the
harness is compiled without execution by Cargo and run only under the stable
name required by the project firewall policy.

The dedicated fail-closed boundary validates the manifest, store source/API,
absence of network/rewrap/desktop authority and ten negative mutations.
Aggregate accepted-tag readiness contains 145 closed gates with rustfmt and
workspace-wide locked/offline strict Clippy.

No second host, product network, release build, archive, Git tag or automatic
GitHub build is used by this stage.
