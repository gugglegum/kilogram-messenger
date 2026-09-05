# RFC-0068: Live publication incident lifecycle (M0.9.46)

Status: implemented in M0.9.46.

## 1. Problem

M0.9.45 separated online Device evidence from offline Account Root authority,
but its desktop ceremony still required stopping the messaging runtime twice:
once to create a `.pcrq` and again to apply the `.pcrp`. The publishing peer
also had to rotate its own channel with an offline command and restart the
listener before a fresh ticket existed.

Those pauses were an implementation limitation rather than a protocol or key
separation requirement. They made a recoverable publication conflict look like
a full account outage and left the operator with several disconnected tools.

M0.9.46 moves all non-Root mutations behind the already authenticated,
serialized runtime actor and presents the ceremony as one observable incident
lifecycle.

## 2. Authenticated IPC v19

Runtime IPC v19 adds three bounded commands:

- `RotatePublicationChannel` rotates the running Device's channel for the
  runtime's exact primary peer Account;
- `CreatePublicationConflictRequest` writes a Device-signed `.pcrq` for a
  selected quarantined channel and exact fresh replacement ticket; and
- `ApplyPublicationConflictResponse` verifies and applies a self-contained
  Root-signed `.pcrp`.

All three calls use the existing private loopback bearer descriptor, protocol
version check, request bound and single actor queue. Artifact paths must be
absolute. New output must be outside protected runtime state and remains
no-clobber; response and replacement-ticket inputs must be bounded regular
files. The actor owns every state mutation, so the GUI never opens `STATE_DIR`.

The CLI adapters expose the actor through:

- `runtime-ipc-rotate-publication-channel`;
- `runtime-ipc-publication-conflict-create-request`; and
- `runtime-ipc-publication-conflict-apply-response`.

Each structured result explicitly reports `runtime_restart_required=false`.
Request creation and response application also report
`root_secret_loaded=false`.

## 3. Live channel rotation

Rotation is restricted to the peer Account already bound as the running
listener ticket's primary audience. It reuses the same live Iroh endpoint,
listener Device certificate, complete Device directory and route policy while
advancing the signed publication-channel epoch.

The actor first appends the local Device-signed rotation record transactionally.
It then derives the new write capability, creates a replacement connection
ticket for the same endpoint and atomically replaces the configured public
ticket file. The in-memory listener ticket changes only after both durable
steps succeed.

This state-first ordering is retry-safe. If the record commit succeeded but
ticket publication failed, the next command detects that durable epoch is one
step ahead of the actor's ticket and republishes that exact epoch with
`AlreadyPresent`; it does not create another rotation. A running ticket ahead
of durable state fails closed.

The report includes old/new channel IDs, epoch, rotation ID/store outcome,
public ticket path and publication status. It also says
`fresh-ticket-transfer-required`: rotating the local channel cannot silently
replace the descriptor already held by the peer.

## 4. Live request and response

The `.pcrq` and `.pcrp` verification rules from RFC-0067 do not change. The
difference is ownership and availability:

- the live actor creates the request from its current durable proof and
  authority state while continuing to serve messaging sessions;
- the isolated Root host still performs the only Root-authorized step with
  `account-publication-conflict-authorize`; and
- the live actor verifies and applies the response, updates the effective peer
  binding and immediately exposes the result to subsequent sessions.

Applying the same response again is idempotent and reports `Unchanged`. The
original `.pcf` remains immutable. Root material is neither accepted by the new
IPC commands nor loaded by their process.

## 5. Desktop lifecycle

The Windows incident panel now presents one sequence:

0. the publishing peer rotates its own channel live and transfers the refreshed
   ticket;
1. the affected side selects its retained conflict, creates `.pcrq` through
   the connected runtime and takes it to the isolated Root signer;
2. the affected side applies `.pcrp` through the same running actor.

The panel shows `quarantined`, `offline Root authorization pending` and
`resolved locally / sibling propagation pending` phases. It has no Root path,
phrase or signing action. Incident buttons are disabled unless the desktop is
connected to the authenticated runtime, and completion explicitly says that no
restart is required.

The selected request and apply results are session-local UI progress. Durable
security state remains the signed `.pcf`, `.pcrq` artifact and applied `.pcr`;
the GUI does not invent a persistent global workflow coordinator.

## 6. Compatibility

- Runtime IPC advances from v18 to v19.
- Connection ticket remains v11.
- Publication-conflict resolution remains v2.
- Endpoint-announcement bundle remains v4 and acknowledgement remains v2.
- Event, ratchet, sync and transport ALPN formats do not change.

IPC descriptors are intentionally version-exact. A v18 desktop/CLI adapter
must reconnect to a matching runtime rather than guessing the shape of v19
reports.

## 7. Verification

The long-lived ticket-publication regression rotates Bob's channel through the
live actor, checks epoch 1, a different channel, atomic replacement at the same
ticket path, the same transport Endpoint ID and a successful IPC ping without
restart. Reload after shutdown confirms that the rotation is durable.

The three-Device conflict regression keeps the affected runtime online while
it creates `.pcrq`, receives an independently Root-signed `.pcrp`, applies it
once as `Inserted` and again as `Unchanged`, and answers another IPC ping. The
existing C -> A -> B encrypted propagation then proves convergence is unchanged.

Workspace checks cover formatting, strict all-target/all-feature clippy,
Windows UI tests, the complete test suite and a release build.

## 8. Honest boundary

This milestone removes process restarts; it does not automate trust decisions.
The operator still decides that an incident warrants rotation, transfers the
fresh peer ticket and carries public request/response artifacts across the
offline Root boundary. A compromised active Device can rotate its own channel
or request denial-of-service recovery, but cannot forge Root authorization.

There is still no global publication witness. Siblings that never receive the
conflict proof or resolution cannot infer it, and disconnected peers continue
using their last authenticated descriptor until a fresh ticket or existing
announcement exchange reaches them. A hardened offline evidence viewer and
removable-media/QR signer ceremony remain future work.
