# RFC-0062: Pairwise own-device ticket discovery (M0.9.40)

Status: implemented in M0.9.40.

## 1. Problem

M0.9.39 can automatically send endpoint announcements to another active
Device of the same Account, but only after the operator places that Device's
fresh runtime ticket at a configured local path. A shared directory is useful
for a test, not as a messenger discovery mechanism. The existing opaque ticket
store already provides bounded HTTPS PUT/GET transport, but its normal contact
channels are not suitable for publishing Account-internal locators: the store
must not learn the Account ID, either Device ID or the Root roster.

This slice lets two Devices that already possess the exact current Root-signed
roster exchange their fresh own-device runtime tickets through that store. It
does not discover an unknown or not-yet-authorized Device.

## 2. Pairwise directional capability

Every v2 Device certificate already contains a long-lived X25519 encryption
public key. A Device derives the corresponding private scalar from its
protected encryption identity and performs X25519 with the sibling's certified
public key. The raw shared secret is never used directly. A BLAKE3 derive-key
KDF binds it to:

- a protocol/version domain;
- the Account ID;
- the digest of the byte-exact current Root-signed Device list;
- source and recipient Device IDs in directional order.

The KDF also binds the two public keys in canonical order and rejects an empty
context or an all-zero, non-contributory X25519 result. It produces an existing
self-authenticating ticket-store write capability. Reversing source and
recipient therefore creates a different channel, while both authorized peers
independently derive the same capability for one direction.

A Root roster update rotates the scoped channels. An absent, revoked,
non-messaging or certificate-mismatched participant cannot derive or use the
current channel through the runtime gate.

## 3. Opaque publication

For each enabled recipient policy, the source builds its current same-account
own-device connection ticket in memory. It wraps a signed, monotonic ticket
publication with HPKE to exactly the recipient Device's certified encryption
key and uploads the resulting bounded ciphertext through the existing
self-authenticating PUT protocol. The store sees only a pseudorandom channel,
generation, expiry, write public key/proof and opaque ciphertext.

The recipient derives the inverse directional channel and fetches its source's
record. Before installing the ticket it verifies:

- the HPKE recipient slot is its exact local Device;
- the inner publication is signed by the expected active sibling Device;
- publisher and recipient Account IDs are the local Account;
- publication and ticket carry the byte-exact current Root roster;
- the ticket listener is the expected source Device and authorizes the local
  Account as requester;
- publication and ticket are fresh and within existing bounds.

The existing local Device-signed observation chain rejects rollback and
same-generation equivocation. Only after all checks does the runtime atomically
replace `kilogram-discovered-own-devices/<source-device-id>.ticket` beside its
public IPC/ticket directory and persist the observation. The discovered file
is not trusted by path: every use still passes the normal M0.9.39 ticket gate.

HPKE sealing is randomized, so retrying a different ciphertext under the same
store generation would correctly be rejected as equivocation. A publication is
reused without another PUT only after a signed successful end-to-end workflow
and while its ticket and refresh window still match. After any incomplete
workflow the retry advances the signed publication generation before sealing
again. This also covers an HTTP timeout whose PUT may have reached the store.

The shared pairwise capability means either of its two holders can send a valid
store PUT proof for that directional channel. It does not let the recipient
forge the source Device signature inside the HPKE plaintext, but a malicious
still-authorized sibling can overwrite or advance the outer store head and
cause denial of service. This mechanism provides authenticated discovery, not
Byzantine availability against the paired Device.

## 4. Foreground automation and IPC

IPC v14 adds `ConfigureOwnDeviceTicketDiscovery` and
`OwnDeviceTicketDiscoveryStatus`. One Device-signed append-only policy per
recipient records:

- enabled state and canonical HTTPS (or loopback-development HTTP) store URL;
- publication TTL and refresh lead;
- the existing announcement interval, bundle validity, retry bounds and
  Ethernet/Wi-Fi/mobile/unknown-network permissions.

One configuration transaction installs both the discovery policy and its
M0.9.39 announcement policy, pointing the latter at the deterministic managed
ticket path. Identical configuration is idempotent. No ticket file needs to
exist at configuration time.

When due, the foreground runtime performs one bounded workflow: publish its
own locator, fetch and verify the sibling locator, then execute the unchanged
authenticated endpoint-announcement push and recipient-signed acknowledgement.
Success and failure use the existing signed attempt/backoff chain and the
global outbound-action budget. At most one recipient workflow starts per
automation check. A network-policy block, disabled policy or revoked recipient
does no network work. A revoked recipient remains inspectable with unavailable
channel diagnostics instead of breaking the status API.

Discovery policies join the encrypted vault-primary runtime snapshot, the
4096-record global bound and the existing transactional signed checkpoint.
Their append-only chains compact to authenticated heads after eight retained
records.

Example for both already-running sibling runtimes:

```powershell
.\kilogram-cli.exe runtime-ipc-configure-own-device-discovery `
  --ipc-file .\alice-runtime.ipc.json `
  --recipient-device <bob-device-id> `
  --service-base-url https://store.example/

.\kilogram-cli.exe runtime-ipc-configure-own-device-discovery `
  --ipc-file .\bob-runtime.ipc.json `
  --recipient-device <alice-device-id> `
  --service-base-url https://store.example/

.\kilogram-cli.exe runtime-ipc-own-device-discovery-status `
  --ipc-file .\alice-runtime.ipc.json
```

Both sides require the policy because each publishes its own directional
locator. Mobile and unknown networks remain denied by default; the existing
explicit flags opt them in.

## 5. Security and privacy boundary

- The store cannot decrypt a ticket and receives no explicit Account or Device
  identifier. It can still correlate IP address, time, direction, record size,
  stable channel use within one roster epoch and later network connections.
- Root-signed authority is never learned from or changed by the store. Both
  Devices must already have the same exact current roster.
- Ticket discovery does not carry message history, ratchet secrets, Root or
  Device private keys, IPC tokens, membership changes or device-link consent.
- The mechanism is online and freshness-bounded, not a durable mailbox. Store
  loss or retention expiry causes retry/backoff, not authority rollback.
- Two runtimes can still initiate at nearly the same time. Each actor bounds
  itself to one serialized workflow and retries after failure; roster-wide
  cross-device scheduling is deferred to the next slice.
- Foreground runtime shutdown stops all discovery. No OS service, Task
  Scheduler registration, autostart, cover traffic or multi-hop anonymity is
  introduced.

## 6. Compatibility and next work

Connection ticket v10, announcement envelope, event/ratchet formats and wire
ALPN v8 remain unchanged. Runtime IPC advances from v13 to v14, so runtime, CLI
adapter and desktop must be upgraded together. Existing M0.9.39 manually
configured ticket paths continue to work.

The next slice should replace per-recipient manual configuration with a single
bounded roster-wide own-device availability policy. It must reconcile policy
creation and removal with live Root roster changes, preserve explicit network
permissions, and avoid parallel publish/fetch/push storms. Cross-device
equivocation evidence and blind mailbox delivery remain separate problems.
