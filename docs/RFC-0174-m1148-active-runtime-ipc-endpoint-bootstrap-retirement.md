# RFC-0174: M1.1.48 active runtime-IPC endpoint bootstrap retirement

Status: implemented as an active M1.1.48 local-control migration.

## 1. Decision

M1.1.48 removes the three manual endpoint-announcement bootstrap commands
`ExportEndpointAnnouncements`, `ImportEndpointAnnouncements` and
`PushEndpointAnnouncements` from the active Postcard runtime IPC. Their three
success responses are removed with them. The matching public CLI subcommands
are no longer available.

These commands were development-era file-transfer helpers. They exposed local
bundle, ticket and descriptor-directory paths across loopback IPC and required
an operator to move artifacts manually. They are redundant now that the
foreground runtime has authenticated own-device announcement automation,
pairwise ticket discovery and roster-wide orchestration.

## 2. Behavior that remains

This retirement does not remove endpoint announcements. The recipient-only
encrypted bundle, source signature, same-account authorization, session-bound
acknowledgement and peer `EndpointAnnouncementPush` network request remain.
The internal export, import and push implementation is retained for the
foreground automations and for protocol regression tests.

Consequently this stage changes active local control but changes no peer
network, relay, mailbox or sync wire. It does not activate canonical CBOR and
does not claim background delivery while the messenger is closed.

## 3. Exact-version migration

Removing enum variants changes later Postcard discriminants. Silent mixed
operation would therefore be unsafe. The active runtime IPC moves from version
27 with descriptor signature domain v8 to version 28 with domain v9. A v27
descriptor is rejected before connect and a v27 request is rejected before
actor dispatch. Co-installed desktop and CLI binaries must be updated together
and the foreground runtime restarted; there is no dual reader or downgrade.

The active inventory now contains 31 commands and 28 responses. Only
`CreateMailboxCapability` and `RotateMailboxCapability` remain explicit
Postcard-only activation blockers because they retain the legacy central-HTTPS
mailbox creation path.

## 4. Verification boundary

The dedicated boundary checks the exact migration manifest, v28/v9 source and
compatibility registry, exact active enum inventory, absence of the retired
IPC and CLI variants, presence of the authenticated peer handler and
foreground automations, fail-closed v27 descriptor regression, completion
ledger and canonical coverage. Its negative self-tests reject a missing
retirement, a forged current version and a reintroduced CLI command.

This resolves `endpoint-announcement-bootstrap-helper-disposition`. The
canonical runtime-IPC target remains incomplete only while
`legacy-central-https-mailbox-command-removal` remains unresolved.
