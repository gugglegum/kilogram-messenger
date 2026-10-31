# C2-R attempt02: actual synthetic positive

2026-10-31. **Worker0 + supervisor/native0; allocated positive predicates observed, root actual review pending.** One fixed fresh attempt02, work420/whole480/lane1/retry0, allocation1338B/SHA256 d04f5318ce16ce95e260ddcdc0f46f96268e5ad8cc25faf9cae315a04883d7e5. Whole S5.3 remains unaccepted and whole_attempt_verdict/continuous_namespace_absence remain not_established. Prior01 failed evidence is not reclassified.

| Actual native evidence | Value |
| --- | --- |
| Request UTC |2026-10-31T06:03:14.285Z |
| Final return UTC |2026-10-31T06:05:43.946Z |
| Request-to-final wall |149.661s; charge once by root, not summed polls |
| Actual app command item |exec-3b7f5bae-130a-4ed0-9ec9-e744f22b2493; status=completed; exitCode=0; durationMs=136026 |
| Session/chunks |9404 / c839ab, deb474, 3def15, 0bad04, 3523de |
| Supervisor/worker |46052 / 40272; same retained worker handle2276/UTC identity |

Both public results observed/no failure, worker assertions_passed=true. Actual app result was read from this chat's native command item; a green UI turn was not used as proof. Worker/supervisor reported135.4832085s/135.83004s, distinct from native duration136.026s and full request-to-final149.661s. Compiler_error=false/diagnostics empty, five fresh-type/once-initializer gates pass. Only accepted wrapper performed compilation/fixture/native work.

Source-format clarification per root a891be: the worker's inverse binding comparison differs from01 by one extra final LF. It is harmless but not strict whole-file byte equality. This run used the accepted pinned1847f1... worker; neither source nor the pinned repair-source-review was edited.

## Completion actually observed

| Observation | QPC | Seconds from common start |
| --- | ---: | ---: |
| Both EOF |2768298557006 |135.5782487 |
| First exit indication, basis has_exited |2768298570036 |135.5795517 |
| First real successful reap, stage normal |2768298624471 |135.5849952 |
| Normal completion gate |2768298641032 |135.5866513 |
| Pre-cleanup gate, stage completed |2768300954784 |135.8180265 |

3924 normal waits:3923 false/1 true; cleanup waits0. Trace records normal_first_wait=false with no indication/EOF/reap/code, then normal_reap_success=true/exit0 with indication+bothEOF/reaped. No false-after-indication or false-after-EOF was observed. This demonstrates the normal false-then-true path; it does not reproduce01's inferred initial false-after-exit condition. Timeout/nonzero/missing-EOF/force and observer API-exit race source traces were not executed here.

normal_gate_passed=true. Normal/pre-cleanup snapshots both show reaped/exit0/bothEOF, force=false/unknown=false/observer_failed=false; first_failure_reason and pre_cleanup_first_reason null. Reaped stayed true; no cleanup promotion/kill. Both EOF, empty worker pipes/no cap, worker process reaped/released, stdio/public inputs disposed, no owned workers after;10 public inputs in both processes, all8 supervisor source-after hashes populated and matched.

Wait request20ms is a requested bound, not a scheduling guarantee: first recorded interval35.6734ms, successful interval3.4658ms. All completion observations and checks remain well inside the same420s deadline; no reset/extension.

## Worker data, phases and cleanup

6956 rows (6954 nonempty+2 zeros)/45597399B per mirror,6 existing fixture locks. Capture complete QPC2768225548177 at128.2773658s:13934 inputs+E=13935 held owners/13912 file notifications, zero payload hashes at snapshot. Four complete passes182389596B, all five tuple digests2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf; observed entry sets equal, inputs released, receipts complete/no error. Bounded cooperative-owner equality only; no continuous namespace or post-release proof.

Core FS/search/receipt accounting peak13938/final0; paths563128B,records18537840B,inventory3465145B,scratch524288B. R1 cleanup41749 file closes/21 directory closes/32 enumerators/41784 strict checks/41753 search closes; all remaining0, no unknown/partial flags. Helper/Core/output/self disposed/job finished/children0. Output6 ancestry guards rechecked after Core/receipts/verifier, mask15/transfers4/final0/no unknown.

