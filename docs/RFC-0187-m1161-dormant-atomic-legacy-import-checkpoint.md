# RFC-0187: M1.1.61 dormant atomic legacy-import checkpoint

Status: accepted locally; local crash-atomic import progress verified, active
runtime unchanged.

## Outcome

The dormant legacy-history importer now stores a fixed-format
`CanonicalSyncV2LegacyImportCheckpoint` in the same redb database as the typed
carriers. Its table key is the exact pair `(conversation ID, import plan ID)`;
the value repeats both identifiers and also binds the membership revision,
membership digest, total event count, last committed page start and next source
index.

The value has an explicit version and exact 129-byte length. Unknown versions,
malformed lengths, key/value plan mismatches and an index beyond the immutable
plan all fail closed. Different immutable plans have different keys, so a plan
rebuilt after new source events cannot overwrite evidence for its predecessor.

## One transaction

For a new page, the store opens one immediate-durability redb write
transaction, reads and verifies the durable cursor, preflights every immutable
carrier key, inserts missing exact carriers and advances the import checkpoint.
Only then is the transaction committed. A carrier conflict or checkpoint write
failure therefore exposes neither a partial page nor an advanced cursor.

The caller still supplies its expected cursor as an optimistic concurrency
claim. A missing checkpoint accepts only index zero. A stored index must equal
the requested start, except for an exact lost-response retry: when it already
equals the requested page end, the requested start must also equal the stored
last-page start, every carrier in that exact page is revalidated and the same
progress result is returned without a second write. A completed empty no-op is
also allowed. Any stale cursor whose page merely reaches the same end, but does
not match the committed start, is rejected.

Regression covers reopen, exact retry, a manually populated prefix without a
checkpoint, stale page bounds including an alternate page with the same end,
malformed and cross-plan checkpoint values, and a later-page carrier conflict
that leaves both the later carrier and checkpoint advance absent.

## Deliberate boundary

This checkpoint is local import progress. It is not the Device-signed
`CanonicalSyncSessionCheckpointV2` used as peer-visible resume evidence. No
session ID, peer Device, inventory/diff evidence or signature is fabricated by
the importer. The next stage must explicitly couple completed local import
state to the existing signed `/cbor/2` session-checkpoint/runtime contract.

No CLI command, Iroh endpoint, active wire path or startup hook invokes the
importer. `/cbor/2` remains unregistered and undialed; Postcard v6 remains the
only active synchronization protocol.

## Remaining activation work

Activation remains blocked on:

1. signed `/cbor/2` checkpoint and runtime integration;
2. mutual authenticated capability selection with no error fallback;
3. mixed-version, downgrade and rollback regression;
4. sustained fuzzing and fresh hosted plus physical-device evidence.
