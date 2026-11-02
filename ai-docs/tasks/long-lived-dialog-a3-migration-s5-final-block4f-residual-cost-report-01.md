# Block4F residual cost — measured contract for review

2026-11-03 Asia/Yekaterinburg. Read-only profiling complete on published `177d51091efb0c571da4f8cd59db21c9b88fb784`. No accepted source edits, build, staging, production allocation, application/network execution, commit or push.

The residual bottleneck is the live path primitive, especially repeated ancestor walks through PowerShell cmdlets. The next proposed change is **only the ancestor loop inside `Get-S54Path`**. Keep each live observation at its existing call site; use native .NET filesystem/path operations inside that loop. No validity or path-result cache is proposed. Pure JSON/canonicalization improvements have much less measured scope and do not justify a validator rewrite.

## Evidence and method

Published uninstrumented reference: normal/tests pair **85.5694328s**, embedded **11.9621168s**. These were not rerun unchanged. One process-local instrumented pair passed: normal **40.7228977s**, tests **45.7319806s**, pair **86.4548783s**; embedded passed in **13.8431759s**. Instrumented times are attribution, not production performance acceptance.

`work.profile-01.original.ps1`, `ledger.cs`, `instrumented-functions.ps1.txt` and `statement-map.json` retain the exact instrumentation and original statement lines/hashes. AST-derived top-level statements and raw-loop children execute their original bodies inside same-scope try/finally timing. Wrappers call the actual saved published path/read/hash bodies. A fresh tiny byte-array/hash smoke passed before measurement. Driver/preservation activity is excluded from predicate metrics.

The 128-path original/wrapped/original calibration was **0.3831700 / 0.4071577 / 0.3425566s**. This exposes overhead/cache variation, not an exact correction. Nested read depth was one. Raw-child timers are inside the raw parent; they must not be added to it. Path time inside a public read is inside read-inclusive time; it must not be added again. The unattributed remainder includes computation, instrumentation and call overhead.

## Attribution

Additive top-level groups across the pair:

| Group | Inclusive seconds |
|---|---:|
| Capture wire/schema validation | 9.4627 |
| Raw artifact loop, including decoding/schema/path checks | 49.7276 |
| Package graph setup/validation | 0.5468 |
| Unit rederivation/canonical comparison/serialization | 12.5398 |
| Compiler source index construction | 2.9015 |
| All-source final reads and pin checks | 9.5468 |
| Compiler tool/shim reads | 0.5927 |
| Selected image/origin checks | 0.9287 |
| Other top-level statements | 0.1625 |

Top timers total86.4093s; the0.0456s difference from the pair is outside those timers. The path/read/hash partition of this same total is:

| Role | Path calls | Direct path seconds | Read-inclusive seconds | Path nested within reads (already included) | SHA seconds | Other incl. overhead |
|---|---:|---:|---:|---:|---:|---:|
| Normal | 9708 | 32.9223 | 3.6282 | 3.0249 | 0.0738 | 4.0533 |
| Tests | 10687 | 37.5068 | 3.8263 | 2.8903 | 0.1279 | 4.2705 |
| Embedded | 4731 | 13.4374 | 0.1187 | 0.1085 | 0.0007 | 0.2690 |

The pair made20395 path calls. Direct path work is70.4291s; including the nested path work gives76.3443s of path attribution. Reads were522/527 calls and128900225/249040001 bytes; these are repeated bytes, not unique input sizes. The embedded predicate made18 reads/138216 bytes. Its before/after source membership loop consumed9.0421s and dependency validation4.4505s, predominantly path walks.

Within the raw loop, strict decoding itself consumed only **0.6031s** for948 messages. CargoArtifact schema validation was22.1807s, including20.7625s of path work. Repeated required manifest/src observations cost15.0640s; output uniqueness and runtime classification each perform another fresh walk. Canonical unit groups spent11.3727s in paths and only1.1671s in all remaining work/overhead. Replacing key membership, JSON parsing or recursive serialization therefore misses the dominant cost.

## Isolated candidate probe

`probe.ps1` changes just the ancestor loop in a separate function; production functions are restored after tiny composition probes. It retains the original absolute/component/1024-character checks, GetFullPath, Test-Path missing-suffix loop, directory condition, return spelling and switches. Only ancestor attributes/parent stepping change from `Get-Item ... .Attributes` / `Split-Path -Parent` to `IO.File.GetAttributes` / `IO.Path.GetDirectoryName(IO.Path.TrimEndingDirectorySeparator(...))`.

The corpus has **8442 calls**, derived from retained normal/tests wire paths,948 decoded raw artifacts and517 current source paths. Its hash is `80c9f3a2ddf0e4f2baf007d56c68199b7ee1d8adfc3f5df1104cd4f001a8dd45`. All original calls accepted; all candidate outputs matched exactly. Reference **43.5065408s**, candidate **5.4532940s** (7.98x for this corpus). The order was reference then candidate; cache/order bias is not eliminated. This is a primitive benchmark, not a second whole-pair check or full candidate admission.

