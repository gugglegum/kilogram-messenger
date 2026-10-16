# RFC-0195: M1.1.68 dormant mutual authenticated candidate selection

Status: accepted locally; inactive transport-selection contract.

## Purpose

M1.1.66 and M1.1.67 made the canonical `/cbor/2` client and responder state
machines durable, but nothing proved that both authenticated peers selected the
same candidate for the same conversation, transport session and migration
epoch. M1.1.68 adds that proof without registering or dialling the candidate
ALPN.

## Three signed links

The existing listener capability remains Device-signed and ticket-bound. Its
new domain-separated artifact ID is the first immutable link. The requester
then signs an offer containing that capability ID, both Account and Device
identities, both authority revisions, conversation, session binding, migration
epoch and the exact `kilogram/m1/sync/cbor/2` candidate. Finally, the listener
signs an acceptance of the exact offer ID and repeats the identities and
session fields needed to prevent cross-session or cross-offer reuse.

Offer and acceptance use distinct signature domains. Capability and offer IDs
also use distinct BLAKE3 domains. Unknown versions, zero security bindings,
wrong signers, substituted capabilities, cross-offer acceptances and any
attempt to enable fallback after a selected canonical failure fail closed.

## Authority and durable-state binding

Selection is not accepted merely because the offer signatures verify. Both
peers must also supply valid canonical Device session authorizations with the
`SyncHistory` capability. Their Account, Device, authority revision and session
binding must match the offer exactly.

The verifier then binds the same identities to the already persisted durable
client and responder checkpoints. Conversation, requester/listener roles,
session binding and `/cbor/2` protocol identifier must agree on both sides. A
valid negotiation therefore cannot be transplanted onto a different durable
session.

## Deliberate dormant boundary

The result deliberately reports `active_transport_authorized=false`. Mutual
candidate selection is necessary evidence, not permission to register an ALPN,
dial a peer, switch the writer or fall back after a canonical decode/network
failure. The active Postcard v6 transport, CLI and Iroh paths remain unchanged.

The next bounded stage is a mixed-version and downgrade matrix around this
selection contract, followed later by an explicit dual-reader activation
slice. This stage performs no network run, release build, archive creation or
stable executable replacement.
