# RFC-0212: M1.1.88 guided canonical sync v2 restart and exact verification

Status: accepted locally on 2026-10-19.

## Decision

The Windows desktop now owns a bounded activation lifecycle for a runtime that
it started itself:

1. request an authenticated clean shutdown;
2. observe the actual child-process exit rather than treating the shutdown
   acknowledgement as proof of exit;
3. inspect the Device-signed activation chain offline under the existing state
   lock;
4. plan and apply one exact signed successor with the existing typed
   confirmation ceremony;
5. spawn a fresh child from the selected launch profile;
6. authenticate the runtime through local IPC; and
7. verify that the policy loaded at process start is the exact applied
   successor before normal conversation refresh begins.

The public default remains `legacy-only`. This lifecycle does not activate a
public dual-reader default and does not alter the peer network or sync wire.

## Why shutdown acknowledgement is insufficient

The IPC acknowledgement only proves that the old actor accepted the shutdown
command. File handles, the endpoint, and the state lock may still exist for a
short interval. The desktop therefore waits until `Child::try_wait` observes
the actual desktop-owned process exit. Only then may it launch the offline
authority helper. A runtime not owned by this desktop can still be stopped with
the ordinary command, but it cannot use the one-click guided proof because the
desktop cannot attest that process's lifetime.

The wait is bounded by the existing desktop stop timeout. If the old owned
child does not exit, the desktop terminates that child and stops the ceremony
fail closed before offline inspection; it never treats a timeout as permission
to apply.

The existing offline helper remains the authority. It independently requires
the state lock and validates the append-only Device-signed policy chain. The
desktop never receives a Device secret or Account Root secret.

## Exact post-restart binding

After apply, the desktop retains an in-memory expectation derived from the
verified post-apply status. The authenticated fresh runtime must report:

- the same Device ID;
- source `signed-device-chain`;
- the exact applied record ID and migration epoch;
- the exact migration state and disposition;
- the exact optional canonical evidence ID;
- the exact six permission bits; and
- byte-for-structure equality between loaded and configured policy snapshots,
  with `configured_matches_loaded=true` and `restart_required=false`.

The check deliberately does not use recent sync-path observations as proof of
activation. A newly started process may have no observations, and a configured
policy alone does not prove what its endpoint loaded.

## Fail-closed behavior and retry

Normal conversation subscription and refresh begin only after exact
verification. A different authenticated Device, implicit default, different
record, epoch, state, disposition, evidence, permissions, unequal snapshots,
or contradictory restart flags terminates the newly spawned desktop-owned
runtime and leaves the UI disconnected.

Old-process stop timeout, startup exit, and startup timeout also fail closed. The durable signed successor
is not silently rolled back: apply may already be committed. The exact
expectation is retained in memory, and the user may retry a fresh start and
verification of that same record. Retry does not append another authority
record. Loading or saving a different launch profile clears the workflow so an
expectation can never cross a profile boundary.

## Compatibility and non-goals

M1.1.88 reuses authenticated read-only IPC v30 / descriptor domain v11 from
M1.1.87. It adds no IPC variants, process nonce, activation mutation, hot apply,
endpoint rebind, release archive, Git tag, or automatic GitHub build. The
desktop-owned child lifecycle plus the existing state lock is sufficient for
this local workflow; independent process attestation is not claimed.

Public-default dual-reader rollout and canonical-native history materialization
remain separate decisions. This milestone provides safe operator UX and
evidence for those decisions; it does not make either decision implicitly.
