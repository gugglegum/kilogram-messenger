# RFC-0182: M1.1.56 authenticated history-compatible sync capability

Status: accepted locally; portable ticket migration active, canonical sync
transport inactive.

## Outcome

Connection ticket v13 carries sync transport capability v2. The capability is
signed separately by the listener Device and advertises exactly one active
protocol, `kilogram/m0/sync/8`, plus exactly one inactive candidate,
`kilogram/m1/sync/cbor/2`. `canonical_dial_enabled` remains false.

The candidate now names the history-compatible generation whose typed event
carrier, closed request/response frames and typed durable checkpoint are all
verified. The history-incompatible `/cbor/1` allocation is not advertised and
cannot be substituted into a v2 capability.

## Exact-version migration

Changing the candidate changes the nested JSON and signature preimage. The
capability therefore moves from version/domain v1 to v2, while its enclosing
connection ticket moves from version/domain v12 to v13. Online and offline
readers accept only the exact current versions. There is no dual reader,
format sniffing, parse-error fallback or silent downgrade; a listener restart
reissues the current ticket and peers must refresh the signed artifact.

The outer ticket is authenticated before the nested capability binding is
trusted. The nested signature exact-binds listener Account and Device,
authority revision, allowed requester Account, endpoint ID, publication epoch
and the common prekey-directory validity window.

## Downgrade resistance

A v12 ticket fails the outer exact-version gate. A v13 ticket containing a v1
capability fails the nested exact-version gate. A capability using the
`canonical-cbor-v1` enum value fails closed during syntax decoding, and any
other active/candidate set or enabled canonical dial fails semantic validation.
Copying a valid v2 capability between tickets fails its exact context binding.

## Non-activation guarantee

This stage changes only the portable ticket artifact and authenticated
migration declaration. `/cbor/2` is still absent from the active endpoint ALPN
list and every dial path. Active sessions continue to use Postcard v6 over
`kilogram/m0/sync/8`; no canonical reader, writer, fallback or network
experiment is enabled here.

Activation remains a separate migration requiring an explicit dual-reader
implementation, mutual authenticated selection, mixed-version and downgrade
regression, sustained fuzzing, rollback evidence, and fresh hosted and
physical-device evidence.
