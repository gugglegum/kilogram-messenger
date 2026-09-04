# RFC-0054: Authenticated bounded runtime ticket compaction

Status: implemented in M0.9.32.

## 1. Problem

M0.9.29 and M0.9.31 deliberately stored ticket publications, peer
observations, automation policies, and publish/refresh attempts as signed
append-only chains. That made rollback and same-generation forks visible, but a
long-running client eventually reached the hard 4096-record limits and scanned
the entire history on every restart.

Deleting old files without a new authenticated boundary is unsafe. A retained
generation greater than one refers to a predecessor that would be missing, and
accepting it merely because it has a valid device signature would turn arbitrary
prefix deletion into an undetected rollback.

## 2. Scope

This RFC compacts only these local runtime chains:

- signed own-ticket publications, keyed by directional channel;
- signed peer-ticket observations, keyed by directional channel;
- signed automation policies, keyed by enrolled contact;
- signed automation attempts, keyed by contact and action.

Messages, local projections, recovery records, outbox records, trust state, and
ratchet state are not covered. The checkpoint contains no message plaintext,
ticket plaintext, seed phrase, Root secret, bearer token, IP address, or SSID.

## 3. Checkpoint contract

The sole runtime actor creates a device-signed checkpoint containing:

- exact local Account ID and Device ID;
- monotonic checkpoint generation and previous checkpoint ID;
- compaction time;
- record counts for this delta and the cumulative compacted history;
- a canonical digest of every removed path and its authenticated bytes;
- a cumulative history digest chained to the prior checkpoint;
- one exact `(chain key, generation, signed record ID)` anchor for every live
  chain; observation and attempt anchors additionally bind their relevant
  publication/policy high-water generation.

Anchors are canonical, unique by chain key, bounded to 2048 entries, and cannot
disappear or move backwards between checkpoints. Equal-generation continuation
requires the exact same signed record ID. The retained head record remains in
the vault and filesystem shadow; the checkpoint does not duplicate large ticket
payloads.

## 4. Compaction algorithm

On the persistent runtime maintenance tick, compaction is considered after
delivery, automatic sync, and ticket automation. If any covered chain contains
more than eight retained records, the actor:

1. loads and verifies the complete DB-primary runtime snapshot;
2. selects the latest signed record of every covered chain as its anchor;
3. verifies that every selected filesystem-shadow byte string exactly matches
   the authenticated DB-primary record;
4. hashes the canonical path and bytes of every non-head record plus the prior
   checkpoint when present;
5. signs and persists the next content-addressed `.rtc` checkpoint;
6. registers each old runtime record for typed compaction in the same state
   transaction;
7. commits one vault-primary delta containing the checkpoint insertion and all
   removals;
8. reloads the snapshot and requires the new checkpoint ID to be current.

The runtime performs no network request during compaction and does not count it
as an outbound action. A successful compaction emits only generation/count
diagnostics and an IPC change notification.

## 5. Crash consistency

The generic state transaction still treats append-only removal as an error.
M0.9.32 adds a narrower operation that accepts only an existing record under the
`runtime` repository. Before deletion it copies and syncs the exact old bytes
under the active transaction directory and records the path in the transaction
manifest.

Rollback restores every registered removal and removes the uncommitted new
checkpoint. Recovery of an interrupted prepared transaction performs the same
restoration. The encrypted vault primary delta supports typed removals, so the
checkpoint and deleted prefix become visible together rather than as two
independent states.

The state layer guarantees atomicity and repository confinement; it does not
decide whether a checkpoint is semantically sufficient. That authorization
belongs to the runtime ticket checkpoint verifier.

## 6. Restart validation

At most one `.rtc` file may be current. It must have a valid local-device
signature, content-addressed filename, bounded canonical anchor set, and the
expected local identity.

For an anchored chain, the first retained record must match the checkpoint's
exact generation and signed record ID. Its signature is verified directly, and
every later record must be a normal contiguous hash-linked successor. A chain
created after the checkpoint still has to start at generation one. Missing
checkpoint, missing anchored head, extra pre-anchor record, identity change, or
non-contiguous successor fails closed.

An automation attempt may refer to a policy generation compacted below the
retained policy head only when the checkpoint proves that policy high-water.

## 7. Boundedness

After compaction, each covered chain retains one signed head plus one global
checkpoint. It may then grow to at most nine records before the next maintenance
pass compacts it again. Chain count remains bounded by runtime contacts and the
existing global record and checkpoint-anchor limits.

This removes time-proportional startup growth for ticket history. It does not
yet compact unrelated append-only repositories.

## 8. Verification

Regression coverage includes:

- state-layer rollback and simulated interrupted-transaction recovery;
- rejection of compaction outside an existing runtime record;
- two consecutive publication compactions with checkpoint generation `1 -> 2`;
- continued publication signing from the retained anchored generation;
- observation, policy, publish-attempt, and refresh-attempt compaction at
  generation nine;
- exact DB-primary restart reload with one current checkpoint;
- checkpoint signature tamper rejection;
- the production two-actor automatic ticket lifecycle after timing-independent
  assertions were converted from physical file counts to logical high-water.

## 9. Honest limits

The checkpoint proves continuity relative to the locally retained vault state.
It does not detect rollback of the entire encrypted vault plus all external
witnesses, and it cannot defend a device whose signing key is compromised.
Filesystem deletion is not a promise of forensic secure erasure on SSDs,
snapshots, backups, or cloud-synchronized directories.

Compaction itself does not make the opaque store authenticated or unlinkable.
M0.9.33 and RFC-0055 subsequently add a self-authenticating per-peer write
capability that prevents a party which merely learns a channel from replacing
it with an arbitrary high generation.
