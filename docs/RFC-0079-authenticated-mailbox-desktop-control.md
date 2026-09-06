# RFC-0079: Authenticated mailbox desktop control (M0.9.57)

Status: implemented in M0.9.57; the IPC/runtime/desktop path is compile-verified
and guarded statically. Network-bearing lifecycle behavior still requires the
controlled two-device field run deferred by RFC-0078.

## 1. Problem

M0.9.55 and M0.9.56 made mailbox capability activation, rotation, revocation,
ACK and crash convergence explicit, but an operator still had to invoke
separate CLI commands. That was not an acceptable normal-client boundary:

- it encouraged a second process to contend for runtime state ownership;
- manual offer files could outlive the intended transfer ceremony;
- aggregate mailbox counters did not identify the current capability chain;
- users could not select the exact recipient Device or distinguish an active
  capability from a transition waiting for its authenticated ACK.

## 2. Authenticated IPC v23

Runtime IPC version 23 adds four actor commands:

- `CreateMailboxCapability`;
- `RotateMailboxCapability`;
- `RevokeMailboxCapability`;
- the existing `MailboxStatus`, extended with lifecycle-head projections.

Every mutation names the exact conversation, peer Account and peer Device.
Activation and rotation additionally carry the public HTTPS service URL,
pinned public store key and bounded validity. These are connection parameters,
not mailbox authority secrets.

The response contains only contact/conversation/peer identifiers, opaque
binding/update IDs, generation, action and delivery state. It never contains a
mailbox read capability, write capability, Account Root secret, device secret
or encrypted/manual offer artifact.

Changing IPC version invalidates an old descriptor rather than attempting an
ambiguous mixed-version command. Restarting the runtime republishes a signed
descriptor for version 23.

## 3. Single runtime owner

The desktop does not run a CLI mailbox subcommand. It submits a typed request
through the existing authenticated loopback IPC worker. The serialized runtime
actor then:

1. parses the pinned public store key;
2. acquires the existing exclusive state transaction boundary;
3. creates, rotates or revokes the signed lifecycle state;
4. persists the local binding/update before returning success;
5. schedules the encrypted peer transition through the existing authenticated
   Device session.

IPC activation/rotation deliberately pass no output path to provisioning, so
they cannot create a manual capability-offer file. The old explicit CLI
commands remain development/recovery adapters but do not participate in the
desktop workflow.

## 4. Secret-free status model

`RuntimeIpcMailboxStatus` keeps the existing aggregate delivery and ledger
counters and adds one projection for each current managed lifecycle head:

- exact contact, conversation, peer Account and peer Device;
- direction (`receive` for a locally owned mailbox or `write` for a peer
  mailbox);
- opaque binding and update IDs;
- generation, revocation flag and convergence state;
- owner-side acknowledgement state when it is locally knowable.

The projected states distinguish active, activation-pending,
rotation-pending, revocation-pending and revoked heads. Legacy unmanaged
bindings remain represented by the aggregate counters but have no fabricated
generation or ACK claim.

## 5. Windows UI

The existing `kilogram-windows` process now has a **Blind mailbox fallback**
panel. It:

- defaults to the primary usable endpoint Device for the selected enrolled
  contact and allows choosing another exact candidate;
- accepts only the public mailbox service URL, public store key and validity;
- exposes Activate, Rotate, Revoke and Refresh status actions;
- displays the last transition and per-device lifecycle/convergence state;
- explains that direct/relay delivery remains preferred and that the mailbox
  operator still observes timing and volume.

All operations use the existing single IPC worker. A successful mutation
publishes the runtime change revision and automatically refreshes the
secret-free status projection. No additional executable, Task Scheduler entry,
background service, release build or ZIP package is introduced.

## 6. Fail-closed boundaries

`scripts/verify-kilogram-mailbox-desktop-control.ps1` rejects the milestone if:

- IPC v23 or any typed lifecycle command/result disappears;
- status/transition types gain known read/write/Root/device secret fields;
- runtime mutations stop using the locked single-owner path, export a manual
  offer, or stop publishing a state change;
- the desktop stops selecting an exact Device or bypasses authenticated IPC;
- either application adds another explicit binary target.

The existing lifecycle, convergence and runtime mailbox gates remain in force.
The integrated desktop IPC test covers exact command mapping and secret-free
responses, but it is not run during ordinary low-impact development because it
opens a loopback listener; all of its targets are compiled by strict Clippy.

## 7. Deferred work

The next controlled two-device test must exercise activation, lost ACK and
restart, rotation overlap and revocation over both direct and relay paths. It
must confirm that only opaque ciphertext reaches the mailbox service and that
the GUI status converges on both devices.

Production work still includes multi-store replication, private retrieval,
push wakeup, traffic padding, volunteer storage/relay policy and abuse/Sybil
resistance. This UI does not claim that one blind mailbox is a durable or
anonymous delivery authority.
