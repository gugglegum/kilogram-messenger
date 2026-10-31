# C2-R producer-size attempt01: actual run

2026-10-31. **Overall failed_or_unknown; native supervisor exit1. No retry/repair.** One accepted synthetic wrapper only, allocation1338B/SHA256 ec8b72a50850d729939dff06149a7d4c6552c2d29adeab5639473836d3381b13. The worker's positive evidence is preserved; it does not override the outer failure. Root actual review is pending.

| Actual timing/identity | Value |
| --- | --- |
| Request UTC |2026-10-31T05:25:45.916Z |
| Final-return UTC |2026-10-31T05:28:06.341Z |
| Full request-to-final wall |140.425s; not summed chunk waits |
| Actual app command item |exec-a8b43f42-9556-46cf-9ea4-6578a9d77467, status=failed, exitCode=1, durationMs=138136 |
| Native session / chunks |74456 / 595c7e, 355024, 7d6131, ad8cce, 75e0fc |
| Supervisor / worker PIDs |54440 / 14096 |
| Scope |one lane/zero retries; work420/whole480; NativeToken accepted |

Public reports remain unchanged:

| Report | Bytes | SHA256 |
| --- | ---: | --- |
| [started](C-s53-cold-core-producer-size-01-attempt-01.started.json) |1112 |ab15277dd939b0a35880c5a1736523b86abb023def904d01188bc8ac8d009a74 |
| [producer-size](C-s53-cold-core-producer-size-01-attempt-01.producer-size.json) |11113 |b29022f436bbb0c8e5f48921f4c98114e65e2fc98b157e2d2aec74eb0846e9e7 |
| [watchdog](C-s53-cold-core-producer-size-01-attempt-01.watchdog.json) |2735 |c782f3034bf32f3f23f31491e1218dc581798d658fe856e90e0a698d060c6bd1 |

Supervisor reports child_return / System.InvalidOperationException / HResult -2146233079 / native0; native app/command exit1. Worker exit0, reaped=true, both EOF=true, empty stdout/stderr, no cap/force/cleanup-kill/cleanup-unknown, registered child released, no owned workers after. Supervisor public inputs9 and source-after map empty show its successful-report validation path was not completed.

**Inference for root review, not an independently instrumented fact:** this is consistent with the initial one-shot WaitForExit(0) success check before opening the worker report; finally later reports reaped=true/exit0. The pre-finally reap return was not recorded separately. No source fix or re-execution was attempted.

Worker public report says observed/assertions_passed=true, compiler_error=false/diagnostics empty; all five initialization counts1 and fresh-type gates passed. Worker observed137.8175539s; supervisor observed137.9503124s. Post-run public-source hashes for all8 sources equal worker before/after and started pins. These direct file checks do not reconstruct the missing supervisor source-after map.

| Worker measured phase | Seconds |
| --- | ---: |
|bootstrap |0.2228367 |
|job_setup |0.6245075 |
|definitions |0.1127719 |
|compilation |0.8931372 |
|fixture_oracle |0.7616427 |
|fixture_setup |122.9475081 |
|guard_handoff |0.0435956 |
|factory |0.011201 |
|output_binding |0.0284864 |
|comparison |10.3468519 |
|receipts |0.1147668 |
|core_dispose |0.0239439 |
|receipt_verification |0.7782629 |
|output_dispose |0.0009028 |
|helper_dispose |0.0019881 |
|source_after |0.0168648 |

"fixture_setup" includes creation/readback59.6625077s, baseline walk32.0312961s, prepared walk31.248906s (observed Check boundaries). 1009601 bounded aggregate probes, 7.5894112s measured probe overhead; no per-file log. Core capture complete QPC2745759109486, 130.3658668s from common start:13934 inputs+E=13935 owners,13912 notifications, zero payload hashes at that snapshot.

| Core measured phase | Seconds |
| --- | ---: |
|allocation |0.0008201 |
|output_binding |0.0156484 |
|locks |0.0103269 |
|inventory |0.1824119 |
|capture_s |1.9558415 |
|capture_bkp |1.6884119 |
|scan_before |1.0274026 |
|hash_s_before |0.6135414 |
|hash_bkp_before |0.6208536 |
|hash_s_after |0.5432308 |
|hash_bkp_after |0.5433762 |
|scan_after |0.9800952 |
|final_inputs |2.1787407 |
|receipts |0.1130427 |

Core report:6956 rows/45597399B per mirror,6 locks, full-set-before-hash=true, four passes182389596B, all five tuple digests H2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf, equal observed entry sets, input handles released, receipts complete; all native/failure/cleanup fields clear. Exact Core FS/search/receipt accounting peak13938, final0; path563128B/records18537840B/inventory3465145B/scratch524288B. These arena/slot figures are not OS RAM/handle guarantees; continuous namespace absence and whole-attempt verdict remain not_established.

R1 worker reports independent verification of all4 receipts/6956 comparison/11 namespace records,10571092B total; lengths in owner/comparison/namespace/result order3783/10559316/6025/1968B. The four independent digests match Core's final map. No post-run private/.tmp receipt access was performed.

