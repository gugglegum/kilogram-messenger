# RFC-0074: Bounded mailbox HTTP adapter and crash-safe client ledger (M0.9.52)

Status: implemented in M0.9.52.

## 1. Scope

M0.9.51 defined a network-free blind mailbox contract. This slice gives those
exact signed objects a bounded HTTPS transport and a persistent client-side
state machine. It deliberately does not yet attach mailbox delivery to the
long-lived messaging actor.

The implementation has three layers:

1. `kilogram-mailbox` owns versioned bounded Postcard request/response objects;
2. the existing `kilogram-ticket-store` process exposes blind-mailbox routes
   behind the same required HTTPS reverse proxy;
3. `kilogram-mailbox-client` owns the no-redirect HTTPS adapter and a Redb
   ledger for crash-safe retry, application commit and conditional deletion.

No additional executable is introduced. Direct Iroh/QUIC and live relay
delivery remain the preferred future runtime paths; mailbox is a delayed
fallback, not a replacement transport.

## 2. Bounded wire contract

All requests contain the exact mailbox address and signed authorization from
RFC-0073. The path is checked against the authenticated body before storage is
accessed:

- `PUT /v1/mailboxes/{mailbox_id}/items/{item_id}`;
- `POST /v1/mailboxes/{mailbox_id}/list`;
- `DELETE /v1/mailboxes/{mailbox_id}/items/{item_id}`.

The media type is
`application/vnd.kilogram.blind-mailbox-v1`. Redirects are rejected by the
client. Non-loopback service URLs must use HTTPS; plain HTTP is accepted only
for a numeric loopback address intended for a local TLS reverse proxy/test.

List authorization signs a fresh nonce, an optional exclusive item cursor and
a page limit. A page contains at most eight items. Response verification checks
the wire version, strict item ordering, cursor consistency, every ciphertext
receipt and the expected store signing key. The serialized response is bounded
to slightly more than eight maximum-size envelopes rather than an unbounded
mailbox dump.

The M0 service does not retain a nonce replay set. The nonce binds and
uniquifies an honest list request, but an exposed signed request could still be
replayed; HTTPS protects it in transit and a future challenge/rotation design
must provide stronger replay resistance.

Positive `put` and `delete` results carry store-signed receipts. Capacity,
conflict, absent and tombstoned results are bounded protocol responses but are
not availability proofs: a malicious store can already withhold traffic or
return a negative answer. Clients advance durable delivery/deletion state only
after a valid positive receipt from the pinned store key.

## 3. Existing service process

`kilogram-ticket-store` now opens a separate blind-mailbox Redb database below
its configured data directory. It creates one 32-byte Ed25519 store secret with
exclusive creation, rejects symlinks and wrong lengths, pins the public key in
the mailbox database and prints the public key at startup. The existing ticket
publication database and API remain separate.

The process still binds only to loopback. Deployment must terminate HTTPS in a
reverse proxy and forward to the loopback listener. The store public key is an
application receipt identity, not a substitute for TLS authentication. A
client must receive the expected service URL and store key through an
authenticated provisioning path; that provisioning is deferred to M0.9.53.

Mailbox cleanup participates in the existing bounded service cleanup loop.
The M0 implementation still performs bounded full-store scans and is not
claimed to be a production-scale index.

## 4. Crash-safe client ledger

`kilogram-mailbox-client` persists four disjoint record classes:

- pending outbound: the exact already-encrypted request and expected store key;
- stored outbound: the verified signed acceptance receipt;
- inbound committed: the stored receipt plus the caller's stable application
  commit ID;
- inbound deleted: the verified signed deletion receipt.

The ledger contains ciphertext and public/signed capabilities, not recipient
HPKE secrets or mailbox read/write secret keys. Those secrets remain owned by
the future protected contact/device state.

The required outbound transition is:

```text
encrypted exact request -> durable pending -> retry identical PUT
                        -> verify signed stored receipt -> durable stored
```

The required inbound transition is:

```text
signed page -> verify receipt/store/path -> HPKE open -> application validates
            -> application durably commits -> ledger records commit ID
            -> signed conditional DELETE -> ledger records delete receipt
```

Before the application commit record exists, the ledger cannot construct a
delete request. Retrying the same logical receipt or application commit after a
crash is idempotent even if the local observation timestamp changed. A cleanup
transaction removes expired pending ciphertext and expired receipts so bounded
tables do not remain permanently full.

The application callback in the next runtime layer must itself be idempotent by
mailbox item/event identity. The ledger cannot prove that an arbitrary caller
really flushed its own database; it only makes the commit boundary explicit and
durable before exposing deletion.

## 5. Enforced boundary and verification

`scripts/verify-kilogram-mailbox-client-boundary.ps1` requires Reqwest, Redb and
the mailbox/crypto contracts while rejecting Iroh and Kilogram identity,
protocol, ratchet, runtime IPC, session, state, event-store and transport
dependencies. It also rejects application identifier types and a new binary in
the client crate. The server integration is required to remain in the existing
opaque service process.

Network-free regressions cover:

- signed paginated wire round trips and request/store binding;
- HTTPS-only URL policy without opening a socket;
- durable pending/stored/commit/delete transitions across reopen;
- fail-closed deletion before application commit;
- idempotent receipt replay and expiry cleanup;
- direct invocation of all three server routes without binding a listener.

Network-bearing workspace harnesses are compile-only. This milestone does not
run a listener, produce a release build or create a ZIP package.

## 6. Follow-up work

M0.9.53 added recipient-bound per-contact/per-device capability and
authenticated store URL/key provisioning in
[`RFC-0075`](RFC-0075-recipient-bound-mailbox-provisioning.md). M0.9.54 should
connect those verified bindings and this ledger to the serialized runtime
actor:

1. attempt direct/relay delivery first and enqueue mailbox fallback only under
   explicit bounded policy;
2. retry pending uploads until a signed receipt or expiry;
3. poll signed pages, validate the opened application object and commit it to
   the existing event/projection transaction before deletion;
4. return acknowledgements through the peer's reverse mailbox when necessary;
5. expose honest pending, stored, received, expired and failed states through
   authenticated IPC.

Multi-store replication, erasure coding, volunteer storage admission, mailbox
rotation, private retrieval, traffic padding and Sybil/DDoS resistance remain
out of scope.
