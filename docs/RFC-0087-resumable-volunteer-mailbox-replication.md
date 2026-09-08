# RFC-0087: Resumable volunteer mailbox replication (M0.9.65)

Status: implemented for sender-side replication and independent durable
receipts. Recipient-side Iroh LIST/DELETE scheduling is intentionally deferred.

## 1. Outcome

When direct authenticated delivery fails, the ordinary Kilogram runtime now
copies the already encrypted mailbox PUT to a small set of discovered volunteer
providers over the dedicated blind-mailbox Iroh ALPN. No new executable,
listener, global directory, Account service or scheduled task is added.

The current compatibility sequence is:

1. try direct delivery to the recipient;
2. prepare one recipient-encrypted, capability-authorized mailbox PUT;
3. durably commit the volunteer replication plan;
4. upload the opaque PUT through the recipient's existing HTTPS mailbox binding
   so current clients can retrieve it without waiting for unavailable volunteers;
5. asynchronously replicate it to three transport-distinct remote providers,
   retain independent store-signed receipts and satisfy the current durability
   policy after two receipts. If HTTPS is unavailable, one volunteer attempt is
   made immediately while the primary delivery remains pending.

The HTTPS upload remains a temporary compatibility path, not a claim that a
central mailbox is required by the target architecture. Removing that path
before volunteer Iroh LIST/DELETE discovery exists would report messages as
delivered while leaving recipients unable to find them.

## 2. Durable per-item plan

A fresh random 32-byte provider-selection salt is created for an exact signed
runtime mailbox dispatch. The separate replication ledger binds:

- the domain-separated digest of the complete Device-signed dispatch;
- the exact capability-authenticated `MailboxPutRequest`;
- mailbox and item identifiers already present in that request;
- the random selection salt;
- target and receipt thresholds;
- creation and expiry times.

The plan is committed with immediate Redb durability before any provider is
dialled. Replay after restart retains the original salt and exact encrypted PUT;
a conflicting dispatch or request fails closed. No Account ID, Device ID or
conversation ID is stored in this ledger.

Keeping the exact request is necessary even after the compatibility HTTPS
upload succeeds: otherwise a temporarily unavailable volunteer could never be
retried after the old outbound ledger removes its pending record.

## 3. Provider selection and PUT

The runtime applies the deterministic M0.9.63 ranking to the durable salt. It
selects at most three usable remote providers and excludes:

- the runtime's own Iroh endpoint, because self-storage is not independent
  durability and a synchronous self-dial could deadlock the accept loop;
- providers sharing a transport identity with an already retained receipt;
- providers whose signed maximum record size is smaller than the envelope;
- store keys that have already issued a retained receipt.

Every remote PUT uses `kilogram/m0/blind-mailbox/1`. The outer response is
bound to the exact peer request digest; the inner response and store-signed
receipt are then verified against the exact request and selected store key.
Only verified receipts enter the durable ledger. A second store key on the same
transport identity cannot count as another replica.

Provider failure is isolated and does not prevent or delay the compatibility
HTTPS delivery. Incomplete plans remain independently resumable after the
primary delivery record is complete. Attempts have a durable 60-second cooldown
and expired plans, attempts and receipts are bounded and removed together.

## 4. Privacy and trust boundary

Providers receive the same opaque E2EE envelope and a scoped, expiring mailbox
write authorization. They do not receive Kilogram Account, Device or
conversation identifiers and cannot decrypt the event. They can observe their
own request timing, byte size, mailbox pseudonym and network peer. Colluding
providers can correlate identical ciphertext and mailbox pseudonyms; padding,
private retrieval and collusion-resistant access patterns remain future work.

A signed stored receipt proves only that one store accepted exact bytes until
the stated expiry. It does not prove future availability, honest deletion,
operator independence or Sybil resistance. The two-of-three rule is therefore
a concrete durability policy and evidence threshold, not a Byzantine-storage
guarantee.

## 5. Compatibility and migration

The replication state uses a new database file and does not change the existing
mailbox-client ledger or signed dispatch encoding. Old queues, bindings and
profiles remain readable. Existing profiles that have volunteer storage
disabled can still send through their current mailbox binding; they simply may
have no discovered volunteer offers yet.

Reverse mailbox acknowledgements are not replicated in this slice because
they have no durable outbox dispatch. They continue through the compatibility
mailbox path.

## 6. Remaining boundary

M0.9.65 does not yet let the recipient discover and poll the exact volunteer
replica set. The next slice should add bounded Iroh LIST/DELETE retrieval over
verified provider offers, preserve application-commit-before-delete semantics,
and produce an Alice/Bob/volunteer field kit. Only after that path is proven may
the HTTPS mailbox copy become optional at runtime rather than merely optional
in the target architecture.

Provider reputation, proof of capacity, erasure coding, private retrieval,
Sybil resistance and push wakeup remain deferred.

## 7. Verification

Network-free unit tests cover immutable plan replay, exact encrypted-request
retention, independent store-signed receipts, restart recovery, cooldown and
expiry cleanup. Existing provider, mailbox-client and protocol tests remain in
place.

`scripts/verify-kilogram-volunteer-replication-boundary.ps1` fails closed if
the durable plan/receipt/attempt tables, random dispatch binding, two-of-three
policy, transport diversity, Iroh PUT carrier, resumable scheduling, HTTPS
compatibility marker or no-new-EXE boundary disappears.
