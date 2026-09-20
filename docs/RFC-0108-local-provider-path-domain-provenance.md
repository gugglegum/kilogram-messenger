# RFC-0108: Local verified provider path-domain provenance (M0.9.85)

Status: accepted locally after network-free verification.

## 1. Purpose and claim boundary

M0.9.84 can prefer a provider offer observed through an authenticated social
session, but that does not say whether two selected stores share one network
path or relay. M0.9.85 adds a narrower, separately evidenced fact: after a
successful authenticated volunteer-mailbox exchange, the client remembers a
local pseudonym for the path domain that actually carried the verified
response.

This stage collects provenance only. It does not change provider ranking,
replica-set commitments or the required receipt threshold. In particular, it
does not claim operator, ASN, geographic or physical independence.

## 2. Evidence semantics

Iroh exposes the selected transport path after a connection. Kilogram maps it
to one of two local observations:

- `direct-remote-ip`: the exact selected remote IPv4 or IPv6 address with the
  ephemeral transport port removed;
- `relay-origin`: the canonical URL origin of the selected relay, with path,
  query and fragment excluded.

Custom or unavailable transports create no path-domain evidence. Arbitrary
IPv4 or IPv6 prefix lengths are deliberately not used: a `/24`, `/48` or other
prefix would not by itself establish an operator boundary.

Equal local tags are positive evidence that two observations shared the same
exact remote IP or relay origin under one derivation epoch. Unequal tags are
not proof of independence. CGNAT, VPNs, multi-homing, address rotation and
common relay infrastructure can all create false grouping or false separation
relative to the operator/physical property that ultimately matters.

## 3. Authentication and lifecycle

The path snapshot is taken only after Kilogram has decoded and authenticated a
mailbox peer response bound to the exact request. Persistence happens only
after operation-specific verification succeeds:

- PUT must contain a verified durable store receipt;
- LIST must verify its mailbox/store binding, including an empty page;
- DELETE must verify the exact delete receipt.

Evidence is attached to the exact signed provider offer ID and current store
key in one immediate redb transaction. A late response cannot attach evidence
to a replacement offer. Only the latest verified domain is retained for an
exact offer; an equal observation refreshes it and a changed selected path
replaces it. Offer replacement or expiry removes the old row. A legacy
registry without the table reads as having no evidence.

Failure to derive or persist this optional provenance never discards an
already verified mailbox receipt or application commit. It is reported as
unavailable and delivery continues; selection must therefore remain safe when
evidence is missing.

## 4. Privacy boundary

The runtime derives a dedicated BLAKE3 subkey from the local Device secret and
keyed-hashes domain-separated canonical path material. The registry stores
only the 32-byte tag, its non-secret local derivation-epoch commitment, path
kind and bounded timestamps. Raw IP addresses, socket ports and relay URLs are
not stored by this feature.

The derivation epoch prevents comparisons across a local Device-secret change:
old and new tags cannot accidentally be interpreted as different path
domains. A successful observation in the new epoch replaces the old evidence;
future policy must treat missing or epoch-mismatched evidence as unknown.

Tags, epochs, path kinds and evidence counts are not encoded into provider
offers, gossip frames, Kilogram protocol messages or authenticated runtime IPC
v26. The tag types redact `Debug`, and the new runtime path does not print raw
domain material or tag bytes. Existing explicit transport diagnostics remain
a separate operator-facing debugging surface and are not copied into the
provider registry.

## 5. Deferred selection policy

M0.9.85 intentionally does not consume the new evidence for selection. A
later stage may first avoid two providers known to share one local path domain,
then fill every remaining slot from unknown evidence so bootstrap and relay
fallback cannot deadlock. It must never interpret different tags as proof of
different operators, and exact recipient-authenticated replica commitments
must remain stable rather than being silently re-ranked.

True operator diversity requires a separately authenticated signal or a field
test on independently controlled hosts and networks. ASN lookup alone is also
not sufficient: it introduces a resolver/database trust boundary and still
does not prove independent administration.

## 6. Verification and stage boundary

Unit tests cover direct-port stripping, relay-origin normalization,
installation-scoped tags, redacted diagnostics, exact-offer binding,
refresh/change behavior, derivation-epoch separation, replacement cleanup,
legacy absence and unchanged gossip offer bytes.

The fail-closed source contract is
`scripts/verify-kilogram-provider-path-domain-boundary.ps1`. It verifies the
post-authentication ordering, local keyed derivation, exact-offer transaction,
wire/IPC exclusions and unchanged IPC version.

This stage is network-free and debug-only. It creates no release build, ZIP,
new executable, background process, field connection, external publication or
Git tag.
