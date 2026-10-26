# A3-S5.2 boundary03 readiness — mechanical allowance change

2026-10-26. Static preparation under
[boundary continuation review2](long-lived-dialog-a3-migration-s5-l1-boundary-continuation-review-2.md),
7582B SHA256 `e6ebba9e204eea61d53438012d2bcf11f98cd9159a34f4008d69e08e0cea6f0f`.
No test executable, C/F, fixture operation, deadline probe or product build ran
in this preparation. A new root review/go-ahead and ordinary native execution
approval are still required; the executor has not created approval03.

## Retained failure and fresh-origin contract

Boundary02 remains whole FAIL568.1367185s, controller560 cutoff, harness101,
work580 unexpired, children after cleanup0. Boundary01 remains whole
FAIL120.1641445s; origin01 remains whole PASS481.4608671s. Earlier Ready02 and
capacity01 split outcomes and the non-contributing prelaunch approval rejection
remain unchanged. No partial result is relabelled PASS.

The new controller still starts a complete fresh isolated copy from the same
frozen origin. It does not resume either failed branch or reconstruct the
original cursor/rows/received-A values lost from RAM. The failed branches and
original were untouched throughout preparation: no typed open, raw traversal,
copy, repair or resume. Historical before-origin verification is not presented
as a post-attempt immutability check.

## Explicit amendment4, no new contribution

Before any accounting/source changes, the exact ledger404540B SHA256
`83ea734311622f0578d843c0bbae4587c22c7538f6618deb974b6855516cf3fc`
was preserved as
`long-lived-dialog-a3-migration-s5-l1-evidence/S-budget-amendment-4-ledger-before.json`.
The same snapshot operation preserved the boundary02 controller/runner and
noncompiled harness before editing/restaging them.

The ledger now has one appended `budget_amendments` entry bound to the root
decision above. Only `new_ceilings_seconds.F`2900->3650 and
`cumulative_maxima_seconds.F`4136.2886628->4886.2886628 changed otherwise.
All36 cases, every charge/outcome, prior actuals, first three amendments and all
other top-level fields retain exact semantic equality. No contributor was added.

| Quantity | Current seconds |
| --- | ---: |
| S5.2 actual C/F/P | 147.8846192 / 2614.1535757 / 0 |
| S5.2 ceilings C/F/P | 500 / 3650 / 0 |
| Prior S5.1 actual C/F/P | 279.3257298 / 1236.2886628 / 0 |
| Combined actual C/F/P | 427.2103490 / 3850.4422385 / 0 |
| Combined maximum C/F/P | 779.3257298 / 4886.2886628 / 0 |
| Remaining S5.2 F | 1035.8464243 |
| Required entry reserve | 1025 = whole800 + contingency225 |
| Remaining after maximum whole800 | 235.8464243 |

Current ledger405538B SHA256
`da67390252936bfde924c013061d8d201e3e678adbcb1e192cba0eb5bb3e1d39`.
[Amendment binding](long-lived-dialog-a3-migration-s5-l1-evidence/S-budget-amendment-4-binding.json)
2822B SHA256 `1f1bea8be58afd7cec90cd4a4d463d485a4423922bd8b2d028653377b69b238b`
records both hashes, snapshots and semantic retention checks. Summing eight C
and28 F records still matches charges. This amendment is neither a refund nor
P borrowing. Root's700-740s completion estimate is not a measured result or an
upper guarantee. Terminal-publication timing disclosure is unchanged.

## Exact source/harness delta

HEAD remains73c499818ba9b699c331aa842e64e8432e90c59a; index is empty.
Exactly two of52 compiled inputs differ from boundary02:

| Input | Before SHA256 | Current bytes / SHA256 |
| --- | --- | --- |
| Exhausted controller | `dd3a4b9e6c76daa1056b1bb3d765bcb251c54092ac14f84db3178cc7c5c510bd` | 40620 / `c3e044594d43bcbf60b8055ba203aa8e23220f3b92b35d75f9bbfc049aae0977` |
| `scripts/invoke-migration-s5-l1-case.ps1` | `22dcaa81179e39986199664adea130c01c22749b3ea69a34664a60b2d66ca79e` | 26849 / `d3324692346ae12da8e10cb2bbeb2a614d9201d39597eb80536d1e8d34b2aac2` |

