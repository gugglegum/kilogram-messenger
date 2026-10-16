# RFC-0177: M1.1.51 authenticated sync transport capability

Status: accepted locally; portable ticket migration active, canonical sync
transport inactive.

## Outcome

Connection ticket v12 contains a separately Device-signed synchronization
transport capability. Its signature and the outer ticket signature bind the
listener Account and Device, exact Account authority revision, allowed
requester Account, Iroh endpoint ID, ticket-publication channel epoch and the
common validity window of the signed prekey directory.

The active protocol set contains only `kilogram/m0/sync/8`. The allocated
`kilogram/m1/sync/cbor/1` identifier appears only in the candidate set and
`canonical_dial_enabled` is false. Thus the artifact can carry authenticated
migration state without claiming that the listener currently accepts canonical
CBOR.

## Exact-version migration

The additional signed field changes JSON bytes and the outer signature
preimage. Ticket version and signature domain therefore make a deliberate hard
cut from v11 to v12. Online and offline verifiers reject v1-v11 and unknown
future versions; there is no legacy reader or silent downgrade. Restarting the
listener reissues a v12 ticket, after which peers must refresh the exact signed
artifact through the existing publication/contact paths.

The shared `kilogram-sync-capability` crate prevents online and offline tools
from implementing different nested capability semantics. The offline binary
remains Iroh-free and network-free; it parses the public endpoint summary only
to exact-bind its endpoint ID to the shared signed object.

## Verification order

Both verifiers first authenticate the outer ticket. They then verify the
nested capability signature, exact context and freshness. A raw endpoint
tamper therefore fails the outer ticket signature, while a valid capability
copied from another endpoint fails the semantic binding.

## Non-activation guarantee

This stage changes a portable connection-ticket artifact, not the active sync
wire. The canonical ALPN remains unregistered and is never dialed. Format
sniffing, parse-error fallback and retrying a failed canonical operation over
legacy remain forbidden. Activation is still blocked on the active canonical
reader/writer, an explicitly enabled current capability, downgrade and
mixed-version tests, sustained fuzzing, rollback evidence, and fresh hosted and
physical-device evidence.
