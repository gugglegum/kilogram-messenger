# RFC-0160: canonical runtime IPC volunteer-provider control

Status: implemented as an inactive M1.1.34 candidate.

## Summary

M1.1.34 adds two capability-free operations to the deterministic-CBOR runtime
IPC candidate: importing one signed volunteer storage offer and selecting up to
eight current volunteer providers. The contract deliberately carries the
original signed offer instead of copying its endpoint, store key, policy and
capacity into unauthenticated strings.

This contract does not activate canonical CBOR. The shipping runtime remains
Postcard IPC version 26 with JSON descriptor version 7. No listener, network
protocol, ALPN, provider registry, selection behavior, desktop behavior or
accepted M1.0.0 tag changes. No network was executed to define or verify this
contract.

## Import boundary

`import-volunteer-storage-offer` embeds the exact canonical CBOR bytes of one
signed blind-mailbox storage offer. Before a future adapter mutates its local
registry it must independently verify:

- the Ed25519 signature by the offer's store key;
- the bounded endpoint and capacity fields;
- current issue and expiry times; and
- at least eight bits of the existing bounded admission-work rule.

The signed runtime result repeats that exact offer, reports `inserted`,
`replaced` or `already-present`, and gives a bounded active-provider count. An
import result for a different offer fails closed even when its store key is the
same.

## Capability-free provider projection

One provider projection contains only the signed storage offer, a nonzero
32-byte transport identity and the local observation time. Offer ID, endpoint,
store key, policy class, capacity, maximum record size and validity are derived
from the signed offer. The representation contains no mailbox ID, read key,
write key, read authorization, write authorization, Account ID, Device ID,
conversation identity or social identifier.

Observation time is explicitly local metadata. It must lie inside the offer
validity interval and cannot be in the verifier's future. A newly imported
offer must additionally be observed inside that import request's short-lived
window; an existing provider selected later may legitimately have an older
observation. The timestamp is not claimed to be signed by the volunteer
provider.

## Selection boundary

`select-volunteer-storage-providers` supplies a nonzero opaque 32-byte salt and
requests between one and eight providers. The signed response repeats the
exact salt and requested count, reports the bounded active registry count and
contains no more than both the requested and active counts. Store keys and
offer IDs are unique; every returned offer is signed, admission-qualified and
current.

The portable response proves what the descriptor Device selected, but cannot
prove that the local registry omitted no better candidate. A future adapter
must execute selection against one read-only registry snapshot and retain the
existing bootstrap-safe rules: authenticated corroboration is only a binary
preference, positively known path co-location is deferred where alternatives
exist, and unknown path evidence is not treated as operator independence.

The selection salt must be freshly generated per selection context and must
not be an Account, Device, conversation, mailbox or capability identifier.

## Authentication and bounds

Requests retain the signed loopback descriptor and short-lived bearer model
from RFC-0154. Every response is signed by the exact descriptor Device and
binds the descriptor ID, caller request ID and digest of the complete request.
Requests are limited to 64 KiB, signed responses to 256 KiB, active registry
count to 4096 and selected results to eight. Request lifetime remains at most
30 seconds.

Request digest, response signature and response ID use separate domains.
Unknown variants, noncanonical CBOR, trailing bytes, zero identities, expired
offers or requests, insufficient admission work, descriptor substitution,
command/result mismatch, duplicate providers and salt or count substitution
fail closed. Import and selection each have an independently reproduced
JavaScript golden request/response pair, and all four objects enter the bounded
decoder mutation corpus.

## Activation boundary

M1.1.34 remains inactive. Activation requires an explicit IPC version
migration, dual-reader and downgrade tests, an atomic active-registry adapter,
stable mapping from legacy base64url offers, sustained fuzzing and fresh
desktop/CLI field evidence. Mailbox capability creation, rotation, revocation
and status remain a separate security-sensitive slice.