Controller path:
`apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs`.
The exact source after newline normalization equals the saved source with only
560->760,580->780,600->800 and case02->case03 substitutions in the existing
deadline/case/comment/metadata positions. All phase records, controlled hooks,
remove-before-atomic-release behavior, child cleanup, assertions, migration argv,
Node transitions, origin requirement, checkpoint semantics and reconnect25 remain.
No shared helper, product source, validation or runtime deadline was altered.

Runner adds the exact case03, work780/whole800 and entry reserve1025. It binds
review2, explicit activated amendment4 and retained failed02 case/log/observation/
phases hashes. Inherited origin, small-gate, failed01, exact selector/package,
no-resume, NativeToken and later root-approval guards remain. The root approval
must bind the new runner, amended current ledger, actual origin case and case03.

| Immutable case | Work / whole | Entry reserve |
| --- | ---: | ---: |
| boundary01 | 120 / 140 | 365 |
| boundary02 | 580 / 600 | 825 |
| boundary03 | 780 / 800 | 1025 |

Old IDs remain unusable for replay. There is one absolute monotonic controller760,
never renewed per child/checkpoint, with the unchanged reconnect25 local bound.
Outer work780/whole800 still includes setup and finalization and remains
authoritative around blocking calls. Controlled pauses/log sync costs stay charged.

Exact noncompiled pre-edit snapshots:
`S-boundary02-controller.rs.txt`, `S-boundary02-runner.ps1.txt` in evidence, and
`target/compatibility/long-lived-dialog-migration-boundary02/migration_s5_process-harness.bin.snapshot`.
Old harness13935104B SHA256
`5a003864af885c09d0274d84f54b2000a774908382169085b42314ee546ec161`.
These snapshots and original/readiness-v1/revision2/boundary02 records are retained.

- [Controller patch](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary03-controller.patch.txt):
  3422B SHA256 `7973a6753fef5af28ede80619093c4fdb28b36e904d4e8ea85e38ab360b90788`.
- [Runner patch](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary03-runner.patch.txt):
  6819B SHA256 `ef4c6ed8e8a5efd3f6accbd879f3cd567a4e96f610b953c0d8c90ffe797424e2`.

## Static verification and stopping boundary

| Record | Result |
| --- | --- |
| S-fmt-28 | PASS, fmt check,3.8370189s |
| S-clippy-25 | PASS, CLI all targets, `-D warnings`,3.3669095s |
| S-compile-21 | PASS, stable wrapper without `-Run`,21.0042926s; actual Cargo5.43s |
| S-boundary03-policy.json | Parser0 errors; exact normalized controller substitution and source policy checks |

Clippy/compile used locked/offline verification, Jobs12/BelowNormal. Compile log
states compiled-only and no test executable started. Policy validation parsed
AST/source strings; it did not dot-source/execute the runner or probe the deadline
mechanism. `git diff --check` passes. No Fast setting was changed.

New test harness at the same fixed path
`target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`:
13935104B SHA256 `83429f6d41f566aadfdaddea0b862a0fe71120beec875560f6d9463dfdd932d0`.
All52 final compile inputs match, all33 product inputs/eight images remain
unchanged. CLI/control peer remain SHA256
`69744fca50f44439b4734e699f416fc6b2bb8aac4336e5dad18ed173b2abbf6c`.
No product build/stage action or product optimization was performed.

[Readiness binding](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary03-readiness-binding.json)
41370B SHA256 `e8276f359e52fe574cdf66b90ce624eb1a632a8b73d51bdca418674e4bbb769a`
contains exact deltas/source/harness/static records,49 protected historical
artifacts, accounting retention/sums and parsed policy. Superseded hashes at old
source/harness/ledger paths denote historical versions now preserved as snapshots;
they are not claimed to match the replaced active paths. No prior binding was
rewritten. Root-owned navigation and future S5.3 task were not edited.

No case03 approval/start/result exists. After root accepts this package and
creates the separately bound approval, the future command is:

```powershell
& scripts/invoke-migration-s5-l1-case.ps1 `
  -Case F-exhausted-boundary-03 -Set F -Action Tests -Package kilogram-cli `
  -Filter migration_s5_l1_genuine_exhausted512_refusal_receive_history_checkpoint `
  -FixtureDirectory E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-l1/F-exhausted-origin-01 `
  -LongApprovalFile ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-boundary-03-root-go-ahead.json `
  -ExecutionMode NativeToken
```

This command was not executed. The prior human permission was for case02 only;
native execution approval remains a separate future step. Stop here for review.
No commit/push, whole-P, full implementation acceptance, product fix/optimization
or S5.3/cut3 was performed or authorized by this preparation.
