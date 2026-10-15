# RFC-0140: Canonical history recovery exchange (M1.1.14)

Status: implemented as an inactive conformance candidate.

## Outcome

M1.1.14 completes the deterministic-CBOR coverage of `history-recovery` without
pretending that the separate `history-rewrap` cryptographic payload has also
been specified. The candidate defines:

- a source-signed recovery link containing bounded endpoint coordinates, the
  exact source certificate, and the exact Root-signed Account device list;
- a recipient-signed approval plan containing the exact link, independently
  derived SAS, explicit network/power policy, and bounded validity;
- a recipient-signed page request bound to the exact plan, recovery identity,
  live session, requested range, and optional previous checkpoint;
- source-signed closed page/rejection response variants bound to the exact
  signed request.

The successful page carries the history-rewrap result only as an opaque,
bounded byte string. Its HPKE/event structure remains in the separate
`history-rewrap` compatibility surface and is still `not-started`.

## Consent and identity binding

The source certificate must be the exact authorized certificate in the
embedded Account device list. The recipient Device must be another authorized
member of that same list. Merely substituting a certificate with the same
Device ID is insufficient.

The SAS is BLAKE3 over a domain-separated deterministic-CBOR tuple containing
the exact signed device list and the ordered source/recipient roles. The plan
signs that SAS and the exact source-signed link. The recovery ID reuses the
existing immutable recovery identity shared with the checkpoint chain.

Requests additionally include the exact plan ID and a transport-derived live session binding.
A valid signature from the recipient is not enough if the
plan, conversation, roles, SAS, session, or approved range differs.

## Routing and resource bounds

Coordinates are printable ASCII and strictly ordered by `(kind, value)`.
Relay coordinates are canonical HTTPS URLs ending in `/`; direct coordinates
are canonical socket addresses. `direct-only` and `relay-only` links reject a
coordinate of the other kind.

The schema bounds coordinates to 16, approved inventory to 4096 events, pages
to 256 events, nested control artifacts to 1 MiB, opaque rewrapped payloads to
8 MiB, and complete responses to 9 MiB. Link validity is at most one hour and
plan validity at most seven days.

## Fail-closed conformance

The independent JavaScript reference derives Ed25519 public keys, exact CBOR,
all signatures, SAS, link/plan/recovery IDs, and both response variants without
calling the Rust implementation. Rust golden tests reproduce those bytes and
reject role substitution, wrong plan/session/range, noncanonical coordinates,
invalid booleans, unknown variants, and signature mutation.

Three new strict-decoder fixtures increase the deterministic offline mutation
corpus from 3,915 to 4,698 cases under a 5,000-case ceiling. This is regression
coverage, not a substitute for sustained coverage-guided fuzzing or an
independent security audit.

## Activation boundary

The active recovery link remains a Postcard payload in a base64url
`kilogram://history-recovery/v1/` URI. The active plan and wire exchange also
remain Postcard. No current version, ALPN, domain, or reader recognizes this
candidate.

Activation requires an explicit new protocol identifier or parallel ALPN,
dual-reader and downgrade tests, coverage-guided fuzzing, and fresh field plus
hosted evidence. M1.1.14 changes no active wire bytes and performs no network,
release, archive, or tag operation.
