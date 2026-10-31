# Cold-A attempt02: actual raw preservation observed

2026-10-31. **One authorized invocation returned exit0. P2 reports raw_preservation_observed; Parent/Supervisor completed.** Submitted for root actual-return review. Property is bounded payload equality under cooperative owner exclusion; whole S5.3 remains open.

## Authority, command and actual timing

Read adjacent activation.md: standing test-data permission, one F45/60 attempt/lane1/retry0. Committed baseline **9df550e45839880f861680b24921d62fb950b402**. Active D1 **4769B / 3dc9c86545b83cfbfc24ac14160a30f01a2c4a8da8044a57e763ad2ee9ae47fd**. Public preflight **171572**, exit0, verified18 pins/preparation and five absent public outputs; no separate private probe.

Exactly one NativeToken command, cwd repository:

```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoLogo -NoProfile -NonInteractive -Mta -File 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s53-cold-preservation.ps1' -DecisionSha256 '3dc9c86545b83cfbfc24ac14160a30f01a2c4a8da8044a57e763ad2ee9ae47fd'
```

Request UTC **2026-10-31T12:59:47.181Z**; final-return UTC **2026-10-31T13:00:06.760Z**; request-to-final **19.579s**. Initial native chunk6f3a54 yielded session **98926**; only continuation3e1f9a resumed it. App `exec-a640465a-4dd1-47b7-a023-dbfc655c1f09`: completed/exitCode0/durationMs **17059**, untruncated. Parent **16.6082251s**, Supervisor **16.8363013s**; these overlap app/request/tool-wait scopes and are not additive. Root owns charging/release of F60; no ledger change.

Complete native chunks:

```json
[
  {
    "requestUtc": "2026-10-31T12:59:47.181Z",
    "returnedUtc": "2026-10-31T12:59:59.706Z",
    "result": {
      "chunk_id": "6f3a54",
      "wall_time_seconds": 10.0082031,
      "session_id": 98926,
      "original_token_count": 0,
      "output": ""
    }
  },
  {
    "requestUtc": "2026-10-31T13:00:03.649Z",
    "returnedUtc": "2026-10-31T13:00:06.760Z",
    "result": {
      "chunk_id": "3e1f9a",
      "wall_time_seconds": 3.1074014,
      "exit_code": 0,
      "original_token_count": 16,
      "output": "Cold-A raw preservation returned; root actual review required.\r\n"
    }
  }
]
```

## Five retained public reports and bindings

All five exist/parse, **14455B total**, each<=65536B:

| Role | Bytes | Raw SHA256 |
| --- | ---: | --- |
| started.json | 1925 | 14389f92d817e08d5c23f99135045c5d9a61f2e94759336934e8e3b2f7d655e4 |
| claim.json | 474 | a77f637409991705aaebb82d6cb333c4b5f9a4a73fa365888344ed36ebec1d7b |
| p2.json | 6335 | 93f52809a0e9279ded13fde0bcd5d64d2b5f35a73bc182cf40b888c7bc7e2c32 |
| parent.json | 782 | 4412f404cfd633887a6c6df5cdd95cfce515ad48c443b948e07a5bddf759486c |
| supervisor.json | 4939 | 76b7cbeee720cf26d946a0db6659b536cba0f270d12d85c6b1776fe584654590 |

Common start QPC **3016870458941**, frequency **10000000**. Supervisor PID **53648**, UTC **2026-10-31T12:59:49.8169008Z**; actual retained Parent PID **30984**, UTC **2026-10-31T12:59:50.5030282Z**, handle **2304**. D1/attempt/clock/identity agree. Claim binds raw started SHA; Parent binds P2 SHA; Supervisor binds raw claim/Parent/P2 hashes. All primary/cleanup causes and failures are null. Supervisor diagnostics is **normal_validated**, cause null; failed diagnostic collection was not needed.

**144 independent comparisons** of retained public data/pins passed; they invoked no project function and read no private evidence.

## Core observation and receipts

- completed / expected_set_captured_before_hash / observed_entry_sets_equal / input_handles_released / receipts_completed = **true**.
- **6956 expected files /45597399 payload bytes per tree; six existing locks; four full hash passes /182389596 hash bytes**.
- Historical, S-before, BKP-before, S-after and BKP-after tuple digests are all **7099e5259d9ec98ba4fb0dff98c610259f2416e5dbae517c6ca507913754b3b5**.
- Capture-complete QPC **3016940576354**, common age **7.0117413s**, before payload hashing as required by the pinned implementation. Core failure_code/phase/role/cause_kind=null; native_error/error_hresult/cleanup_native_error=0, cleanup_unknown=false.
- Peak owned handles **14076** (cap40062), remaining **0**; path arena **2107448B**, record arena **18586224B**, inventory **6765525B**, scratch **524288B**, within caps.
- Receipt bytes charged/known-written both **11361485B**, receipt_write_unknown=false. Four restricted receipts reported complete with digests below. **Private files were not reopened/rehashed after return**; these are the pinned writer's public observations.

