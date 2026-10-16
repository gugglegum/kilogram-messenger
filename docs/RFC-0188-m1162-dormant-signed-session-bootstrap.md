# RFC-0188: M1.1.62 dormant signed session bootstrap

Status: accepted locally; completed legacy import is coupled to the first
Device-signed `/cbor/2` checkpoint, active runtime unchanged.

## Outcome

`CanonicalSyncV2CarrierStore` now owns an immutable signed-checkpoint journal
and one atomic head per exact `(conversation ID, local Device ID, peer Device
ID, session binding)` identity. A journal key adds the monotonically positive
checkpoint sequence. Each journal record repeats the immutable legacy-import
plan ID and the content-derived checkpoint ID beside the canonical signed CBOR
bytes. The head repeats the plan ID, sequence and checkpoint ID.

`CanonicalSyncV2Client::bootstrap_from_completed_legacy_import` creates the
only accepted bootstrap shape: checkpoint sequence 1, sync round 1, no
predecessor, empty received/sent evidence, both availability flags false,
`SendInventory`, and not paused. The local Device signs it using the existing
`CanonicalSignedSyncSessionCheckpointV2` contract.

## Import gate and atomic persistence

The store verifies the exact immutable import plan, re-reads its durable
`CanonicalSyncV2LegacyImportCheckpoint` inside the same immediate-durability
redb write transaction, and rejects a missing or incomplete import. It then
preflights the session head and immutable sequence record, inserts the signed
record and head, and commits them together. Exact replay returns the verified
existing record; a different head, conflicting signed bytes, a missing record
or an orphan record fails closed.

Every read verifies the record length/version, canonical checkpoint decode,
Device signature, content-derived checkpoint ID, identity key, sequence and
head relation. Reopen regression proves the head still resolves to the same
signed record. A cross-crate regression proves that the concrete client cannot
be created through this gated constructor before import completion and begins
a signed inventory round after successful bootstrap.

Negative storage regression also corrupts the fixed head, mutates a signed
record and injects a journal record without its atomic head. All three states
fail closed; no scan, repair guess or unsigned fallback is attempted.

The import plan ID is deliberately local evidence. It is stored beside the
signed checkpoint but is not added to the peer-visible checkpoint schema and
is not inferred by a peer. This stage therefore does not claim that another
Device has verified the local migration procedure; it proves only that this
Device cannot enter the new local session through the gated constructor before
its exact import is complete.

## Deliberate boundary

Only checkpoint sequence 1 is installed here. The runtime state machine does
not yet persist signed successor checkpoints after inventory, diff, batch,
complete, pause or resume transitions. The public dormant state-machine
constructor remains available for isolated protocol tests, so this is an
integrated gated bootstrap path rather than active-runtime enforcement.

No CLI command, Iroh endpoint, startup hook or active Postcard path invokes the
bootstrap. `/cbor/2` remains unregistered and undialed. No format sniffing,
parse-error fallback, network execution, release build, archive or stable EXE
replacement occurred.

## Remaining activation work

Activation remains blocked on:

1. persisting and restoring every valid signed checkpoint successor from the
   dormant runtime state machine;
2. mutual authenticated capability selection before any `/cbor/2` dial;
3. mixed-version, downgrade and rollback regression;
4. sustained fuzzing and fresh hosted plus physical-device evidence.
