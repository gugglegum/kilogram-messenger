# Block4F path implementation — candidate for review

2026-11-03 Asia/Yekaterinburg. Baseline `177d51091efb0c571da4f8cd59db21c9b88fb784`. The amended drive-only optimization is implemented and verified. No commit/push, Cargo/build, staging of images, production Prepare/activation/runtime, F02 or P01 allocation was performed.

## Change and preserved contract

The only production edit is inside `scripts/invoke-migration-s54-whole.ps1::Get-S54Path`: after the unchanged validation/GetFullPath/missing-suffix/directory checks, select from the **canonical existing probe**. ASCII drive-letter + colon + backslash uses fresh `IO.File.GetAttributes` at each ancestor and `GetDirectoryName(TrimEndingDirectorySeparator(...))` for stepping. Every other probe retains the complete old Get-Item/Split-Path ancestor loop. There is no retry/fallback after a native lookup exception; it terminates validation.

This supersedes the unconditional-loop proposal in the retained residual-cost report. UNC and extended forms cannot use an unconditional .NET parent walk: it stops at the share where Split-Path continues. Mocked filesystem/lexical tests preserve the old operations; no network/share interoperability is claimed. A device form already rejected by the initial component policy remains rejected before any loop.

Static comparison confirms all production text outside Get-S54Path is identical to baseline. No caller, read cap, before/after read identity check, source enumeration/index head/query/final tail, schema/raw/graph/unit comparison, tool/config/origin/image hash, 4 PowerShell / 3 Rust validation boundary, T0, selector, deadline or F/P workload changed. There is no path/attribute/validity cache. The test script contains process-local test instrumentation only; production contains no mutation hook.

The two changed helpers remain outside the frozen compiler-input projection: neither is among the first three compiler-input helper scripts, and neither occurs in either genuine capture's source_inputs. Fresh candidate source pins permit these checks without recompiling. No Rust files changed.

## Focused evidence

`path-05/path-results.json`: **28 checks passed**. The retained public corpus has8442 calls; actual old/new attribute observation sequences and returned paths matched at71140 positions each. This includes ordinary drive paths from both capture schemas,948 decoded raw artifacts and517 source files. Small fresh fixtures also compare actual sequences and outcomes for root, trailing/slash/case spellings, literal brackets, optional/required missing suffixes, file used as parent, and reparse leaf/ancestor; component/length errors retain precedence.

UNC/extended/device forms use isolated filesystem mocks and real lexical parent operations. Old/new complete operation logs agree; the regression explicitly detects the different unconditional native UNC chain. Tests label mock operations and perform no UNC network access. Native missing/denied attribute exceptions and a post-read denied observation are injected fail-closed checks. Real filesystem checks move a file after existence discovery, and replace a directory with a junction between leaf/parent attribute observations; both refuse. No ACL changes/tests were performed. Localized native exception text is not treated as a stable wire error.

`index-03/compiler-index-results.json`: **53 checks passed**, retaining all previous52 plus a fixed-image substitution refusal. Fixture construction creates29 independent roots from public constants and current process descriptors, never cloning/rebinding/reading a historical fixture tree. Actual compiler positives retain23 sources,24 normal /32 test membership queries, one local index and a complete23-read final tail with the same8MiB cap. Normal/tests indexes remain distinct. The matrix retains P/F public admission, canonical duplicate/foreign-source refusals, queried/unqueried and after-last-query content/missing/junction mutations, embedded changes, malformed wire/raw/graph/unit evidence and tool/config/environment/capture/origin/source-set/selector/attempt boundaries. The new fixed-image case refuses changed pinned image bytes.

Index-01 and index-02 also passed52 checks; the second checked added canonical ownership guards. Index-03 then covered the final index branch plus the added fixed-image negative. The later path-only assertion strengthening does not change that index branch. Every actual diagnostic write used a fresh canonical owned destination, or an explicitly checked owned fixture target for a mutation. Test output/fixture roots were never reused for retries.

## Genuine uninstrumented final measurement

Final `measure-02` uses the actual production functions from invoke-migration-s54-whole.ps1, original serialized captures and retained nine images, with fresh pins for the exact final517-file candidate source set. No function wrappers or diagnostic AST copies are installed in this measurement.

