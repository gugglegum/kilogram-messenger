# Timeout controls attempt01: actual return

2026-10-31. **One accepted C20/30 invocation returned exit0; three public dummy controls and five exact data refusals passed.** Submitted for root actual-return review; no private Cold-A preservation result or whole-S5.3 acceptance.

## Invocation and timing

Read adjacent source-accepted.md. Public preflight **ab8d31**, exit0, verified18 source pins plus preflight/review and absence of17 fixed outputs. An earlier public-only wrapper whitelist-count expression failed (ce5890) and was corrected before invocation; no project source/control was executed or artifact created by that preparation error.

Exactly one NativeToken invocation, cwd repository:

```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoLogo -NoProfile -NonInteractive -Mta -File 'E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\C-s53-cold-timeout-controls-01-attempt-01.preflight.ps1'
```

Request UTC **2026-10-31T12:42:31.433Z**, final-return UTC **2026-10-31T12:42:50.480Z**, request-to-final **19.047 s**. Session **2478** was resumed only as the same execution. App `exec-0ff5dfe3-aacb-4c9b-aca6-52d3bd958dcf`: completed, exitCode0, durationMs **10826**, output untruncated. Control reports **10.2919406 s**. These overlapping measurements and individual tool waits must not be added together. Root owns charging/release of C30; no ledger edit.

Complete native chunks:

```json
[
  {
    "requestUtc": "2026-10-31T12:42:31.433Z",
    "returnedUtc": "2026-10-31T12:42:44.965Z",
    "result": {
      "chunk_id": "d1c451",
      "wall_time_seconds": 10.0147418,
      "session_id": 2478,
      "original_token_count": 0,
      "output": ""
    }
  },
  {
    "requestUtc": "2026-10-31T12:42:50.478Z",
    "returnedUtc": "2026-10-31T12:42:50.480Z",
    "result": {
      "chunk_id": "597062",
      "wall_time_seconds": 0.0000063,
      "exit_code": 0,
      "original_token_count": 18,
      "output": "Three fixed timeout controls returned; root actual review required.\r\n"
    }
  }
]
```

## Actual controls

Supervisor fixture PID **54944**, UTC **2026-10-31T12:42:35.0377447Z**. Child identity/UTC is linked through each raw started/claim record; context clock/source pins agree. Fixture D1 value is64 hex f characters, not an active private decision. Started records'45/60/permission_consumed fields are schema fixtures; actual allocation20/30 and mapped clocks below apply.

| Case / actual child | Normal waits false/true | Cleanup waits false/true | Reap age s | Exit / main kill | Observed classification |
| --- | --- | --- | ---: | --- | --- |
| normal /57204 |27/1|0/0|1.0839389|0 /false|normal gate passed |
| grace /31708 |64/0|36/1|3.1664021|0 /false|work_expired stays failed; diagnostics bound |
| forced /31904 |64/0|98/1|5.0263623|-1 /true|work_expired stays failed; diagnostics unknown |

All reaps use an actual successful retained Process.WaitForExit signal; neither HasExited nor EOF is substituted. Requested waits are<=20ms, not hard real-time duration promises.

Normal: reap QPC **3006544388856**, normal gate QPC **3006544454497**, bothEOF QPC **3006544448132**, before mapped work2. Trace at the signal still had EOF=false; the subsequent normal gate explicitly records bothEOF=true.

Grace: start **3006544777935**, cleanup first wait after work expiry, successful cleanup reap **3006576441956**. No kill. Late exit0 does not pass the normal gate; first/pre-cleanup reason remains work_expired. Raw grace Parent→P2 SHA equals **3ff772a7de2489e6e0766881e9092fcde41d99fd1082c42577530079d4fc7c77**; parent PID/UTC, common QPC/frequency/D1, claim→started and source map bind. P2 QPC **3006575152967**, Parent elapsed **3.0826628**, within mapped Parent4. **Core=null, cleanup_known=false**: valid diagnostics do not establish Core cleanup or preservation.

Forced: start **3006578200265**, successful cleanup reap **3006628463888**, after grace5 and before reap6. Main kill_attempted=true, exit-1. P2/Parent are absent. Secondary diagnostic cause **System.IO.FileNotFoundException**, HResult **-2147024894**, native_error0, empty marks; chain MethodInvocationException(-2146233087)→FileNotFoundException(-2147024894). This expected missing-evidence condition does not replace primary work_expired.

For grace/forced, trace at successful reap still shows EOF=false. Final bothEOF, managed reader closure, zero public remaining and known control cleanup were **enforced by executed source assertions before appending each case**, as attested by native exit0. Their final EOF booleans/QPC were not separately retained in cases.json; no timestamps are reconstructed. No native CloseHandle-return proof is inferred.

Five in-memory mutated records were refused by actual validators:

