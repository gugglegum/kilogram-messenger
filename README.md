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
The consent-bound retry coordinator is specified in
[`docs/RFC-0032-consent-bound-history-recovery-scheduler.md`](docs/RFC-0032-consent-bound-history-recovery-scheduler.md).
Persistent signed scheduler state is specified in
[`docs/RFC-0033-persistent-history-recovery-scheduler-state.md`](docs/RFC-0033-persistent-history-recovery-scheduler-state.md).
The first native Windows recovery platform context is specified in
[`docs/RFC-0034-windows-recovery-platform-context.md`](docs/RFC-0034-windows-recovery-platform-context.md).
The bounded event-driven Windows recovery worker is specified in
[`docs/RFC-0035-bounded-windows-recovery-worker.md`](docs/RFC-0035-bounded-windows-recovery-worker.md).
The first long-lived multi-session messaging runtime is specified in
[`docs/RFC-0036-long-lived-messaging-runtime.md`](docs/RFC-0036-long-lived-messaging-runtime.md).
The persistent signed contact and durable runtime outbox are specified in
[`docs/RFC-0037-persistent-runtime-contact-and-outbox.md`](docs/RFC-0037-persistent-runtime-contact-and-outbox.md).
The authenticated local runtime actor API is specified in
[`docs/RFC-0038-authenticated-local-runtime-ipc.md`](docs/RFC-0038-authenticated-local-runtime-ipc.md).
The first desktop client over that API is specified in
[`docs/RFC-0039-minimal-desktop-runtime-client.md`](docs/RFC-0039-minimal-desktop-runtime-client.md).
The actor-owned chat list and paginated local history are specified in
[`docs/RFC-0040-actor-owned-chat-read-model.md`](docs/RFC-0040-actor-owned-chat-read-model.md).
Desktop contact onboarding and foreground runtime lifecycle are specified in
[`docs/RFC-0041-desktop-contact-onboarding-and-runtime-lifecycle.md`](docs/RFC-0041-desktop-contact-onboarding-and-runtime-lifecycle.md).
Desktop runtime-profile editing and change notifications are specified in
[`docs/RFC-0042-desktop-runtime-setup-and-change-notifications.md`](docs/RFC-0042-desktop-runtime-setup-and-change-notifications.md).
Desktop creation of a recoverable first account/device is specified in
[`docs/RFC-0043-desktop-first-account-bootstrap.md`](docs/RFC-0043-desktop-first-account-bootstrap.md).
Existing-account enrollment and recipient-encrypted authority transfer are
specified in
[`docs/RFC-0044-existing-account-device-link.md`](docs/RFC-0044-existing-account-device-link.md).
The desktop enrollment and multi-source recovery orchestration boundary is
specified in
[`docs/RFC-0045-desktop-device-link-and-recovery-wizard.md`](docs/RFC-0045-desktop-device-link-and-recovery-wizard.md).
Portable Account Root authority recovery and its desktop ceremony are specified
in [`docs/RFC-0046-account-root-authority-recovery.md`](docs/RFC-0046-account-root-authority-recovery.md)
and [`docs/RFC-0047-desktop-account-root-recovery.md`](docs/RFC-0047-desktop-account-root-recovery.md).
Recovery freshness modes and the exact-export lifecycle are specified in
[`docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md`](docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md).
The current two-network Windows procedure is in
[`docs/M0.3-CROSS-NETWORK-TEST-RU.md`](docs/M0.3-CROSS-NETWORK-TEST-RU.md), and
the pause/reconnect procedure is in
[`docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](docs/M0.4-RESUMABLE-SYNC-TEST-RU.md).

## Current milestone: M0.9.22 current-device recovery quorum core — complete

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

M0.9.5 makes retries possible without turning discovery into implicit trust. An
exact recipient explicitly confirms SAS once and signs a short-lived local plan
that binds the source/device list, conversation, range, page size, route, and
network/power policy. A bounded runner may then discover a fresh endpoint after
the source restarts, but only an otherwise exact matching descriptor can resume
the existing signed checkpoint chain. Mobile and unknown networks are denied by
default. In that milestone the CLI accepted caller-supplied network/power
context and released the device-state lock during discovery and backoff;
M0.9.7 replaces the default context path while scheduling remains platform work.

M0.9.6 makes the retry state survive process restart. The exact recipient signs
an append-only per-plan state chain containing monotonic generations, attempt
leases, counters, the observed clock high-water mark and the next deadline.
Failures use bounded exponential equal-jitter backoff; an immediate restart
before the signed deadline performs no discovery. A terminal signed cancel
record prevents later connection for that Plan ID, while a completed recovery
is reconciled with the existing signed checkpoint chain. The state is mirrored
through the encrypted vault when enabled. Whole-directory rollback still needs
an external witness. OS wakeups and live network/power change subscriptions
remain platform work.

M0.9.7 replaces the default caller-supplied recovery context with a native
Windows snapshot. It classifies exact Ethernet/Wi-Fi/WWAN interfaces, connection
cost, metering, roaming, data-limit signals, power supply, battery and Energy
Saver without exposing profile names, SSIDs or adapter identifiers. A VPN tunnel
is never treated as Ethernet by guess: the adapter may use one unambiguous active
physical profile underneath it, while absent or conflicting evidence becomes
`unknown`. Metered or roaming profiles use the already signed `mobile` policy
bucket. The old network/power arguments remain an all-or-none development
override; other platforms fail closed until they gain their own adapter.
Optional OS background registration remained future work at that boundary;
M0.9.8 adds the bounded process and change-event wakeups without installing an
OS task.

M0.9.8 adds `history-recovery-plan-watch`, a bounded foreground worker ready for
optional later Windows background integration. It waits for the
recipient-signed retry deadline or native WinRT network/power change events,
polls the signed scheduler chain for cross-process cancellation without holding
the state lock, and stops at explicit runtime and wakeup limits. The runner
re-probes policy immediately before discovery and again before connection; a
newly forbidden context opens no connection and cannot bypass consent. OS
service/Task Scheduler installation is deliberately not required: an ordinary
running client can host the worker, while explicit autostart/background mode
remains a future user-facing option.

M0.9.9 adds `runtime`, the first long-lived messaging process boundary. One
stable Iroh endpoint and signed ticket now serve successive delivery and sync
connections; every connection is independently device/account authorized, and
a failed session does not stop the listener. Network wait holds no device-state
lock. Each accepted application session instead gets one bounded exclusive
state/vault transaction, preserving the existing single-writer ratchet and
sequence invariants while allowing foreground commands between sessions.
Runtime exits cleanly on Ctrl+C or optional test bounds, and atomically replaces
its public ticket after restart.

M0.9.10 adds signed contacts pinned to exact peer account/device, conversation,
route and an atomically refreshable public ticket file. `runtime-queue-message`
seals plaintext immediately to the local device and appends it to a typed,
authenticated outbox. The running process materializes one signed ratchet event
per Queue ID, retries that exact event with persistent equal-jitter backoff,
stores ACK plus delivery marker atomically, and periodically synchronizes idle
contacts. A replayed delivery returns the original ACK without allocating a new
sequence. The encrypted vault treats all runtime records as a bounded
append-only state kind. Wide-area descriptor discovery and the GUI itself
remain future work.

M0.9.11 adds the reusable `kilogram-runtime-ipc` crate and an authenticated
local actor API. A runtime may bind an ephemeral loopback TCP port and
atomically publish a device-signed descriptor containing a random bearer
token. Bounded framed requests are authenticated before entering the runtime's
serialized actor loop. Local clients can ping the runtime, queue an idempotent
message, and read structured outbox status without opening or writing the
device state themselves. The descriptor is removed only by the runtime
instance that published it. This is a same-user local boundary, not a remote
network API; stronger OS-specific peer credentials remain future work.

M0.9.12 adds `kilogram-windows.exe`, the first safe-Rust desktop shell over
that actor API. It authenticates a private runtime descriptor, displays the
runtime Account/Device IDs, queues messages without opening `STATE_DIR`, and
polls structured outbox state every two seconds on a background IPC worker.
An uncertain send retains its request ID for an idempotent retry. Contact and
history views wait for an expanded actor-owned read API rather than reading
device files from the GUI.

M0.9.13 adds that actor-owned read model. The runtime verifies signed contacts,
membership, event authorization and device-local encrypted projections before
returning bounded conversation summaries or snapshot-bound history pages over
the authenticated loopback channel. The desktop now selects a chat from the
contact list, displays readable local history, loads older pages and routes the
composer from the selected signed contact. The GUI still has no `STATE_DIR`,
storage, ratchet, session or transport access.

M0.9.14 moves signed contact import and foreground runtime start/stop into the
desktop flow. A versioned secret-free launch profile carries only public
settings and absolute paths; the GUI starts the same CLI runtime implementation
and shuts it down through authenticated IPC. Runtime remains the only authority
and state writer, and no service, autostart entry or Scheduled Task is created.

M0.9.15 lets the desktop load, edit and atomically save that launch profile for
an already enrolled device. IPC v4 adds a bounded per-runtime change revision:
long polls are served outside the actor queue, while committed contact, queue,
delivery, retry and synchronization work publishes a wake-up hint. A dedicated
desktop worker coalesces those hints and refreshes actor-owned chat, history and
outbox snapshots, replacing unconditional two-second polling.

M0.9.16 adds a separate one-shot `kilogram-bootstrap` process and first-run
desktop panel. A new 24-word BIP39 phrase deterministically encodes the Account
Root key; the local root is stored in a versioned Windows DPAPI CurrentUser
envelope, while legacy raw root keys migrate without changing Account ID. The
helper atomically creates the first certified device, signed device list and
prekey pool, migrates it to a verified encrypted vault, and persists only a
public receipt. The phrase is shown once through a bounded redacted/zeroizing
response and is never written to the receipt or runtime profile. Phrase-only
restore remains deliberately disabled until current authority history can be
authenticated.

M0.9.17 adds an existing-account enrollment ceremony without transferring the
seed or any private key. A new device atomically creates a provisional encrypted
vault and a short-lived device-signed request. After offline inspection and an
exact 12-digit SAS confirmation, the existing Account Root idempotently issues
the certificate and atomically publishes a new complete device list. The exact
Root-signed authorization is HPKE-encrypted to the requesting device; accept
reads its identity only from the authenticated DB-primary vault and commits the
certificate/authority through the existing trust transaction. The newly linked
device is then eligible for one or more independent recipient-bound resumable
history recovery plans; reconciliation still reports divergence without
claiming global completeness.

M0.9.18 exposes that ceremony in the desktop client without moving Account Root
or device secrets into the GUI. The user explicitly creates or drops a request,
inspects it, compares a large 12-digit SAS, authorizes the exact inspected file,
and returns a recipient-encrypted response. Accept fills the new device's
secret-free runtime-profile draft but does not pretend that authority enrollment
also transferred history. The recovery panel can hold several independent
recipient-signed plan files, run one bounded attempt at a time, and display
signed scheduler progress. Reconciliation reports `incomplete`,
`single-source`, `agreed`, or `divergent` while always preserving
`global_completeness_proven=false`. No background service or Task Scheduler entry
is installed.

M0.9.19 enables phrase recovery without silently resetting Root authority to
revision zero. A Root-signed portable package contains the current authority
snapshot, complete device list, revocations and all current conversation
membership heads. A separate Root-signed witness pins the exact package digest
and revision. Restore accepts only an agreeing phrase/package/witness set and
publishes a newly reconstructed Root directory atomically; it never overwrites
an existing Root. An old package paired with the newest witness is rejected.
The witness must be stored independently and kept current: rolling back both
files together still requires a future monotonic service or current-device
quorum to detect.

M0.9.20 exposes export, offline inspect, and phrase restore in the desktop
client while retaining `kilogram-bootstrap` as the only Root writer. The phrase
is masked, zeroized after submission, redacted from diagnostics, and sent to the
helper only over stdin. Restore is bound to the exact package ID and authority
revision displayed by inspect, requires an explicit newest-witness confirmation,
and writes only a new Root directory. Success pre-fills the existing device-link
ceremony; device enrollment and message-history recovery remain explicit later
steps.

M0.9.21 makes recovery-backup maintenance observable instead of relying on a
remembered manual step. A successful export records the exact Root-signed
witness inside the Account Root only after revalidating that Root state did not
change during external publication. Status rebuilds the current package under
the authority lock and reports `current` or `update-required`, including for a
membership-only change at the same authority revision. This local receipt is a
lifecycle check, not an anti-rollback oracle. RFC-0048 therefore defines a fresh
challenge-bound strict-majority current-device ceremony as the next serverless
freshness layer, while retaining an explicitly weaker offline fallback. Every
approval binds one exact recovery roster; safe changes between roster epochs
require joint majorities of the old and new rosters rather than a Root signature
alone.

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

For a client-like process that stays online across successive deliveries and
syncs, use `runtime` with the same public inputs:

```powershell
cargo run -p kilogram-cli -- runtime `
  --state-dir .tmp/bob `
  --allow-account <ALICE_ACCOUNT_ID> `
  --device-list-file .tmp/bob-devices.snapshot `
  --ticket-file .tmp/bob-runtime.ticket `
  --route-policy auto
```

The ticket remains valid while this process is running. Separate `connect` and
`sync` invocations can reuse it; stop the runtime with Ctrl+C. On restart the
runtime atomically replaces the ticket with one containing its new Endpoint ID.
`--max-sessions` and `--idle-seconds` provide optional bounded test/embedding
modes; zero means no such bound.

On Alice, pin Bob's current public descriptor once:

```powershell
cargo run -p kilogram-cli -- runtime-contact-add `
  --state-dir .tmp/alice `
  --conversation m0-local-smoke `
  --expect-account <BOB_ACCOUNT_ID> `
  --descriptor-file .tmp/bob-runtime.ticket
```

Then run Alice's own runtime with a private, machine-local IPC descriptor. It
sends queued work to Bob, retries failures with persistent bounded backoff,
stores verified ACKs, and periodically synchronizes the contact:

```powershell
cargo run -p kilogram-cli -- runtime `
  --state-dir .tmp/alice `
  --allow-account <BOB_ACCOUNT_ID> `
  --device-list-file .tmp/alice-devices.snapshot `
  --ticket-file .tmp/alice-runtime.ticket `
  --ipc-file .tmp/alice-runtime.ipc.json `
  --route-policy auto
```

While that runtime stays open, a UI or these development adapters can use the
local actor API without writing `STATE_DIR`:

```powershell
cargo run -p kilogram-cli -- runtime-ipc-ping `
  --ipc-file .tmp/alice-runtime.ipc.json

cargo run -p kilogram-cli -- runtime-ipc-queue-message `
  --ipc-file .tmp/alice-runtime.ipc.json `
  --conversation m0-local-smoke `
  --peer-account <BOB_ACCOUNT_ID> `
  --message "hello through the runtime actor"

cargo run -p kilogram-cli -- runtime-ipc-outbox-status `
  --ipc-file .tmp/alice-runtime.ipc.json
```

The desktop client uses the same API. It can connect to an existing runtime:

```powershell
cargo run -p kilogram-windows -- --ipc-file .tmp/alice-runtime.ipc.json
```

You can also drop `runtime.ipc.json` onto the window. For an already enrolled
device, expand **Runtime launch settings** to load/edit/save the secret-free
profile and use **Start runtime**; the old `runtime-profile-create` command is
not mandatory. Contacts can be imported with **+ Contact** from a signed peer
runtime ticket. On a first run, expand **First run · create account**, choose a
new non-existing workspace and run the sibling `kilogram-bootstrap` helper.
Save the phrase offline before hiding it; the panel then fills the local public
profile paths. A peer Account ID and current peer prekey pool are still required
before that profile can be saved and the runtime started. Account Root recovery
is an explicit helper ceremony and remains separate from device enrollment and
message-history recovery.

The desktop client now includes both existing-account link and Account Root
recovery panels. The same operations remain available through the helper:

```powershell
# New device
kilogram-bootstrap device-link-request `
  --workspace-dir .\kilogram-linked-device `
  --account-id <ACCOUNT_ID>

# Existing Account Root, after comparing the printed SAS
kilogram-bootstrap device-link-authorize `
  --account-root-dir .\kilogram-account\account-root `
  --request-file .\kilogram-linked-device\device-link\request.kdl `
  --confirm-sas 1234-5678-9012 `
  --response-file .\response.kdl `
  --device-list-file .\kilogram-account\public\account-device-list.snapshot

# Exact requesting device
kilogram-bootstrap device-link-accept `
  --workspace-dir .\kilogram-linked-device `
  --response-file .\response.kdl
```

Export and inspect an Account Root authority recovery checkpoint after account
creation and after every authority or membership change:

```powershell
kilogram-bootstrap account-recovery-export `
  --account-root-dir .\kilogram-account\account-root `
  --package-file .\kilogram-root-20260904.karp `
  --witness-file .\kilogram-root-latest.karw

kilogram-bootstrap account-recovery-status `
  --account-root-dir .\kilogram-account\account-root

kilogram-bootstrap account-recovery-inspect `
  --package-file .\kilogram-root-20260904.karp `
  --witness-file .\kilogram-root-latest.karw

$RecoveryPhrase = Read-Host 'Enter the 24 recovery words'
$RecoveryPhrase | kilogram-bootstrap account-recovery-restore `
  --account-root-dir .\kilogram-account-restored\account-root `
  --package-file .\kilogram-root-20260904.karp `
  --witness-file .\kilogram-root-latest.karw `
  --expected-package-id <PACKAGE_ID_SHOWN_BY_INSPECT> `
  --expected-authority-revision <REVISION_SHOWN_BY_INSPECT> `
  --recovery-phrase-stdin
$RecoveryPhrase = $null
```

The package and the latest witness must be retained in independent places; a
matching old pair cannot by itself prove global freshness. Neither file contains
the phrase or a private key, but both expose account/device/membership metadata.
The full contract is in
[`docs/RFC-0046-account-root-authority-recovery.md`](docs/RFC-0046-account-root-authority-recovery.md),
with the desktop boundary in
[`docs/RFC-0047-desktop-account-root-recovery.md`](docs/RFC-0047-desktop-account-root-recovery.md)
and freshness/lifecycle rules in
[`docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md`](docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md).

Create a fresh approval request, let each current device approve it from its
DB-primary vault, then verify the collected distinct approvals:

```powershell
kilogram-bootstrap account-recovery-quorum-request `
  --package-file .\kilogram-root-20260904.karp `
  --witness-file .\kilogram-root-latest.karw `
  --request-file .\recovery-attempt.karq

kilogram-bootstrap account-recovery-quorum-approve `
  --state-dir .\kilogram-account\device `
  --request-file .\recovery-attempt.karq `
  --approval-file .\this-device.kara

kilogram-bootstrap account-recovery-quorum-verify `
  --request-file .\recovery-attempt.karq `
  --approval-file .\device-1.kara `
  --approval-file .\device-2.kara `
  --require-majority
```

This stage implements the bounded artifacts, exact-roster majority verifier and
DB-primary anti-equivocation head. The helper does not yet discover or contact
the other devices: M0.9.23 will add authenticated local/LAN-or-relay collection
and desktop orchestration. A roster change remains rejected until joint
old/new-majority transitions exist.

The queue command prints `runtime_ipc_request_id` before connecting. If its
result is uncertain, repeat the same message with `--request-id <PRINTED_ID>`;
the runtime returns the existing Queue ID instead of creating a duplicate.

The contact stores the canonical absolute descriptor path, so the same file
must be atomically refreshed after Bob restarts. In M0.9.10 this path is a local
development adapter (for example a synchronized folder), not a global
discovery protocol. The queued body is encrypted at rest and is not printed by
the status command. `--poll-milliseconds`, `--retry-base-seconds`,
`--retry-max-seconds`, and `--auto-sync-seconds` tune the runtime; setting the
last option to zero disables periodic sync.

Keep the IPC descriptor in a private local directory: it contains a bearer
secret and must not be placed in a synchronized/shared folder. It is signed by
the runtime device and names only `127.0.0.1`; it is not part of the P2P wire
protocol. The older direct outbox commands remain useful only as offline M0
adapters when no runtime/UI owns the state.

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

The one-shot listener exits after acknowledging one event; `runtime` returns to
listening. Reusing a state directory preserves the application device ID and
advances its author sequence across restarts. Ordinary commands retain an
exclusive outer lock. The runtime instead holds the same lock only during one
accepted application session and waits briefly for a foreground command, so do
not start two runtimes or otherwise introduce multiple concurrent state writers
for one state directory.

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
