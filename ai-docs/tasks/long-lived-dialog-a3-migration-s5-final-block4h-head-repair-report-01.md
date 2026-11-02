# Block4H HEAD resolver / fixed F03 — source review candidate

2026-11-03 Asia/Yekaterinburg. Baseline `d62a525eca008adfe0e2784645db4e8871986acd`. Packed HEAD resolution and an early preparation prerequisite are implemented; the next fixed F identity is consistently F03. Eight source files change. No real .git write, production Prepare/F03 allocation, activation, F/P scenario, private profile, commit or push occurred. This is source/focused-test readiness, not runtime acceptance.

## Behavior and source scope

`Get-S54SourceHead` in invoke-migration-s54-whole.ps1 uses strict UTF8 and the existing bounded/nonreparse public reader: HEAD4096 bytes, loose4096, packed1048576. Detached/resolved values must be exactly40 lowercase hex; HEAD/loose retain the established trim. Symbolic refs stay under local refs/heads with traversal/component/Git-directory confinement. Empty components, trailing separators, backslashes and colon/ADS forms refuse. External gitdir/worktree files, symbolic chains, arbitrary namespaces and reftable are not added.

A valid present loose ref wins over absent/stale packed data. After fresh path/parent checks, native GetAttributes distinguishes precise FileNotFoundException/DirectoryNotFoundException from directory/reparse/denied/IO failures. Only the former permit packed lookup; Test-Path false alone cannot authorize it. Malformed/unreadable present loose files never fall through. Packed lookup rechecks loose absence/parents and rejects an intervening appearance. There is no cache across checks.

Packed records match the whole reference case-sensitively. Count matches before digest validation, including malformed/empty-digest duplicates, then require one valid digest. Comments, peeled lines and unrelated refs are ignored. Rust head() is unchanged: supported ordinary formats are the compatibility reference, not a byte-for-byte parser equivalence claim. PS deliberately refuses present directory/reparse/invalid-name cases and explicitly handles comments/peeled lines rather than mirroring incidental Rust split behavior on every possible file.

Assert-S54Inventory invokes the helper at the previous HEAD-resolution position, retaining comparison with observed_head. Invoke-S54Preparation invokes the SAME helper after observing Git HEAD, before run/public reservation, leases, guard or compilation; disagreement refuses early. This additional prerequisite does not replace a final/runtime validation. Four PowerShell/three Rust boundaries, T0, deadlines, selectors, workloads and all other source/image/read/hash checks remain.

FeasibilityF attempt02 becomes attempt03 in PowerShell purpose and Rust Purpose; exact corresponding tests/fixtures admit F03 and refuse consumed F01/F02. F case/selector/450-480-510 and P01/2350-2375-2400 are unchanged. The admission script adds an opt-in HeadOnly test lane; no production mutation/denial hook exists.

## Final focused checks

**67 HEAD/admission checks**, **29 purpose checks**, **35 preparation-order checks**, and **3 exact Rust tests passed**.

Fresh tiny fixtures cover detached/loose/packed, loose precedence, branch subpaths, comments/peeled/unrelated/case variants, absent/no-match/duplicate and malformed/empty-digest duplicates, uppercase/short/long/bad digests, strict UTF8, each cap at/over its boundary, traversal/nonlocal/empty/ADS/backslash refs, actual directory/junction/dangling junction loose metadata, unsupported gitdir file and packed directory. Denied/IO attribute/read failures are explicitly injected; false Test-Path plus denied native attributes still refuses. No real ACL changes were used. Between-call mutations show no cache; a loose ref appearing during packed read refuses.

Actual complete production Test-S54Admission on a fresh public packed-ref F03 fixture passes. Wrong observed HEAD and F01/F02 identities refuse via actual predicates; public policy supplies no runtime capability. Read-only real checkout resolution agrees with Git. Actual Invoke-S54Preparation under isolated policy/call interception rejects malformed/mismatching metadata with zero reservations/guards/compile calls. Its valid control reaches the intercepted first reservation, still creating no directory. Only newly owned fixture .git metadata was written.

Relevant purpose/order checks retain marker/frozen-leaf boundaries, P01, seven leases/exact stages, interrupted/duplicate/foreign refusals and public F03/P admission. No historical fixture was cloned, rebound or written. No old full path/index/performance matrix was rerun.

Exact Rust selectors through the stable runner:

