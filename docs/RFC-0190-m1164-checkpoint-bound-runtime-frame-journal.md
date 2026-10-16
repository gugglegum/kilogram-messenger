# RFC-0190: M1.1.64 checkpoint-bound runtime frame journal

Status: accepted locally; inactive storage contract.

## Purpose

A signed action checkpoint alone is insufficient for exact restart. While a
client is waiting for a diff it must retain the exact signed inventory that the
peer answered. While a batch is pending it must retain the exact carrier set.
Rebuilding either frame from a later local inventory could silently produce a
different exchange even though the signed checkpoint ID had not changed.

M1.1.64 adds a local-only runtime record bound to the current signed checkpoint
ID. It does not yet drive the state machine and changes no active wire path.

## Exact pending frame

The additive `canonical-sync-v2-session-runtime-v1` redb table is keyed by the
same exact conversation, local Device, peer Device and session binding as the
checkpoint head. Its versioned value contains the exact current checkpoint ID
and either no pending frame or one strict canonical client-frame byte string.

Restore first verifies the complete signed checkpoint chain, then decodes and
verifies the pending frame with the closed `/cbor/2` client-frame codec. The
frame conversation must match the checkpoint. A pending inventory additionally
must name the checkpoint's local Device and exact session binding.

## Write and upgrade rules

Bootstrap creates an empty runtime record in the same immediate-durability
transaction as sequence one and its head. Every generic signed-successor append
rebinds the existing runtime record to the new checkpoint ID in the same
transaction as the immutable record and replacement head.

Installing a pending frame is write-once for the current checkpoint. An exact
lost-response retry is idempotent; a different frame or stale checkpoint fails
closed. Exact replay of an M1.1.62/M1.1.63 bootstrap safely backfills only a
missing empty runtime record. Missing, orphaned, malformed or checkpoint-mismatched
runtime state is never guessed or ignored.

## Deliberate boundary

This stage provides the durable exact-frame prerequisite only. The dormant
adapter does not yet advance checkpoints on Inventory, Diff, EventBatch or
Complete transitions, and received carriers are not yet committed atomically
with the corresponding successor. Resume does not yet dispatch a restored
action. Mutual authenticated capability selection is also absent.

Therefore `kilogram/m1/sync/cbor/2` remains unregistered and undialed. The next
bounded slice couples adapter transitions, received carrier writes, signed
successors and this exact frame journal, then proves interruption and restart
continuation without changing active Postcard v6.
