# RFC-0138: Canonical synchronization frames (M1.1.12)

Status: candidate, not active.

## Outcome

M1.1.12 defines deterministic-CBOR equivalents for every current synchronization
request and response: signed inventory, authorized-event batch, signed diff,
complete, pause, paused and rejected. Closed client and server frame registries
bind each payload to an explicit variant. The active Postcard v6 protocol and
its ALPN remain unchanged.

## Encoding and authentication

All objects use positional arrays from the narrow Kilogram canonical profile.
Event ID lists are bytewise strictly ordered and duplicate-free. Authorized
events are independently verified, must name the frame conversation, and are
strictly ordered by their derived Event IDs. Inventories are signed by the
requester Device; diffs are signed by the responder Device. Both signatures
commit to the exact transport-derived 32-byte session binding.

The profile intentionally represents booleans as the closed unsigned values
zero and one because generic CBOR simple values are outside the selected
profile. Unknown client, server and rejection variants fail closed. A complete
standalone object or nested frame payload is bounded to eight MiB; inventory
lists remain bounded to 4,096 IDs and per-round event lists to 64.

An independent JavaScript encoder using Node's Ed25519 implementation recreates
the exact CBOR contents, domains, preimages, signatures and all seven frame
variants. Three representative strict decoders join the deterministic mutation
corpus, increasing it from 2,871 to 3,654 cases under a 4,000-case ceiling.

## Remaining gap

These frames define one round, but they do not yet define a durable authenticated
checkpoint across disconnects. Therefore the aggregate `sync` surface remains
partial. A separate canonical multi-round session state must bind round number,
both peers, cumulative event evidence and terminal/paused state before this
surface can be marked complete.

## Activation boundary

This stage changes no active wire bytes and assigns no protocol identifier.
Activation still requires the canonical migration contract, a new explicit
version or parallel ALPN, dual-reader downgrade tests, sustained fuzzing and
fresh hosted and field evidence.
