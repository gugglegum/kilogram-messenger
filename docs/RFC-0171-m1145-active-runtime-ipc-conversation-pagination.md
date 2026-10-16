# RFC-0171: M1.1.45 active runtime IPC conversation pagination

Status: implemented as the active local-control migration in M1.1.45.

## 1. Scope

M1.1.45 replaces the active unpaged Postcard `ConversationList` exchange with
an explicitly paginated `ConversationListPage` exchange. The migration changes
only authenticated loopback control traffic between the co-installed Kilogram
runtime and desktop client. It does not change any peer-to-peer, relay, mailbox,
sync or discovery network wire.

The active local IPC version advances from 26 to 27 and the signed JSON
descriptor domain advances from `v7` to `v8`. Version 26 descriptors and
requests are rejected. In particular, a legacy request fails before actor
dispatch. Kilogram ships the runtime and desktop as one build, so this
pre-release migration deliberately uses a clean hard cut rather than trying to
decode two incompatible Postcard enum shapes under one descriptor. A
desktop/runtime version mismatch requires both processes to be updated and the
runtime to be restarted.

## 2. Page contract

Every request carries an optional cursor and a limit from 1 through 64. The
cursor is the exact tuple of the UTF-8 conversation label and peer Account ID.
The Account ID is nonzero and the label contains between 1 and 4096 encoded
bytes. Results are sorted by the bytewise UTF-8 label and then Account ID.

Every returned page:

- contains no more than the requested limit;
- is strictly ordered and begins strictly after the request cursor;
- has a continuation only when the page is full;
- binds the continuation to the exact last returned tuple; and
- rejects duplicated cursor keys instead of silently skipping a contact.

The active contact ID is already derived from the local Account, peer Account
and label-derived Conversation ID, so two valid stored contacts cannot share
the cursor tuple. The runtime nevertheless checks this invariant before serving
a page and fails closed if persisted state violates it.

## 3. Bounded work and desktop behavior

The runtime orders authenticated contact records first, selects at most one
page, and only then loads membership, history, endpoint health and message
preview state for those selected contacts. The existing 16,384-event inspection
limit now applies to one bounded page rather than to an unbounded account-wide
response.

The desktop follows continuations automatically so the visible conversation
list remains unchanged. It independently validates every page and cursor before
appending it. A defensive limit of 1,024 pages bounds one refresh to 65,536
conversations; reaching that limit is an explicit error, never truncation
presented as success. A concurrent contact change can make a refresh
point-in-time non-atomic because the cursor intentionally carries no snapshot
ID; the existing change notification causes a later complete refresh.

## 4. Compatibility and activation boundary

The deterministic-CBOR runtime IPC model remains an inactive candidate. Its
`current_runtime_encoding` record now names Postcard v27 with JSON descriptor
v8, while `current_runtime_changed=false` continues to mean that the CBOR
candidate itself has not been activated. Future CBOR activation still requires
its own explicit identifier/version migration, dual-reader and downgrade
tests, sustained fuzzing and fresh field evidence. This active Postcard
migration does not activate canonical CBOR.

This RFC closes only `active-unpaged-conversation-list-migration`. The remaining
canonical runtime IPC work is the explicit inventory and verification of
associated mutation-result contracts not already represented in the aggregate
schema.

## 5. Verification

Unit and integration coverage proves strict page validation, multi-page desktop
aggregation, rejection of an authenticated version 26 request before actor
dispatch, and rejection of a correctly version-26/v7-signed legacy descriptor.
A dedicated fail-closed boundary pins the active version/domain, compatibility
registry, bounded server selection, desktop continuation loop and canonical
coverage bookkeeping. No network listener beyond the existing loopback test
server, release build, archive, executable packaging or public tag is required.
