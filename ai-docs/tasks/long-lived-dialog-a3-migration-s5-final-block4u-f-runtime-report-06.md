# Block4U: sole F06 runtime failed; evidence preserved

2026-11-03. The one externally activated published FeasibilityF Run on HEAD `907d30e7f828cf12fbb6b0f91f2dd441efe4e69e` returned actual child/owner/tool exit **1/1/1** after **176.0358224 seconds** of supervised execution. Public runtime failure is **`harness_nonzero`**: native harness PID27812 exited101; launcher PID28644 exited124. The selected test reports one failure,0 passed,106.70s. No assertions or success markers were produced. Exact functional cause is **unknown from retained public evidence**; no arbitrary error/private command-log or profile contents were read or exported. The attempt is consumed and was not retried.

## Original bindings and invocation

Purpose `FeasibilityF`; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-06`; selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`; run ID `7d620f3dc567c342782db79db51c41335912e2205052080629871ef8762f6218`.

- Inventory `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-06\inputs.json`: 117304B, SHA256 `37268d5168da78a3b1a9d7f7a7b627762384b282e1235391aa936c8bfcc76ac7`.
- Root activation `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-06\activation.json`: 1003B, SHA256 `e3ada0e6a2bfd78ccd2d7c0a8a8937e4236b3d3856f9249a086630435cddf0c4`.
- Root activation validity `[1793674829852,1793682029852)` Unix ms,2026-11-03T03:00:29.852Z through05:00:29.852Z. It permits one dispatch and does not extend F runtime deadlines.
- Preparation review and root activation receipt remain in `.tmp/s54-block4t-f-preparation-06/root-review.json` and `root-activation-01.json`; exact pins are in the adjacent manifest.

Actual child command, launched once:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s54-whole.ps1' '-Action' 'Run' '-Purpose' 'FeasibilityF' '-InventorySha256' '37268d5168da78a3b1a9d7f7a7b627762384b282e1235391aa936c8bfcc76ac7' '-ActivationSha256' 'e3ada0e6a2bfd78ccd2d7c0a8a8937e4236b3d3856f9249a086630435cddf0c4'
```

Fresh administration `.tmp/s54-block4u-f-runtime-06` adapts reviewed Block4Q owner/preflight/postcheck with current F06 bindings,1471 history pins and F05 shallow-name preservation. The original Q files are unchanged. Administration additionally requires exact19 environment rows, records standalone-ok cardinality and checks EOF against original outer deadline. These are administrative evidence checks; published runtime source and clock policy are unchanged. A shell-quoting error while preparing the fresh postcheck made no file change; a file-based helper completed that reporting adjustment, without another Run.

Owner invocations use fresh pinned installed pwsh `-NoProfile -NonInteractive -File .tmp/s54-block4u-f-runtime-06/owner.ps1 -Step <step>`. Original `*.invocation.json`, receipts and stdout/stderr retain exact child argv and native evidence.

| Step | Actual tool returns | Child/owner exit | Supervised elapsed seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `b3d45e → 1013c9` | 0/0 | 14.8451514 | 270/300/30 |
| run-01 | `d6607c → e86f27` | 1/1 | 176.0358224 | 570/600/30 |
| postcheck-01 | `94c9ed → 7da30e` | 0/0 | 13.4751536 | 570/600/30 |

All three have actual verified handle/Job membership, waited child, bounded stdout/stderr EOF, no reader failure, no supervisor work expiry, nonforced verified supervisor cleanup and after[]. Actual Run owner PID65772, child PID5064, child started `2026-11-03T03:02:58.2427286Z`. The run supervisor's570/600/30 containment does not alter F450/480/510. Original command return and EOF observations were recorded before cleanup/pin reads/supplementary diagnosis.

## Preflight and original clock

Cheap fresh native preflight passed in 14.8451514s. It checked root input pins and identity, actual HEAD/shared resolver, empty index, run entries exactly inputs.json+activation.json, WholeP absence, complete521 source/9 image pins, installed tool resolutions, successful native exact-image process inventory with0 conflicts,1471 immutable history pins and sufficient activation validity (7080013ms remaining at sample). No extra complete Fresh validation, compiler replay, rebuild, list or closed matrix was run outside production admission.

All19 inventory-bound environment rows matched in the fresh child without printing values. Process-local regular Rust1.98 bin was prepended once to original PATH; CARGO_NET_OFFLINE=true; exact RUST_LOG inherited; other16 bound variables absent; TOKIO_WORKER_THREADS absent and actual available/effective24. NativeToken/x64/BelowNormal/Jobs2/offline/locked/skip-cache descriptors were unchanged. Original parent PATH SHA256 `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`; configured parent `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`; actual fresh child `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. No persistent settings were changed.

| Clock event | Windows tick ms | Elapsed from original T0 ms |
| --- | --- | --- |
| original owned launch T0 | 525071156 | 0 |
| controller cutoff | 525521156 | 450000 |
| work cutoff | 525551156 | 480000 |
| outer cutoff | 525581156 | 510000 |
| outer checks observation | 525246640 | 175484 |
| actual child exit observed | 525246750 | 175594 |
| stdout/stderr EOF observed | 525246750 | 175594 |

