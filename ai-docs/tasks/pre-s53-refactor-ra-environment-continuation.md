# R-A: process environment correction and final bounded continuation

2026-10-26. Baseline `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`.
Supplement to [Native continuation](pre-s53-refactor-ra-native-continuation.md).

## Evidence and diagnosed runner defect

NativeToken `C-ready-legacy-02` is whole FAIL/exit1, measured 11.3103439 seconds.
Its kernel failed 0/1 in 0.90 seconds after two real device-enrolled outputs,
with Windows path-not-found/os3. No Ready acceptance follows from that run.
Source/image equality, unexpired deadline and verified zero owned children were
recorded. Terminal accounting completed normally.

Case SHA256 `142dab6147e9d7d9dfad1b4aa276c052f34ab22b051bc5bf6b2013f2c8610d81`;
log `5b9848253622aca0094ba7046c58c4ef8c5071419eb06db0b24cc98027a0bdd5`;
r3 ledger `33eaad603d4fde91bf8483cc451285fa85c897d4e6439688151904ec5c7990cb`.
Keep those and all earlier artifacts unchanged.

Root reproduced an environment-handling defect in a fresh PowerShell process
using only a new task-specific variable: `SetEnvironmentVariable(name, $null,
'Process')` left a present empty value; `Remove-Item Env:name` removed it. The
case adapter used the former to clear the test hooks. In `Pair::new`, after the
two enrollments, `engine::now` reads KILOGRAM_TEST_OD1_CLOCK_FILE when present,
including an empty path. This source path and observed environment behavior
explain a guaranteed failure and match the logged sequence; no Rust defect or
positive Ready result is inferred from the failed run.

## Exact repair

Retain the executed f9b58b93... runner source before editing. Correct only its
temporary process-environment cleanup/restoration, plus literal continuation
case policy and binding/ledger paths:

- Remove absent hook variables through the Env provider, rather than a null
  argument to SetEnvironmentVariable.
- Preserve the distinction between an originally absent variable and an
  originally present empty or nonempty string when restoring the saved values.
- Check these three states in memory in a disposable PowerShell process, using
  the actual small environment helper if one is introduced. Do not execute the
  product/harness, open fixtures or change persistent/user/system environment in
  this diagnostic.
- Keep source/image checks, parser, cleanup, accounting, terminal-failure guards
  and independent deadlines unchanged. No Rust/test-body change or rebuild.

## Authorized continuation

Prior charge: conservative C60 + measured C10.2520644 + measured C11.3103439 =
**C81.5624083**. Remaining **C38.4375917/F210/P0**. The original R-A C120/F210/P0
ceiling and inactive R-B C60 remain unchanged.

Use a new r4 binding and continuation ledger linking the preserved r1-r3 records,
and allow only:

| Case | Mode | Work / whole seconds |
|---|---|---:|
| C-ready-legacy-03, unchanged exact selector | NativeToken | 25 / 35 |
| F-ready-scope-01, still unstarted | NativeToken | 180 / 210 |

The narrowed C limit fits the remaining allocation. The stable wrapper's previous
C02 cached compilation took 2.41 seconds; this is a bounded retry after a specific
runner correction, not permission for repeated attempts or a performance claim.

Submit the environment helper/changed lines and exact new binding hash for a
short root source review. Once cleared, run C03 once and F once if C03 passes.
Any failure, overrun, new source drift or tool rejection stops execution and is
reported. The standing human permission applies through ordinary tool review.

After successful completion, submit the complete R-A candidate, preserving and
distinguishing all three earlier whole failures, their observed kernels and
their measured/conservative accounting. Root implementation acceptance remains
required before commit/push. R-B and S5.3 are still inactive.
