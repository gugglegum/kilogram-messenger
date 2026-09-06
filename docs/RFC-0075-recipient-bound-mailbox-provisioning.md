# RFC-0075: Recipient-bound mailbox capability provisioning (M0.9.53)

Status: implemented in M0.9.53.

## 1. Scope

RFC-0073 and RFC-0074 intentionally left the blind mailbox read/write keys and
the authenticated service URL/key outside tickets, discovery and client
ledgers. M0.9.53 closes that provisioning gap without yet enabling mailbox
traffic in the long-lived runtime actor.

The new boundary provides:

1. a full local receive binding encrypted to its owning Device;
2. a peer artifact containing only the write capability, signed by the mailbox
owner and HPKE-encrypted to one exact recipient Device. The outer artifact
contains only a version and opaque binding ID, not clear Account/Device IDs;
3. authenticated append-only runtime records for both local and imported
   bindings;
4. fail-closed import against the current pinned peer authority and exact
   conversation/contact scope;
5. explicit status `provisioned-not-yet-enabled`, so capability exchange is not
   reported as message delivery.

It creates no executable, performs no background work and does not put a
mailbox capability in a public connection ticket.

## 2. Capability split

Each receive mailbox has independent Ed25519 read and write capabilities. The
owning Device retains both. A peer receives only the write secret needed to
upload opaque envelopes:

```text
owner Device state
  read secret + write secret + mailbox address + HTTPS URL + store key
  -> Device-signed
  -> HPKE sealed to the owner's local encryption key

peer offer artifact
  write secret + mailbox address + HTTPS URL + store key + exact scope/expiry
  -> signed by owner Device
  -> HPKE sealed to one exact peer Device certificate
```

Possession of the write capability permits bounded uploads and is therefore a
spam authority. It is not public metadata. The read secret never leaves the
mailbox owner and neither secret is stored in the RFC-0074 Redb client ledger.

## 3. Authenticated binding

The provisioning object binds all of the following under the owner Device
signature and recipient-key HPKE encryption:

- owner Account and Device;
- owner encryption public key;
- recipient Account and Device;
- opaque application scope supplied by the runtime (the conversation ID);
- mailbox address and therefore both public capability keys;
- canonical service base URL and expected store signing key;
- creation and expiry timestamps.

The binding ID hashes both device directions, the scope and the mailbox ID.
Import accepts an offer only when:

- it decrypts under the current local Device encryption key;
- its recipient matches the local certificate exactly;
- its Device signature matches one enrolled endpoint in the contact;
- that endpoint is active at the pinned Root-signed peer authority high-water;
- its conversation scope matches the selected contact;
- it has not expired and its validity is between 60 seconds and 30 days;
- the embedded write secret derives the advertised write public key.

An expired or revoked-device offer remains auditable append-only state but is
reported unusable. Re-importing the byte-identical offer is idempotent;
reusing a binding ID for different ciphertext fails closed.

## 4. Service authentication

The signed service descriptor contains a canonical URL and an Ed25519 store
public key. Remote services require HTTPS. Plain HTTP is accepted only for a
numeric loopback IP, matching the RFC-0074 client rule. Credentials, query and
fragment components are rejected. The store public key is parsed and
canonicalized at provisioning time and later pins signed mailbox receipts.

TLS authenticates the network endpoint; the pinned store key authenticates
application receipts. Neither alone authorizes a peer to read or write a
mailbox.

## 5. Runtime state and commands

The CLI exposes three offline/local-state operations:

- `runtime-mailbox-offer-create` creates a fresh per-contact/per-device receive
  mailbox, persists the full capability sealed to the local Device and writes
  one no-clobber encrypted peer offer;
- `runtime-mailbox-offer-import` decrypts and validates a received offer,
  checks the current peer authority and persists the original encrypted offer
  inside a local Device-signed runtime record;
- `runtime-mailbox-status` reopens local capabilities, revalidates peer offers
  against current endpoint/authority state and reports usable/unusable counts.

The records live below `runtime/local-mailbox-bindings` and
`runtime/peer-mailbox-bindings`. They use the existing state transaction and
encrypted-vault mirror; there is no parallel state owner. Total retained
mailbox bindings are bounded to 1024.

Offer transfer is an explicit M0 ceremony. The artifact is safe to copy through
an untrusted file transport because its contents are recipient-HPKE encrypted,
but availability and traffic-analysis privacy are not provided by that file
transport. A later authenticated online exchange can carry the same object.

## 6. Enforced boundary and verification

`kilogram-mailbox-provisioning` depends on identity, crypto, the blind mailbox
contract and URL parsing. It has no direct network, database, runtime, protocol,
event-store or transport dependency and defines no binary.

`scripts/verify-kilogram-mailbox-provisioning-boundary.ps1` fails if those
dependencies or runtime/event identifiers cross into the crate. Four
network-free tests cover:

- local sealed binding and peer offer round trip;
- exact recipient/source/scope/expiry enforcement;
- ciphertext tamper rejection;
- HTTPS/loopback URL policy.

Network-bearing test harnesses remain compile-only. M0.9.53 does not run a
listener, build a release binary or create a ZIP package.

## 7. Runtime data path follow-up

M0.9.54 implemented the runtime data path in
[`RFC-0076`](RFC-0076-runtime-mailbox-fallback-and-ack.md):

1. try current direct/relay endpoint candidates first;
2. on bounded policy failure, create one deterministic mailbox item and retry
   its exact encrypted PUT until a signed store receipt or expiry;
3. report `mailbox-stored` separately from peer-acknowledged `delivered`;
4. poll bounded pages, validate the opened `AuthorizedEvent`, and commit the
   event plus local projection in the existing transaction before deletion;
5. persist and return the recipient-signed acknowledgement through the reverse
   mailbox when no live route exists;
6. expose pending/stored/received/deleted/expired/failure states over
   authenticated runtime IPC.

Capability rotation/revocation, automatic online offer exchange, multi-store
replication, volunteer storage admission, private retrieval, padding and
Sybil/DDoS resistance remain separate work.
