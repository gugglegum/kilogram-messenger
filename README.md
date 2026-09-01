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
The crash-consistent local state transaction is specified in
[`docs/RFC-0010-crash-consistent-local-state.md`](docs/RFC-0010-crash-consistent-local-state.md).
The consent-gated network history recovery flow is specified in
[`docs/RFC-0011-network-history-rewrap.md`](docs/RFC-0011-network-history-rewrap.md).
The resumable authenticated history-recovery flow is specified in
[`docs/RFC-0012-resumable-history-recovery.md`](docs/RFC-0012-resumable-history-recovery.md).
The encrypted transactional shadow-vault migration is specified in
[`docs/RFC-0013-encrypted-transactional-state-vault.md`](docs/RFC-0013-encrypted-transactional-state-vault.md).
The recoverable live shadow dual-write is specified in
[`docs/RFC-0014-recoverable-shadow-dual-write.md`](docs/RFC-0014-recoverable-shadow-dual-write.md).
The typed incremental encrypted mirror is specified in
[`docs/RFC-0015-typed-incremental-shadow-repositories.md`](docs/RFC-0015-typed-incremental-shadow-repositories.md).
The first immutable vault primary-read canary is specified in
[`docs/RFC-0016-immutable-vault-primary-read-canary.md`](docs/RFC-0016-immutable-vault-primary-read-canary.md).
The vault-primary history-rewrap source cutover is specified in
[`docs/RFC-0017-vault-primary-history-rewrap.md`](docs/RFC-0017-vault-primary-history-rewrap.md).
The command-local vault-primary sync overlay is specified in
[`docs/RFC-0018-command-local-sync-read-overlay.md`](docs/RFC-0018-command-local-sync-read-overlay.md).
The vault-primary transaction checkpoint is specified in
[`docs/RFC-0019-vault-primary-transaction-checkpoint.md`](docs/RFC-0019-vault-primary-transaction-checkpoint.md).
The typed journal delta and mutable sequence canary are specified in
[`docs/RFC-0020-typed-journal-delta-and-mutable-sequence.md`](docs/RFC-0020-typed-journal-delta-and-mutable-sequence.md).
The DB-primary ratchet workspace and registered append contract are specified in
[`docs/RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md`](docs/RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md).
Repository-owned write receipts and the authenticated incremental manifest
index are specified in
[`docs/RFC-0022-repository-write-receipts-and-manifest-index.md`](docs/RFC-0022-repository-write-receipts-and-manifest-index.md).
The DB-primary authority/contact trust repository is specified in
[`docs/RFC-0023-db-primary-trust-repository.md`](docs/RFC-0023-db-primary-trust-repository.md).
The versioned protected vault-key envelope is specified in
[`docs/RFC-0024-protected-vault-key-provider.md`](docs/RFC-0024-protected-vault-key-provider.md).
Portable vault-key recovery and the external rollback witness are specified in
[`docs/RFC-0025-portable-vault-key-recovery-and-rollback-witness.md`](docs/RFC-0025-portable-vault-key-recovery-and-rollback-witness.md).
DB-primary and DB-only device identity are specified in
[`docs/RFC-0026-db-primary-device-identity.md`](docs/RFC-0026-db-primary-device-identity.md)
and [`docs/RFC-0027-db-only-device-identity-layout.md`](docs/RFC-0027-db-only-device-identity-layout.md).
The bounded multi-page recovery coordinator is specified in
[`docs/RFC-0028-bounded-multi-page-history-recovery-session.md`](docs/RFC-0028-bounded-multi-page-history-recovery-session.md).
The compact signed recovery device link is specified in
[`docs/RFC-0029-signed-history-recovery-device-link.md`](docs/RFC-0029-signed-history-recovery-device-link.md).
The bounded PNG/JPEG QR ceremony is specified in
[`docs/RFC-0030-bounded-history-recovery-qr-ceremony.md`](docs/RFC-0030-bounded-history-recovery-qr-ceremony.md).
The opt-in authenticated LAN recovery discovery slice is specified in
[`docs/RFC-0031-authenticated-lan-recovery-discovery.md`](docs/RFC-0031-authenticated-lan-recovery-discovery.md).
The current two-network Windows procedure is in
[`docs/M0.3-CROSS-NETWORK-TEST-RU.md`](docs/M0.3-CROSS-NETWORK-TEST-RU.md), and
the pause/reconnect procedure is in
[`docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](docs/M0.4-RESUMABLE-SYNC-TEST-RU.md).

## Current milestone: M0.9.4 authenticated LAN recovery discovery — complete

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

M0.7.7 serializes every CLI command that uses a device `STATE_DIR` with an
exclusive OS file lock. A prepared write-ahead journal snapshots mutable
ratchet and author-sequence state and records the baseline of the immutable
event, local-projection, and history-rewrap stores. Delivery, synchronization,
history seeding/import, and prekey rotation either commit all related files or
restore the previous ratchet/sequence and remove only newly created immutable
files on error or the next startup. Network frames are sent only after the
corresponding local transaction commits.

M0.7.8 moves history rewrap from manual file exchange into the authenticated
Iroh device session. The recipient signs a session-bound request for one
bounded range; the source must explicitly approve the same account device,
conversation, range, and independently compared 12-digit SAS. The source signs
the response over the exact request and encrypted bundle. Import stores events,
local projections, the original bundle, and network transfer provenance in one
crash-consistent transaction. Local reconciliation merges ranges per signed
source claim and reports `incomplete`, `single-source`, `agreed`, or `divergent`
while always stating that global completeness is not proven.

M0.7.9 adds a recipient-signed append-only recovery checkpoint chain. Each
fresh authenticated session fetches the next bounded page from one explicitly
selected source, pins the first source inventory claim, and atomically commits
the imported page with the next checkpoint. A completed plan retries without a
network connection.

M0.9.1 removes the per-page reconnect ceremony without changing the trust
model. One explicitly approved source listener now serves up to 64 contiguous
pages over one authenticated Iroh connection. `history-recovery-resume` commits
every page and its signed checkpoint atomically before requesting the next one,
and `--max-pages` provides a hard per-session resource bound. A disconnect or
limit leaves a durable plan that resumes from the exact next page with a fresh
ticket.

M0.9.2 replaces that manual recovery-ticket ceremony with a compact,
recipient-specific signed URI. It binds the source endpoint and certificate,
root-signed device list, exact recipient, conversation, approved range, page
size, route policy and a short expiry without embedding the large prekey pools.
`history-recovery-link-inspect` verifies it offline, while
`history-recovery-link-accept` requires the exact local recipient and explicit
SAS confirmation before opening a connection. The link is public bootstrap
metadata, not a bearer capability; the listener still authenticates the device
and independently enforces its local consent. The implemented payload is
QR-ready; M0.9.3 adds the file-based image round-trip, while live capture and
wide-area automatic discovery remain future client work. M0.9.4 now covers an
explicit local-network discovery mode.

M0.9.3 implements that QR image boundary. A listener may directly publish a
no-clobber PNG, and a separate command can render an existing verified link.
Offline inspect and explicit accept decode exactly one QR from a bounded PNG or
JPEG before running the same signed-link verifier. File size, dimensions,
decoded payload and accepted formats are bounded; multiple QR codes are
rejected as ambiguous. This is file-based CLI scanning, not yet a live camera,
clipboard, GUI or OS deep-link integration.

M0.9.4 adds explicit opt-in discovery of the same signed, expiring,
recipient-specific descriptors on an IPv4 local-network multicast group. The
recipient verifies the source/root signatures, exact local device and
conversation, expiry, membership and local authority freshness while making no
Iroh connection. A unique candidate may be saved as a no-clobber link file, but
discovery never grants consent: the user must still compare the displayed SAS
and invoke the separate accept command. LAN observers can see the public
descriptor metadata, so publication remains disabled by default and is not a
global, anonymous, or privacy-preserving discovery service.

M0.8.1 adds a reversible encrypted shadow snapshot of the entire device state.
`state-vault-migrate` publishes encrypted records and a keyed manifest in one
immediate-durability `redb` transaction. `state-vault-verify` authenticates the
complete vault and compares it with the retained legacy tree;
`state-vault-restore` reconstructs an exact snapshot only in a new directory.
Paths and contents are encrypted with XChaCha20-Poly1305, while keyed BLAKE3
identifiers avoid plaintext path keys in the database.

M0.8.2 mirrors every live device-state CLI command once a vault is initialized.
Before the command, an immediate-durability transaction stores a keyed,
authenticated intent bound to the exact active generation and snapshot. After
the command—even when it reports a later network error—the actually committed
legacy tree is atomically mirrored and the intent removed. A restart may repair
drift only when that valid intent exists; unexplained drift remains fail-closed.
Read-only commands keep the generation unchanged, while changed state advances
it monotonically. `state-vault-recover` exposes the same constrained recovery
explicitly.

M0.8.3 replaces the full encrypted rewrite with an atomic per-record delta.
Changed and new records are encrypted and upserted, removed records are
deleted, and unchanged ciphertext values stay untouched. Nine typed state
categories expose exact DB/legacy shadow-read inventory through
`state-vault-shadow-read`; any per-path mismatch fails closed. Live commands
also report upsert/remove/unchanged counters. Encryption and DB mutations are
now proportional to the delta, while the conservative full scan/decrypt/compare
remains proportional to total state.

M0.8.4 makes the read-only `history` command the first real DB-primary canary.
When a vault is initialized, event, authorization, and local-projection bytes
come from an owned authenticated vault snapshot. The complete retained legacy
tree must still match exactly before those bytes are returned. Strict read
adapters reapply event signatures, IDs, Account Root and membership
authorization, writer sequencing, and projection binding. A drifted or invalid
initialized vault fails closed; it never silently downgrades history to the
filesystem. States that have never been migrated remain legacy-compatible.

M0.8.5 applies the same fail-closed immutable read-set to both history-rewrap
source paths. Manual export captures authenticated vault events/projections
before local authority updates; an explicitly approved network listener
captures the same owned snapshot before transport-side authority, prekey, and
request processing. Bundle construction now depends only on read traits, and
listener diagnostics expose the physical primary source. The event read trait
also supports authorized inventory and events-by-ID for the next sync stage.
Mixed read/write sync remains legacy-primary until a command-local overlay can
make newly committed events and projections visible without weakening crash
recovery.

M0.8.6 moves sync inventory and events-by-ID reads onto an authenticated
immutable vault base plus a command-local committed overlay. Incoming event and
local-projection batches are validated and staged first, written through the
existing crash-consistent filesystem transaction, and published to the overlay
only after that transaction commits. Subsequent bounded rounds therefore see
records accepted earlier in the same connection. Both sync peers report their
physical primary source and overlay sizes; an initialized invalid or drifted
vault still fails closed without filesystem fallback.

M0.8.7 makes the encrypted vault the irreversible commit point for every
operation already covered by the crash-consistent filesystem journal. The
legacy tree is first used as staging; one immediate redb transaction publishes
the encrypted delta, manifest, generation, rotated mirror intent, and an
authenticated primary-shadow intent. Only then is the filesystem journal
committed and verified as an exact shadow. A crash in between restores the
legacy shadow from the committed vault, including coupled ratchet and sequence
state. Delivery and sync frames are emitted only after this barrier.

M0.8.8 replaces the live full-filesystem checkpoint with a typed delta derived
from the active crash journal. Ratchet and sequence changes, new append-only
events/projections/recovery records, and permitted removals are validated by
canonical kind and path before one direct vault transaction. Existing history
payloads are no longer reread from filesystem while constructing the
pre-commit delta; the post-commit exact shadow confirmation still scans them.
Author sequence is the first mutable DB-primary adapter: an initialized vault
supplies the authenticated current counter, while `next-sequence` is retained
only as transactional compatibility shadow. Sequence, ratchet, event, and
projection still cross the same vault-primary commit barrier.

Authority and contact writers have not yet moved into `StateTransaction`, so
the coordinator also folds only the bounded trust namespaces into that same
commit and rejects removal of an already committed trust record. This is a
compatibility bridge, not a DB-primary trust repository.

M0.8.9 makes ratchet reads DB-primary at every transaction boundary. The
authenticated vault snapshot is installed into a crash-journaled staging
workspace before `RatchetState` opens it; commit compares that workspace with
the DB baseline, and rollback restores a durable DB-primary backup. Sequence
uses the same rollback rule. Append-only writers now register exact canonical
paths, replacing the second history-directory walk with a typed write-set.
Committed append records cannot be removed or modified in place.

M0.8.10 moves append-only path ownership into the repositories that actually
write each record. Event, authorization, local-projection, history-rewrap,
transfer, and recovery-checkpoint writes return exact receipts; the transaction
coordinator validates those absolute paths against its canonical state root and
no longer reconstructs repository extensions in the CLI. Vault schema v2 adds
an encrypted authenticated index of canonical path, payload length, and payload
hash. A normal typed commit updates this index and the keyed snapshot manifest
from the journal delta without enumerating or decrypting unchanged DB payload
records. Existing schema-v1 vaults are verified and rebuilt once, then continue
on the incremental path. Diagnostics expose the index mode and exact number of
DB payload records loaded by the commit.

M0.8.11 removes the bounded filesystem trust ingress from normal direct
commits. Certificate, own/peer authority snapshot, and conversation-membership
reads now come from an authenticated DB-primary trust repository whenever a
vault exists. Schema-v2 reads decrypt only selected trust payload records after
verifying the encrypted manifest index. Every trust mutation explicitly opens
a crash-journaled workspace hydrated from the DB baseline; typed trust delta is
committed to the vault before the retained filesystem shadow. Rollback and
next-start recovery restore DB-authoritative trust bytes, and an unregistered
filesystem trust change can no longer become an implicit authority update.

M0.8.12 replaces the adjacent raw 256-bit vault master key with a versioned
key envelope. Windows builds protect the key with DPAPI CurrentUser scope and
persist only the protected blob; a fresh key is never written to disk in
plaintext. Existing 32-byte development key files are detected and atomically
rewrapped without re-encrypting the vault. Corrupt envelopes, unavailable
providers, and DPAPI failures are fail-closed before the database is opened.
Non-Windows builds retain an explicitly reported plaintext-development
provider until platform keystore/passphrase support is implemented.

M0.8.13 adds an explicit portable recovery package. The vault master key is
wrapped with Argon2id and XChaCha20-Poly1305 in an external no-clobber file,
bound to an authenticated schema/generation/snapshot witness, and verified
against the database before a local provider envelope is installed.

M0.8.14 makes the immutable device signing and encryption identity DB-primary.
Once a vault exists, all production commands decrypt exactly those selected
records from the authenticated manifest index and never fall back to raw
filesystem keys.

M0.8.15 introduces vault schema v3 as the DB-only identity layout marker. The
upgrade commits the new schema and generation before deleting matching
`device-secret.key` and `device-encryption-secret.key` files. Exact shadow
checks merge the immutable DB identity with the remaining filesystem shadow,
and primary-shadow crash recovery no longer recreates either raw key. A
mismatched reappearing copy is rejected rather than imported.

This is still a narrow integration spike. The vault is now primary for the
implemented immutable history, ratchet, sequence, and authority/contact trust
paths, and device identity has been removed from the normal retained shadow.
Other state still has a filesystem compatibility shadow. The initial
crash-journal baseline, outer
pre-command equivalence gate, and final exact shadow confirmation still perform
full-state work. The indexed commit itself avoids the active DB payload scan,
but index decode/re-encode is `O(record count)` metadata and the whole command
path is therefore not yet `O(changed)`.
The Windows vault key is OS-protected and has an explicit passphrase recovery
export, but the live envelope is still bound to its Windows user/machine and
the external package is not a monotonic service. Account-root and ratchet
pickle keys are not all protected by the same provider. The prototype does not
yet provide a
global DHT or gossip freshness proof, atomic remote prekey reservation,
portable protected local key storage, full DB-primary repository cutover,
PQXDH, or
cross-account recovery. Losing every readable projection
still cannot be repaired from old ciphertext with only the device signing key.
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
