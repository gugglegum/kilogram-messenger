# RFC-0073: Blind mailbox contract and durable store (M0.9.51)

Status: implemented in M0.9.51.

## 1. Scope

Kilogram needs bounded asynchronous delivery when the recipient is offline and
no direct or live relay path is available. The first mailbox slice deliberately
defines a network-free cryptographic/storage boundary before choosing an HTTP,
P2P or volunteer-node adapter.

The storage node receives only:

- two unrelated Ed25519 public capability keys and their derived mailbox ID;
- a random item ID;
- an HPKE ciphertext envelope;
- requested TTL, byte length and access timing;
- signed capability requests and storage receipts.

The mailbox API has no Account, Device, contact, conversation, message or event
identifier. It does not receive a decryption key or readable history.

This is not an anonymity claim. A future network adapter can still expose source
IP, request timing, sizes, repeated mailbox access and the relationship between
operations observed by the same node.

## 2. Capabilities and address

A mailbox address contains two independently generated Ed25519 public keys:

- the write key authorizes `put` for an exact mailbox ID, random item ID, TTL,
  ciphertext size and BLAKE3 digest;
- the read key authorizes either `list` with a random request nonce or
  conditional `delete` for an exact item and stored-receipt ID.

The mailbox ID is a domain-separated BLAKE3 hash of the two public keys. No
registration is necessary and possession of the public address does not grant
read or write authority. The recipient retains the read signing capability and
distributes a scoped write capability to an authorized sender.

Address, authorizations and receipts use bounded Postcard encodings. Every
public decode path validates the version, key encoding, operation binding,
size bounds and signature before returning a value.

## 3. Recipient-encrypted envelope

`MailboxEnvelope` seals opaque application bytes to the recipient encryption
public key through the existing `kilogram-crypto` HPKE construction. The HPKE
associated data binds:

- protocol version;
- mailbox ID;
- random item ID;
- creation and expiry time.

Opening requires the expected mailbox and item IDs, the recipient encryption
secret and a time before expiry. A copied envelope therefore cannot be silently
relabelled into another mailbox or item.

Protocol limits are:

- TTL: 60 seconds through 7 days;
- plaintext: at most 512 KiB;
- serialized envelope: at most 1 MiB.

The payload is intentionally not coupled to the current Kilogram event schema.
The later client adapter will place already authenticated application material
inside this encryption boundary and verify it again after opening.

## 4. Durable storage semantics

`BlindMailboxStore` uses Redb and an explicit store signing identity. The first
open pins the store public key in the database; reopening the same database with
a different identity fails closed.

`put` performs one immediate-durability transaction and returns one of:

- `Created(signed receipt)`;
- `AlreadyPresent(same receipt)` for an exact retry;
- `Conflict` when the same item ID is reused with different content or TTL;
- `Tombstoned` after a previously accepted item was deleted;
- `CapacityExceeded` when a configured bound is reached.

The signed stored receipt binds the store key, mailbox/item IDs, ciphertext
length and digest, acceptance time and expiry. It proves that this store key
accepted those exact bytes; it does not prove future availability, replication
or honest deletion.

`list` requires a read-key signature and returns only live items with their
signed receipts. Expired or malformed records are removed before the result is
returned.

`delete` requires the read capability and exact stored-receipt ID. A successful
delete atomically removes the item and creates a signed tombstone lasting until
the original expiry. Exact retries return the same delete receipt; a stale or
different receipt cannot delete a replacement. The tombstone prevents delayed
replay of the original `put` from resurrecting an acknowledged item.

## 5. Resource bounds

Defaults are intentionally finite:

- 128 live items per mailbox;
- 100,000 live items per store;
- 1 GiB of live ciphertext per store;
- tombstones bounded by the global item limit;
- configurable envelope limit no larger than the 1 MiB protocol limit.

Configuration also has hard upper bounds. Live item/byte counts and per-mailbox
counts are persisted and updated in the same Redb transaction as item changes.
Cleanup recomputes counters from authenticated records after removing expired
or malformed data.

The M0 implementation uses a full bounded store scan for cleanup and mailbox
listing. This is correct for the current finite test store, not the final
large-scale index design.

## 6. Enforced architecture boundary

`kilogram-mailbox` depends on `kilogram-crypto`, Redb and small serialization /
signature primitives. It has no Iroh, Tokio, Reqwest, runtime IPC, session,
state, event store or transport dependency.

`scripts/verify-kilogram-mailbox-boundary.ps1` checks the locked normal
dependency graph and rejects application identifier types in the mailbox
source. This is a fail-closed guard against accidentally turning a blind store
into a second messaging backend with readable identifiers.

All M0.9.51 executable tests are network-free. They cover HPKE binding, wrong
capabilities, TTL expiry, capacity, exact put replay, conditional deletion,
tombstone replay protection, persistence across reopen and store-identity pinning.

## 7. Deferred work

M0.9.52 adds the bounded wire/HTTPS adapter and crash-safe client ledger in
[`RFC-0074`](RFC-0074-bounded-mailbox-http-and-client-ledger.md). The following
live integration remains deferred:

1. provision mailbox capabilities and authenticated store URL/key through the
   contact/device protocol;
2. connect runtime outbox retry to the implemented ledger and `put` receipts;
3. transactionally ingest opened application events before the implemented
   conditional-delete boundary;
4. expose honest pending/delivered/expired states in runtime IPC;
5. retain direct/relay delivery as the preferred path and mailbox as fallback.

Replication, erasure coding, Sybil-resistant admission, privacy relays, cover
traffic, nonce freshness challenges and volunteer-node incentives remain out of
scope. A single mailbox operator can still observe metadata, withhold data or
delete ciphertext early.