- live_l1::handoff::final_p5::admission::tests::s54_admission_valid_public_binding_has_real_blake3_but_no_runtime_capability
- live_l1::handoff::final_p5::admission::tests::s54_f_admission_full_public_fixture_binds_fixed_purpose_and_refuses_p_substitution
- live_l1::handoff::final_p5::feasibility::tests::s54_f_closed_purpose_rejects_cross_case_selector_attempt_and_keeps_p_limits

Each ran1test/0fail. Four aggregate harness invocations selected0tests; no unrelated test body or F scenario ran. Scoped Rust formatting, PS parsing and whitespace checks passed.

## Separate stable slot and actual commands

Harness options were inspected before execution: arbitrary StableArtifactsDirectory/CargoTarget selects the frozen production lane, which is intentionally unused. Profile verification source checks use fixed new target `target/s54-head-source-checks`; the main stable path is `target/s54-head-source-checks/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`.

A process-local cargo function forwards the runner's real compile-only argv plus explicit --offline/--target-dir for test compilation; metadata gets --offline. CARGO_TARGET_DIR stays unset so strict policy fixtures do not inherit a forbidden override. An isolated Copy-Item guard confines every source/destination to the new slot. Runner source and production images are not patched. Actual argv/copy logs and pins are retained. Compile-02 is compile-only393.2944935s; rust-01 reuses its valid cache then runs exact selectors in22.0627159s overall. All four separate-slot stable EXE pins match between compile/run. Hashes are evidence, not filenames.

Installed pinned pwsh/Rust1.98, Jobs2/BelowNormal/offline/locked and process-local environment were used. Entries take this form:

```powershell
& C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe -NoProfile -NonInteractive -File .tmp/s54-block4h-head-repair-01/owner.ps1 -Step ps-05
```

| Step | Actual child exit | Owner seconds | Work/whole/cleanup seconds |
|---|---:|---:|---:|
| compile-01 | 1 | 6.1044934 | 1170/1200/30 |
| compile-02 | 0 | 393.2944935 | 1170/1200/30 |
| format-01 | 0 | 10.2464684 | 570/600/30 |
| ps-01 | 1 | 8.1917860 | 570/600/30 |
| ps-02 | 0 | 23.8860431 | 570/600/30 |
| ps-03 | 1 | 8.3868505 | 570/600/30 |
| ps-04 | 0 | 22.5503545 | 570/600/30 |
| ps-05 | 0 | 21.5869541 | 570/600/30 |
| rust-01 | 0 | 22.0627159 | 570/600/30 |
| seal-01 | 0 | 11.6454189 | 570/600/30 |

All10 bounded commands verify child/Job membership, wait, EOF and cleanup with no remaining members or expiry. Compile-02 cleanup forcibly terminated one owned remaining Job member PID26900; executable identity was not recorded/inferred. Other cleanup was unforced. Cargo/line-ending diagnostics in successful stderr are retained.

Three failed bounded attempts are preserved administrative/test defects: compile-01 had zero native cargo calls due to callback script-scope ledger lookup (fixed with unique process-local globals); ps-01 needed explicit byte-array assignment for an empty file; ps-03 needed empty-safe read-cap aggregation for refusal before the first read. Fresh retries never repair historical data. Final ps-05 includes later component/false-exists coverage. A first diagnostic export failed on missing nested destination parents; its partial evidence-01 directory remains retained/excluded. The complete evidence-02 export is the candidate; no source/image changed during export repair.

## Preservation, limits and submission

The native final seal reverified83 protected pins covering F02 public artifacts, current9images, compiler origins/dep-info and failed F01 evidence. All509 baseline sources outside the intended8 retain their pins; the complete517-source final set is recorded. HEAD d62a525..., index empty and root navigation three are unchanged. No relevant historical/new-slot image process remains. Failed F01/F02 runs stay empty; F02 preparation_succeeded=false/no inventory remains. Production F03/P directories are absent. Other work is preserved.

The reviewed failed F02 report/pin manifest are carried unchanged and link large originals. New `long-lived-dialog-a3-migration-s5-final-block4h-head-repair-evidence-02/manifest.json` lists byte-identical administration/code/argv/receipt/log/result/source/preservation exports. No binaries, captures or fixture trees are copied. Root-owned navigation/task/review files are outside the allowlist.

This repairs the immediate PS metadata defect and prepares F03 source identity. It does not complete F02, admit a real F03 inventory or demonstrate whole runtime budget fit. Publication and a fresh root preparation identity/dispatch remain prerequisites for real F03 Prepare; remaining predicates/costs need actual execution.

Exact candidate: `.tmp/s54-block4h-head-repair-01/candidate.json`, with raw length/SHA256 and normalized Git blob/mode for every path. Submitted for architectural review; no commit/push yet.
