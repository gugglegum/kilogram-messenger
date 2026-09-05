# RFC-0059: Authenticated own-device endpoint announcements (M0.9.37)

Status: implemented in M0.9.37.

## 1. Problem

M0.9.36 lets one installation retain an endpoint's publication lookup key
after its transport ticket expires. Another already-authorized device of the
same account can still have an incomplete contact/candidate set and an older
publication observation high-water. Copying runtime files is unsafe: those
records are signed for one local Device and contain installation-specific
absolute paths.

This slice introduces a bounded portable transfer without making the transfer
file, an opaque store, or a future gossip carrier an Account identity authority.

## 2. Export envelope

The running runtime exposes IPC v11 command
`ExportEndpointAnnouncements`. It snapshots at most 256 contacts and four
endpoints per contact. Every endpoint contains:

- conversation label/ID and peer Account/Device;
- primary hint and route policy;
- the structurally authenticated connection ticket, which may be expired;
- the expiry-independent self-authenticating publication write key;
- the source Device's latest signed local publication observation, when one
  exists.

The complete payload also carries the exact current Root-signed own-device
list, source and recipient Device IDs, creation time and an expiry no more than
one hour ahead. The source Device signs the canonical payload. The result is
then HPKE-encrypted to the exact encryption key in the recipient's certificate;
the public envelope metadata is authenticated as HPKE AAD. A content-derived
bundle ID binds export, encryption and later acceptance evidence. The output is
created without overwriting an existing file and must live outside protected
runtime state.

## 3. Import gates

Only the running recipient runtime can import the envelope. Before writing any
authenticated state it requires:

1. successful HPKE open by the addressed local Device;
2. a current, non-expired source signature;
3. byte-exact equality between the embedded Root-signed device list and the
   recipient runtime's current list;
4. active messaging authorization for both source and recipient;
5. an already-installed conversation membership containing local and peer
   Accounts;
6. complete ticket signatures/Root/device-list/prekey structure, exact peer
   Account/Device, requester Account, route policy and publication key;
7. compatibility with every already-enrolled immutable endpoint contract and
   the existing four-candidate bound.

The embedded own-device list is evidence for the source authorization check,
not an update. Import never installs Root authority or conversation membership
from the bundle. A stale roster, revoked source, wrong recipient, changed
binding, duplicate candidate, expired envelope or tampered ciphertext fails the
whole operation.

## 4. Local materialization

Portable source records are not copied. The recipient creates its own
Device-signed contact, candidate and `.epb` binding records with local absolute
descriptor paths. Missing public ticket files are written outside the state
vault before one vault-primary state transaction; an interrupted import can
therefore leave an unreferenced public descriptor, never authenticated state
that points to a missing file. Repeating the same bundle is byte-exact and
idempotent.

If an announced ticket is currently fully valid, its own Root-signed peer
authority and prekey directory can be observed in that same transaction. An
expired ticket is never made dialable; only its fully authenticated immutable
contract and publication key are retained so normal refresh can fetch a fresh
ticket.

## 5. Sibling observation evidence

Each source observation is preserved inside a recipient-Device-signed `.aeo`
acceptance record. Runtime restart verifies both signatures and requires a
matching enrolled endpoint/binding. The highest imported publication generation
is a lower bound for later refresh:

- a lower generation is rejected as rollback;
- the same generation must have the exact publication ID and ticket digest;
- different authorized sibling claims at the same highest generation are
  treated as equivocation and fail closed.

This evidence is intentionally separate from the recipient's contiguous local
observation chain. A successful fresh fetch still creates or advances the
normal local chain. Source-device revocation blocks new imports but does not
erase already-accepted historical anti-rollback evidence.

## 6. Compatibility and limits

- Runtime IPC advances from v10 to v11; runtime, CLI adapter and desktop must be
  upgraded together. Ticket v10 and network/session/event formats do not
  change.
- Transfer is explicit file transport in this slice. The file is confidential
  and authenticated, so it can later be carried by an already-authorized
  device session, QR/file exchange or opaque mailbox without changing its trust
  semantics.
- The bundle does not discover a first contact, authorize a new account device,
  rotate a publication capability, guarantee store availability or hide
  transfer metadata.
- Accepted observation records share the existing global 4096 runtime
  ticket-state bound. A future acknowledgement/compaction protocol is needed
  before automatic high-frequency announcement gossip.

The next slice should carry the same bounded encrypted bundle automatically
over an authenticated same-account Device session, with replay acknowledgement
and backpressure, while preserving this import gate as the sole state mutation
path.

## 7. Current CLI ceremony

With both runtimes already running and using the same exact current device
list, the source exports:

```powershell
.\kilogram-cli.exe runtime-ipc-export-endpoint-announcements `
  --ipc-file .\source-runtime.ipc `
  --recipient-device <RECIPIENT_DEVICE_ID> `
  --output-file .\source-to-recipient.eab
```

After transferring that opaque file, the recipient imports it before its
bounded expiry:

```powershell
.\kilogram-cli.exe runtime-ipc-import-endpoint-announcements `
  --ipc-file .\recipient-runtime.ipc `
  --bundle-file .\source-to-recipient.eab `
  --descriptor-directory .\imported-peer-tickets
```

The export file and descriptor directory deliberately remain outside each
runtime state directory. These are diagnostic M0 commands; desktop workflow and
same-account network transport are not yet wired to them.
