# RFC-0157: canonical runtime IPC own-device directory transition

Status: implemented as an inactive M1.1.31 candidate.

## Summary

M1.1.31 adds the authority-changing `apply-own-device-directory` operation to
the deterministic-CBOR runtime IPC candidate. The request embeds the exact
Root-signed successor Account device list and anchors the exact current
authority revision and a domain-separated digest of the complete signed current
list. The signed result commits to both states, the exact removed Device set and
the resulting read-only directory status from RFC-0156.

This contract does not activate canonical CBOR. The shipping runtime remains
Postcard IPC version 26 with JSON descriptor version 7. No network protocol,
ALPN, endpoint publication, desktop behavior or accepted M1.0.0 tag changes.

## Why the successor is embedded

The active command currently receives a path to a device-list file. A path by
itself is not an authority object: its contents can change between validation
and commit. The canonical request therefore carries both:

- a bounded UTF-8 path used only as an application-owned provenance and commit
  target; and
- the complete canonical Root-signed successor list whose bytes are covered by
  the request digest and the descriptor Device's signed response.

An eventual active adapter must read the named file without following an
untrusted link, require its bytes to match the embedded signed list exactly,
and keep that verified handle or an equivalent immutable snapshot through the
transaction. Revalidating only the pathname would reintroduce a
time-of-check/time-of-use authority substitution.

## Exact current-state anchor

The request contains the expected current authority revision and the
domain-separated digest of the complete canonical signed current device list.
The runtime must compare both values with its durable current state before any
mutation. A stale, substituted or merely semantically similar current list is
rejected. The response verifier repeats the same checks instead of trusting the
reported previous revision and digest.

An equal-revision successor is valid only when it is byte-for-byte the same
signed list. A newer successor must retain every previously observed Root
revocation. Authority revision rollback and revocation disappearance fail
closed.

## Removal-only live transition

This command deliberately models only safe removal of already known Devices.
It cannot add a Device or replace any certificate. Every certificate remaining
in the successor must be byte-identical to one in the current list. The Device
running the descriptor must remain present with its exact certificate. Every
active Device that disappears must have a permanent Root-signed revocation in
the successor authority snapshot.

Device enrollment belongs to the separately authorized device-link ceremony;
mixing enrollment into this local bearer-authenticated command would silently
expand its authority.

## Transaction and result

An active implementation must treat verification, durable roster replacement,
revocation-derived local retirement, directory receipt update and launch
profile decision as one recoverable transaction. A successful signed result
binds:

- the exact previous and resulting revisions and signed-list digests;
- the exact sorted set of Devices removed by this transition;
- the resulting active count, bounded to 32 Devices;
- retired ratchet-session and prekey-observation counters;
- pending unmaterialized and materialized message counters;
- ticket-publication and launch-profile-update decisions; and
- the complete resulting device-directory status.

The active Postcard implementation currently reports all revoked Device IDs in
one apply result. That is not the canonical meaning. During migration it must
report exactly `current active Devices - successor active Devices`; historical
revocations that were already absent are not newly removed Devices.

Crash recovery must either expose the old anchored state or the complete new
state and authenticated receipt. It must never acknowledge an authority
transition whose durable roster or status projection can still fall back to an
unrelated state after restart.

## Non-erasure semantics

Removing and revoking a Device excludes it from newly refreshed recipient sets
and future ticket publication. Preexisting ciphertext is immutable: the command
does not remotely rewrite or erase ciphertext
already delivered to that Device, and existing local history copies remain
readable wherever their keys still exist. The response encodes these semantics
as closed status variants so a caller cannot mistake future exclusion for
retroactive deletion.

## Authentication and bounds

The request reuses the signed loopback descriptor, nonzero bearer token,
caller-generated request ID and at-most-30-second lifetime from RFC-0154. The
request is limited to 64 KiB, the signed response to 128 KiB, paths to 4096
bytes and rosters to 32 Devices. Directory requests, signed responses and signed
device-list digests use distinct domain separators.

Unknown variants, noncanonical CBOR, trailing bytes, expired requests, current
anchor mismatch, Account or descriptor substitution, unrevoked disappearance,
certificate replacement, running-Device removal and response substitution all
fail closed. Golden vectors are reproduced independently in JavaScript and both
request and response enter the bounded decoder mutation corpus.

## Activation boundary

M1.1.31 remains an inactive candidate. Activation requires a versioned
dual-reader migration, an exact transactional adapter with the path/embedded
bytes check above, downgrade and crash-recovery tests, sustained fuzzing, and
fresh desktop/CLI evidence. No network was executed to define or verify this
contract.
