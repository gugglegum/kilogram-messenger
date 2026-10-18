# RFC-0209: M1.1.85 canonical sync v2 opt-in dark-launch acceptance

Status: accepted.

## Decision

Kilogram accepts the production canonical sync v2 path as a verified opt-in
dark launch. This is deliberately narrower than public activation. A missing
activation authority still yields `legacy-only`; an ordinary endpoint registers
and dials only `kilogram/m0/sync/8`. The canonical
`kilogram/m1/sync/cbor/2` ALPN is registered and may be selected only when the
local Device-signed policy grants the exact rights and the peer completes the
mutually authenticated, durably persisted selection ceremony.

`changes_active_wire=true` means that an explicitly opted-in production runtime
can now use the canonical wire. It does not mean that the public default changed.
Every aggregate ledger must therefore also state `public_default=legacy-only`
and must distinguish default registration from verified opt-in registration.

## Accepted evidence chain

The decision composes, rather than rewrites, the earlier evidence:

1. M1.1.80 proved the feature-gated codec and transport matrix on hosted and
   physical Windows topologies, including relay and decoder/fuzz evidence.
2. M1.1.81 defined independent publish, register, accept, initiate and resume
   rights plus emergency containment.
3. M1.1.82 made the authority a Device-signed append-only chain with monotonic
   high-water validation.
4. M1.1.83 integrated the shared adapter into the ordinary production CLI,
   kept the default endpoint legacy-only and prohibited fallback after durable
   canonical selection.
5. M1.1.84 verified default, mixed, mutual opt-in, restart, containment,
   selected loss and rollback between two physical Windows hosts using one
   exact debug executable.

The accepted M1.1.84 implementation revision is
`7ee5ef808cb56c3c48d7a0b6dc07c0984b81992c`; the executable SHA-256 on both
hosts is `be8ca16754c2ad1291d4ea56c0a339b6e6a0f5994e6816c1c3d966871f42c532`.
The run ID is `m1184-exact-20261018T165652Z`.

## Timeout boundary learned in the field

Responder state-lock, vault, import and durable-session preparation is not
stream I/O. A production-path field probe transferred responses from 1 through
8123 bytes intact and ruled out QUIC response size and the Windows firewall.
The requester now permits at most 120 seconds for bounded responder preparation
and the first response, while each actual stream I/O remains limited to 20
seconds. Neither timeout grants activation authority or permits a Postcard
retry after canonical selection.

## Aggregate acceptance gate

Acceptance requires all changed boundary self-tests and their negative
mutations, the closed M1 readiness inventory, rustfmt, locked/offline strict
Clippy and the stable Rust regression with two Cargo jobs. The decoder and frame
codec surfaces did not change after the already accepted 31,851 legacy and
2,863 canonical sync v2 mutations, so those long corpora and the sustained fuzz
campaign are not repeated. The stable regression must run every other harness.

The final tracked tree passed all changed boundary self-tests and negative
mutations. Accepted-tag readiness reported `112/112`, rustfmt and
locked/offline strict Clippy passed, and the stable runner finished with
`status=passed`, profile `test`, two Cargo jobs and its isolated runtime test
`1/1`. Every discovered workspace harness ran except the two explicitly named
unchanged corpus tests:
`bounded_golden_decoder_mutation_corpus_fails_closed` and
`bounded_canonical_sync_v2_decoder_mutation_corpus_fails_closed`.

An initial in-sandbox execution was discarded after Windows temporary-state
creation returned `os error 5` under the redirected sandbox Temp. Repeating the
identical command outside that filesystem sandbox passed; the discarded run is
infrastructure diagnostics, not product evidence.

No release build, ZIP/archive, tag or automatic GitHub build is part of this
gate.

## Explicit non-claims

This acceptance does not enable a public dual-reader default, provide desktop
activation UI, materialize canonical-native history, retire Postcard, establish
non-Windows interoperability or replace an independent security audit. Those
remain separate product and security decisions.
