# RFC-0126: M1.1 protocol compatibility baseline

Status: accepted for the M1.1 experimental baseline.

## 1. Problem

Kilogram accumulated several independently versioned protocol families during
the M0 technical work: Iroh ALPN identifiers, signed events and sync frames,
connection tickets, device authority, ratchet public artifacts, history
recovery, endpoint publication, blind mailbox capabilities and local IPC.
Their constants were protected by tests in their owning modules, but there was
no single reviewable inventory or common migration rule.

That creates two risks:

1. a constant or domain separator can drift without an explicit compatibility
   decision; and
2. an old identifier can be silently reinterpreted as a new format, producing
   downgrade, split-network or signature-transcript ambiguity.

## 2. Decision

`PROTOCOL-COMPATIBILITY.json` is the machine-readable M1.1 inventory of declared
cross-process and interoperability surfaces. It records 20 protocol families
and 155 exact Rust source bindings. The registry is intentionally marked
`experimental` and `changes_wire=false`: creating it does not change bytes sent
by existing clients and does not declare the formats stable.

The inventory has three classes:

- `network-wire`: values carried between peers or used to select a network
  protocol;
- `portable-artifact`: files, tickets, links or signed objects that can cross a
  device/process boundary; and
- `local-control`: versioned contracts between separately built local
  processes.

Internal database schema versions, cache records, journals and encrypted local
implementation records are deliberately excluded unless they cross one of
these boundaries. They still require their owning migration tests, but they are
not public network compatibility claims.

## 3. Registered families

| Surface | Class | Encoding in the current implementation | Compatibility rule |
| --- | --- | --- | --- |
| `account-authority` | portable artifact | Postcard | exact version |
| `account-recovery` | portable artifact | Postcard | exact version |
| `authorized-event` | network wire | Postcard | exact version |
| `blind-mailbox-capability` | portable artifact | Postcard | exact version |
| `blind-mailbox-provider-gossip` | network wire | Postcard | exact version |
| `blind-mailbox-transport` | network wire | Iroh ALPN plus Postcard | exact version/identifier |
| `connection-ticket` | portable artifact | base64url JSON | exact version |
| `desktop-bootstrap` | local control | JSON | exact version |
| `device-session-authorization` | network wire | Postcard | exact version |
| `endpoint-announcement` | network wire | Postcard | exact version |
| `history-recovery` | portable artifact | Postcard/base64url URI | exact version |
| `history-rewrap` | network wire | Postcard | exact version |
| `mailbox-provisioning` | network wire | Postcard | explicit v1/v2 reader set |
| `publication-conflict` | portable artifact | Postcard/base64url URI | explicit reader set |
| `ratchet-public-artifacts` | portable artifact | Postcard | exact version |
| `runtime-ipc` | local control | Postcard frames, JSON descriptor/profile | exact version |
| `runtime-mailbox-envelope` | network wire | Postcard | exact version |
| `sync` | network wire | Postcard | exact version |
| `ticket-publication-capability` | portable artifact | Postcard | exact version |
| `transport-sync` | network wire | Iroh ALPN | exact identifier |

The strings containing `m0` are legacy protocol identifiers. They are not
rewritten merely because the product milestone is M1.1; changing an ALPN would
create a distinct network protocol and must follow the migration rules below.

## 4. Binding model

Every registry binding contains:

- a canonical repository-relative Rust source path;
- one constant symbol;
- its Rust literal type (`u8`, byte string or string); and
- the exact literal value expected in source.

The verifier accepts only an exact closed surface set, exact per-surface binding
counts, stable ordinal ordering, known classifications/policies and source
paths below `apps/` or `crates/`. It derives declaration regexes itself; the
JSON cannot inject a regex or an arbitrary file path.

The same connection-ticket version and signature domain are bound in both the
online CLI and network-free offline inspector. This makes divergence between
the two implementations visible immediately.

## 5. Compatibility and migration rules

### 5.1 No silent reinterpretation

An existing version, ALPN, URI prefix, signature domain, ID domain, KDF/HPKE
context or derivation context must retain its existing meaning. A field/layout,
transcript or cryptographic semantic change requires a new version or
identifier even when the old decoder could technically parse the bytes.

### 5.2 Writer and reader transition

A version migration must state:

1. which versions readers accept;
2. which single version new writers emit;
3. how peers discover or select support without trusting an unauthenticated
   downgrade signal;
4. what durable state prevents rollback after a newer version is observed; and
5. the evidence and removal condition for the legacy reader.

`new-version-dual-reader` means a bounded transition may read old and new while
writing only the selected new version. `new-version-explicit-migration` means a
portable artifact must be converted or replaced through an explicit operation.

### 5.3 Identifier transition

An exact ALPN or URI identifier migrates by introducing a parallel identifier.
The old identifier is never assigned a new message grammar. Negotiation must be
authenticated or its downgrade consequences must be explicitly bounded.

### 5.4 Domain separation

A signature, hash-ID, KDF, HPKE or deterministic-selection transcript changes
only with a new domain/context value. The version carried in a signed object and
the domain used to authenticate it must be reviewed together. Reusing one
domain for structurally different transcripts is forbidden.

### 5.5 Removal

Legacy support is removed only after its replacement has:

- deterministic positive and negative vectors;
- upgrade and downgrade tests;
- rollback behavior for persisted state;
- evidence from every supported implementation/platform; and
- an explicit removal decision documenting stranded-client behavior.

## 6. Fail-closed boundary

`scripts/verify-kilogram-protocol-compatibility-boundary.ps1` verifies the
registry and source without starting a Rust test executable or using the
network. Its synthetic negative cases require rejection of:

- a changed source value without a registry change;
- a removed binding;
- a source path escaping the repository boundary;
- an unknown policy property; and
- an undeclared claim that this baseline changes wire behavior.

The verifier emits `protocol_compatibility_boundary=verified` only after all 20
families and 155 source bindings match. It is part of the closed M1 readiness
inventory, which therefore contains 34 gates after this RFC.

## 7. Deliberate limitation: codec and layout are not final

The registry freezes identifiers, versions and cryptographic/domain literals;
it does not make Rust type layout a language-independent schema. Postcard and
the current Serde-derived structures remain an experimental implementation
codec. A structural field change without a version bump is prohibited by this
RFC, but the current script cannot infer every Rust/Serde layout change.

Before another-language or production-compatible client is claimed, a successor
stage must select a canonical encoding and add normative schemas plus golden
byte/signature/ID vectors. Those vectors, not Rust struct source alone, will be
the cross-language compatibility authority.

RFC-0127 starts that work with an inactive deterministic-CBOR profile and two
golden vector families. It does not retroactively change this registry or the
active Postcard wire.

RFC-0128 extends the same inactive profile through the minimum Account Root,
Device certificate, revocation, authority-snapshot and session-authorization
chain. Its CDDL, strict Rust decoder and independent JavaScript/Node Ed25519
vectors still do not activate or reinterpret any family in this registry.

## 8. Security and evidence boundary

This baseline does not prove the cryptographic design secure, does not add a
downgrade negotiation protocol, does not refresh M1.0.0 field evidence and does
not imply interoperability with any released third-party client. It makes the
current experimental boundary explicit so later migrations can fail closed
instead of relying on memory.
