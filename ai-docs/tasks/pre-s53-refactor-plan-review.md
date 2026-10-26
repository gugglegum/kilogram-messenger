# Pre-S5.3 refactor plan: root decision

2026-10-26. Root accepts the bounded plan at published baseline
`eafd0f873eb31b64377e8e65e06bc84a911f6dc6`.

Reviewed [plan](pre-s53-refactor-plan.md): 22331 bytes, SHA256
`fdd411fb4a2519af494648858abcbe4c870f725e901479644048b4a3a85036a9`.
Authority: [assignment](pre-s53-refactor-task.md). This is technical acceptance
of the plan, not acceptance of an implementation or permission to publish an
unreviewed diff.

## Review result

Root inspected both actual Ready handlers and their scope blocks. The blocks in
`Engine::ready` and `runtime7::bootstrap::ready_claims` have identical field order,
arity, D/E/SID, peer-role, suite/format checks and refusal text. Extracting only
that predicate is appropriate. Engine's event reads precede it; Owner7's event
reads follow its eager genesis parsing. Those orders must remain unchanged.

In particular, Engine's first conflicting genesis ID short-circuits parsing the
second ID, while Owner7 parses both. Preserve this observed compatibility behavior,
Engine's eager fallback evaluation, and all witness, signature, lease and commit
boundaries. Harmonizing those algorithms is outside this refactor.

The independent GPT-6.1 Sol Max review of R-B and verification/accounting found no
plan blocker. Its implementation review must still check that every existing
assertion field is retained, the snapshot handle is closed before the tail,
collision/write failure prevents the tail, and a failed tail still fails the
test and whole attempt. An assertion snapshot is not a whole PASS.

Deferring a broad main.rs extraction is accepted: the relevant ordered/IPC3
adapters already exist, and moving unrelated orchestration does not address the
observed defects. Independent storage/runtime authorization guards remain intact.

## Activated work and budget

Activate **R-A only** now. Its permitted production and test changes are exactly
the plan's R-A scope, plus its separately reviewed verification adapter and
new evidence/docs. No S5.3 behavior, R-B source change, product format change or
private exhausted-checkpoint action is part of this authorization.

Use a new ledger. The accepted overall refactor allocation is C180/F210/P0;
the R-B C60 portion is reserved and remains inactive until R-A publication is
independently verified and root assigns R-B.

| R-A case | Work / whole seconds | Classification |
|---|---:|---|
| C-ready-scope-01: existing indexed scope/shape component | 45 / 60 | C |
| C-ready-legacy-01: genuine legacy handler characterization | 45 / 60 | C |
| F-ready-scope-01: existing signed Ready public-dispatch scenario | 180 / 210 | F |

Active R-A cumulative limits: **C120/F210/P0**. Each case is authorized once;
there is no automatic retry reserve. Keep every started attempt and actual
failure/elapsed time. Stop and report on failure, insufficient source provenance,
budget/deadline exhaustion or unexpected scope expansion. Do not spend historical
S5.2 remainder or the reserved R-B allowance.

For F, establish genuine small-origin provenance, stopped prior branches and raw
source integrity within the charged attempt, as the plan requires. If that input
is unavailable, report it; do not silently generate a replacement. No traversal
or activation of the exhausted NCjBHb checkpoint or its backup is needed.

Retain the existing owned-job deadline and stable executable paths. Bind exact
source/image hashes and preserve pre-run inventory before execution. Use locked,
offline, Jobs12/BelowNormal compilation and the stable Windows harness runner;
no direct network cargo test. Compile/fmt/strict Clippy are source-bound static
checks; each executable case includes its real wrapper/setup/finalization work.
The whole watchdog remains armed through results, ledger and cleanup, with
actual verdict, matched count and owned-child cleanup checked independently.

## Delivery and publication sequence

1. Implement and verify R-A; submit a concise report, source diff, new evidence,
   reuse assessment and exact candidate path/hash allowlist. Include this
   assignment/plan/decision chain in the candidate, preserving the queued S5.3
   handoff task unchanged and outside the R-A allowlist.
2. Stop for root implementation review. No commit/push until root explicitly
   accepts that diff and assigns publication. Ordinary accepted publication has
   standing user authorization.
3. After root independently verifies R-A's commit and push, root assigns R-B
   against that published SHA, with its reserved C60/F0/P0 case.
4. Review and publish R-B separately. Only then resume the queued S5.3 mapping
   against the actual new published baseline.

All published S5.2 evidence, failed/split outcomes and accounting remain
immutable. Reused checks retain their original scope and source/image binding;
they are not relabeled as fresh complete coverage of a new image.

## R-B caller compatibility clarification

Additional read-only review found that the old S5.2 runner calculates source/image
inventory before execution but keeps it in RAM until its final case record. It
does not supply a pre-run inventory binding to the exhausted controller. The
historical case03 record cannot establish provenance of a new invocation.

Accordingly, the plan's mandatory pre-run binding applies to new controlled
verification and future S5.3 controllers. Preserve the existing exhausted
controller's caller contract without editing the historical runner:

- The persistence helper receives an explicit envelope; it does not read
  environment variables or discover historical evidence automatically.
- Case and actual run/output path are always present. Provenance is explicitly
  either `bound_pre_run_inventory` (path, digest and inventory identity) or
  `unavailable_legacy_runner`. No historical substitution or invented binding.
- New C verification supplies its actual pre-run binding to all three lifecycle
  checks. The existing exhausted seam uses the explicit legacy state; its snapshot
  is retained observations, not independently source-bound evidence or whole PASS.
- Keep the old case03 guard, functional body, runner and preserved historical
  outputs unchanged. Its new seam receives source/compile review, not a claim of
  another long functional execution.

This clarifies the future R-B assignment and does not activate it during R-A.
