# RFC-0086: Authenticated bounded volunteer provider gossip (M0.9.64)

Status: implemented and locally verified without opening a network endpoint.
Actual mailbox-envelope replication is not included.

## 1. Outcome

Kilogram runtimes now discover volunteer blind-storage providers from their
existing contacts without a global directory or another server. At most once
per five minutes, when higher-priority delivery work is idle, a runtime chooses one
contact and performs a bidirectional provider-offer exchange over the existing
Kilogram Iroh ALPN. The normal Root-authorized Device session handshake happens
before the gossip request is accepted.

This is a short dedicated application request on an authenticated connection;
it is not a new listener, executable, background service or unauthenticated
broadcast. A later optimization may piggyback the same bounded frame onto a
message/sync connection.

The exchange payload contains only:

- exact store-signed provider offer bytes;
- an offer hop count;
- short-lived frame times and random entropy;
- an optional opaque request-frame digest in a response.

It contains no Account ID, Device ID, conversation ID, mailbox ID, read
capability or write capability. The authenticated peers necessarily know each
other at the session layer, so this milestone does not claim to hide contact
access correlation from either endpoint or a network observer.

## 2. Bounds and verification

Every frame is canonical postcard data, at most 24 KiB on the protocol wire
and at most 20 KiB at the mailbox-client boundary. It carries zero through
eight offers, limits each amplified offer to 2 KiB, and expires after at most
60 seconds. An empty request is allowed
so a new client can pull its first provider set.

Before any registry mutation the receiver verifies:

1. the frame version, size, canonical encoding and time window;
2. every individual store signature and current offer expiry;
3. a maximum offer age of 15 minutes and amplified offer size of 2 KiB;
4. a hop count from one through two;
5. unique store keys within the frame;
6. every signed provider endpoint as a valid Iroh endpoint.

The response carries the digest of the exact request frame. The initiator
rejects a response that is not bound to its request. Invalid transport data
rejects the whole frame before durable writes. Registry capacity conflicts or
same-generation offer conflicts reject the affected entry without evicting a
live provider selected by local state.

## 3. Hop provenance and randomized subsets

The Redb provider registry now stores one byte of provenance in a separate
table, preserving compatibility with the M0.9.63 offer records. A direct local
observation, including the runtime's own provider offer or explicit operator
import, has hop zero. A received frame stores the transmitted hop. A client may
gossip an offer only while the stored hop is less than two; transmission adds
one.

Replaying an exact offer through gossip cannot lower already stored provenance.
A later direct observation may lower it to zero. A newer store-signed offer
starts a new generation with the newly observed provenance.

Candidates are ranked with a domain-separated BLAKE3 score over fresh random
frame entropy, store key, transport identity and offer ID. At most one store
key per parsed Iroh endpoint identity is sent. Therefore two exchanges need not
expose the same subset of a large registry.

Hop values are an honest-client bandwidth and propagation bound, not a
cryptographic path proof: a modified client can originate a new frame and lie
about how it learned an unchanged public offer. Signature, expiry, count,
frame-size and local registry bounds remain enforceable against such a peer.

## 4. Provider bootstrap and refresh

When bounded volunteer storage is active, the runtime imports its own freshly
signed endpoint offer into the same local provider registry. It re-signs and
reimports that offer every five minutes; the offer itself remains valid for 15
minutes. This keeps the 15-minute gossip freshness window useful during a
long-running client session without asking the user to restart the messenger.

The existing manual import and diagnostic selection IPC remain available for
testing and recovery, but normal peer exchange no longer depends on copying a
base64url offer by hand.

## 5. Scheduling and failure behavior

Provider gossip is lower priority than message delivery, pending mailbox work
and capability convergence, and starts at most once per five minutes globally.
The first automatic exchange waits for that interval rather than adding traffic
to runtime startup.
It uses the contact's authority-current endpoint
candidates and the existing endpoint failover order. A failed exchange is
reported and retried only after the per-contact interval; it does not mark a
message delivered or alter mailbox capabilities.

The exchange itself consumes no volunteer mailbox storage or volunteer data
quota. Those quotas apply when the dedicated blind-mailbox ALPN later carries
PUT/LIST/DELETE operations. This milestone still reports
`runtime_volunteer_storage_replication=false`.

## 6. Remaining boundary

The next slice should bind a fresh random provider-selection salt to each
durable outbox dispatch, encrypt one opaque mailbox item, send it to a small
transport-distinct provider set over the existing blind-mailbox ALPN, and keep
independent store-signed receipts. Direct peer delivery must remain preferred;
provider replication is the offline fallback, not a mandatory server path.

Private retrieval scheduling, provider reputation, Sybil resistance, erasure
coding and collusion-resistant access patterns remain outside this milestone.

## 7. Verification

Network-free unit tests cover frame round trips, response binding, exact offer
signature and endpoint parsing, count/size/time limits, hop propagation,
non-resettable gossip replay, direct re-observation and stale-offer suppression.
Protocol tests cover the opaque authenticated carrier and its wire limit.

`scripts/verify-kilogram-volunteer-provider-gossip-boundary.ps1` fails closed
if the bounds, separate hop state, authenticated request/response carrier,
automatic scheduling/refresh, social-ID-free payload or no-new-EXE boundary
disappears.
