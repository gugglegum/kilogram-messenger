# RFC-0175: M1.1.49 active runtime-IPC legacy mailbox retirement

Status: implemented as an active M1.1.49 local-control migration.

## 1. Decision

M1.1.49 removes `CreateMailboxCapability` and `RotateMailboxCapability` from
the active Postcard loopback IPC. Those variants accepted a central HTTPS
mailbox service URL and public store key and could still create new legacy v1
capabilities even though the desktop had already moved to service-free exact
volunteer replica sets.

The active create and rotate path is now exclusively
`CreateExactMailboxCapability` and `RotateExactMailboxCapability`. It selects
an admission-qualified, transport-distinct volunteer replica set inside the
runtime and does not accept a central service URL or central store key over
IPC. `RevokeMailboxCapability`, bounded capability status and the shared
secret-free transition response remain.

## 2. Legacy state is readable, not newly provisioned

This retirement does not make existing local data unreadable. The runtime may
still read an already persisted legacy v1 capability, deliver its explicit
revocation, or perform the existing one-way acknowledged v1-to-service-free-v2
upgrade. A v2 capability cannot downgrade to v1. The manual foreground CLI
recovery adapters and historical formats are not claimed to be erased in this
stage; they are outside the active desktop/runtime IPC contract.

No mailbox peer wire, stored capability encoding, relay protocol or sync wire
changes. This is removal of a local control path for creating new central
mailboxes, not deletion of compatibility required to retire old state safely.

## 3. Exact-version migration

Removing two enum variants changes later Postcard discriminants. The active
runtime IPC therefore moves from version 28 with signed descriptor domain v9
to version 29 with domain v10. A v28 descriptor is rejected before connection
and a v28 request is rejected before actor dispatch. Co-installed desktop and
CLI binaries must be updated together and the foreground runtime restarted;
there is no dual reader or silent downgrade.

The active inventory now contains 29 commands and 28 responses. Every active
variant has a canonical target or bounded canonical replacement, no
Postcard-only activation blocker remains, and the canonical runtime-IPC
surface becomes complete.

## 4. Completion is not activation

Completing the command/result schema and active adapter inventory does not
activate deterministic CBOR. Postcard v29 remains the current local-control
encoding. A future activation still requires an explicit negotiated protocol
identifier, a parallel reader/writer with downgrade tests, sustained fuzzing
and fresh desktop/CLI multi-device field evidence.

This stage changes no peer network wire and runs no network test. It creates no
release build, archive, distribution executable, push or tag.
