# RFC-0077: Authenticated mailbox capability lifecycle (M0.9.55)

Status: implemented in M0.9.55; network-bearing path is compile-verified and
awaits a controlled two-device integration run.

## 1. Scope

M0.9.53 introduced recipient-bound mailbox offers and M0.9.54 used imported
bindings as a direct/relay fallback. The remaining correctness gap was
lifecycle: an offer still had to be copied manually, and neither side retained
an authenticated ordered statement saying which binding is current.

M0.9.55 adds that statement and transports it through the existing encrypted,
mutually authenticated Device session. It does not add another executable,
discovery service or public capability channel.

## 2. Ordered capability artifact

`kilogram-mailbox-provisioning` now owns a network-free
`SignedMailboxCapabilityUpdate`. Each update binds:

- owner and exact recipient Account/Device identities;
- opaque conversation-derived mailbox scope;
- positive generation and exact previous update ID;
- creation time;
- either an activation containing the existing recipient-HPKE-encrypted offer,
  or a revocation naming the exact active binding.

Generation one must be an activation with no predecessor. Every later update
must be the exact next generation and name the predecessor's content-derived
ID. A revocation can only name an active predecessor. A later activation must
install a different binding. Consequently a retained chain rejects rollback,
gaps, same-generation forks, repeated rotation to the same secret and
revocation of an unrelated binding.

The outer artifact reveals identities, scope, generation, timing and binding
ID only inside the already authenticated E2EE session and in the two clients'
encrypted local state. The mailbox write secret and service details remain
inside the recipient-bound HPKE offer.

## 3. Explicit local lifecycle

The existing `runtime-mailbox-offer-create` command now creates generation one
and persists the new local binding and update in one state transaction. Its
no-clobber offer file remains as a manual/offline compatibility artifact, but a
running runtime no longer requires that file to reach the peer.

Two commands make later transitions explicit:

- `runtime-mailbox-rotate` creates a fresh mailbox, sealed local capability and
  recipient offer, then appends the exact next activation;
- `runtime-mailbox-revoke` appends the exact next revocation of the current
  active binding.

Creating a second generation-one head is rejected. Rotation/revocation also
requires a durable recipient ACK for the current generation; this guarantees
that no chain can have several locally pending transitions. Rotation without a
chain, double revocation, stale/non-enrolled recipient Devices and capacity
overflow also fail closed. A legacy M0.9.53 binding without a lifecycle head
remains readable for compatibility; creating a new first offer starts its
authoritative chain and makes older same-direction bindings inactive.

## 4. Automatic authenticated exchange

Every 30 seconds, before ordinary outbound message work, the serialized
runtime considers at most one local update that lacks a durable ACK. A later
generation is eligible only after its predecessor has been acknowledged. This
keeps retries idempotent and prevents a recipient from seeing a gap. In-memory
oldest-attempt ordering prevents one unavailable contact from starving fresh
updates for other contacts; durable convergence remains defined only by ACKs.

The runtime resolves the exact current endpoint of the update's recipient,
performs the existing Account-Root/Device authorization handshake, then sends
one bounded `MailboxCapabilityUpdatePush` request over QUIC. Direct/relay route
selection follows that endpoint's authenticated ticket; capability material is
never copied into the ticket or endpoint-publication system.

Failures are isolated to this periodic action and do not terminate the runtime.
No Windows Task Scheduler task or second background owner is created.

## 5. Recipient application and ACK

The receiver accepts an update only when its signed owner/recipient direction
exactly matches the authenticated Device session and a unique enrolled runtime
contact with the same opaque scope. It then validates the complete retained
chain before changing state.

For activation, the receiver HPKE-opens the offer with its local encryption
identity and the authenticated sender certificate, verifies the exact scope and
binding ID, and atomically persists both the local wrapper around the peer
write capability and the peer update. For revocation it requires the named old
binding to be retained and atomically appends the update. An expired historical
activation can still be cryptographically imported at its signed creation time
so the chain can advance; its embedded expiry remains unchanged, so it is never
usable for message delivery at the current time.

Only after durable application does the receiver return a
`SignedMailboxCapabilityAcknowledgement`. The ACK is signed by the exact
recipient Device and binds the transport session, update ID, both identities,
generation, binding and activation/revocation bit. The sender verifies it and
persists it append-only before considering that generation converged.

## 6. Current-head enforcement

The chain head is authoritative wherever mailbox capabilities are used:

- a local activation head is polled; during one unacknowledged rotation the
  previous acknowledged active binding is also polled as a bounded handover
  overlap, preventing messages sent just before peer convergence from being
  stranded;
- a local revocation head stops polling immediately;
- a peer activation head is the only binding eligible for fallback delivery;
- a peer revocation head disables fallback for that Device/scope;
- a pre-existing dispatch whose binding was superseded or revoked fails closed
  instead of uploading with stale authority.

Old append-only records are retained for audit and chain verification. A
revocation cannot erase an already leaked write key or ciphertext already
accepted by an untrusted store. It prevents conforming clients from using or
polling the old binding; normal mailbox expiry and storage quotas bound residual
opaque data.

## 7. State and IPC v22

The encrypted runtime state adds three bounded append-only record families:

- local signed updates (`.lmu`);
- accepted peer signed updates (`.pmu`);
- recipient-signed session ACKs (`.mua`).

Load validates authenticated filenames, identities, binding references, every
contiguous chain and every ACK. Local updates, peer updates and ACKs share a
hard total limit of 4096 records.

Authenticated IPC v22 mailbox status reports local and peer update counts,
acknowledged/unacknowledged local generations and local/peer revoked-head
counts in addition to the existing binding, dispatch and client-ledger state.

## 8. Verification boundary

`scripts/verify-kilogram-mailbox-capability-lifecycle.ps1` fails closed unless:

- the network-free signed chain and rotation/revocation rules remain present;
- bounded push/ACK/rejection protocol variants remain wired;
- automatic push precedes ordinary delivery work;
- durable recipient application precedes its signed ACK;
- current-head guards protect both read and write capability selection;
- IPC exposes pending convergence and revoked heads;
- capability types do not enter endpoint-publication source;
- no new executable target appears.

The provisioning and protocol network-free regressions exercise chain
round-trip, gap/fork/wrong-revocation rejection and bounded wire frames.
Workspace check and strict Clippy compile the complete runtime path. In keeping
with the stable-executable/firewall policy, no network-bearing Cargo harness or
runtime executable is launched automatically, and no release/ZIP artifact is
created.

## 9. M0.9.56 follow-up and deferred work

M0.9.56 completed the transport-independent convergence state machine and
deterministic activation/lost-ACK/restart/rotation/revocation regression. The
shared contract and runtime wiring are specified in
[`RFC-0078`](RFC-0078-transport-independent-mailbox-capability-convergence.md).
A controlled two-device live run remains necessary before claiming the online
path field-verified.

Still deferred are multi-store replication, private retrieval, traffic
padding, push wakeup, volunteer storage/relay admission and quotas, Sybil/spam
resistance, unlinkability and production service deployment.