| Worker phase | Seconds |
| --- | ---: |
|bootstrap |0.2264769 |
|job_setup |0.5960658 |
|definitions |0.102413 |
|compilation |0.9223702 |
|fixture_oracle |0.7648964 |
|fixture_setup |120.8945368 |
|guard_handoff |0.0389945 |
|factory |0.0084349 |
|output_binding |0.0185864 |
|comparison |10.1149615 |
|receipts |0.1145675 |
|core_dispose |0.0222521 |
|receipt_verification |0.7322461 |
|output_dispose |0.0008981 |
|helper_dispose |0.0019756 |
|source_after |0.0165331 |

Observed setup subintervals: creation/readback58.2684043s, baseline walk31.3181982s, prepared walk31.3032306s.1009601 aggregate probes/7.4975381s overhead already included, not added. These are observed stage boundaries; no performance optimization or generalized speed claim.

| Core phase | Seconds |
| --- | ---: |
|allocation |0.0006901 |
|output_binding |0.0053111 |
|locks |0.0081884 |
|inventory |0.1710975 |
|capture_s |1.9747613 |
|capture_bkp |1.6490666 |
|scan_before |1.0161613 |
|hash_s_before |0.6119202 |
|hash_bkp_before |0.6113191 |
|hash_s_after |0.5708067 |
|hash_bkp_after |0.5379901 |
|scan_after |0.9641774 |
|final_inputs |1.9968603 |
|receipts |0.1125519 |

Worker R1 independently streamed4 receipts/6956 comparison/11 namespace rows; total10626796B equals charged/known-written and independent lengths/hashes. Owner/comparison/namespace/result lengths3795/10614964/6069/1968B. Four digests match Core final map. Provisional result2 owners/3 prior digests versus final0/4 distinction was validated by the accepted worker. No post-run .tmp/private receipt read was performed.

OS sampler1202 successful/1202 attempts, nominal100ms; sampled handles peak14511, process-lifetime observed PeakWorkingSet252944384B (241.2265625MiB). Max return gap0.1412619s/measurement upper0.1497056s, sample duration max0.0219956s/total10.3247192s; observer_failed=false/exit_races0/known-exit skip1. First sample0.5861945s from start. Exit-tail return gap0.1348308s/upper0.143541s explicitly unobserved. Short OS peaks can be missed; high-water includes shell/compiler/setup/CLR/native overhead through observation. Arena/slot accounting is not RAM/OS-handle peak.

## Immutable public reports and source checks

| Report | Bytes | SHA256 |
| --- | ---: | --- |
| [started](C-s53-cold-core-producer-size-01-attempt-02.started.json) |1112 |8415fceeab3a45e373794e441dfecd7e699e3608e807cfb932a7fd6e2311e620 |
| [producer-size](C-s53-cold-core-producer-size-01-attempt-02.producer-size.json) |11114 |430bcbd9d1b52c9ba3aa316499d00a14d9844584d25e3f86043b216d39c02f19 |
| [watchdog](C-s53-cold-core-producer-size-01-attempt-02.watchdog.json) |4847 |83b649f15dd59f2766f531a3900a92683983f6bf52225bf817a2b60de79643a3 |

| Post-run source | Bytes | SHA256 |
| --- | ---: | --- |
|preflight |43583 |1847f1ea879cb3e3cb06db7b7f2761bc8045d8561b918daa4f931fa61c48c2b0 |
|watchdog |47280 |9d36eb48fa86e71759098f2e4bdafaf6c34377490bf42fac15dcd89ba1363ae3 |
|core |81937 |ed56b2c3b632d81b8a775db45da93bd6657aef6ca47cd1d81f237605c89b2f43 |
|output |16100 |40710b0a2993e768a0f0b52d9949947258a3d44281f9f1aa7d10417efb7571c2 |
|root_native |25596 |a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382 |
|deadline |6038 |0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45 |
|fixture_support |56156 |adb6f2ffe7cc45d2d4eee41bf7e0d97114a7edc9a4213bf5d3af8dcf56e3c039 |
|shell |301368 |362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139 |

