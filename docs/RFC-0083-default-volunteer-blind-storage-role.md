# RFC-0083: Default volunteer blind-storage role (M0.9.61)

Status: implemented. RFC-0084 subsequently added dedicated Iroh ingress and a
manual store-signed offer; automatic discovery, selection and replication
remain later protocol slices.

## 1. Product decision

An ordinary Kilogram client is intended to contribute a small amount of disk
and traffic to offline delivery. Requiring a user to discover and enable this
role would leave almost all clients as consumers, so newly created runtime
profiles enable it by default.

The default policy is:

- 200 MiB total opaque ciphertext storage;
- 500 MiB of application payload per 30-day window on Ethernet;
- 500 MiB per 30-day window on Wi-Fi;
- zero traffic on mobile networks;
- zero traffic when the OS cannot classify the network.

The desktop profile editor exposes an enable/disable checkbox, a separate data
directory and every limit. Existing profiles which predate this additive field
still deserialize safely and remain disabled until explicitly migrated; new
desktop/bootstrap and `runtime-profile-create` profiles use the defaults.

## 2. Same runtime, no new service process

The normal `kilogram-cli.exe runtime-from-profile` process embeds the existing
blind mailbox store library. No new executable, Windows service or Task
Scheduler registration is created. The role exists only while the ordinary
messenger runtime is running.

The store runs in `MailboxOnly` mode. `/healthz` and blind mailbox operations
remain available, while ticket-publication routes fail closed with 404. This
prevents an enabled volunteer role from silently becoming unrelated public
infrastructure.

The volunteer database lives outside the protected account/device state. It
contains opaque envelopes, capability-derived identifiers, signed receipts,
quota counters and a store signing identity, but no message plaintext or
Kilogram Account, Device or conversation identifiers. It is availability data,
not an authority or history replica.

## 3. Enforced limits

The 200 MiB capacity is passed through to the blind mailbox store itself; it is
not merely a UI value. Mailbox envelope size, item count, per-mailbox count,
request rate and concurrent connection limits remain bounded as well.

Transfer accounting is stored durably in the Redb metadata table, independently
for `ethernet`, `wifi`, `mobile` and `unknown`. A restart does not reset it. A
window is a fixed 30-day Unix-time bucket. Both read request bodies and
successful response bodies count; HTTP/TLS headers and lower-layer relay/QUIC
overhead are not yet measured. Therefore the configured number is explicitly an
application-payload budget, not an exact ISP byte counter.

When the current network-class budget is zero the storage role is paused and no
adapter is bound. Network class is sampled at runtime start in this slice; a
network transition requires restarting the runtime before the new policy takes
effect.

## 4. Current ingress boundary

At the M0.9.61 boundary, the embedded engine exposed only an ephemeral loopback
mailbox adapter. M0.9.62 and RFC-0084 have since added dedicated Iroh ingress
plus a manual signed offer, but decentralised selection remains incomplete:

- it is not automatically advertised to strangers;
- remote peers cannot yet discover or select it automatically;
- its signed short-lived storage offer must still be transferred manually;
- there is no multi-peer replication or erasure coding;
- no HTTPS reverse proxy is installed or required by the client.

The loopback adapter remains a local integration/compatibility boundary, not a
recommendation to deploy a central HTTPS mailbox. Remote provider traffic now
uses RFC-0084's Iroh ALPN. The standalone `kilogram-ticket-store` remains useful
as an optional bootstrap/reference store and controlled test fixture, but it is
not the target default topology.

## 5. Subsequent slice

M0.9.62 completed the dedicated Iroh mailbox ALPN, capability-authenticated
blind PUT/LIST/DELETE frames and signed expiring storage offer described by the
original next-step plan. Automatic privacy-bounded discovery, multi-provider
selection and replication are now the next slice. Sybil resistance, reputation
and erasure coding remain later work.

## 6. Verification

The stage is covered without opening a public network surface:

- launch-profile round trip and unsafe-boundary rejection tests;
- durable per-network transfer-window tests across store reopen;
- a pure route test proving `MailboxOnly` rejects publication routes;
- desktop/profile compilation;
- `verify-kilogram-volunteer-storage-boundary.ps1`, which checks defaults,
  disable controls, embedded-only ownership, durable limits and the honest
  no-discovery/no-replication markers.
