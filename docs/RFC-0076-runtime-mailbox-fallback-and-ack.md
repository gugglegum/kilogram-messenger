# RFC-0076: Runtime mailbox fallback and reverse acknowledgement (M0.9.54)

Status: implemented in M0.9.54.

## 1. Scope

RFC-0073 defined the blind mailbox, RFC-0074 added its bounded HTTPS adapter
and crash-safe client ledger, and RFC-0075 bound mailbox capabilities to exact
contacts, conversations and Devices. M0.9.54 connects those boundaries to the
single serialized runtime actor.

The runtime now:

1. tries the existing direct/relay endpoint candidates first;
2. after that bounded live-route attempt fails, creates exactly one durable,
   deterministic mailbox dispatch for the already materialized event;
3. distinguishes a store-signed `mailbox-stored` receipt from a recipient
   acknowledgement and therefore from `delivered`;
4. polls bounded pages of its explicitly provisioned receive mailboxes;
5. commits an authenticated event and its application projection before the
   ciphertext becomes eligible for deletion;
6. returns the recipient-signed acknowledgement through the reverse mailbox
   when the live session is unavailable;
7. exposes the resulting states over authenticated runtime IPC v21.

No second service or client executable is introduced. The existing
`kilogram-ticket-store` remains the optional opaque HTTPS storage process, and
the long-lived `kilogram-cli` runtime remains the only application-state
writer.

## 2. Outbound state machine

The live path remains authoritative and is attempted first. A mailbox is not a
parallel fast path and is used only when all current live endpoint candidates
have failed within their existing bounds.

```text
queued
  -> materialized AuthorizedEvent
  -> bounded direct/relay attempt
  -> signed RuntimeMailboxDispatch persisted in runtime state
  -> exact encrypted PUT retained in the client ledger
  -> store-signed receipt
  -> mailbox-stored
  -> recipient-signed acknowledgement (live or reverse mailbox)
  -> delivered
```

The dispatch binds the local Account/Device, queue/contact/peer, conversation,
provisioning binding, mailbox item and event, plus creation and expiry. It is
Device-signed and stored append-only through the existing state transaction and
vault mirror. Its item ID is a domain-separated deterministic hash of the
queue, event and binding, so a crash or retry cannot create several logical
copies of one dispatch.

The runtime persists both the signed dispatch and the exact pending PUT before
performing HTTPS. If a crash lands between those two local writes, the actor
recognizes the signed orphan dispatch, revalidates its queue/event/current
binding and recreates a pending envelope with the dispatch's exact item and
time bounds before any network request. A lost HTTPS response therefore leaves
a retryable identical PUT. The client ledger accepts only a positive receipt
under the pinned store key. That receipt changes the queue to
`mailbox-stored`, but does not create a delivered marker. Only an
AuthorizedEvent acknowledgement from the intended peer can do that.

The default item lifetime is 24 hours, clipped to the provisioned binding
expiry and to the mailbox protocol bounds. Expired or no-longer-current
bindings fail closed and are reported separately instead of silently falling
back to stale authority.

## 3. Authenticated mailbox payload

The HPKE plaintext is a versioned `RuntimeMailboxPayload` containing:

- the exact provisioning binding ID;
- source and recipient Account/Device IDs;
- the exact conversation ID;
- one existing `AuthorizedEvent`.

The mailbox envelope separately binds mailbox ID, item ID, creation and expiry
as HPKE AAD. After decryption, the runtime verifies the payload direction and
scope against the local receive binding, then applies the normal membership,
Device signature, event causality and ratchet checks. Storage possession or a
valid write capability is never sufficient to create an application event.

Only current ratchet text and acknowledgement event kinds cross this adapter.
The mailbox path does not define a second message or authorization protocol.

## 4. Commit-before-delete receive order

One serialized poll handles at most the first item from a signed page of at
most eight items. Usable receive bindings are selected fairly by their last
poll time. The periodic check is bounded to one action every five seconds when
no live outbound work is due.

