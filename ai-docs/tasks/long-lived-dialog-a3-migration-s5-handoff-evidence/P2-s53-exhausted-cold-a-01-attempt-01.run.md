# Cold-A attempt01: actual return — failed or unknown

2026-10-31. **One authorized invocation completed, exit1; work_expired. Raw preservation was not established.** No retry/repair was performed. Whole S5.3 remains open.

## Authority, invocation and actual timing

Read adjacent activation.md and preparation-accepted.md before invocation. Activation records the user's direct permission and root F60 reservation for this exact45/60/lane1/retry0 attempt. Active D1 **4769 B / f93374eb068eb65250b115722f74494386b0be43a34d34e53b257a5fb46efafb**; committed baseline **064021611e95d30e4196f0ed4dfa7ad1c7726902**.

Preflight chunk **dbb261**, exit0:18 public pins/accepted preparation matched, all five public report paths absent. No separate private preflight. Exactly one NativeToken command, cwd repository:

```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoLogo -NoProfile -NonInteractive -Mta -File 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s53-cold-preservation.ps1' -DecisionSha256 'f93374eb068eb65250b115722f74494386b0be43a34d34e53b257a5fb46efafb'
```

Request UTC **2026-10-31T11:40:37.495Z**; final-return UTC **2026-10-31T11:41:33.285Z**; request-to-final **55.790 s**. The tool yielded session **36986**; all subsequent calls only resumed that same execution. App `exec-ac1707ef-c2bf-43ee-96d8-023e4c936bb1`: **failed, exitCode1, durationMs45312**, complete/untruncated output. Supervisor reports **45.1212995 s**. Request-to-final, app duration, supervisor time and individual tool waits are overlapping/differently scoped measurements, not additive. Root owns charging/release; no ledger was edited.

Complete native chunks, including resume-call UTC and final output:

```json
[
  {
    "requestUtc": "2026-10-31T11:40:37.495Z",
    "returnedUtc": "2026-10-31T11:40:52.052Z",
    "result": {
      "chunk_id": "38b6b1",
      "wall_time_seconds": 10.0034931,
      "session_id": 36986,
      "original_token_count": 0,
      "output": ""
    }
  },
  {
    "requestUtc": "2026-10-31T11:40:56.269Z",
    "returnedUtc": "2026-10-31T11:41:01.280Z",
    "result": {
      "chunk_id": "e9f20f",
      "wall_time_seconds": 5.010094,
      "session_id": 36986,
      "original_token_count": 0,
      "output": ""
    }
  },
  {
    "requestUtc": "2026-10-31T11:41:05.878Z",
    "returnedUtc": "2026-10-31T11:41:10.895Z",
    "result": {
      "chunk_id": "7c8e1f",
      "wall_time_seconds": 5.0161828,
      "session_id": 36986,
      "original_token_count": 0,
      "output": ""
    }
  },
  {
    "requestUtc": "2026-10-31T11:41:15.677Z",
    "returnedUtc": "2026-10-31T11:41:25.691Z",
    "result": {
      "chunk_id": "86e776",
      "wall_time_seconds": 10.0137044,
      "session_id": 36986,
      "original_token_count": 0,
      "output": ""
    }
  },
  {
    "requestUtc": "2026-10-31T11:41:33.283Z",
    "returnedUtc": "2026-10-31T11:41:33.285Z",
    "result": {
      "chunk_id": "6babff",
      "wall_time_seconds": 0.0000063,
      "exit_code": 1,
      "original_token_count": 20,
      "output": "{\"classification\":\"s53-cold-a-supervisor-failed\",\"result\":\"failed_or_unknown\"}\r\n"
    }
  }
]
```

## Public evidence retained

Read only the five fixed public report paths; three files exist, **5835 B total**; two missing reports were not reconstructed:

| Role | Bytes | SHA256 / availability |
| --- | ---: | --- |
| started.json | 1925 | 497ad0fa30812ab0707da15608ecb271250d1e5a3f66b7802f672840737c92b1 |
| claim.json | 474 | c1eeea0afda74bb5d67b5652ffbb41601282e1f9a3750871af004efa944bda58 |
| p2.json | — | absent |
| parent.json | — | absent |
| supervisor.json | 3436 | 6c26f93d2bfa9a9e87b06a0a3ed5138f0b592f4304c7976696e64c62f724e89c |

All three existing reports parse. Start/claim/supervisor agree on attempt, D1, common start QPC **2969393940882** / frequency **10000000**, parent PID **30204**, parent UTC **2026-10-31T11:40:42.8928201Z**. Supervisor PID **13036**, UTC **2026-10-31T11:40:42.1654137Z**; retained child handle **2308**. Claim's started SHA256 equals the raw started file. Start reports permission_consumed=true/work45/whole60. **55 public consistency comparisons** passed; they are analysis of retained data, not another runtime test.

Supervisor primary cause: **System.InvalidOperationException**, HResult **-2146233079**, native_error **0**, mark `cold_launch_code=work_expired`; one-element same exception chain. Cleanup cause is null. Result **failed_or_unknown**. The pre-cleanup first reason remains work_expired; normal success was never promoted.