For2651 distinct returned paths, the old/new calculated parent sequences matched at22059 positions. These are calculated ancestor positions, not measured kernel syscall counts. Seventeen new tiny path cases matched acceptance or exact named error: root, directory, trailing separator, slash/case aliases, literal brackets, required/optional missing paths, file used as missing ancestor, junction leaf/ancestor, relative/dot/dotdot/long/empty input, and successful query followed by directory-to-junction replacement. Both functions refused the later query, proving this candidate has no cross-call successful-result cache.

Thirteen unchanged predicate composition probes matched refusals: nested duplicate JSON keys, case-sensitive object fields, bool/integer distinctions, negative unsigned values, string/raw/JSON byte bounds, ordered arrays, null versus empty arrays, canonical number versus bool, duplicate Windows path aliases, and array count bound. These are focused probes; the previous52-case matrix was not repeated and these results are not whole admission acceptance.

## Proposed implementation contract

1. Modify only the ancestor loop in `scripts/invoke-migration-s54-whole.ps1::Get-S54Path` as the ignored candidate does. Retain the current missing-path discovery and directory test. Trim an ending separator **only for parent stepping**, preserving root and returned full spelling. Preserve the same leaf-to-root set/order of observations; no skipped ancestors, reuse or memoization. Each call freshly reads attributes for every observed ancestor and refuses any reparse point.
2. Retain every existing caller, switch, argument, branch and timing position. This includes wire paths, raw required inputs/output classification, unit canonical paths, source-index head/query/final-tail checks, image/tool/config reads and their read-time identity checks. A faster path observation does not authorize bytes. Hashes, read caps, complete source enumeration, duplicate checks and final source reads stay intact.
3. Keep current malformed policy errors and their precedence. Missing-required/directory-suffix/component/reparse errors matched in the probe. Any native attribute lookup exception must remain fatal: no swallowed exception, accepted fallback, cached attribute or missing-ancestor bypass. Exact OS/provider exception text for denied access/disappearance was not established; focused implementation review must explicitly assess this difference before acceptance.
4. Preserve4PowerShell inventory validations and3Rust admission validations, all authority/process boundaries, T0 placement, controller/work/outer deadlines, F/P workloads and selectors. No .NET optimization in Rust is proposed, no F02 allocation is authorized by this report.

Focused implementation regressions should cover root/drive and supported UNC paths, optional suffixes under files/directories, slash/case/trailing spellings, literal wildcard characters, missing/denied attributes and nonreparse/reparse attributes; compare old/new observation sequences. Add deterministic removal and junction replacement at head/query/tail and during ancestor/read boundaries, ensuring a native lookup failure cannot become acceptance. Retain the existing queried/unqueried source mutations, tool/config/origin/image substitutions and malformed wire/raw/graph/unit refusal coverage. Run accepted focused checks through the Windows stable harness as applicable, plus one bounded genuine candidate pair/embedded measurement if separately dispatched; do not reopen prior publication review without a new change.

## Budget implication and limits

The primitive ratio is0.12534. Applying it mechanically to this instrumented sample's path attribution gives approximately **19.68s for a pair** and **2.00s embedded**, versus86.45s/13.84s instrumented. This is sensitivity arithmetic only: the corpus weights differ, filesystem/cache state and instrumentation vary, and no full candidate validator was executed. It demonstrates a meaningful target, not an expected-runtime promise. Four sampled pair+embedded components would arithmetically shrink from about390.13s (published reference) to roughly87s; that is not a runtime bound or a measured aggregate. Rust admission, remaining full inventory work and functional cost remain unknown. Existing F/P budgets have not been proved sufficient.

## Commands, preservation and delivery

All commands used the pinned installed pwsh, process-local Rust1.98 bin and BelowNormal, offline Cargo environment; no Cargo/build was invoked. From the workspace:

```powershell
& C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe -NoProfile -NonInteractive -File .tmp/s54-block4f-residual-cost-01/owner.ps1 -Step profile-01
# Subsequent bounded microprobe and final pin seal: same owner, -Step probe-01 / seal-01
```

| Receipt | Actual child exit | Owner elapsed | Work/whole/cleanup seconds |
|---|---:|---:|---:|
| profile-01 | 0 | 131.1865940 | 1170/1200/30 |
| probe-01 | 0 | 83.7405406 | 570/600/30 |
| seal-01 | 0 | 22.8808372 | 570/600/30 |

All three receipts verify actual child/Job membership, wait and stdout/stderr EOF. Stderr was empty; no timeout or forced cleanup; cleanup was verified with no remaining owned members. Final native process inventory found zero exact retained image processes. Final seal reverified the original profile's517 source pins, both capture pins,9image pins, retained origins/dep-info/context, root navigation and failedF01 pins. HEAD is177d510..., index empty; tracked changes remain only the root-owned navigation three. All other existing untracked work was left alone. Production F02/P01 directories remain absent. No historical fixture was cloned, rebound or written.

The adjacent `long-lived-dialog-a3-migration-s5-final-block4f-residual-cost-evidence-01/manifest.json` lists byte-exact exports, original/export paths, lengths and SHA256. It includes aggregate metrics, maps/generated functions, profiler/probe/owner code, receipts/logs and before/after verified pin carriers. No large captures/images or fixture trees were copied. Only this report and that fresh evidence directory are executor deliverables. Implementation awaits architectural review.
