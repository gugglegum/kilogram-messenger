# RFC-0116: Fresh field evidence and candidate refresh (M0.9.93)

Status: accepted locally after committed production candidate and readiness verification.

## 1. Purpose

M0.9.92 changed `apps/kilogram-cli` to support extended-length Windows runtime
journal paths. The existing M0.9.89 candidate correctly failed closed because
that application belongs to the exact field-tested runtime/protocol surface.
M0.9.93 composes fresh physical evidence for the changed surface with the
still-current independent reproduction of the unaffected offline surface.

This record is a Windows technical proof of concept. It does not claim a
public security release, and strict independent-provider field acceptance
remains pending.

## 2. Fresh field evidence

Clean run `20261014-104506` was generated from revision
`d33dd0950484665ee397d161e37ce74743ee8f93`. Alice and Provider 1 ran on the
desktop PC; Provider 2 and Bob ran on the Windows laptop. Only public
offers/tickets/attestations and closed evidence logs crossed the SSH
synchronization boundary; account roots, Device secrets, Redb state and open
runtime logs stayed on their originating hosts.

Both providers proved `relay_home_url=https://aps1-1.relay.n0.iroh.link./`.
Alice obtained exact authenticated receipts `2/2`, stopped before Bob
retrieval, and emitted no central mailbox descriptor. Bob committed the same
message from both volunteer replicas, deleted each after commit and observed
no redelivery after restart. The verifier reported
`verified-reduced-two-host` and
`independent_provider_field_acceptance=false`.

The canonical 83-entry exact runtime/protocol Git surface has SHA-256
`95c1771de3ca71e3912ab028d50fde3facaa8d05cab81873bdefbb95804f2d34`.
The production verifier recomputes it at the field revision and candidate
`HEAD`; any tracked, staged or untracked drift fails closed. Because candidate
control files are outside that surface, composition does not require another
field network run.

## 3. Independent reproduction continuity

The independently reproduced offline surface is unchanged by M0.9.92. The
candidate therefore retains revision
`68804c93f43b405008cfecf35dc8660db360a3a3`, GitHub run `38507114425`,
attestation `55683374`, and the 2,432,000-byte `kilogram-offline.exe` with
SHA-256 `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`.
Its exact 63-entry protected surface remains
`2a7db2c5cacd0419a8ece61b0dc8386075a558d4f052b44c0eedd81cd9354d68`.

## 4. Honest residual boundary

Boundary claims: does not claim a public security release; does not require another field network run;
exact runtime/protocol Git surface; strict
independent-provider field acceptance remains pending; Sybil resistance
remains incomplete.

- strict independent-provider field acceptance remains pending;
- operator and network independence remain unproven;
- Sybil resistance remains incomplete;
- access correlation is not hidden;
- field evidence remains Windows-only;
- this is not a public security release.

## 5. Composition boundary

The candidate verifier itself is network-free and build-free. It creates no
executable, archive, background service or Git tag. The fresh field run is
upstream evidence; candidate composition only verifies its immutable revision
and selected-tree continuity. Production verification must run from a clean
committed tree before M0.9.93 can be accepted.

## 6. Acceptance evidence

Candidate-control revision `d8252c660dcde49d93ad32c16104c46e91ac8c84`
passed the production candidate verifier from a clean working tree. It reported
the field and independent protected surfaces unchanged and
`m1_candidate_status=verified`.

The complete readiness gate then passed all `33/33` closed boundaries, format
and locked/offline strict Clippy with two Cargo jobs. It reported
`m1_readiness_status=verified`, started no Rust test executable, and performed
no release build, archive creation, network action or Git tagging. Hosted full
Rust execution remains a separate required confirmation before an M1 tag.
