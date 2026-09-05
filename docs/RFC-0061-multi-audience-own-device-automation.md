# RFC-0061: Multi-audience runtime and own-device announcement automation (M0.9.39)

Status: implemented in M0.9.39.

## 1. Problem

M0.9.38 can move endpoint announcements over an authenticated same-account
Device session, but the receiving runtime has to be launched with its single
ticket audience switched from the normal peer Account to its own Account. The
push is also a one-shot IPC action with no durable retry state. A normal
foreground messenger must serve its current peer and active sibling Devices at
the same time, then repeat the bounded exchange after failure or restart.

## 2. Two signed tickets, one listener

The long-lived runtime keeps one Iroh endpoint and publishes two ticket views:

- the existing primary ticket, restricted to the configured peer Account;
- a stable own-device ticket, restricted to the listener's own Account.

The own-device ticket is atomically replaced beside the runtime IPC descriptor
(or the primary ticket when IPC is disabled) as
`kilogram-own-device-<full-device-id>.ticket`. It contains public connection,
certificate, exact Root-signed roster and prekey material only. It contains no
Device secret, local IPC bearer token or message key. Its requester-specific
ticket-publication write capability is covered by the existing Device ticket
signature.

Both tickets address the same transport endpoint. The runtime accepts a Device
authorization only when its Account is either the exact primary audience or
the listener's own Account. A same-account requester must carry the byte-exact
current Root authority already installed by the listener and must be an active
Device with the messaging capability. A third Account, a stale or forked own
roster, and a revoked Device fail closed. Normal peer authority continues to
use the existing monotonic pin/rollback gate. One-shot `listen` remains
single-audience.

When a live Root-signed device directory is applied, the runtime atomically
replaces the primary ticket and republishes the own-device ticket from the same
new roster before reporting the operation as successful.

## 3. Durable opt-in schedule

IPC v13 adds `ConfigureOwnDeviceAnnouncementAutomation` and
`OwnDeviceAnnouncementAutomationStatus`. Configuration names a locally
available stable own-device ticket file and must prove that it:

- belongs to another active Device of the same Account;
- permits the local Account as requester;
- carries the byte-exact current own roster and valid listener authorization;
- is a bounded regular non-symlink file outside protected runtime state.

The canonical absolute path is stored in a Device-signed append-only policy
chain keyed by recipient Device. Replacing the ticket contents at that path
does not require a policy update. The policy records enabled state, transfer
interval, encrypted-envelope validity, retry bounds and explicit network-class
permissions. Identical configuration is idempotent; changing or disabling it
appends a new signed generation.

Defaults and hard bounds are:

- successful interval: 300 seconds; minimum 30 seconds, maximum 24 hours;
- envelope validity: 900 seconds; maximum 1 hour;
- retry: 5 seconds initially, exponentially bounded at 300 seconds by default
  and 1 hour absolutely;
- Ethernet and Wi-Fi allowed by default; mobile and unknown network classes
  require explicit opt-in.

At each five-second automation check the actor first gives existing contact
ticket automation a chance to run, then selects at most one due own-device
policy in deterministic Device order. It reuses the exact M0.9.38 push/import
gate and recipient-signed acknowledgement. It also shares the runtime's global
outbound-action limit. No parallel fan-out or hidden worker is introduced.

## 4. Attempts, restart and compaction

Every completed network attempt appends a Device-signed attempt generation.
A success records the exact bundle ID and verified direct/relay path, then
schedules the normal interval. A failure records only bounded backoff state;
local diagnostic details stay in the runtime log. Updating a policy resets the
effective schedule while preserving the historical chain.

Policies and attempts live in the encrypted vault-primary runtime state and
are verified again on startup. Status distinguishes `due`, `fresh`, `backoff`,
`network-blocked`, `recipient-revoked` and `disabled`, and reports the last
attempt/success, next due time, failure count, bundle ID, path and current
network permission.

Each policy and attempt chain is compacted after more than eight retained
records. The existing Device-signed runtime ticket checkpoint now anchors both
new chain types, and prefix removals plus the new checkpoint are committed in
one vault transaction. A global 4096-record cap and the Root roster's bounded
Device count apply before compaction.

## 5. Failure and security properties

- A missing, expired or atomically changing recipient ticket causes an ordinary
  signed retry/backoff attempt; it cannot alter authority.
- Revocation immediately prevents new scheduled attempts after the runtime
  applies the new roster and republishes its tickets.
- The recipient still materializes only its own signed records through the one
  canonical import transaction; the source cannot choose destination paths.
- Exact bundle replay remains idempotent and receives a fresh session-bound
  acknowledgement.
- Multi-audience authorization does not make the endpoint public to arbitrary
  Accounts and does not weaken primary peer pinning.

This automation improves availability, not anonymity. A direct path can expose
IP addresses, while relay selection remains governed by the signed route
policy in the recipient ticket.

## 6. Compatibility and commands

Connection ticket v10, announcement envelope, event/ratchet formats and wire
ALPN v8 are unchanged. Runtime IPC advances from v12 to v13, so runtime, CLI
adapter and desktop must be upgraded together. Older runtimes do not provide
the second audience or the new IPC commands.

With both runtimes online and the recipient's automatically published
own-device ticket available locally:

```powershell
.\kilogram-cli.exe runtime-ipc-configure-own-device-announcements `
  --ipc-file .\source-runtime.ipc.json `
  --recipient-ticket-file .\kilogram-own-device-<recipient-device-id>.ticket

.\kilogram-cli.exe runtime-ipc-own-device-announcement-status `
  --ipc-file .\source-runtime.ipc.json
```

Use `--allow-mobile` or `--allow-unknown-network` only when that cost/privacy
tradeoff is intended. Use `--deny-ethernet`, `--deny-wifi`, or
`--enabled false` to narrow or stop an existing schedule.

## 7. Non-goals and next work

This slice does not discover or distribute sibling ticket files, run while the
application is closed, register Windows Task Scheduler, provide offline
mailbox delivery, hide traffic correlation or create/revoke Devices. It remains
an explicitly configured foreground mechanism.

The next availability slice should distribute recipient-specific own-device
tickets through an authenticated, privacy-preserving and bounded channel so a
shared folder is no longer required. OS autostart remains a separate explicit
user choice.
