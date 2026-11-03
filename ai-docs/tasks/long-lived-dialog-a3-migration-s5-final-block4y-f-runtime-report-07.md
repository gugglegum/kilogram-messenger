# Block4Y: sole F07 runtime failed; evidence preserved

2026-11-03. The one externally activated published FeasibilityF Run on HEAD `e463ee55f0a113d775498aaa6c5c4179334d0f12` returned actual child/owner/tool exit **1/1/1** after **167.8447363 seconds** of supervised execution. Public runtime failure is **`harness_nonzero`**: native harness PID61088 exited101; launcher PID64468 exited124. The selected test reports one failure,0 passed,100.78s. No assertions or success markers were produced. Exact functional cause is **unknown from retained public evidence**; no arbitrary error/private command-log or profile contents were read or exported. The attempt is consumed and was not retried.

## Original bindings and invocation

Purpose `FeasibilityF`; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-07`; selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`; run ID `18a6df4fae213ef1e0efdd1e3d667cd4db4b87a22f93aee18f5a20bc373d278e`.

- Inventory `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-07\inputs.json`: 117304B, SHA256 `988f17cadc0b5ae630f4c04c89a0e91ee8e1cfd44531aa9910b133fc8b2984e2`.
- Root activation `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-07\activation.json`: 1003B, SHA256 `e8d7c91a3c1dffd1fafd1b987f1a60dbdc1c9e8c96203b1e1c3895b59c8b5fa7`.
- Root activation validity `[1793679040083,1793686240083)` Unix ms,2026-11-03T04:10:40.083Z through06:10:40.083Z. It permits one dispatch and does not extend F runtime deadlines.
- Preparation review and root activation receipt remain in `.tmp/s54-block4x-f-preparation-07/root-review.json` and `root-activation-01.json`; exact pins are in the adjacent manifest.

Actual child command, launched once:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s54-whole.ps1' '-Action' 'Run' '-Purpose' 'FeasibilityF' '-InventorySha256' '988f17cadc0b5ae630f4c04c89a0e91ee8e1cfd44531aa9910b133fc8b2984e2' '-ActivationSha256' 'e8d7c91a3c1dffd1fafd1b987f1a60dbdc1c9e8c96203b1e1c3895b59c8b5fa7'
```

Fresh administration `.tmp/s54-block4y-f-runtime-07` adapts reviewed Block4U owner/preflight/postcheck with current F07 bindings,1663 history pins and F05 shallow-name preservation. The original U files are unchanged. Administration additionally requires exact19 environment rows, records standalone-ok cardinality and checks EOF against original outer deadline. These are administrative evidence checks; published runtime source and clock policy are unchanged. No administrative check failed or required retry in Y.

Owner invocations use fresh pinned installed pwsh `-NoProfile -NonInteractive -File .tmp/s54-block4y-f-runtime-07/owner.ps1 -Step <step>`. Original `*.invocation.json`, receipts and stdout/stderr retain exact child argv and native evidence.

| Step | Actual tool returns | Child/owner exit | Supervised elapsed seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `c68ce8 → 56736c` | 0/0 | 15.5655329 | 270/300/30 |
| run-01 | `a5fbc4 → da5c48` | 1/1 | 167.8447363 | 570/600/30 |
| postcheck-01 | `a8735a → 1064cc` | 0/0 | 12.7449765 | 570/600/30 |

All three have actual verified handle/Job membership, waited child, bounded stdout/stderr EOF, no reader failure, no supervisor work expiry, nonforced verified supervisor cleanup and after[]. Actual Run owner PID59972, child PID19960, child started `2026-11-03T04:12:35.8645548Z`. The run supervisor's570/600/30 containment does not alter F450/480/510. Original command return and EOF observations were recorded before cleanup/pin reads/supplementary diagnosis.

## Preflight and original clock

Cheap fresh native preflight passed in 15.5655329s. It checked root input pins and identity, actual HEAD/shared resolver, empty index, run entries exactly inputs.json+activation.json, WholeP absence, complete521 source/9 image pins, installed tool resolutions, successful native exact-image process inventory with0 conflicts,1663 immutable history pins and sufficient activation validity (7111703ms remaining at sample). No extra complete Fresh validation, compiler replay, rebuild, list or closed matrix was run outside production admission.

All19 inventory-bound environment rows matched in the fresh child without printing values. Process-local regular Rust1.98 bin was prepended once to original PATH; CARGO_NET_OFFLINE=true; exact RUST_LOG inherited; other16 bound variables absent; TOKIO_WORKER_THREADS absent and actual available/effective24. NativeToken/x64/BelowNormal/Jobs2/offline/locked/skip-cache descriptors were unchanged. Original parent PATH SHA256 `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`; configured parent `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`; actual fresh child `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. No persistent settings were changed.

| Clock event | Windows tick ms | Elapsed from original T0 ms |
| --- | --- | --- |
| original owned launch T0 | 529248781 | 0 |
| controller cutoff | 529698781 | 450000 |
| work cutoff | 529728781 | 480000 |
| outer cutoff | 529758781 | 510000 |
| outer checks observation | 529416062 | 167281 |
| actual child exit observed | 529416171 | 167390 |
| stdout/stderr EOF observed | 529416140 | 167359 |