Worker actual cleanup:41749 files closed/21 directories closed/32 enumerators disposed/41784 strict stream checks/41753 searches closed, all remaining0 and unknown/partial flags false. Output created/transferred;6 output ancestors rechecked after Core, receipts and verifier; receipt mask15/transfers4, output final0/no unknown. Helper/Core/output/self disposed, public inputs10 closed, job finished/children0. These are worker observations; overall wrapper acceptance still failed.

OS observer:1218 successful/1218 attempted samples, nominal100ms; sampled peak OS handles14510, observed process-lifetime peak working set253427712B (241.6875MiB). Maximum return gap0.1399317s, measurement-gap upper0.1482439s, sample duration max0.0217061s/total10.7092819s. Observer failed=false; exit races0, known-exit skip1. Last-return-to-observed-exit0.0750253s, upper interval0.0833685s. First sample0.5946043s after common start. Short OS handle peaks can be missed; memory high-water includes shell/compiler/setup/CLR/native overhead through observation. Exit tail remains explicitly unobserved.

Source/evidence left immutable. No extra fixture/helper/compile/native invocation, external .tmp probe, private/original/network/product/Rust/S5.4 access, source edit, ledger charge, commit or push. Only this run note is added after public-report/source-pin/native-item reads. Root alone charges actual request-to-final once and reviews the worker/outer result distinction.

## Verbatim tool evidence

Raw launch request, all received chunks and actual app command item follow. No outputs/exits were synthesized from the worker report. The app item was read from this chat's current native execution record, not inferred from a completed UI turn.

```json
{
  "request_utc": "2026-10-31T05:25:45.916Z",
  "request": {
    "cmd": "& 'C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-producer-size-01-attempt-01.watchdog.ps1' -AllocationSha256 'ec8b72a50850d729939dff06149a7d4c6552c2d29adeab5639473836d3381b13'",
    "shell": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
    "login": false,
    "workdir": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "yield_time_ms": 10000,
    "max_output_tokens": 4000,
    "sandbox_permissions": "require_escalated",
    "justification": "Один принятый архитектором локальный запуск на свежих тестовых данных, с общим лимитом 480 секунд и без повторов."
  },
  "chunks": [
    {
      "received_utc": "2026-10-31T05:25:58.215Z",
      "result": {
        "chunk_id": "595c7e",
        "wall_time_seconds": 10.011258,
        "session_id": 74456,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T05:26:07.982Z",
      "result": {
        "chunk_id": "355024",
        "wall_time_seconds": 5.0142981,
        "session_id": 74456,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T05:26:55.551Z",
      "result": {
        "chunk_id": "7d6131",
        "wall_time_seconds": 30.0081732,
        "session_id": 74456,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T05:27:31.767Z",
      "result": {
        "chunk_id": "ad8cce",
        "wall_time_seconds": 30.0027238,
        "session_id": 74456,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T05:28:06.341Z",
      "result": {
        "chunk_id": "75e0fc",
        "wall_time_seconds": 14.6698559,
        "exit_code": 1,
        "original_token_count": 71,
        "output": "{\"classification\":\"cold-core-producer-size-supervisor-failed\",\"failure\":\"child_return\",\"cause\":{\"type\":\"System.InvalidOperationException\",\"hresult\":-2146233079,\"native_error\":0,\"marks\":{\"c2r_code\":\"child_return\"}},\"cleanup_first_cause\":null,\"forced\":false,\"cleanup_unknown\":false}\r\n"
      }
    }
  ],
  "session_id": 74456,
  "final_utc": "2026-10-31T05:28:06.341Z",
  "request_to_final_seconds": 140.425,
  "native_exit_code": 1,
  "native_item": {
    "type": "commandExecution",
    "id": "exec-a8b43f42-9556-46cf-9ea4-6578a9d77467",
    "command": "\"C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe\" -NoProfile -Command \"& 'C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\\\AI-Projects\\\\kilogram-p2p-messenger\\\\ai-docs\\\\tasks\\\\long-lived-dialog-a3-migration-s5-handoff-evidence\\\\C-s53-cold-core-producer-size-01-attempt-01.watchdog.ps1' -AllocationSha256 'ec8b72a50850d729939dff06149a7d4c6552c2d29adeab5639473836d3381b13'\"",
    "cwd": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "status": "failed",
    "exitCode": 1,
    "durationMs": 138136,
    "output": {
      "text": "{\"classification\":\"cold-core-producer-size-supervisor-failed\",\"failure\":\"child_return\",\"cause\":{\"type\":\"System.InvalidOperationException\",\"hresult\":-2146233079,\"native_error\":0,\"marks\":{\"c2r_code\":\"child_return\"}},\"cleanup_first_cause\":null,\"forced\":false,\"cleanup_unknown\":false}\r\n",
      "truncated": false
    }
  },
  "native_thread_id": "01a1653c-0154-7611-8e8c-044fbe1e4d7a",
  "native_turn_id": "01a1906a-43f6-7853-a96f-017a7efcecfc"
}
```

