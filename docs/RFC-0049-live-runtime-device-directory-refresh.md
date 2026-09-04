# RFC-0049: Live runtime device-directory refresh

Status: M0.9.27 implemented (2026-09-04).

## 1. Goal

After Account Root permanently revokes a device, a running client must adopt the
new complete local-account device list without restarting. The operation must
not give the GUI Root or device private keys, must preserve the single runtime
state-writer boundary, and must distinguish future delivery guarantees from
data that a removed device already possesses.

M0.9.27 therefore adds one authenticated loopback actor command:

```text
ApplyOwnDeviceDirectory { device_list_file }
```

The CLI adapter exposes it as:

```text
kilogram-cli runtime-ipc-apply-device-directory \
  --ipc-file <PRIVATE_RUNTIME_DESCRIPTOR> \
  --device-list-file <REFRESHED_ROOT_SIGNED_DEVICE_LIST>
```

The desktop uses the same typed IPC operation. The descriptor remains a private
local bearer capability signed by the runtime device; neither the GUI nor the
CLI adapter opens the protected state directly.

## 2. Accepted transition

The supplied list must be an absolute, canonical, regular, non-symlink file and
must pass its ordinary bounded decoder and Account Root signature checks. The
runtime then requires all of the following:

- the Account ID is unchanged;
- the authority revision never decreases;
- an equal revision is byte-equivalent to the currently active list;
- the running device certificate remains present and byte-exact;
- every retained certificate is byte-exact from the previous list;
- no device is added or replaced by this live removal path;
- every disappeared Device ID has a permanent Root-signed revocation.

Consequently this command accepts only an idempotent replay or a monotonic
same-roster subset transition. Enrollment and key replacement remain separate,
stopped-runtime ceremonies.

The runtime constructs a new complete prekey directory by retaining only pools
whose certificates remain active and re-verifies its freshness before changing
state. A stale or incomplete directory fails closed.

## 3. Crash-consistent state change

One existing vault-primary state transaction installs the new own-account
authority high-water mark and retires, for every revoked Device ID:

- the pairwise ratchet session record;
- the remembered peer-prekey-pool high-water observation.

File removals are captured by the same authenticated vault delta as other
ratchet mutations. Failure before commit restores both records; a successful
commit makes both removals durable. Repeating the same update is safe and
reports zero newly retired records.

Only after the state transaction commits does the runtime create a replacement
connection ticket with its existing endpoint, requester authorization and route
policy. If a public ticket path was configured, it is replaced through the
existing same-directory temporary-file, fsync and atomic-replace publication.
Publication failure is reported as failure; retrying the exact command safely
republishes the already committed state. The in-memory ticket changes only
after the full operation succeeds.

The launch profile is intentionally not rewritten by the runtime. When the
applied path differs from the profile path, IPC returns
`launch_profile_update_required=true`; the desktop shows that the same path must
be saved before the next restart. This keeps arbitrary configuration-file write
authority outside the long-lived state actor.

## 4. Sender-side enforcement

An outbound runtime already reloads the contact's current public ticket before
materializing a queued message. M0.9.27 extends that transaction so the sender
first pins the refreshed peer authority and retires its local ratchet session
and prekey observation for every device revoked by that ticket. Only active
prekey pools are then observed and used to construct the new recipient table.

Thus an unmaterialized queued message and every later message exclude the
removed Device ID once the sender observes the refreshed ticket. This is the
strong guarantee reported as:

```text
future_recipient_slot_status=removed-devices-excluded-by-refreshed-ticket
```

Privacy-preserving wide-area publication of that refreshed ticket is still a
future protocol. In M0 the contact descriptor points at an atomically refreshed
public ticket file, so propagation is only as current as that adapter.

## 5. Immutable old events and history boundary

A recipient table is inside the signed append-only event. Once an event is
materialized, deleting or rewriting one recipient slot would change the event
ID and signature. It could also create duplicate visible messages, while copies
held by another participant or by the removed device would remain unchanged.

Kilogram therefore does not claim retroactive erasure. A pending materialized
event is preserved and, if it contains a now-revoked slot, the runtime emits an
explicit `not-rewritten` diagnostic. IPC separately reports counts of local
pending unmaterialized and materialized messages and the fixed statuses:

```text
preexisting_recipient_slot_status=immutable-cannot-be-remotely-rewritten-or-erased
history_availability_status=existing-copies-remain-readable
```

Revocation prevents future authorization and future fanout after peers observe
the new directory. It cannot remove ciphertext, plaintext, screenshots, exports
or backups already controlled by the revoked device.

## 6. IPC and UI result

Runtime IPC version 5 returns a typed result containing the Account and local
Device IDs, old/new authority revisions, active-device count, enforced revoked
Device IDs, retired ratchet/prekey record counts, local pending-queue counts,
ticket-publication state, launch-profile convergence state and the three scoped
claims above.

The Windows desktop exposes `Apply refreshed device list` only for a connected
runtime. Its device-removal panel changes directory refresh and ratchet
retirement to complete only when the returned revision and removed Device ID
match the exact removal result. Recovery-policy activation and old-history
availability remain independent lifecycle states.

## 7. Verification

Regression covers:

- authenticated live application without process restart;
- exact authority-revision and active-roster replacement in the published
  connection ticket;
- ratchet-session and prekey-observation retirement;
- idempotent replay of the same update;
- rollback of removals on an injected state-transaction failure and durable
  vault delta on commit;
- the Windows worker's exact selected device-list path and typed response path.

## 8. Implemented continuation

M0.9.28 implements the restart-safe receipt and bounded profile convergence in
[`RFC-0050-restart-safe-runtime-device-directory.md`](RFC-0050-restart-safe-runtime-device-directory.md).
The runtime no longer falls back to a revoked launch-profile roster after its
new authority state has committed.