T0 UTC Unix ms `1793679156272`; exit observation `2026-11-03T04:15:23.6655455+00:00`, EOF `2026-11-03T04:15:23.6351085+00:00`. Failure occurred before controller/work/outer deadlines; no timeout extension or clock reset is inferred. Durable outer record keeps `actual_desktop_return=pending` because it precedes that return. Separately observed actual command exit1 and EOF are authoritative return evidence; the durable field was not rewritten.

## Native/public evidence and limitations

Outer result: 3758B, SHA256 `e4114343bc50986b0cf6c46ce0db213e4eee7212b2177ac09dea649554c7590b`. It records `outer_checks=failed`, `work_expired=false`, `failure=harness_nonzero`, `whole_attempt_verdict=not_established`.

All five binding checks passed: outer claim, launch/capture, native claim, original clock request/reply and one join/permit pair. Native claim binds original owner19960, launcher64468, harness61088, nonce/run/attempt/inventory/launch. One native process membership join and its verified permit are retained. This establishes the recorded rendezvous; it does not establish successful functional completion or full final-tail validation.

Original filtered protocol files are pinned without exporting arbitrary native errors:

- `native-stdout.protocol.log`: 215B, SHA256 `dc551b51da6b3f8f40d0afa2702beac629fe7dc3566d14cd3a14ad29ae896732`.
- `native-stderr.protocol.log`: 0B, SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

| Stream | Stored protocol bytes | Captured prefix bytes | Captured prefix SHA256 | Redacted lines | Partial tail bytes | Admission diagnostics count/dropped |
| --- | --- | --- | --- | --- | --- | --- |
| stdout | 215 | 326 | `a8af55611ff55ac66984095e57882ab6c5d82c03f919e4b5bd3838d38356363d` | 8 | 0 | 0/0 |
| stderr | 0 | 42 | `5ddfe126df495cab8c7bb43447765fa46168768c7863e67ec992a7fa02b004ae` | 1 | 0 | 0/0 |

Both stream snapshots report source_eof/reader_stopped/complete true; no source overflow/failure, truncation, invalid UTF8, malformed protocol or partial tails. However combined transcript `complete=false` and `native_capture_status=not_observed`: no S54_CAPTURE separator was observed. Stream completeness alone therefore does not satisfy runtime success. `S54_ADMISSION_DIAGNOSTIC` count is0 on both streams; no stage/reason advisory was saved and no admission cause is inferred. Such diagnostics, when present, are advisory with authority none.

| Exact retained protocol cardinality | Count |
| --- | --- |
| `failed_summary` | 1 |
| `standalone_ok` | 0 |
| `success_summary` | 0 |
| `named_invocations` | 1 |
| `running_one` | 1 |
| `native_capture` | 0 |
| `wrapper_completed` | 0 |
| `named_failed` | 1 |
| `native_functional` | 0 |
| `named_ok` | 0 |

`assertions.json` and memory-observer-final.json are absent; memory-observer-abort.json is retained as a public artifact. Assertion envelope and stored success layout are false/absent. No functional assertion, wrapper completion, successful native summary or successful final-tail result may be claimed from this run. Read-only postcheck verifies unchanged source/images/inputs/navigation separately; this does not substitute for missing in-clock production final-tail completion.

Runtime cleanup was **forced**: owned before `[64468,66748]`, after `[]`, verified true. Launcher64468 is known from launch; PID66748's executable was not captured and is not inferred. Native harness61088 exited101 and was waited; launcher64468 was waited with124. Forced runtime cleanup is failure, regardless of successful containment. Supervisor cleanup was nonforced and its after[] verified. Successful postcheck native process enumeration found0 remaining exact-image processes; no foreign process was stopped.

## Preservation and review boundary

Postcheck took 12.7449765s and returned0; it only diagnoses the failed Run. HEAD/source521/images9/inventory/activation/navigation and empty index are preserved. F01/F02 remain empty, F03/F04 retain original8 files, F05/F06 retain their approved89/397 shallow top-level names and all1663 immutable history pins remain unchanged. No F05/F06 raw-log reanalysis or historical private fixture access occurred. Preparation originals and root activation evidence remain pinned.

F07 retains 14 allowlisted public artifacts and 337 shallow top-level names, including 322 command-log files (161 pairs) and the owned synthetic f-origin directory. Command-log/profile/key/database contents were not traversed or exported; artifacts remain in place for a separately scoped review. Manifest references public originals, source/images/compiler inputs, historical pins and preserved preparation evidence; it does not copy captures or binaries. Finalization checks all current pins and outside-report status preservation.

No retries, new activation, state deletion, output rewriting, source edits, rebuild, WholeP allocation, commit or push occurred. No product repair or next-stage work started. Return this failure and its evidence to root for review. F feasibility is not accepted; whole S5.4 remains unaccepted.
