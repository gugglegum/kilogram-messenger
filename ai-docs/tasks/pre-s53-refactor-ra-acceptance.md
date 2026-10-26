# R-A implementation acceptance

2026-10-26. Root accepts R-A for allowlisted commit/push on master, with sole
parent `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`.
R-B and S5.3 remain unstarted; this decision does not activate either.

Reviewed [implementation report](pre-s53-refactor-ra-implementation.md):
11263 bytes, SHA256 `eed5b74d71c36ea4262fa15513936085cc20943df1d4c65b63b5064860c5b5ab`.
Reviewed [candidate](pre-s53-refactor-ra-candidate-files.json):
25033 bytes, SHA256 `8d8deb37d2141b3be30fb15e9a6d9d30c47853ba322f3d974c1722a597e664b0`,
normalized Git blob `d0be7807425936d8eb84258bb165e4babc4e3b0b`.

## Accepted scope and evidence

The private Ready1 predicate retains the identical field checks, error text and
evaluation order. Both complete handler files differ only at the extracted
predicate call; their event reads, genesis parsing differences, witness selection,
lease/signature checks and persistence remain unchanged. The module declaration
and private helper expose no new public API. Independent storage guards, product
formats, cryptography and main.rs are unchanged.

Root reviewed the genuine two-role legacy characterization, including the exact
seven-field negative fixture signed and self-verified with the actual peer device.
Normal-shaped controls retain the ordinary production signer. The test does not
weaken the production parser or synthesize positive Native lineage.

- C-ready-legacy-04: whole process exit0, 24.7713883 seconds; named kernel1/0,
  16 refusals, 16 valid retries, two short-circuit forks, retained peer Ready
  witnesses and blocks after cold reopen.
- F-ready-scope-01: whole process exit0, 148.7417507 seconds; named kernel1/0
  in131.31 seconds, actual public dispatch, eight malformed signed controls,
  valid continuation, scoped fork, renewal/reopen and witness preservation.
- Both have normal stable-wrapper completion, exact source/image equality,
  unexpired work deadlines and verified zero owned children. F's current raw
  before/after inventory of197 original files matches; it is not a retrospective
  producer-time inventory. No typed original open is claimed.
- Fresh strict CLI Clippy, fmt-check and compile-only verification cover the
  corrected test. The unchanged product build is reused with explicit source
  equality. Root verified all820 r5 input records and7 actual image records.

All four prior whole failures remain failures. The first parser kernel's narrow
assertions are accepted under the separate root reconciliation; its optimistic
draft JSON and unfinished ledger remain unchanged and do not establish whole
PASS. The other failures identify restricted identity I/O, the runner's empty
environment-path bug, and the malformed test-fixture construction respectively.
The final successful characterization supersedes no historical failure record.

The verification adapters' reviewed corrections preserve real work/whole
deadlines, failure evidence, exact counts and cleanup status. Explicit scalar
accounting and absent/empty/nonempty process-environment restoration were checked
with small source-bound diagnostics. No user/system environment was changed.

Final R-A charge: **C118.3327995/F148.7417507/P0**. C includes a conservative60
allocation and measured58.3327995; it is not wholly measured elapsed time. The
explicit revised R-A ceilings are C150/F210/P0. Unused allowance is not a new
assignment. The separate R-B C60 reserve remains inactive.

## Independent candidate and publication checks

Root independently verified all90 candidate entries against current raw
bytes/SHA256 and normalized Git blobs: zero mismatches. Candidate plus self and
the excluded queued S5.3 task exactly cover the pre-acceptance dirty tree; the
index is empty and HEAD/branch are the required baseline/master. The relevant
source diff passes `git diff --check`.

Root also verified516 unchanged entries of the published S5.2 candidate, the
original candidate/acceptance hashes, and the queued S5.3 task's raw hash:
`75cbc0f9950971820beffb08edbd7c6ccb4465d8be44aea331c51687f1807f7c`.
The private exhausted checkpoint/backup, private images and data remain outside
this change. No R-B implementation, S5.3 work or MVP acceptance is included.

Authorize exactly **92 paths**: the90 candidate entries, the candidate itself,
and this root acceptance document. Preserve the queued S5.3 task untracked and
unchanged. Stage and verify only that allowlist; do not rewrite raw evidence,
as-submitted report status, old decisions or historical ledgers to make them
appear uniformly successful.

Create one normal commit and push origin/master under the standing user
instruction. No amend, reset, force push or extra tests/builds are needed.
Return the actual commit SHA, sole parent, pushed remote tip and publication
result. Root will independently verify the committed paths/blobs and remote
equality before assigning R-B against the published SHA.
