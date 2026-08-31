# Kilogram

Kilogram is an experimental peer-to-peer messenger intended to provide
end-to-end encryption. The protocol is currently at the architecture,
authorization, and transport proof-of-concept stage. Do not use it for
sensitive communication.

The project goals and draft architecture are documented in
[`docs/RFC-0001-core-architecture.md`](docs/RFC-0001-core-architecture.md).
The implemented Account Root / device authority slice is specified in
[`docs/RFC-0002-account-device-authority.md`](docs/RFC-0002-account-device-authority.md).
The implemented M0 conversation authorization slice is specified in
[`docs/RFC-0003-conversation-membership.md`](docs/RFC-0003-conversation-membership.md).
The implemented pairwise HPKE payload spike is specified in
[`docs/RFC-0004-pairwise-hpke-payload.md`](docs/RFC-0004-pairwise-hpke-payload.md).
The local-history separation required before a ratchet is specified in
[`docs/RFC-0005-local-encrypted-history-projection.md`](docs/RFC-0005-local-encrypted-history-projection.md).
The authenticated persistent ratchet spike is specified in
[`docs/RFC-0006-pairwise-double-ratchet.md`](docs/RFC-0006-pairwise-double-ratchet.md).
The signed account device-list and ratchet fan-out slice is specified in
[`docs/RFC-0007-multi-device-ratchet-fanout.md`](docs/RFC-0007-multi-device-ratchet-fanout.md).
The authenticated same-account history recovery slice is specified in
[`docs/RFC-0008-authenticated-history-rewrap.md`](docs/RFC-0008-authenticated-history-rewrap.md).
The authenticated prekey-pool and concurrent-initiation slice is specified in
[`docs/RFC-0009-authenticated-prekey-pools.md`](docs/RFC-0009-authenticated-prekey-pools.md).
The current two-network Windows procedure is in
[`docs/M0.3-CROSS-NETWORK-TEST-RU.md`](docs/M0.3-CROSS-NETWORK-TEST-RU.md), and
the pause/reconnect procedure is in
[`docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](docs/M0.4-RESUMABLE-SYNC-TEST-RU.md).

## Current milestone: M0.7.6 authenticated prekey pools — complete

Plaintext `Text` events and static peer HPKE boxes no longer exist in the
replicated protocol. Account Root now signs one complete canonical device list
at each authority revision. Ticket v9 combines that list with exactly one
fresh device-signed Olm prekey pool per authorized device. Every pool contains
16 independently consumable keys by default, a monotonic generation and
sequence range, and signed publication/expiry times. One `RatchetText` v5
event carries a sorted recipient table with a separate persistent pairwise
ratchet ciphertext for every device in the peer account. A connected device
decrypts its own slot; another offline device can later receive the same event
through sync and decrypt its different slot.

A device enrolled after an event was created has no old ratchet slot by design.
M0.7.5 lets a live device of the same account export an explicit canonical
text-event range to that new device. The source signs the complete source
inventory digest, range, original `AuthorizedEvent` values and HPKE ciphertexts
addressed to the recipient certificate. Import verifies account/device
authority and conversation membership, then stores unchanged events plus an
immutable local projection v2 with durable source provenance. Partial bundles
are explicitly marked incomplete relative to the source inventory.

The sender-readable copy is no longer part of the replicated event. Every
endpoint writes a separate immutable `STATE_DIR/local-messages/*.local-text`
projection encrypted to its own device key. A sender creates it from the text it
authored; a recipient creates it only after decrypting and authenticating the
ratchet message. `history` reads this local projection, while sync transfers
only ratchet ciphertext events and public authorization proofs. This allows
the ratchet to delete consumed message keys without losing the user's local
chat history or retaining a static sender box that defeats forward secrecy.

Olm account and the bounded active/retained session set per peer Device ID are
encrypted before being stored under `STATE_DIR/ratchet`. The development pickle
key is kept next to them, so this is a structural persistence boundary rather
than a protected keystore. The root-signed DeviceCertificate v2 encryption key
is now used for the local projection, not for replicated message decryption.

M0.7.6 persists the maximum prekey generation seen for every peer device and
rejects rollback or same-generation equivocation. `connect` and `sync` observe
this authenticated directory directly from the ticket. Crossed first messages
may temporarily create two Olm sessions for one Device-ID pair; both endpoints
choose the same active session by session ID and retain the loser only for
already in-flight messages.

This is still a narrow integration spike. It does not yet provide a global DHT
or gossip freshness proof, atomic remote prekey reservation, protected local
key storage, PQXDH, cross-account recovery, or automatic history-transfer
transport. Losing every readable projection still cannot be repaired from old
ciphertext with only the device signing key.
Do not use it for sensitive communication.

An owner Account Root now signs a complete, canonical, add-only conversation
membership snapshot. Each participant installs the same public snapshot as a
local trust anchor. Devices reject older revisions, conflicting state at one
revision, a different owner, and updates that remove an existing member.

Delivery and synchronization no longer accept a bare `SignedEvent`. An
`AuthorizedEvent` carries the event, the author's root-signed device
certificate, and the author's complete authority snapshot. Every receiver
checks conversation membership, account authority, device capability and
revocation state, and finally the event signature. Direct delivery also binds
the author to the already authorized account/device session. The original event
remains content-addressed; its authorization proof is stored in a mandatory
immutable sidecar.

The `kilogram-identity` crate now separates an account's Ed25519 root authority
from per-installation device keys. The root issues capability-bearing device
certificates and permanent root-signed revocations. Public verification binds a
certificate to the expected Account ID, checks the required `sign-events` and
`sync-history` capabilities, and rejects a revoked device key even if a later
certificate is issued for it.

Ticket v9 embeds the listener's root-signed certificate, complete signed device
list/authority snapshot and one fresh device-signed prekey pool for every listed
device. It authorizes one requester Account ID rather than one hard-coded
device. Before any event or
inventory is sent, the requester presents its certificate, authority snapshot,
and a device-signed proof bound to the listener's current Endpoint ID. Both
peers persist the maximum seen snapshot revision per account. Older state is
rejected as rollback; conflicting signed state at the same revision is rejected
as root equivocation. A revoked device is rejected on the authorization stream.

The development CLI covers the authority lifecycle with `account-create`,
`account-show`, `account-snapshot`, `account-device-list`, `device-enroll`,
`device-authority-update`, `device-authorize`, and `device-revoke`.
Conversation lifecycle commands are `conversation-create`,
`conversation-member-add`, and `conversation-membership-install`.
`listen` now uses `--allow-account`; `connect` and `sync` require
`--expect-account`. Ticket/session snapshots replace the former manually copied
`--peer-revocation-file` lists.

M0.4 resumable synchronization remains complete. Its tested transport and
storage behavior is summarized below.

The CLI exchanges a signed Double Ratchet text event and a signed acknowledgement over an
authenticated Iroh/QUIC connection. Application-level device identities are
persistent and deliberately separate from ephemeral Iroh transport identities.
Every verified event is also persisted locally before the corresponding send
or acknowledgement. Repeated writes are idempotent and stored corruption is
detected when history is read.

Certified devices of the allowed account can reconcile bounded batches in both
directions until their event logs converge. The inventory is signed by the
requesting application device and bound to the listener's current Iroh Endpoint
ID. The listener signs its diff with the certified device key embedded in the
ticket. A newly enrolled device can authenticate and sync the immutable log
without first authoring a synthetic event. Old ratchet text requires a prior
authenticated history rewrap from a live device because the new device was not
one of the original ciphertext recipients.

The transport-independent reconciliation state machine lives in
`kilogram-session`; the Iroh ALPN and typed stream framing live in
`kilogram-transport-iroh`. The CLI only orchestrates these layers. This is the
first concrete transport-replacement boundary, not yet the final transport API.

After a delivery or synchronization exchange, both peers wait up to three
seconds for Iroh relay-to-direct migration and print `transport_path` (`direct`,
`relay`, `custom`, or `unknown`), the selected remote transport address, RTT,
and number of open paths. These development diagnostics made the two-host LAN
test distinguish a real direct path from a successful relay fallback.

The listener signs one of three application route policies into ticket v9:

- `auto` accepts Iroh's selected direct or relay path;
- `direct-only` permits relay-assisted connection establishment and NAT traversal,
  but withholds Kilogram protocol frames until a direct IP path is selected;
- `relay-only` disables all IP transports at both endpoints, so the ticket and
  the established connection contain only a relay path.

Connection establishment, route selection, stream opening, and framed wire I/O
have bounded timeouts with operation-specific diagnostics. M0.3 external tests
verified direct LAN, a valid no-direct result between home and cellular NAT,
automatic relay fallback, and strict relay-only delivery/sync through a pinned
`aps1` public relay.

M0.4 adds a clean `SyncPause` / `SyncPaused` boundary after completed bounded
rounds. The full-ID M0 profile resumes from the durable event set with a fresh
session-bound inventory rather than reusing an old transport authorization.
An external two-host test paused after 64/64 events on direct LAN, moved Bob to
cellular, created a new relay-only session, and transferred only the remaining
6/6 events. Both verified histories then contained the same 142 events and
causal frontier.

The listener treats failed QUIC Initial/handshake attempts as recoverable network
input and keeps accepting. This is required for public UDP endpoints because Iroh
documents that retransmitted or unrelated datagrams may fail early authentication.

The file event store canonicalizes its root before deriving content-addressed
event paths. On Windows this produces verbatim absolute paths and avoids the
legacy 260-character limit even when `LongPathsEnabled=1` is insufficient for a
specific atomic-file operation. A Windows regression test and a full release
recovery smoke cover event paths longer than 260 characters.

M0.2 is complete on two physical Windows PCs. A signed text event and its
signed acknowledgement travelled over a direct LAN path with approximately
1 ms RTT. A second client store containing only Alice's device key recovered
both events from Bob in one sync round, verified them on read, and reconstructed
the acknowledgement as the single causal frontier.

This remains a development prototype. It now implements persistent pairwise
Olm sessions, bounded signed prekey pools, crossed-session resolution,
account-wide ciphertext fan-out and same-account history rewrap, but does **not**
yet implement a production-audited pairwise protocol, protected local key
storage, global device/prekey discovery, seed phrases/root
recovery, member removal, or production groups. Account Root and local
device secrets remain unencrypted in their explicitly selected directories.
A snapshot proves state at its signed revision and prevents
rollback after a newer revision has been observed. The protocol does not yet
discover whether newer account or membership state exists at first contact.
Do not use it for sensitive communication.

Create separate Account Roots and enroll Alice and Bob devices locally. Preserve
the two printed Account IDs:

```powershell
cargo run -p kilogram-cli -- account-create --account-dir .tmp/alice-account
cargo run -p kilogram-cli -- account-create --account-dir .tmp/bob-account
cargo run -p kilogram-cli -- device-enroll `
  --account-dir .tmp/alice-account `
  --state-dir .tmp/alice `
  --certificate-file .tmp/alice.cert
cargo run -p kilogram-cli -- device-enroll `
  --account-dir .tmp/bob-account `
  --state-dir .tmp/bob `
  --certificate-file .tmp/bob.cert
cargo run -p kilogram-cli -- account-device-list `
  --account-dir .tmp/bob-account `
  --device-certificate-file .tmp/bob.cert `
  --device-list-file .tmp/bob-devices.snapshot
```

Create one membership owned by Alice, then install the same public snapshot on
both devices:

```powershell
cargo run -p kilogram-cli -- conversation-create `
  --account-dir .tmp/alice-account `
  --conversation m0-local-smoke `
  --member-account <BOB_ACCOUNT_ID> `
  --membership-file .tmp/m0-local-smoke-v1.membership
cargo run -p kilogram-cli -- conversation-membership-install `
  --state-dir .tmp/alice `
  --membership-file .tmp/m0-local-smoke-v1.membership
cargo run -p kilogram-cli -- conversation-membership-install `
  --state-dir .tmp/bob `
  --membership-file .tmp/m0-local-smoke-v1.membership
```

Start Bob's listener with Alice's Account ID authorized:

```powershell
cargo run -p kilogram-cli -- listen `
  --state-dir .tmp/bob `
  --allow-account <ALICE_ACCOUNT_ID> `
  --device-list-file .tmp/bob-devices.snapshot `
  --ticket-file .tmp/listener.ticket `
  --route-policy auto
```

For every additional device in Bob's signed list, export its current public
pool and repeat `--peer-prekey-pool-file <DEVICE.prekeys>` on the listener:

```powershell
cargo run -p kilogram-cli -- ratchet-prekey-pool `
  --state-dir .tmp/bob-2 `
  --pool-file .tmp/bob-2.prekeys
```

The listener adds its own current pool automatically and rejects incomplete,
duplicate, expired, rolled-back, or mismatched device coverage. A pool contains
16 OTKs and is valid for seven days by default; `--refresh` publishes the next
generation explicitly.

In another terminal, connect and send a message:

```powershell
cargo run -p kilogram-cli -- connect `
  --state-dir .tmp/alice `
  --ticket-file .tmp/listener.ticket `
  --expect-account <BOB_ACCOUNT_ID> `
  --message "hello"
```

The listener exits after acknowledging one event. Reusing a state directory
preserves the application device ID and advances its author sequence across
restarts. Do not run two processes against the same state directory: the M0
sequence allocator is intentionally single-process only.

Inspect and cryptographically verify the local history after either process has
exited:

```powershell
cargo run -p kilogram-cli -- history --state-dir .tmp/alice
cargo run -p kilogram-cli -- history --state-dir .tmp/bob
```

Events are stored as immutable content-addressed files beneath
`STATE_DIR/events`, together with immutable authorization sidecars. Readable
message bodies are separate encrypted local-only projections beneath
`STATE_DIR/local-messages`; they are never part of sync. The `history` command
verifies the event and authorization against the installed membership, then
opens the matching local projection and prints the current causal frontier. Its
file-order output is deterministic but is not yet a chat timeline. M0.6.1 state
without authorization sidecars and pre-M0.7.4 single-recipient events are
intentionally not migrated. Direct local projections v1 remain readable;
history-rewrapped projections use v2 and retain signed source provenance.

To recover old text history on a newly enrolled device of the same account,
publish a fresh root-signed list containing both devices. On the live source:

```powershell
cargo run -p kilogram-cli -- history-rewrap-export `
  --state-dir .tmp/bob-1 `
  --conversation m0-local-smoke `
  --device-list-file .tmp/bob-devices.snapshot `
  --recipient-device <BOB_2_DEVICE_ID> `
  --range-start 0 `
  --count 256 `
  --bundle-file .tmp/bob-2-history.rewrap
```

After installing the same conversation membership on the new device:

```powershell
cargo run -p kilogram-cli -- history-rewrap-import `
  --state-dir .tmp/bob-2 `
  --conversation m0-local-smoke `
  --bundle-file .tmp/bob-2-history.rewrap
```

`source_inventory_complete=true` means that this bundle covers the source
device's entire signed text inventory. It is not a global completeness proof if
the source itself has a stale or incomplete replica. Bundles contain no ratchet
session keys and can be followed by ordinary sync for remaining events.

To synchronize missing events, start `listen` again on one device and run:

```powershell
cargo run -p kilogram-cli -- sync `
  --state-dir .tmp/alice `
  --ticket-file .tmp/listener.ticket `
  --expect-account <BOB_ACCOUNT_ID>
```

Each round accepts at most 4,096 inventory IDs and transfers at most 64 events
in each direction. One `sync` invocation automatically continues for up to 64
rounds on the same Iroh connection and finishes with
`sync_more_available=false`. This full-ID inventory is an M0 mechanism, not the
future compact Merkle summary: once a local conversation exceeds 4,096 events,
this development profile must be replaced rather than treated as scalable sync.

For an interruption/reconnect test, pass `--max-rounds 1`. If more events
remain, both peers finish the completed round cleanly with `status=paused` and
`sync_resume_checkpoint=event-store`. Restart the listener, transfer its new
ticket, and run `sync` again. A fresh session-bound inventory is signed, while
the durable event stores ensure that only still-missing events are transferred.
An explicit portable cursor is intentionally deferred until full-ID inventory
is replaced by a compact authenticated summary. The development-only
`seed-history` command creates ratchet-encrypted fixture events and requires the
other account's public certificate, root-signed device list, and signed pool
through `--peer-certificate-file`, `--peer-device-list-file`, and
`--peer-prekey-pool-file`. Generate the latter offline with
`ratchet-prekey-pool --state-dir <PEER_STATE> --pool-file <FILE>`;
see [`docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](docs/M0.4-RESUMABLE-SYNC-TEST-RU.md).

The connection ticket is public addressing and authorization data: it contains
the listener's Iroh address, root-signed public device certificate, complete
root-signed device list/authority snapshot, one fresh device-signed prekey pool for
every listed device, allowed requester Account ID, and route policy.
The certified listener device signs the whole mapping, so tampering is detected
before a connection or inventory is sent. Possession of the ticket alone is
insufficient: the requester must present a certificate and authority snapshot
for the allowed account and prove possession of its device key.
The ticket contains neither the Iroh endpoint secret nor any application secret.
Ticket JSON, Postcard messages, and development conversation-label derivation
remain provisional M0 choices. Ticket v9 intentionally does not decode v1-v8
tickets; restart the listener to generate a ticket matching this build. See
[`RFC-0002`](docs/RFC-0002-account-device-authority.md) for snapshot and
first-contact freshness boundaries.

## Development checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```
