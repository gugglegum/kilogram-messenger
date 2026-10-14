# RFC-0112: Current M1 candidate evidence refresh (M0.9.89)

Status: accepted locally after committed production verification.

## 1. Purpose

The M0.9.81 candidate record correctly failed closed after later runtime and
dependency changes. M0.9.89 refreshes the evidence composition without
weakening that continuity rule. It pins the newest completed field lifecycle
and the newest independently reproduced protected surface.

This record is still a Windows technical proof of concept. It does not claim a
public security release, and strict independent-provider field acceptance
remains pending.

## 2. Field evidence

The candidate pins M0.9.87 run `20261014-080905` at revision
`162836bb7a54180c3a00915749cf17fe238019e2`. The two-host reduced topology
completed exact two-replica volunteer delivery, sender-offline retrieval,
commit-before-delete and restart without redelivery. Its explicit result is
`verified-reduced-two-host` and
`independent_provider_field_acceptance=false`.

The canonical 83-entry exact runtime/protocol Git surface has SHA-256
`1aacaca665c9a2ab4dbc43107c2556128274d7fb9f8727d2dcc78681026bb320`.
The production verifier recomputes that surface at the field revision and at
candidate `HEAD`; any tracked, staged or untracked drift fails closed. Because
the surface is unchanged, composition does not require another field network
run.

The reduced result proves the complete service-free lifecycle across two
physical machines. It does not prove independent operators or access networks.
A strict three-/four-host experiment remains a residual acceptance item.

## 3. Independent reproduction

The candidate pins M0.9.88 revision
`68804c93f43b405008cfecf35dc8660db360a3a3`, GitHub run `38507114425`,
attestation `55683374` and the 2,432,000-byte `kilogram-offline.exe` with
SHA-256 `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`.
Production verification accepted all four signed subjects.

The 63-entry independent artifact surface has SHA-256
`2a7db2c5cacd0419a8ece61b0dc8386075a558d4f052b44c0eedd81cd9354d68`.
It includes the corrected host-neutral Cargo identity verifier. Any change to
the offline application, dependency graph, reproducibility workflow, native
lock or verifier invalidates the candidate and requires fresh reproduction.

## 4. Honest residual boundary

M0.9.89 records six exact residual risks:

Boundary claims: does not claim a public security release; does not require another field network run; exact runtime/protocol Git surface; strict independent-provider field acceptance remains pending; Sybil resistance remains incomplete.

- strict independent-provider field acceptance remains pending;
- operator and network independence remain unproven;
- Sybil resistance remains incomplete despite admission work and local
  provenance/selection hardening;
- access correlation is not hidden;
- field evidence remains Windows-only;
- this is not a public security release.

The closed schema and self-test reject a changed revision or artifact, a
narrowed protected path set, an omitted risk and any attempt to turn the
reduced result into `independent_provider_field_acceptance=true`.

## 5. Composition stage boundary

The candidate-composition verifier is network-free and build-free. It creates
no executable, archive, background service or Git tag. The M0.9.88 release
build and GitHub publication are already accepted upstream evidence; M0.9.89
only verifies their immutable identity and selected-tree continuity.

The candidate does not require another field network run while its exact
runtime/protocol surface remains unchanged. Tagging or publishing M1 remains a
separate explicit product decision after the committed production verifier
passes.

## 6. Acceptance evidence

Candidate revision `d9142003eeb33f4e9ff44f100d88edd556c058c1` passed the
production verifier from a clean working tree. It reported both protected
surfaces unchanged, `network_retest_required=false`, no network/build/archive
or tag activity, and `m1_candidate_status=verified`.