T0 UTC Unix ms `1793674978659`; exit observation `2026-11-03T03:05:54.2378635+00:00`, EOF `2026-11-03T03:05:54.2402062+00:00`. Failure occurred before controller/work/outer deadlines; no timeout extension or clock reset is inferred. Durable outer record keeps `actual_desktop_return=pending` because it precedes that return. Separately observed actual command exit1 and EOF are authoritative return evidence; the durable field was not rewritten.

## Native/public evidence and limitations

Outer result: 3758B, SHA256 `b2f76cc94867ed34c6367a93d98faa7cdc7ce029c3bd166afac64d3c63450a18`. It records `outer_checks=failed`, `work_expired=false`, `failure=harness_nonzero`, `whole_attempt_verdict=not_established`.

All five binding checks passed: outer claim, launch/capture, native claim, original clock request/reply and one join/permit pair. Native claim binds original owner5064, launcher28644, harness27812, nonce/run/attempt/inventory/launch. One native process membership join and its verified permit are retained. This establishes the recorded rendezvous; it does not establish successful functional completion or full final-tail validation.

Original filtered protocol files are pinned without exporting arbitrary native errors:

- `native-stdout.protocol.log`: 215B, SHA256 `95d3dcf31f0f86b9acff4dda063f8a12b39d6c4ccdf531b923aace8a4cb7bec5`.
- `native-stderr.protocol.log`: 0B, SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

| Stream | Stored protocol bytes | Captured prefix bytes | Captured prefix SHA256 | Redacted lines | Partial tail bytes | Admission diagnostics count/dropped |
| --- | --- | --- | --- | --- | --- | --- |
| stdout | 215 | 326 | `c1573d547bb5e80d91fa0fca1ae3bbf0188d66a231cd647015daf2d02f115d18` | 8 | 0 | 0/0 |
| stderr | 0 | 46 | `0246275d40455e5c97c056fbc65955876fc37ba505cd6d2fe21075d230515a28` | 1 | 0 | 0/0 |

Both stream snapshots report source_eof/reader_stopped/complete true; no source overflow/failure, truncation, invalid UTF8, malformed protocol or partial tails. However combined transcript `complete=false` and `native_capture_status=not_observed`: no S54_CAPTURE separator was observed. Stream completeness alone therefore does not satisfy runtime success. `S54_ADMISSION_DIAGNOSTIC` count is0 on both streams; no stage/reason advisory was saved and no admission cause is inferred. Such diagnostics, when present, are advisory with authority none.

| Exact retained protocol cardinality | Count |
| --- | --- |
| `named_invocations` | 1 |
| `native_functional` | 0 |
| `native_capture` | 0 |
| `standalone_ok` | 0 |
| `failed_summary` | 1 |
| `running_one` | 1 |
| `success_summary` | 0 |
| `wrapper_completed` | 0 |
| `named_failed` | 1 |
| `named_ok` | 0 |

`assertions.json` and memory-observer-final.json are absent; memory-observer-abort.json is retained as a public artifact. Assertion envelope and stored success layout are false/absent. No functional assertion, wrapper completion, successful native summary or successful final-tail result may be claimed from this run. Read-only postcheck verifies unchanged source/images/inputs/navigation separately; this does not substitute for missing in-clock production final-tail completion.

Runtime cleanup was **forced**: owned before `[28644,66488]`, after `[]`, verified true. Launcher28644 is known from launch; PID66488's executable was not captured and is not inferred. Native harness27812 exited101 and was waited; launcher28644 was waited with124. Forced runtime cleanup is failure, regardless of successful containment. Supervisor cleanup was nonforced and its after[] verified. Successful postcheck native process enumeration found0 remaining exact-image processes; no foreign process was stopped.

## Preservation and review boundary

Postcheck took 13.4751536s and returned0; it only diagnoses the failed Run. HEAD/source521/images9/inventory/activation/navigation and empty index are preserved. F01/F02 remain empty, F03/F04 retain original8 files, F05 retains its approved89 shallow top-level names and all1471 immutable history pins remain unchanged. No F05 raw-log reanalysis or historical private fixture access occurred. Preparation originals and root activation evidence remain pinned.

F06 retains 14 allowlisted public artifacts and 397 shallow top-level names, including 382 command-log files (191 pairs) and the owned synthetic f-origin directory. Command-log/profile/key/database contents were not traversed or exported; artifacts remain in place for a separately scoped review. Manifest references public originals, source/images/compiler inputs, historical pins and preserved preparation evidence; it does not copy captures or binaries. Finalization checks all current pins and outside-report status preservation.

No retries, new activation, state deletion, output rewriting, source edits, rebuild, WholeP allocation, commit or push occurred. No product repair or next-stage work started. Return this failure and its evidence to root for review. F feasibility is not accepted; whole S5.4 remains unaccepted.
