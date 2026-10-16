# RFC-0196: M1.1.69 dormant mixed-version and downgrade matrix

Status: accepted locally; inactive transport-policy contract.

## Purpose

M1.1.68 proves that both authenticated devices selected the exact canonical
`kilogram/m1/sync/cbor/2` candidate for one durable session. It did not define
what each migration state may do when the peer is old, when selection evidence
is absent, or when a selected canonical attempt fails. M1.1.69 makes those
outcomes explicit and executable before any dual-reader transport is enabled.

## State matrix

`LegacyOnly` and `AdvertiseCanonicalV2` may plan only the active Postcard v6
protocol. A canonical proposal is premature in those states. A dormant signed
selection may already exist as evidence, but it is not an activation permit and
does not change the active protocol.

Both dual-reader states accept Postcard only when no canonical selection exists;
this represents a genuinely old peer. They accept canonical only with the exact
mutual authenticated selection from M1.1.68. Once that selection exists, a
legacy proposal is an authenticated downgrade and fails closed. The preference
name affects future scheduling policy, not these security requirements.

`CanonicalRequired` accepts only canonical with mutual selection. Legacy is
rejected even when no selection is available.

## No error-driven fallback

A selected canonical decode, network or protocol failure never authorizes a
legacy retry. The evaluator rejects an explicit fallback flag before considering
the migration state. It also rejects a selection whose migration epoch is below
the local minimum. Format sniffing and parse-error fallback remain forbidden.

The no-selection legacy row is deliberately narrow: it models an authenticated
connection to a peer that did not complete the canonical ceremony. This local
matrix alone does not claim to prevent an attacker from stripping the offer
before an authenticated transport context exists. The future dual-reader
integration must carry the signed ceremony inside that context and persist its
outcome before choosing a writer.

## Deliberate dormant boundary

Every successful evaluation returns a `CanonicalSyncV2DormantTransportPlan`
with `active_transport_authorized=false`. The evaluator is absent from the
active wire, Iroh transport and CLI. The canonical endpoint remains unregistered
and undialed; Postcard v6 remains the only active path.

The next bounded implementation step is a dormant dual-reader runtime
coordinator that consumes this plan without yet registering or dialling the
canonical ALPN. Rollback drill, sustained fuzzing and fresh hosted/field
evidence remain separate gates. This stage performs no network run, release
build, archive creation or stable executable replacement.
