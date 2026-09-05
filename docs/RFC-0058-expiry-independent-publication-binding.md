# RFC-0058: Expiry-independent endpoint publication binding (M0.9.36)

Status: implemented in M0.9.36.

## 1. Problem

M0.9.35 derives each endpoint's opaque ticket-publication channel from its
current connection ticket. The strict ticket decoder also requires every signed
prekey pool to be current. A device returning after both the ticket and its
publication automation window expired could therefore know an enrolled peer
but could not discover that peer's fresh ticket.

The publication lookup capability must survive transport/prekey expiry without
making an expired endpoint dialable.

## 2. Durable binding

Every newly enrolled primary or alternate endpoint now receives a separate
local-device-signed append-only `.epb` record in the vault-primary Runtime
repository. It binds exactly:

- local Account and Device;
- stable contact and peer Account/Device;
- conversation, route policy and canonical descriptor path;
- the self-authenticating ticket-publication Ed25519 public write key.

Its ID uses the same domain-separated local Account/contact/peer Device tuple as
the endpoint candidate ID. The record has no network address, prekey or expiry;
it authorizes only an opaque publication GET. Signature, path, identity and
endpoint-contract mismatches fail closed during snapshot loading.

Contact/candidate enrollment stores the binding in the same state transaction
as the signed enrollment and peer-authority/prekey updates. Re-importing an
existing endpoint idempotently repairs a missing binding.

## 3. Legacy migration

An M0.9.35 contact has no `.epb` record. Before its first refresh, the runtime
may decode the exact enrolled descriptor in an authentication-only mode. This
mode still verifies ticket version, publication key, listener certificate,
complete Root-signed authority/device list, every prekey signature and range,
listener signature, peer Account/Device, requester Account, route policy and
the local Device authorization. It deliberately skips only the current-time
validity check on signed prekey pools.

The runtime then signs and commits the extracted binding before making the
remote GET. A missing/unreadable/tampered descriptor cannot be migrated and
requires a fresh verified bootstrap. The authentication-only ticket is never
returned by endpoint resolution, never observed into ratchet state and never
used for delivery or synchronization.

## 4. Refresh and connection safety

Refresh preparation reads the durable binding rather than the expiring ticket.
After the fetch, the runtime rechecks the unchanged enrollment and binding,
opens the recipient HPKE envelope and performs the complete ordinary ticket
verification, including current prekey validity. The fresh ticket must contain
the exact pinned write key in addition to matching publisher Account/Device,
requester, route policy and channel. Existing publication observation
high-water, authority pinning, ratchet retirement/observation and atomic
descriptor replacement remain unchanged.

Endpoint resolution also rejects an otherwise valid manually replaced ticket
if it changes an existing pinned publication key. An expired endpoint remains
`stale`; the conversation read model can nevertheless show its pinned channel
and observation high-water with the diagnostic
`descriptor-unusable-refresh-channel-pinned`.

## 5. Compatibility and limits

- Ticket v10, event/session/transport formats and IPC v10 do not change.
- Existing M0.9.35 contacts migrate locally on first refresh when their old
  signed descriptor is still present, even if its prekeys expired.
- The binding is local state and is not a global freshness proof. It does not
  discover unknown peer devices, reconcile channel observations between the
  user's devices, hide store access patterns or guarantee store availability.
- Rotation to a genuinely new publication capability requires an explicit
  authenticated protocol rather than silent descriptor replacement.

The next availability slice should distribute authenticated endpoint
announcements and observation evidence between already-authorized devices
without turning the opaque store or gossip peers into identity authorities.
