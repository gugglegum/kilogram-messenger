# RFC-0218: Durable monotonic peer capability store

Status: accepted for M1.1.94.

## Purpose

The M1.1.92 floor is now persisted in a separate, bounded redb store owned by
the local Device. Every non-legacy floor transition is an immutable local
Device-signed record with a per-peer sequence and predecessor ID. The head and
record are committed with immediate durability. Restart re-verifies the owner,
record IDs, signatures, peer binding and complete predecessor chain.

The store contains Account and Device IDs, floors, compact evidence and local
signatures. It never stores the ticket or invitation body, message content,
network address, relay URL or filesystem path. It has no network dependency and
grants no transport authority.

## Import ceremony

`current-peer-invitation-import` accepts only the signed outer invitation. It
re-verifies that artifact, its exact embedded connection ticket and exact
canonical capability, then verifies that the invitation names the Account of
the local installed Device certificate. It first commits the invitation floor,
then commits the exact authenticated capability pin. A crash between those two
writes leaves the stricter `current-invitation-required` state and is safe to
resume. Only after the durable pin succeeds is the embedded ticket written to
a new no-clobber output file.

The importer never accepts a detached legacy ticket as a substitute. A
quarantined peer cannot be recovered by presenting network data or another
invitation. Operator-authorized quarantine resolution is intentionally absent.

## Rollback and boundary

Retained signed records expose a head rollback or deleted tail as an orphan.
Corruption, wrong owner Device, gaps, cycles, forks and signature changes fail
closed. Coordinated rollback of the entire database to an internally
consistent older copy remains possible without an external monotonic witness;
this is an explicit non-claim.

The active runtime and rollout policy do not read this store in M1.1.94. No
wire or IPC format changes, no contact enrollment occurs automatically, and
the public default remains `legacy-only`.
