# RFC-0050: Restart-safe runtime device directory

Status: M0.9.28 implemented (2026-09-04).

## 1. Goal

M0.9.27 could apply a permanent Root-signed device removal to a running actor,
but its secret-free launch profile deliberately remained unchanged. A crash
before the user edited that profile could therefore select the older roster on
the next launch.

M0.9.28 makes the actor's committed choice authoritative across restart without
giving the runtime general permission to rewrite desktop configuration. The
runtime persists a device-signed receipt in protected runtime state, recovers
that receipt before constructing its ticket, and exposes a bounded profile
convergence operation in the desktop.

## 2. Authenticated receipt chain

Each successful live directory application has an append-only receipt binding:

- local Account ID and Device ID;
- monotonic receipt generation and previous receipt ID;
- Root authority revision;
- a domain-separated BLAKE3 digest of the canonical signed device list;
- active-device count;
- the absolute canonical source-file path.

The local device signs the complete content with a dedicated domain. Receipt
filenames contain their authenticated generation and content ID. Startup loads
all receipt records from the vault-primary `Runtime` repository, checks the
identity, filename, signatures, contiguous chain, monotonic authority revision
and same-revision non-equivocation, and caps the chain at 1024 entries.

An exact replay reuses the existing receipt rather than appending another
generation.

## 3. One state commit

The same state transaction now performs all three durable changes:

1. install the new own-authority high-water mark;
2. retire ratchet sessions and observed prekey generations for revoked devices;
3. append the signed directory receipt.

With an initialized vault, the receipt is a registered append write in the same
authenticated DB-primary delta as the trust and ratchet changes. A failed
transaction exposes none of them. Ticket replacement remains after that commit,
because the public ticket lives outside protected state.

This gives every interruption a deterministic result:

- before the state commit, the previous roster remains authoritative;
- after the state commit but before ticket replacement, restart selects the
  receipt roster and republishes its ticket;
- after ticket replacement but before profile convergence, restart still
  selects the receipt roster;
- after bounded profile convergence, receipt and profile name the same file.

## 4. Restart selection and failure behavior

Without a receipt, startup uses the launch-profile device list as before. With
a receipt, it ignores an older profile roster and reads the exact canonical path
named by the latest receipt. It verifies the Root signature, Account and local
certificate, authority revision, active count and device-list digest before
opening IPC or publishing a ticket.

A missing, replaced, symlinked or digest-mismatched receipt source fails startup
closed with the exact path in the error. Restoring the same authenticated file
is a repair; silently falling back to the older profile is forbidden. Prekey
paths for permanently revoked devices may be absent or are ignored, while the
ordinary complete-directory check still requires a fresh pool for every active
device.

## 5. IPC status

Runtime IPC version 6 adds `OwnDeviceDirectoryStatus`. It reports:

- Account, local Device and canonical state directory;
- authority revision and active-device count;
- receipt ID/generation when present;
- device-list digest;
- applied and launch-profile paths;
- `current` or `convergence-required` profile state;
- whether startup used the profile or recovered an authenticated receipt.

The CLI exposes the same information:

```text
kilogram-cli runtime-ipc-device-directory-status --ipc-file <PRIVATE_DESCRIPTOR>
```

The desktop requests this status immediately after authenticated ping and
rejects an identity mismatch.

## 6. Bounded desktop convergence

The runtime never writes the launch profile. The connected desktop offers
`Reconcile launch profile` only for `convergence-required` state and checks all
of the following before replacement:

- the runtime supplied an authenticated receipt ID and generation;
- the selected profile is a regular non-symlink file;
- its state directory equals the running runtime's canonical state directory;
- its IPC path resolves to the exact connected descriptor;
- its current device-list path equals the launch path reported by the runtime;
- the applied roster remains a regular non-symlink file at the exact canonical
  path;
- the applied roster still has a valid Root signature and the exact reported
  Account, local Device, authority revision, active count and canonical digest;
- the applied roster bytes did not change during validation;
- the profile bytes did not change during validation.

It clones the validated profile, changes only `device_list_file`, atomically
replaces the file, reloads it and requires exact equality. Any unrelated edit or
path drift aborts the operation. Root, device, vault and IPC bearer secrets do
not enter the profile or GUI.

## 7. Verification

Regression covers:

- receipt signature/chain verification, tamper, rollback and same-revision
  equivocation rejection;
- one DB-primary vault transaction for authority, ratchet retirement and the
  registered receipt append;
- idempotent live-apply retry with one receipt generation;
- stop and restart from an intentionally stale launch profile, refreshed ticket
  publication and continued exclusion of the revoked device;
- automatic desktop status query after ping;
- exact one-field profile convergence and rejection after external profile-path
  or applied-roster content drift.

## 8. Boundary and next stage

This stage proves local restart continuity. It does not make the public ticket
globally discoverable, prove freshness to a first-time contact, erase history
already copied to a revoked device, or create an OS background service.

The next protocol stage should replace synchronized ticket-file distribution
with a signed, privacy-conscious wide-area publication/fetch contract whose
freshness and rollback behavior are explicit before mailbox storage or
user-provided relay capacity is added.