| Mutation | Enforced cold_launch_code |
| --- | --- |
| parent PID | schema_value |
| P2 QPC | diagnostic_clock |
| P2 SHA binding | schema_value |
| source SHA | schema_value |
| extra field | schema_fields |

Each executed assertion requires InvalidOperationException/HResult **-2146233079** and exact code. cases.json retains **data_refusals=5**, not individual exception payloads. No extra native scenario was used.

## Mappings, closure and raw artifacts

Actual **43 definitions +7 AST fragments**,50 unique mapping entries; every source_sha256 equals inverse_sha256. Fixed maps: work45→2, Parent diagnostic50→4, grace51→5, reap52→6, drain54→7; root guardian child20/self29/outer30. Normal full production Check-ChildReports is outside this focused run; its unchanged source was reviewed separately.

Public peaks **1/3/2**, remaining **0/0/0**. Forced peak2 includes the reserved failing P2 acquisition. Assertions check each registered managed handle Closed/IsClosed. Core/owners/production entrypoint/private data/native compilation were not invoked. Resource sampler identity/range checks ran; exact OS peak metrics are not retained by this summary and are not claimed.

Thirteen fixed public artifacts exist, **62991 B total**, each<=65536B; four whitelisted files are absent as designed. All existing JSON parses. Exact raw SHA256:

| Artifact suffix | Bytes | SHA256 / availability |
| --- | ---: | --- |
| child.ps1 | 27746 | 8e0a6ee95f10155ceb5fc82508c16e0969284c699ae642e4fe1570dfebb23e2f |
| cases.json | 19525 | b6f77d874c0a5532baf67cc3367aadfe80415ce5248506d2472add5704ce9b9d |
| normal.context.json | 1530 | 410f3f4606961e12fb6614e5a953b01e300caaf3bc99d6c018ba9048cc81539d |
| normal.started.json | 1936 | 1765c681b66c4d753c28d4c68501e0d0b05c04493489389a944db531c5adf07e |
| normal.claim.json | 485 | d01fcede2f1ad046fc606405fd50d0683c9d3534580e1c02e1f2a1a15ad5023a |
| normal.p2.json | — | absent |
| normal.parent.json | — | absent |
| grace.context.json | 1530 | a2e9c313408ba221979fa0cd91caaeb8df45cffc8c6582308dafa06d6dbdbfc0 |
| grace.started.json | 1935 | c823617d6d5ace9e47dfcb6d574e3f9d18fabe63308ed46423c8d1f2d958cf9e |
| grace.claim.json | 484 | 0adf450e50594eea80a49991121ae002d20eec4a334e703ce153598f93fdb99d |
| grace.p2.json | 2855 | 3ff772a7de2489e6e0766881e9092fcde41d99fd1082c42577530079d4fc7c77 |
| grace.parent.json | 1014 | c70f8548f1f1d7ed2e85c8e94a53fc7e288f05e5f75eff41012a6a3019fc779b |
| forced.context.json | 1530 | 266c7163a960f16242d7284c96117affad9493c157c267cf140b5ca9514a6013 |
| forced.started.json | 1936 | d1f30b4a60912d9fe7a0bbb315b0c7369d4176ba021cdd2c2b055fb61d20c441 |
| forced.claim.json | 485 | 210ddb857bcd8e17183164a08bb3921c6678f8fdb2ba83d3fc80e362c47c8435 |
| forced.p2.json | — | absent |
| forced.parent.json | — | absent |

Generated child27746B/SHA8e0a6ee95f10155ceb5fc82508c16e0969284c699ae642e4fe1570dfebb23e2f matches the statically reviewed assembly. All artifacts retained without rewrite. **165 independent public-data consistency comparisons** passed after return; no project functions were invoked by that analysis.

## Current public source proof / stop

All18 public source pins plus accepted preflight/review agree before/after, including cases.source_sha256:

| Role | Bytes | Post-return SHA256 |
| --- | ---: | --- |
| launcher | 76869 | 620753ecddd1ff1b1d07fa5d3868c8fdfa7170d7770f407165e112ade8551fad |
| core | 82999 | eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f |
| output | 21721 | ced08d88656e7bad142d1e23a826c42e7be4f75da8380c838167610504db7ae0 |
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
| preflight | 26131 | 3dbdc5c973fa287096ff794473af0ca057c3ec71e7074a5d70f98b8a00ecebac |
| source_review_current | 11477 | 4aa5959d69151bb9e0c74ca03ae96c07667d41cfc1c02f819511b825bec0f706 |

After return, only fixed generated public artifacts, public source pins and actual app metadata were read. This run record is the only authored artifact. No retry, source/report repair, private input/E/receipt access, attempt02 D1/E activation, allocation/nav/ledger edits, commit or push.

The run supports these focused supervision/diagnostic behaviors; it does not identify the original45s private overrun phase, execute real Core/Parent production flow, test unchanged job/guardian failure scenarios anew, or prove private equality/cleanup. Root actual-return review is the next gate. **STOP.**