## Observed supervisor cleanup versus unknown private outcome

Normal waits **1422 false / 0 true**; normal gate null/passed=false. Pre-cleanup QPC **2969844235616** still showed no exit/reap/EOF and preserved work_expired. Supervisor attempted cleanup kill. Cleanup waits **3 = 2 false + 1 true**; the explicit successful20ms wait at QPC **2969844951472** observed child exit **-1**, both EOF and reap, at common age **45.101059 s**. First reap stage **cleanup**, basis **wait_signal**. This is cleanup completion, not normal45s work success.

Supervisor reports: registered child released, stdio disposed, managed public inputs disposed, public remaining0/peak20, stdout/stderr character counts0, bothEOF=true, forced=false, cleanup_kill_attempted=true, cleanup_unknown=false. **Managed native-close results were not independently observed**. These are supervisor/control-wrapper facts only.

**P2 and Parent are absent.** There is no final Core/output/guard observation, no proven private-input disposal/job-children count, no receipt-completion proof and no phase/comparison result. E/parent creation, inventory validity, capture completion, pass count/hash totals, equality, private receipts and Parent/Core cleanup remain **unknown**, not negative findings or success. No inference is made from sampled handles. Private inputs/E/receipts were not inspected after return.

Observer: **362 attempts/362 live samples**, no observer failure or exit race. Sampled handle peak **14513**, observed lifetime peak working set **230580224 B** (caps45000/1GiB/max601 samples). Maximum successful sample gap **0.1272778 s**, measurement-gap upper bound **0.1359281 s**, maximum sample duration **0.0208692 s**, cumulative sample duration **3.139924300000003 s**. Last sample QPC **2969843319859**. Exit tail remains explicitly unobserved; sampled handles are not an exact OS peak and WS is not a hard-memory guarantee.

## Source proof and limits

All18 independent post-return public source pins and the active D1 match preflight. Start source map agrees with every current source SHA. Supervisor `source_after={}` and its claim/parent/p2 hash fields are null because its normal validation was not completed; independent current pin reads do **not** replace those missing in-run proofs.

| Public role | Bytes | Post-return SHA256 |
| --- | ---: | --- |
| launcher | 71513 | 8cb43e3e6dfba31b21f014ea4f82be1451417a15424ed046d87d2d455d6f3557 |
| core | 82999 | 59787bd263052b12cc77ab524ce48b27f18e2e7a83e0039d9b40f8c3066573af |
| output | 21721 | 331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98 |
| root_native | 25596 | a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382 |
| deadline | 6038 | 0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45 |
| shell | 301368 | 362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139 |
| r42 | 20846 | 20e427de24a510c12fbd328350edb61bdc9d8c2e8752291423c37eaf6d9655c1 |
| r43 | 9037 | 03e4954c7627b9074717231ba616c3725dc7653b8382ad318c0da0c44d595bb3 |
| r44 | 7498 | eccdf671997812dd699e99fc7e600e178f208b0cd2b5340863707b7fed03ff79 |
| r41 | 7566 | 193436d3bd2ac5223ef4af74beaf18b66d32b59e50a303a52b137ff52d2c5780 |
| small | 4869 | 8693c60a808f2c054871fd3f5c05d7ca3e79e08ef3e95e842a4a16bcbf8863a2 |
| c2r | 3255 | 892b4085172b1c4a2b2cc4a4e9a6c162176f57f6417362be904781a6d21d03b2 |
| c2s | 3704 | b8a9d0532ea3f90209b0dd5bcc2d67fa270eda771460d3ac7e1f3f7763eff5ce |
| delta | 10614 | 26bd1c419b61cc35eb11612cc38648c3c2b4d530c69e571468e51b938ac2641c |
| integration_accepted | 4365 | b0611fef9a040cfe6dc1189f48c59a82dc910009789835ee7559f8cb8a4e58ab |
| reserve_amendment | 2567 | 0d4cb8e9f127454692b88d6977b45c82520274d8d1dbf804cea1639a76db842b |
| source_review | 16909 | 93178bb38eb5e031bb276261968feeb463b2be2dcb4fbf816f129ea762d719e4 |
| source_acceptance | 4655 | 85f7f1049712d8f2abeda152e4faa5c94e861813c85fdfca65f79807346329c0 |
| decision | 4769 | f93374eb068eb65250b115722f74494386b0be43a34d34e53b257a5fb46efafb |

After return, reads were restricted to fixed public reports, public source/D1 pins and actual app execution metadata. This run record is the only authored artifact. No source/report repair, private post-return query, new attempt, nav/ledger edit, commit or push.

The later standing test-account authorization clarification was received while preparing this report. It is **not a second execution dispatch** and does not permit reusing/overwriting this consumed fixed identity. No additional runtime follows this report; any future diagnosed attempt needs root's distinct reviewed identity/output and bounds. Real user data remain outside this test scope.

Outcome: **work limit reached; preservation unknown; supervisor reaped its child in cleanup; private cleanup unconfirmed.** Evidence is retained. Callback root and **STOP for actual-return review**.