All8 current hashes match started pins, worker before/after and supervisor after.106 additional public-data consistency assertions passed, including identity/clock/completion snapshots, actual cleanup/receipt lengths/digests and source hashes; these are not extra native tests or private receipt access.

Only this new run note was written after public-report/source-pin/native-item reads. No source/review edit, preliminary helper/compile/fixture/probe, retry, private/original/network/Rust/product access, .tmp probe, ledger/navigation/commit/push/S5.4 action. Root controls one-time charge/release and actual acceptance. Known legacy output-wrapper failing ownership follow-up remains open before private use.

## Verbatim launch/return evidence

All five received chunks plus actual app item are preserved below; exits were not synthesized from reports.

```json
{
  "request_utc": "2026-10-31T06:03:14.285Z",
  "request": {
    "cmd": "& 'C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-producer-size-01-attempt-02.watchdog.ps1' -AllocationSha256 'd04f5318ce16ce95e260ddcdc0f46f96268e5ad8cc25faf9cae315a04883d7e5'",
    "shell": "C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe",
    "login": false,
    "workdir": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "yield_time_ms": 10000,
    "max_output_tokens": 4000,
    "sandbox_permissions": "require_escalated",
    "justification": "Один утверждённый запуск attempt02 на свежих тестовых данных, с общим лимитом 480 секунд и без повторов."
  },
  "chunks": [
    {
      "received_utc": "2026-10-31T06:03:26.935Z",
      "result": {
        "chunk_id": "c839ab",
        "wall_time_seconds": 10.0121074,
        "session_id": 9404,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T06:04:00.009Z",
      "result": {
        "chunk_id": "deb474",
        "wall_time_seconds": 30.0118715,
        "session_id": 9404,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T06:04:45.573Z",
      "result": {
        "chunk_id": "3def15",
        "wall_time_seconds": 30.0142921,
        "session_id": 9404,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T06:05:18.857Z",
      "result": {
        "chunk_id": "0bad04",
        "wall_time_seconds": 30.0079679,
        "session_id": 9404,
        "original_token_count": 0,
        "output": ""
      }
    },
    {
      "received_utc": "2026-10-31T06:05:43.946Z",
      "result": {
        "chunk_id": "3523de",
        "wall_time_seconds": 0.0000064,
        "exit_code": 0,
        "original_token_count": 19,
        "output": "ColdA producer-size positive returned; root actual-return review required.\r\n"
      }
    }
  ],
  "session_id": 9404,
  "final_utc": "2026-10-31T06:05:43.946Z",
  "request_to_final_seconds": 149.661,
  "native_exit_code": 0,
  "native_item": {
    "type": "commandExecution",
    "id": "exec-3b7f5bae-130a-4ed0-9ec9-e744f22b2493",
    "command": "\"C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe\" -NoProfile -Command \"& 'C:\\\\Users\\\\Paul\\\\.cache\\\\codex-runtimes\\\\codex-primary-runtime\\\\dependencies\\\\native\\\\powershell\\\\pwsh.exe' -NoLogo -NoProfile -NonInteractive -File 'E:\\\\AI-Projects\\\\kilogram-p2p-messenger\\\\ai-docs\\\\tasks\\\\long-lived-dialog-a3-migration-s5-handoff-evidence\\\\C-s53-cold-core-producer-size-01-attempt-02.watchdog.ps1' -AllocationSha256 'd04f5318ce16ce95e260ddcdc0f46f96268e5ad8cc25faf9cae315a04883d7e5'\"",
    "cwd": "E:\\AI-Projects\\kilogram-p2p-messenger",
    "status": "completed",
    "exitCode": 0,
    "durationMs": 136026,
    "output": {
      "text": "ColdA producer-size positive returned; root actual-return review required.\r\n",
      "truncated": false
    }
  },
  "native_thread_id": "01a1653c-0154-7611-8e8c-044fbe1e4d7a",
  "native_turn_id": "01a1908d-e5ad-78c3-9dd0-6ae49473b028"
}
```