| Component | Published reference | Final candidate seconds |
|---|---:|---:|
| Normal compiler predicate | part of85.5694328 pair | 9.0018963 |
| Tests compiler predicate | part of85.5694328 pair | 9.4525188 |
| Pair | 85.5694328 | 18.4544151 |
| Embedded predicate | 11.9621168 | 1.9342237 |

The pair is4.64x faster /78.43% shorter; embedded is6.18x faster in these samples. The unchanged published baseline was not rerun. The actual earlier successful measure-01 pair18.6770899s/embedded1.9373597s is retained as an **intermediate** result: afterwards the permanent tiny-fixture test was strengthened to compare exact ancestor sequences as well as outcomes. Measure-02 is the one final pair after that last source/test fix, with updated helper pins. The production function did not change between these measurements; they are not combined into a statistical benchmark.

Four sampled pair+embedded components now arithmetically total81.5546s versus390.1262s of previous component samples. Neither figure is measured runtime or a hard bound. Full inventory overhead, Rust admission and functional costs remain unknown. This is implementation/performance evidence, not F-fit, runtime acceptance or authority to allocate F02.

## Actual bounded command receipts and failures

Installed pinned pwsh, BelowNormal, process-local Rust1.98 bin/PATH and offline environment were used. Commands all take this form from the workspace:

```powershell
& C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe -NoProfile -NonInteractive -File .tmp/s54-block4f-path-implementation-01/owner.ps1 -Step path-05
# Corresponding Step values: index-03 and final measure-02; all earlier attempts are retained.
```

| Step | Actual exit | Owner elapsed seconds | Work/whole/cleanup seconds |
|---|---:|---:|---:|
| index-01 | 0 | 24.6404815 | 570/600/30 |
| index-02 | 0 | 23.8569913 | 570/600/30 |
| index-03 | 0 | 25.1041061 | 570/600/30 |
| measure-01 | 0 | 36.0950982 | 1170/1200/30 |
| measure-02 | 0 | 35.5973661 | 1170/1200/30 |
| path-01 | 1 | 1.2118049 | 570/600/30 |
| path-02 | 1 | 60.6336351 | 570/600/30 |
| path-03 | 1 | 64.9779385 | 570/600/30 |
| path-04 | 0 | 63.5943976 | 570/600/30 |
| path-05 | 0 | 57.8401637 | 570/600/30 |

All10 commands have verified child/Job membership, actual wait, stdout/stderr EOF and verified cleanup with no remaining members, no forced cleanup and no deadline expiry. Successful runs have empty stderr. Three failed path attempts are test administration defects, with original receipts/logs and fresh fixture state retained: path-01 compared indentation as semantic code; path-02 needed parentheses around concatenations in case tuples; path-03 wrongly assumed a device spelling reached the fallback rather than the unchanged initial rejection. No production correction was needed for these failures. Their snapshots preserve the relevant tested branch; path-02-test-script.ps1.txt contains that unchanged path branch with later index guards and is not represented as an exact loaded full-file image.

## Preservation and review boundary

Before/after checks reverified all517 final candidate source pins; all515 baseline source files outside the intended two helpers retained their previous pins. Both genuine captures, nine images, normal/test origin and dep-info carriers,38 failed F01 pins, HEAD/index/navigation and absence of production F02/P01 directories were checked. Native exact-image inventory found zero processes; no foreign process was stopped. Final static verification keeps HEAD177d510..., index empty, tracked dirty files only root navigation three plus the two intended scripts. Other untracked work remains untouched.

Adjacent evidence manifest contains byte-exact administrative exports (code, diffs, focused results, failed/successful receipts/logs, fresh context/pins). No captures/images or historical/new fixture trees are copied into publication files. Newly created ignored fixtures and failed diagnostics remain retained locally. The logical candidate additionally carries the previously accepted residual-cost report and its29 exports/manifest unchanged. Root-owned task/review/navigation files are excluded.

The exact candidate manifest lives at `.tmp/s54-block4f-path-implementation-01/candidate.json`; it lists raw SHA256/length and normalized Git blob/mode for every proposed path. Only that manifest's files are submitted. Commit/push and any production action await explicit review acceptance.
