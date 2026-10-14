# RFC-0121: Current M1 candidate refresh (M0.9.98)

Status: composed locally; committed production verification pending.

## 1. Purpose

M0.9.96 changed test-only CLI source inside the deliberately coarse physical
field surface. M0.9.97 then added explicit configured-relay diagnostics to the
production CLI. The previous M0.9.93 candidate correctly failed closed after
that drift. This refresh composes new physical field evidence with the still
current independent reproduction of the unaffected offline surface.

This record is a Windows technical proof of concept. It does not claim a
public security release, and strict independent-provider field acceptance
remains pending.

## 2. Fresh field evidence

Two-host reduced run `20261014-141058` used product revision
`be80d28ec12b41987bdbfa800016fb86b54b4723`. Alice plus Provider 1 ran on the
desktop, while Provider 2 plus Bob ran on the SSH-controlled laptop. Private
keys, runtime databases and open logs stayed on their role-owning machines;
only public protocol material and closed evidence crossed the synchronization
boundary.

The run proves two distinct volunteer stores, exact authenticated receipts
`2/2`, Alice offline before receive, commit-before-delete for both replicas,
one history occurrence and no restart redelivery. Every endpoint-bearing log
pins canonical APS1 through `relay_config_url`; observed relay homes/targets,
when present, match that pin. The strict result is
`verified-reduced-two-host` with
`independent_provider_field_acceptance=false`.

The exact 83-entry runtime/protocol Git surface has SHA-256
`b5cb9bc9e06d24c73e63289e662ba019f88b7b052638c6a4b30099470f558095`.
The production verifier recomputes it at the field revision and candidate
`HEAD`; any drift fails closed.

## 3. Independent reproduction continuity

The independently reproduced offline surface remains byte-identical to its
accepted baseline. Candidate composition retains revision
`68804c93f43b405008cfecf35dc8660db360a3a3`, GitHub run `38507114425`,
attestation `55683374`, and the 2,432,000-byte `kilogram-offline.exe` with
SHA-256 `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`.
Its exact 63-entry protected surface remains
`2a7db2c5cacd0419a8ece61b0dc8386075a558d4f052b44c0eedd81cd9354d68`.

## 4. Honest residual boundary

Boundary claims:

- does not claim a public security release;
- does not require another field network run while the exact runtime/protocol Git surface remains unchanged;
- strict independent-provider field acceptance remains pending;
- Sybil resistance remains incomplete.

- strict independent-provider field acceptance remains pending;
- operator and network independence remain unproven;
- Sybil resistance remains incomplete;
- access correlation is not hidden;
- field evidence remains Windows-only;
- this is not a public security release.

## 5. Composition boundary

The candidate verifier is network-free and build-free. It creates no
executable, archive, background service or Git tag. It verifies the exact field
revision, both selected-tree identities, ancestor relations, a clean candidate
control surface and the full residual-risk list. Because candidate/control
files are outside the field runtime surface, committing this composition does
not require another field network run.

Production verification must run from a clean committed tree. Complete M1
readiness and a fresh green hosted Windows CI run remain required before any
separate decision about an M1 tag.