For a text event the order is:

1. verify the signed list page and HPKE-open the exact item;
2. verify the runtime payload, membership and persistent ratchet transition;
3. atomically persist the AuthorizedEvent, local projection and locally signed
   acknowledgement through the existing state transaction;
4. durably enqueue that acknowledgement for the peer's separately provisioned
   reverse mailbox;
5. record the stable application commit ID in the client ledger;
6. derive the conditional delete request only from that committed ledger
   state;
7. accept only a store-signed delete receipt;
8. attempt the already durable reverse-acknowledgement PUT.

An acknowledgement item follows the same ingress verification. Its application
transaction stores the event and matching delivered marker for the exact local
outbox event before deletion.

All application commits are idempotent. A crash before step 5 leaves the remote
item available for replay; a crash after step 5 leaves an eligible pending
delete; a failed reverse PUT leaves the exact encrypted request in the ledger
for retry. The store can still withhold or discard data, but it cannot make the
runtime claim recipient delivery without the peer's signed acknowledgement.

## 5. Runtime scheduling and failure isolation

Mailbox work runs in the same actor loop as IPC, inbound Iroh sessions, live
outbox delivery and automatic synchronization. Network waits happen outside
short state-lock sections. There is no hidden Task Scheduler registration and
no second background owner.

On an idle mailbox interval the actor first retries one retained outbound PUT;
otherwise it polls one receive binding. Errors are logged as mailbox-specific
failures and do not terminate the runtime. Live delivery still has priority,
and a queue with a durable mailbox dispatch is not rematerialized into another
live attempt while awaiting its acknowledgement.

This is a correctness-oriented M0 policy. Adaptive backoff, batching, push
wakeup, traffic padding and production-scale scheduling are deferred.

## 6. Honest IPC v21 states

`RuntimeIpcQueueState` now includes:

- `mailbox-pending`: exact encrypted PUT is durably retained;
- `mailbox-stored`: the configured store signed acceptance of the item;
- `mailbox-expired`: the dispatch lifetime ended;
- `mailbox-failed`: retained dispatch state or its current binding is invalid.

`delivered` remains separate and requires the peer acknowledgement. Outbox
status exposes counts for all four mailbox queue states.

The new authenticated `MailboxStatus` command reports local/peer binding and
usable counts, dispatch count, ledger pending/stored/received/deleted counts,
expired/failed dispatch counts and the literal policy
`active-direct-relay-first-mailbox-fallback`. The Windows desktop projection
maps the same queue states; it does not infer delivery from a storage receipt.

## 7. Verification boundary

`scripts/verify-kilogram-runtime-mailbox-flow.ps1` fails closed unless:

- live direct/relay delivery is ordered before mailbox fallback;
- a signed orphan dispatch has an automatic repair path before network retry;
- the application commit marker is ordered before conditional delete;
- the signed payload, dispatch and deterministic item-ID definitions remain;
- reverse-mailbox acknowledgement preparation remains wired;
- the IPC pending/stored/expired/failed states and mailbox status remain;
- no new executable target is added.

The mailbox-client network-free ledger regression verifies the outbound
pending-to-stored transition and cleanup. Workspace checks and strict Clippy
compile all runtime paths and test harnesses. Network-bearing harnesses remain
compile-only so an unsolicited Windows Firewall dialog is not created. No
release build or ZIP package is produced for this milestone.

## 8. Deferred work

The capability lifecycle gap is implemented by
[RFC-0077](RFC-0077-authenticated-mailbox-capability-lifecycle.md): the same
recipient-encrypted offer now travels through an authenticated Device session,
supports explicit ordered rotation/revocation and converges current bindings
without placing secrets in public tickets or endpoint publications.

Still deferred are multi-store replication/erasure coding, private retrieval,
push wakeup, volunteer storage admission and quotas, spam/Sybil resistance,
padding, unlinkability and a production storage deployment. A single mailbox
service can still observe IP address, timing, ciphertext size and repeated
access to one pseudonymous mailbox.
