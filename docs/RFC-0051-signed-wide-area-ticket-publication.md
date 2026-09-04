# RFC-0051: Signed wide-area runtime ticket publication

Status: M0.9.29 implemented (2026-09-05).

> Current protocol note: M0.9.33 / RFC-0055 replaces the deterministic
> identity-input channel below with a self-authenticating per-peer capability
> channel and advances connection tickets to v10. The text below preserves the
> original M0.9.29 design record.

## 1. Goal

Before this stage, a runtime contact retained one canonical ticket-file path.
The peer had to replace that file after every endpoint restart, commonly through
a synchronized folder. That was adequate for local development but was not a
protocol for two unrelated networks.

M0.9.29 defines the first minimal publication/fetch contract for an already
verified contact. A running actor can publish its current connection ticket to
an HTTPS object service and another actor can fetch, authenticate, and atomically
install it. The service receives no readable ticket, Account ID, Device ID,
conversation label, device directory, endpoint address, or prekey pool.

This is endpoint refresh after contact enrollment. It is deliberately not a
public username directory, first-contact protocol, mailbox, push service, relay,
or anonymity system.

## 2. Directional lookup channel

The object path contains a 256-bit domain-separated BLAKE3 channel ID derived
from:

- conversation ID;
- publisher Account ID;
- publisher Device ID;
- recipient Account ID.

Publisher Device ID is included so two online devices of one account never
overwrite each other's independent endpoint publications. The recipient account,
rather than one recipient device, selects the channel because one encrypted
record fans out to every active recipient device.

The value is pseudonymous, not secret. An uninformed store cannot read the
social graph directly, but anyone who learns all derivation inputs can recompute
the channel. The store can always observe source IP, destination channel, timing,
frequency, record size, and the clear outer generation. Hiding those correlations
requires a later capability-addressed or oblivious lookup design, proxying,
padding, and possibly cover traffic.

## 3. Signed publication chain

The publishing device signs a bounded record containing:

- the exact directional channel and publisher/recipient identities;
- generation and previous publication ID;
- publication and expiry times;
- a digest of the ticket and the complete encoded connection ticket.

Lifetime is between 30 seconds and one hour; the client default is 15 minutes.
The publisher keeps the chain append-only in the vault-primary `Runtime`
repository. Generation starts at one and every successor names the exact prior
content ID. A retry reuses a still-fresh byte-identical publication instead of
creating an unnecessary generation.

The existing connection ticket remains the authority object: it is signed by
the endpoint device and binds the Root-signed account/device directory, route,
requester account, endpoint address, and fresh prekey directory. Publication
signing does not weaken any of those checks.

## 4. Recipient-encrypted envelope

The signed record is HPKE-sealed independently to the persistent encryption key
of every active device in the recipient account's last verified directory. Each
slot uses a selector derived from the channel and recipient Device ID plus AAD
binding the channel, publication ID, generation, and selector. Slots are sorted,
unique, and bounded to 64; the whole response is bounded to 16 MiB.

The outer envelope reveals only protocol version, pseudonymous channel,
publication ID/generation, pseudonymous recipient selectors, ciphertexts, and
their sizes. It never contains the ticket plaintext. A device not in the active
recipient directory has no decryptable slot. A newly enrolled recipient device
therefore becomes eligible only after the publisher observes its refreshed
directory and republishes.

## 5. Fetch, validation, and rollback

Fetch has two local-state phases. The actor briefly locks state to derive the
exact channel from its signed contact, releases the lock during the HTTPS
request, then reacquires it and reloads all trust/contact state before accepting
the response. A slow service cannot hold the profile or vault lock.

After HPKE open, the actor verifies:

- publication signature, expiry, digest, and outer/inner binding;
- exact channel, publisher Account/Device, and recipient Account;
- the complete connection ticket signature and listener authorization;
- exact enrolled contact Device ID and route policy;
- exact local requester Account and current local Device authorization;
- conversation membership and peer authority/prekey high-water rules.

Every accepted head creates a local device-signed observation chain binding the
remote generation, publication ID, and ticket digest. A lower generation or
same-generation alternative is rejected after a newer head has been observed.
Exact replay is idempotent. Peer authority, revoked-device ratchet retirement,
prekey observation, and the observation append share one state transaction;
only then is the external contact ticket atomically replaced.

The observation high-water is local to one recipient device in M0.9.29. An
attacker controlling the store can suppress updates, replay a still-unexpired
head to a device with no prior observation, or show different valid publisher
forks to different devices. Cross-device gossip or an independent witness is
required to detect those cases globally.

## 6. HTTPS object API

The minimal adapter uses:

```text
PUT <base>/v1/ticket-publications/<channel-id>
GET <base>/v1/ticket-publications/<channel-id>
Content-Type: application/vnd.kilogram.ticket-publication
```

PUT success is HTTP 200, 201, or 204; GET requires 200. Redirects are disabled,
connection/request timeouts are bounded, and both advertised and streamed body
sizes are checked. Production URLs must use HTTPS. Plain HTTP is accepted only
for a numeric loopback address so a local test service cannot accidentally
become a cleartext remote deployment.

The service is treated as untrusted for confidentiality, integrity, freshness,
and availability. It may store only the opaque envelope, apply a short receipt
TTL, and replace the current value for a channel. Client signatures and local
high-water state remain authoritative. A reference Internet service is not part
of this stage.

## 7. Runtime IPC and clients

Runtime IPC version 7 adds actor-owned commands:

- `PublishOwnTicket` with exact conversation, peer account, service URL, and
  bounded TTL;
- `RefreshContactTicket` with exact conversation, peer account, and service URL.

The CLI exposes them as:

```text
kilogram-cli runtime-ipc-publish-ticket --ipc-file <PRIVATE_DESCRIPTOR> \
  --conversation <LABEL> --peer-account <ACCOUNT_ID> \
  --service-base-url https://store.example

kilogram-cli runtime-ipc-refresh-contact-ticket --ipc-file <PRIVATE_DESCRIPTOR> \
  --conversation <LABEL> --peer-account <ACCOUNT_ID> \
  --service-base-url https://store.example
```

The Windows desktop has the same two operations for the selected enrolled
contact and shows publication generation, expiry, recipient-device count,
record size, authority revision, local freshness status, and the first-contact
boundary. The GUI sees typed results but never Root, device, vault, ratchet, or
ticket plaintext secrets. Publication is explicit; no background scheduler or
Task Scheduler registration is introduced.

## 8. Verification

Regression covers:

- signature, expiry, recipient binding, wrong-key rejection, chain continuity,
  observation advancement, rollback rejection, and exact replay;
- distinct channel derivation per publishing device;
- HTTPS/loopback URL policy and real local HTTP PUT/GET of the opaque envelope;
- two live runtime actors, mutual enrolled contacts, publisher upload, recipient
  fetch/install, idempotent second fetch, and durable publication/observation
  recovery from vault-primary state;
- desktop generation of exact typed IPC commands and typed result handling.

## 9. Boundary and next stage

M0.9.29 removes synchronized folders from the steady-state ticket refresh path,
but an initial verified ticket is still exchanged out of band and the test suite
uses a loopback mock object service. It does not provide user discovery, global
freshness, cross-device anti-equivocation, offline message storage, endpoint
failover across peer devices, metadata anonymity, or service abuse controls.

The next bounded stage should implement a self-hostable opaque publication
service with fixed retention, conditional generation replacement, size/rate
limits, no application identifiers, and an Internet test procedure. Automatic
refresh should follow only after service failure and privacy behavior are
observable and configurable.
