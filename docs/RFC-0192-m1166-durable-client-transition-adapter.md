# RFC-0192: M1.1.66 dormant durable client transition adapter

Status: accepted locally; inactive client runtime contract.

## Purpose

The preceding stages created a completed-import gate, an immutable signed
checkpoint chain, a checkpoint-bound pending-frame journal and one atomic
runtime-transition transaction. None of those primitives selected client
transitions or proved that a restarted adapter would dispatch the exact frame
that may already have crossed an uncertain network boundary.

M1.1.66 integrates those primitives into the concrete dormant
`CanonicalSyncV2Client<CanonicalSyncV2CarrierStore>` without registering or
dialling `kilogram/m1/sync/cbor/2`.

## Durable client transitions

Before an Inventory can be written to a transport, `begin_durable_round`
creates its Device signature and atomically advances `SendInventory` to
`ReceiveDiff` while storing that exact Inventory as the pending frame.

`accept_durable_diff` restores that Inventory instead of rebuilding it,
verifies the signed responder, exact conversation/session binding, requested
typed keys, every received carrier and membership policy, then prepares the
exact EventBatch. Received carriers, cumulative typed evidence, the signed
`SendEventBatch` successor, head and pending EventBatch commit through the
single M1.1.65 transaction.

`mark_durable_batch_sent` advances to `ReceiveComplete` while retaining the
same EventBatch. `accept_durable_complete` verifies the acknowledgement against
the current round's exact sent keys and continuation flag. It then either
clears the pending frame in terminal `Complete`, or advances exactly one round
to `SendInventory` without inventing a frame.

The local availability bit remains set while an EventBatch, including an empty
batch, is available for transmit or retransmit. This makes a fully converged
empty round resumable through `ReceiveComplete`; terminal completion clears
both availability bits.

## Exact restart dispatch

`durable_action` accepts only checkpoint/frame pairs that have one unambiguous
meaning:

- `SendInventory` with no frame prepares a fresh signed Inventory;
- `ReceiveDiff` retransmits the exact persisted Inventory;
- `SendEventBatch` transmits the exact persisted EventBatch;
- `ReceiveComplete` retransmits that same EventBatch after an uncertain send;
- paused and terminal checkpoints do not produce a transport frame.

A missing session, wrong exact session identity, unexpected action or wrong
pending-frame variant fails closed. The adapter never derives a replacement
frame from a newer local inventory. Exact transition retries after a lost local
response are deterministic and idempotent at the store boundary.

## Regression evidence

The concrete regression bootstraps from an empty but durably completed legacy
import, commits one received carrier in round one, retries the Diff and batch
sent transitions, advances through a continuation, drops and reopens redb,
restores round two, completes an empty round, retries terminal completion and
verifies the nine-record signed checkpoint chain.

## Deliberate boundary

This is client-side durable orchestration only. The dormant server adapter does
not yet journal its own exact response transitions, and mutual authenticated
capability selection is not connected to either side. No CLI, Iroh endpoint or
active Postcard v6 path references these methods.

`kilogram/m1/sync/cbor/2` remains unregistered and undialed. There is no format
sniffing, parse-error fallback, network execution, release build, archive or
stable executable replacement in this stage.