| Private receipt name | Reported digest |
| --- | --- |
| owner.private.json | 7071ac0e56cdd73cc1215aab4f23dc4afe56030bf4f8bd3e16e0b838835f4fd8 |
| comparison.private.jsonl | 6032cd84343567417139a1240d4c6fe7d7831bf4179346a743a578921a4799a9 |
| namespace.private.jsonl | ec2e44db261c1c3cf78211c5f6f7fcaf665d33568aac329d2714860081421b22 |
| result.private.json | 3eb3a091bc45e57e611ed4417be69b0e757113526ccfbfe7914d1c710407366e |

Fresh output created/transferred, ancestry5/prepared6, remaining0, receipt mask15/transfers4, disposed=true/partial=false/native_error0/cleanup_unknown=false. Parent guard verified existing parent/ACL, did not create it; peak5/open attempts5/native close attempts5/closed5, remaining0, disposed/no unknown/native_error0. These are in-run observations, not separate post-return ACL/filesystem probes.

Core phase seconds as reported (phase endpoints can include final input cleanup, not pure hash-speed measurements):

| Phase | Seconds |
| --- | ---: |
| allocation | 0.0008135 |
| output_binding | 0.0013789 |
| locks | 0.0094529 |
| inventory | 0.4926881 |
| capture_s | 1.5768482 |
| capture_bkp | 1.9322262 |
| scan_before | 1.3179781 |
| hash_s_before | 0.8920952 |
| hash_bkp_before | 0.9449619 |
| hash_s_after | 0.7323172 |
| hash_bkp_after | 0.686825 |
| scan_after | 1.7591508 |
| final_inputs | 2.6382917 |
| receipts | 0.4655112 |

Inputs were released **before receipts**, interval_ended_before_receipts=true. No typed credential/DB open/decrypt/recovery/repair/rewrap is established or implied.

## Parent/Supervisor cleanup and completion

Parent cleanup_known=true; Core/output/guard/public remaining all0; job_finished=true, children_after0, process wrappers disposed. All five helper types fresh, each initializer once; compiler diagnostics empty/failed=false. Bounded process checks2.

Supervisor child exit0/reaped; registered child released, stdio disposed, public remaining0, no forced action/cleanup kill/cleanup unknown. Parent public peak21, Supervisor22 (cap24); external controlled bound59 remains the source bound. Managed public-close-native-observed=false in all reports: managed disposal is distinct from independently observed native CloseHandle results.

**510 normal waits =509 false+1 true**, cleanup waits0; actual successful signal wait, not HasExited/EOF inference. Normal gate passed, first failure null. Reap QPC **3017037009829** /age **16.6550888s**; bothEOF QPC **3017036913630** /age **16.6454689s**; normal gate **3017037034443**, pre-cleanup gate **3017038601375**, all before work45. Empty child stdout/stderr, bothEOF=true.

Observer **130/130 samples**, no failure/race; sampled OS handle peak **14725**, observed lifetime peak working set **256917504B** (caps45000/1GiB). Maximum successful gap **0.1288454s**, measurement-gap upper **0.1369229s**, max sample duration **0.0227121s**, total sampling **1.1353558000000004s**. Unobserved exit tail remains: public-clock-derived last-live-to-exit-indication gap **0.0880053s**, sample-start upper **0.0977537s**. These are sampled/lifetime observations, not exact OS handle peak or hard memory guarantees.

## Source proof and scope limits

All18 start/Parent-before/Parent-after/Supervisor-after source maps match independent current pins; active D1 unchanged:

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
| decision | 4769 | 3dc9c86545b83cfbfc24ac14160a30f01a2c4a8da8044a57e763ad2ee9ae47fd |

Continuous namespace absence and whole_attempt_verdict remain **not_established**. Cooperative locks/retained objects and observed entry sets establish the contracted bounded raw equality interval; state after release, continuous absence, later typed ownership/open and logical history remain separate. This successful run does **not** retrospectively diagnose attempt01's45s overrun or prove that the cleanup correction accelerated comparison. No failed Core phase exists in attempt02; failure fields are null.

After return only five fixed public report paths, public source/D1 pins and actual app metadata were read. This run record is the only authored artifact. No private E/receipt/input re-open, source/report repair, retry, new runtime, nav/ledger edit, commit or push. Partial/old attempt01 evidence remains untouched.

Outcome: **raw_preservation_observed with known reported controlled cleanup and normal completion before45; root actual-return review pending. STOP.**
