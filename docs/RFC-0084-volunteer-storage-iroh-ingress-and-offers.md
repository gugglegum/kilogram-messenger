# RFC-0084: Volunteer storage Iroh ingress and signed offers (M0.9.62)

Status: implemented as a provider-side protocol and manual offer-export
boundary. Automatic discovery, selection and replication are not included.

## 1. Outcome

An enabled ordinary Kilogram runtime now advertises two independent Iroh
application protocols on the same endpoint:

- `kilogram/m0/sync/8` for authenticated messenger sessions;
- `kilogram/m0/blind-mailbox/1` for volunteer blind storage.

The second ALPN is registered only when the volunteer role is enabled for the
network class detected at startup. Mobile and unknown networks therefore keep
the default zero-traffic policy and do not expose the mailbox ALPN.

There is still one ordinary runtime process and one Iroh endpoint. No public
TCP listener, new executable, Windows service or Task Scheduler entry is
introduced. The loopback HTTP adapter remains available as a local integration
and compatibility surface, but remote volunteer requests use Iroh directly.

## 2. Capability-authenticated operations

The peer protocol carries the existing bounded mailbox PUT, LIST and DELETE
requests. Their capability signatures are verified before any mailbox state is
read or mutated:

- PUT proves possession of the unrelated mailbox write capability and binds
  the exact mailbox, item, TTL and ciphertext digest;
- LIST and DELETE prove possession of the mailbox read capability and bind the
  exact operation, nonce/cursor or stored receipt;
- store responses retain the existing store-signed receipts;
- the outer response includes the exact operation and a domain-separated hash
  of the complete request, preventing cross-request response substitution.

Iroh authenticates the remote endpoint identity at the transport layer. That
identity is used only for per-peer rate limiting; it does not grant mailbox
access. Account, Device, conversation and event identifiers are not added to
the blind-storage wire format.

## 3. Bounded provider execution

Mailbox connections are dispatched separately from messenger sessions after
ALPN negotiation. They run in bounded tasks guarded by the configured
connection semaphore, so a slow stranger cannot monopolize the main runtime
accept loop. Each connection accepts one bounded bidirectional request and
then closes.

The provider reuses M0.9.61's durable network-class transfer counter and blind
store capacity. Request and successful response frames consume the same
application-payload budget as the loopback adapter. Requests are also limited
per authenticated Iroh endpoint and globally. Invalid capability frames fail
closed before storage access.

## 4. Store-signed expiring offer

At startup the provider emits a base64url-encoded
`SignedMailboxStorageOffer`. Its store-key signature binds:

- the complete serialized Iroh provider endpoint, including endpoint ID and
  currently known direct/relay addressing;
- the mailbox store public key;
- the `BoundedVolunteer` policy class;
- capacity and maximum-record hints;
- issue and expiry timestamps;
- a fresh random nonce.

Offer validity is bounded by the protocol to 30 seconds through one hour; the
runtime currently uses 15 minutes. A client must verify the signature and
freshness before decoding and dialing the enclosed endpoint, and must continue
to verify store receipts against the same store key.

Offer distribution is deliberately manual in this slice. Printing an offer is
not anonymous discovery, proof of free capacity, reputation or Sybil
resistance. It is a verifiable object that the next discovery layer can carry
without redefining provider trust.

## 5. Remaining boundary

M0.9.62 makes the embedded provider remotely addressable when a verified fresh
offer is already available. It does not yet make ordinary delivery choose that
provider. The next slice should add privacy-bounded offer discovery and a
client-side provider set, then replicate one opaque envelope to several
unrelated providers while retaining application-commit-before-delete rules.

No claim is made yet about availability, fair global allocation, resistance to
colluding providers or metadata anonymity. The standalone HTTPS store remains
an optional bootstrap/reference/test fixture rather than mandatory
infrastructure.

## 6. Verification

Network-free tests cover:

- store-signed offer encoding, tamper rejection and expiry;
- exact Iroh endpoint round trip inside a verified offer;
- capability validation and request-digest response binding;
- durable mailbox mutation plus idempotent replay through the peer service;
- per-Iroh-identity rate limiting.

`verify-kilogram-volunteer-iroh-boundary.ps1` additionally fails closed if the
dedicated ALPN, all three operations, signed offer, bounded runtime dispatch,
durable quota reuse or no-new-executable boundary disappears.
