# Block4O: fixed F05 purpose source candidate

2026-11-03. Baseline HEAD `438662ed957ba523b90626a2fe776eaa768de1eb`. This candidate closes consumed F04 and selects `F-s54-mixed-hooks-feasibility-01-attempt-05` in the shared PS/Rust purpose policy. It does not allocate, prepare, activate or run F05/P. No commit/push was performed. F04 remains consumed/failed; F03's historical failure cause is not inferred. Full admission/runtime acceptance is not claimed.

## Source scope

Eight source/test paths, plus this report and its `.pins.json` manifest:

- `migration_s54_purpose.rs` and `invoke-migration-s54-whole.ps1`: only the fixed FeasibilityF attempt suffix changes 04 to 05. Case, selector, namespaces, workloads and deadlines remain unchanged: F450/480/510 and P01/P2350/2375/2400.
- `migration_s54_feasibility_tests.rs`: explicit F01/F02/F03/F04 identity refusals, an executed count of four, and exact F05/P01 controls. Existing cross-purpose and deadline checks remain.
- `migration_s54_admission_tests.rs`: all ten existing/new mutations execute, including F01–F04 at indices 6/7/8/9. Each old-attempt refusal must contain the actual `S54 cross-purpose identity` error. The native transcript records every refusal and the executed count. Public F05/P01 fixtures are inspected successfully, and fixture policy still refuses production capability. A shared test-only export helper adds precisely the owned Block4O admin root alongside the existing root; canonical path validation and exclusive file creation remain.
- `migration_s54_compiler_parity_tests.rs`: exactly one comparison changes from the active purpose to literal historical F04. The probe retains the original public F04 inventory path/length/SHA256, implementation HEAD and run ID. Its ordinary test uses a fresh synthetic fixture; the historical probe opt-in was absent. No historical baseline/after replay or compiler regression matrix was repeated, and the accepted native compiler implementation is unchanged.
- `test-migration-s54-feasibility.ps1`: owned Block4O evidence root, F05/P01 binding, explicit four consumed output-policy refusals and an executed count. Existing grammar/cross-purpose/leaf controls remain.
- `test-migration-s54-preparation-order.ps1`: owned Block4O evidence root, current F05 control and four inventory refusals with exact `S54 required identity.` evidence. Test-only `-PurposeOnly` runs the shared actual inventory checks without the unrelated synthetic staging-order matrix; its default remains the original complete test.
- `test-migration-s54-admission.ps1`: the existing HEAD test's current expected fixture is F05; its old-attempt list now includes F03/F04. No HEAD matrix was rerun; PS parsing passed and the common actual inventory policy was exercised by the focused purpose-only checks.

## Actual checks

Fresh administration: `.tmp/s54-block4o-f05-purpose-01`. Compile-only and execution use the existing mutable nonproduction `target/s54-diagnostics-source-checks` and fixed stable EXE names, through `scripts/run-cargo-tests-stable.ps1`, verification profile, locked/offline, Jobs2/BelowNormal. Historical source-check image pins remain historical; production images/origins were not replaced.

| Step | Actual tool return | Native child/owner | Elapsed seconds | Result |
| --- | --- | --- | --- | --- |
| format-01 | `08b4cc` | 0/0 | 1.6525264 | scoped rustfmt and four PS parses |
| compile-01 | launch `05fc03`, final `805c8a` | 0/0 | 48.3498410 | compile-only; no test EXE launched |
| rust-01 | launch `1f62b4`, final `d85b52` | 0/0 | 13.7723249 | exactly four isolated native tests passed |
| ps-01 | `a45524` | 1/1 | 2.7182484 | administrative variable conflict before any PS test; retained |
| ps-02 | `6510f4` | 0/0 | 7.7751133 | 32 purpose/output and nine actual inventory checks passed |
| seal-01 | launch `f88e3c`, final `ca3571` | 0/0 | 10.9845839 | scope, pins, executed markers, formatting and process inventory passed |

Compile deadline1170/1200 seconds with30 seconds reserved for cleanup; other checks570/600/30. Every actual receipt records verified handle/Job membership, waited actual child and bounded stdout/stderr EOF. Compile cleanup terminated owned residual PID60368, verified after[]; its identity was not captured and is not inferred. All other cleanup was verified/nonforced with after[]. No failed receipt was overwritten. Compile stderr is the ordinary successful Cargo status; seal stderr only contains Git line-ending notices.

The ps-01 administration failure came from assigning a directory to `$purpose`, whose dot-sourced PS parameter carried a ValidateSet. The preserved `ps-01.work.ps1` snapshot and stderr show that no PS check had begun. The administrative variables were renamed, and ps-02 used fresh evidence directories with unchanged source and fresh, independently exported synthetic F/P fixtures. No production attempt was consumed by this correction.

The first evidence finalizer (`a75417`, exit1) compared canonical Windows pin path strings against exported Rust path strings containing mixed separators. It stopped before writing this manifest/candidate; fixture bytes, sizes and hashes were unchanged. Preserved `finalize.py` records that assertion. `finalize-02.py` compares the same canonical path and exact byte/hash identity; it retains the prior finalizer's outputs and records the failed return. No source/test rerun or fixture rebinding was needed.

The four selected native tests were:

1. `feasibility::tests::s54_f_closed_purpose_rejects_cross_case_selector_attempt_and_keeps_p_limits`
2. `admission::tests::s54_f_admission_full_public_fixture_binds_fixed_purpose_and_refuses_p_substitution`
3. `admission::tests::s54_admission_valid_public_binding_has_real_blake3_but_no_runtime_capability`
4. `admission::tests::compiler_parity::s54_compiler_public_f04_replay_without_runtime_admission`, ordinary synthetic lane only

All have prefix `live_l1::handoff::final_p5::`. The aggregate harness invocations selected zero tests; four exact isolated results each report one passed. Native output has four purpose refusals and four shared-admission refusals, F05/P01 positive markers and `s54_f_shared_admission_mutations_executed=10`. The historical `S54_PUBLIC_COMPILER_REPLAY` marker is absent.

The actual PS output-policy cases explicitly execute F01/F02/F03/F04 refusals and count four. Public inventory cases accept F05, reject each old attempt with `S54 required identity.`, reject the P selector for F and accept P01. This is actual `Assert-S54Inventory` on newly exported public fixtures, not production preparation or a full production admission verdict.

## Preservation and review handoff

Final seal rechecked416 immutable production/history/Block4N evidence pins and the520-file source closure: only the eight allowed current source/test files changed, with512 other source files unchanged. It verified untouched root navigation, baseline HEAD, empty Git index, no relevant live native images and absence of production F05/P in both run and public-evidence namespaces. The finalizer additionally compares the outside-allowlist Git status digest against the pre-edit snapshot. Historical F01–F04 images, origins, captures, attempts and reports remain unchanged. No private access, F05/P allocation, Prepare, activation or Run occurred.

`candidate.json` contains the exact ten paths, raw lengths/SHA256, regular100644 modes and normalized Git blobs for review. `source-diff.patch` contains only the eight source/test changes. This report's pins manifest binds the source closure, current source-check images, fresh F/P fixture files, all actual receipts/captures, preserved administration failure and historical preservation pins. Root navigation, task/acceptance/review files and all unrelated changes are excluded. Stop for root review; publication and actual F05 preparation require separate assignments.
