# RFC-0097: Crash recovery for the mailbox replication ledger (M0.9.74)

Status: implemented locally; a fresh M0.9.74 replacement field run is pending.

## 1. Problem

The first M0.9.73 field run proved that cooperative outbound scheduling fixed
the earlier reciprocal-peer lockstep: Alice and Bob converged, Alice stored the
message with exact volunteer durability, and no HTTPS compatibility copy was
attempted. Bob nevertheless could not retrieve the replicas. His runtime
repeatedly reported:

`open mailbox replication ledger read-only: Database repair aborted`

The field harness intentionally stops disposable runtimes between phases. An
unclean process exit can leave Redb's recovery marker set even when the logical
tables are intact. The normal idle path uses `ReadOnlyDatabase` so that an
inspection cannot silently modify byte-authenticated vault state. A read-only
open also cannot complete Redb recovery, so retrying the same inspection can
never make progress.

## 2. Recovery boundary

`inspect_runtime_mailbox_replication_ledger` keeps read-only inspection as the
normal path. It escalates to one writable open only when the error chain
contains the typed `redb::DatabaseError::RepairAborted` condition.

Recovery is performed as follows:

1. acquire the existing per-state-directory runtime lock;
2. prepare `VaultDualWriteGuard` for the same state tree;
3. open and close `MailboxReplicationLedger` through the normal writable
   constructor, allowing Redb to finish recovery;
4. finish the vault mirror before releasing the lock;
5. repeat the original read-only inspection and return only that result.

The runtime does not delete, recreate, truncate or accept a partially inspected
ledger. Permission, I/O, corruption and every other error remain fatal. Cleanup
of expired replication rows remains a separate explicit mutation under the
same lock and vault boundary.

## 3. Crash regression

The regression test launches the current test executable as a child process,
opens the real replication ledger and exits without destructors. The parent
then proves all three boundaries:

- the interrupted database fails its first direct read-only inspection with a
  repair-required error;
- the runtime helper emits
  `runtime_mailbox_replication_ledger_repair_status=repaired-after-interrupted-runtime`
  and returns the expected empty inspection;
- a second direct read-only inspection succeeds without another repair.

This tests the actual Redb recovery marker rather than a mocked error string.

## 4. Field replacement

The incomplete M0.9.73 run is diagnostic evidence, not acceptance evidence. A
replacement kit uses the distinct `M0.9.74` milestone, `m0974` evidence label
and `%LOCALAPPDATA%\Kilogram\M0974` private root. It inherits the entire
M0.9.72 no-HTTPS contract and the M0.9.73 cooperative-scheduling boundary, and
additionally requires the replication-ledger recovery gate.

The generated kit remains debug-only, uses two Cargo jobs, contains stable-name
executables, creates no ZIP or release build, and starts no network process
during generation. The operator flow remains the same six launches.

## 5. Security and operational limits

Automatic recovery is deliberately narrow. It does not establish that arbitrary
database corruption is safe, and it does not weaken the fail-closed vault or
signed-receipt rules. A machine crash can still leave a pending network action;
existing durable queue and idempotent receipt semantics decide whether that
action is retried.

The M0.9.74 field run must still prove two exact signed volunteer receipts,
Alice offline before Bob retrieval, application commit before two signed
deletes, restart without redelivery, `http_put=not-attempted`, and identical
clean source revision. Local recovery tests do not replace that external run.
