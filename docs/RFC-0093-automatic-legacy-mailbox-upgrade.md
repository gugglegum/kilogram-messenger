# RFC-0093: Automatic legacy mailbox upgrade (M0.9.70)

Status: implemented.

## 1. Outcome

A running client now upgrades an acknowledged receive-mailbox capability from
`legacy-random-fallback` to an authenticated exact volunteer replica set as
soon as at least two active transport-distinct provider offers are available.
The user does not have to find the old capability and perform a manual
rotation.

This is a capability-route migration, not a message migration. It creates no
outbox message, changes no event and does not retransmit conversation history.
At most one eligible capability is upgraded per bounded runtime check.

## 2. Fail-closed eligibility

Automatic rotation is allowed only when all of these statements are true:

- the local Device owns the receive binding and can still open it;
- the ordered capability-chain head is an activation, not a revocation;
- the head has no signed replica-set commitment;
- the exact head has a durable recipient-signed acknowledgement;
- the contact and current peer Device remain authorized;
- selection yields at least the two required transport-distinct volunteer
  providers.

The provisioning primitive has a separate `require_exact_replica_set` guard.
Even after the read-only eligibility check, it refuses to persist a new
generation if the provider set is no longer sufficient. An incomplete check
therefore leaves the legacy head unchanged rather than producing another
legacy rotation.

## 3. Ordered transition

The migration reuses the existing mailbox rotation protocol. It generates a
new recipient-bound HPKE capability and appends one Device-signed
`ActivateWithReplicaSet` update whose `previous_update_id` names the
acknowledged legacy head.

Until the recipient acknowledges this new generation, the owner retains the
old receive binding in `RotationOverlap`. The recipient switches only after it
authenticates and durably applies the ordered update. Existing convergence and
session-bound acknowledgement rules then retire the predecessor. Restart does
not duplicate the migration because an exact head is never eligible again.

## 4. Runtime and observability

The runtime checks for one eligible upgrade every 30 seconds, before ordinary
capability-update delivery. A newly created exact update can therefore be sent
by the normal authenticated direct/relay path in the same scheduler tick.
State-lock and encrypted-vault dual-write boundaries are identical to manual
rotation.

Mailbox status now reports, for every receive and write head:

- `replica_set_discovery` as `legacy-random-fallback`,
  `exact-authenticated`, or `not-applicable` for revocation;
- the signed commitment ID when present;
- the exact number of committed store keys.

Because these fields extend the authenticated binary status response, the IPC
schema version advances from 24 to 25. Runtime, diagnostic CLI and desktop GUI
must come from the same build; a stale GUI is rejected instead of decoding a
new status shape as an old one.

The runtime emits `runtime_mailbox_legacy_upgrade_status=rotated` only after
the new binding and ordered update are durably committed.

## 5. Compatibility boundary

This milestone keeps the HTTPS mailbox descriptor and upload copy. The exact
volunteer set changes discovery and replication authority; it
does not yet remove the compatibility store from existing bindings. Retiring
that path is a separate decision and migration.

No server, global directory, scheduled Windows task or new executable is
introduced. The automatic check runs only while the normal Kilogram runtime
is running.

## 6. Verification

The integration test creates a legacy activation, proves that an
unacknowledged head does not rotate, adds two transport-distinct providers,
persists the recipient acknowledgement, and verifies a single exact
generation with predecessor overlap. It also verifies that no message was
queued and a second check creates no duplicate generation.

`scripts/verify-kilogram-mailbox-legacy-upgrade-boundary.ps1` fails closed if
the acknowledgement requirement, exact-provider guard, scheduler wiring,
status projection, HTTPS compatibility boundary, test or no-new-EXE boundary
disappears.
