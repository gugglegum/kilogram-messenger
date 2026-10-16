# RFC-0194: M1.1.67 dormant durable responder transition adapter

Status: accepted locally; inactive responder runtime contract.

## Purpose

M1.1.66 made every initiator-side `/cbor/2` send boundary restart-safe, but the
responder could still lose whether a Diff or Complete had been durably prepared
or whether an incoming EventBatch had already been applied. M1.1.67 closes that
asymmetry without registering or dialling the candidate ALPN.

## Separate responder checkpoint

The initiator checkpoint actions cannot honestly describe responder work. This
stage therefore defines `CanonicalSignedSyncResponderCheckpointV2` with
separate signature and identifier domains. Its immutable identity binds the
conversation, responder Device, allowed requester Device and exact transport
session. Cumulative typed keys record received and sent evidence, while the
state is one of ReceiveInventory, SendDiff, ReceiveEventBatch, SendComplete,
Complete or Rejected.

Responder checkpoints, heads and pending frames live in three responder-only
redb tables. They cannot be decoded from or written into the initiator journal.
The first checkpoint is accepted only after the same exact legacy-import plan
has a completed durable checkpoint.

## Atomic responder transitions

`accept_durable_inventory` verifies the signed Inventory, computes the Diff and
commits its signed successor plus exact Diff before transport write.
`mark_durable_diff_sent` retains that same Diff while awaiting an EventBatch.

`accept_durable_event_batch` restores the requested keys from the persisted
Diff, verifies every carrier and membership decision, and uses one immediate
redb transaction for the received carriers, signed successor, head and exact
Complete frame. There is no carrier-without-checkpoint or checkpoint-without-
response crash window. `mark_durable_complete_sent` either advances exactly one
round or commits terminal Complete and clears the frame.

Exact lost-response retries accept only byte-identical checkpoint, frame and
carrier evidence. A stale runtime token, fork, gap, changed frame, malformed
record or mismatched evidence fails closed.

## Restart regression

The concrete regression persists a Diff, marks its uncertain send boundary,
drops and reopens redb while waiting for an EventBatch, atomically stores the
batch with Complete, retries every transition, reaches terminal Complete and
verifies the five-record responder chain. The exact persisted Diff and Complete
are returned by restart dispatch rather than reconstructed from newer state.

## Deliberate boundary

Both dormant client and responder adapters now exist, but there is still no
mutual authenticated capability-selection coordinator connecting them to Iroh.
`kilogram/m1/sync/cbor/2` remains unregistered and undialed. Active Postcard v6,
CLI and runtime wire paths are unchanged. This stage performs no network run,
release build, archive creation or stable executable replacement.
