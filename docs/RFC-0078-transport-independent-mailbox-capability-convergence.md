# RFC-0078: Transport-independent mailbox capability convergence (M0.9.56)

Status: implemented in M0.9.56; deterministic network-free behavior is
verified. The network-bearing exchange remains compile-verified and still
needs a controlled two-device field run.

## 1. Problem

M0.9.55 introduced signed mailbox activation, rotation and revocation updates,
recipient-signed ACKs and automatic exchange. However, the decisions about
which update to retry, whether an old receive binding may overlap, and whether
an inbound update is an append or a replay were still distributed through the
large CLI runtime.

That made crash behavior difficult to test without opening QUIC sockets and
risked small semantic differences between state loading, sender selection,
recipient application and mailbox use.

## 2. Shared state machine

`kilogram-mailbox-provisioning` now owns
`MailboxCapabilityConvergence`. It is reconstructed from two immutable inputs
for one exact owner/recipient Device and mailbox scope:

- the retained contiguous `SignedMailboxCapabilityUpdate` chain;
- any retained `SignedMailboxCapabilityAcknowledgement` records for that
  chain.

The state machine has no clock, socket, retry timer, filesystem, runtime IPC or
transport dependency. Its decisions are pure consequences of signed durable
artifacts, so recreating it after a process crash produces the same result.

## 3. ACK ownership and session binding

The recipient-signed ACK codec moved from the CLI module into the same
network-free provisioning crate as the update chain. Its signing domain,
version and serialized fields remain unchanged. Therefore update and ACK
validation can no longer drift between a test-only contract and runtime code.

The provisioning crate represents the authenticated session as an opaque
32-byte `MailboxCapabilitySessionBinding`. The protocol layer maps its
`SyncSessionBinding` bytes at the boundary. Provisioning neither knows nor
assumes how the transport derived them. A received ACK is still verified
against the exact live session before durable commit; reconstructed state can
later verify the retained recipient signature using the binding embedded in
the ACK.

## 4. Deterministic transitions

The convergence view exposes one fail-closed rule set:

- owner history may advance to generation N+1 only if generation N has a
  retained valid ACK;
- `next_outbound_update` returns at most one unacknowledged transition whose
  exact predecessor is acknowledged;
- a recipient classifies an exact retained update as `AlreadyPresent` and the
  exact next link as `Append`; a gap, fork or collision is rejected;
- the recipient writes only with the current non-revoked activation head;
- the owner receives on the current activation head and, only while a rotation
  head lacks its ACK, may also poll the immediately preceding active binding;
- a revocation makes the named receive/write capability inactive immediately,
  even while the revocation ACK is still pending.

`Unmanaged` is retained solely for pre-M0.9.55 bindings that have no lifecycle
chain. Once generation one exists, the signed chain is authoritative.

## 5. Runtime integration

The runtime snapshot now constructs the shared convergence view for every
local and peer direction. The same implementation is used to:

- validate loaded owner chains and predecessor ACK ordering;
- compute the next automatic control-plane retry;
- classify recipient replay versus append before persistence;
- select current peer write bindings;
- permit and terminate owner-side receive overlap.

The CLI no longer contains a second mailbox ACK codec or a private chain-head
algorithm. Timers and endpoint fairness remain runtime concerns; they choose
when to attempt work, while the library state machine decides what work is
eligible.

## 6. Crash/retry regression

The network-free regression
`convergence_survives_lost_ack_restart_rotation_and_revocation` executes:

1. generation-one activation and recipient append;
2. loss of the first ACK;
3. owner reconstruction and deterministic retry of the same update;
4. recipient idempotent replay and retained ACK;
5. rotation, current/new plus previous/overlap receive states;
6. reconstruction after recipient append but before rotation ACK retention;
7. deterministic rotation retry and overlap termination after ACK;
8. revocation, immediate inactivity, recipient append and final ACK;
9. rejection of owner history that advanced without predecessor ACKs.

Each simulated restart encodes and decodes the signed artifacts before
reconstructing the state machine. No executable listener, QUIC endpoint, HTTP
client or Windows Firewall surface is involved.

## 7. Verification boundary

`scripts/verify-kilogram-mailbox-capability-convergence.ps1` fails closed unless
the shared state machine, signed ACK, opaque session binding, crash regression
and all runtime call sites remain present. It also rejects a duplicate CLI ACK
implementation, forbidden network/runtime dependencies in the provisioning
crate and any new explicit executable target.

The earlier M0.9.55 lifecycle gate was updated to require the shared ACK from
the provisioning contract. The provisioning dependency boundary continues to
reject protocol, runtime, network and direct storage dependencies.

## 8. Deferred work

This milestone proves deterministic control-plane behavior, not live Internet
availability. A controlled two-device run must still cover activation, forced
ACK loss/restart, rotation overlap and revocation over the authenticated
direct/relay path before the online lifecycle is field-verified.

Mailbox lifecycle controls also still need first-class IPC and desktop UI so a
normal user can inspect, rotate and revoke capabilities without CLI commands.
Multi-store replication, private retrieval, push wakeup, traffic padding,
volunteer storage/relay admission and Sybil/spam resistance remain later work.
