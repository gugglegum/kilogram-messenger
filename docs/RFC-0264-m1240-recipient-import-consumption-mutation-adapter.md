# RFC-0264: recipient import-consumption mutation adapter

Status: accepted for M1.2.40; dormant crash-safe mutation verified.

## Purpose

M1.2.39 durably records explicit user authorization, but intentionally does not
close the completed recovery workflow. M1.2.40 adds the narrow mutation adapter
that consumes exactly that authorized active intent. Consumption only records a
signed terminal fact and clears the active-intent pointer. It never deletes the
intent record, imported events, membership, message bodies or any other history.

The adapter remains a dormant library API. Active runtime IPC stays at version
33/domain 14, CLI and desktop do not depend on it, and the public transport
default remains `legacy-only`.

## Exact commit boundary

Only a validated consumption `Commit` is accepted. Before a new mutation, the
adapter requires the exact unexpired M1.2.39 authorization of the current owner
Device. It reopens the exact M1.2.37 completed mutation, reconstructs and
compares its evidence, reloads caller-owned live memberships, checks the current
coordinator, and requires the named intent to be active, complete and
unconsumed. A missing, expired, foreign or changed authorization, evidence,
membership, coordinator or progress fails closed.

The intent store itself revalidates the same durable progress under the shared
state-directory lock. Its only mutation is a Device-signed consumption record
plus atomic removal of the exact active pointer. Existing membership and
history stores are not opened for deletion and no network is opened.

## Crash-safe ledger and replay

`history-recovery/recipient-import-consumption-mutations-v1.redb` holds at most
4096 nonce-keyed ceremonies. Every ceremony has immutable, content-addressed
Device-signed `Prepared` and `Completed` records. `Prepared` binds the exact
authorization, evidence, plan, membership-set digest and intended consumption
timestamp; it is committed with immediate durability before intent mutation.
`Completed` binds the prepared record ID and canonical consumption receipt; it
is committed with immediate durability before success is returned.

Only one unfinished ceremony is allowed. Nonce rebinding, a completed record
without its prepared parent, a changed parent, owner mismatch, invalid signature
or malformed relationship fails closed. Exact completed replay returns the
persisted receipt without invoking the membership loader or the intent consume
method.

If the process stops after the intent consumption transaction but before the
completed ledger transaction, retry observes `Prepared`, reopens the signed
consumption record, verifies its exact timestamp, coordinator and event counts,
and writes the missing `Completed` record without consuming again. An already
consumed intent without that exact prepared record is rejected. The ledger does
not claim detection of a whole-database rollback without an external witness.

## Stored information and activation boundary

The ledger stores owner IDs, authorization ID, completed-evidence and plan
digests, membership-set digest, timestamps and the bounded receipt. It stores no
membership snapshot, path, message content, plaintext/ciphertext history,
transport capability or raw runtime token.

No active `RuntimeIpcCommand`, server dispatcher, response signer, GUI action,
timer, background worker or network route is added. A signed runtime response
dispatcher is the next separate authority boundary.

## Verification

Two ledger tests cover durable restart/replay, nonce rebinding and wrong owner.
Three real signed recovery fixtures cover exact one-time consumption, completed
replay without a loader, missing-authorization rejection before reload, and an
injected crash after consume followed by exact reconciliation without a second
consume. The formal boundary rejects 10/10 semantic mutations. Aggregate
accepted-tag readiness contains 166 closed gates with rustfmt and locked,
offline strict Clippy.

No product network, second host, release build, archive, tag or automatic
GitHub build is used.
