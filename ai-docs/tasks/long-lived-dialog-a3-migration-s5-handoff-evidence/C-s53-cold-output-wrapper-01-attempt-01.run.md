# C2-S attempt01 — actual focused wrapper return

2026-10-31. One authorized NativeToken/require_escalated watchdog invocation, fixed bundled pwsh, work20/whole30/lane1/retry0. Source-accepted and canonical allocation1261B/SHA256 `753b55e47d04dff84416868be15936021318e677af4f9e8064c055844e02a51b`. Result: native exit0; both focused cases observed. Root actual review remains pending. No retry, continuation, standalone compilation/preliminary helper/probe or in-place repair.

## Invocation, clocks and immutable native result

Request UTC **2026-10-31T07:02:59.975Z**; final native tool return UTC **2026-10-31T07:03:05.999Z**. Request-to-final **6.024s**. Exactly one native chunk, no session/polling. Native tool duration **3.1882073s**; app commandExecution `exec-e8c16e47-45ef-478a-b420-858b057d202d`, completed/exit0, **3188ms**. Worker report observed_seconds **2.7841064s** and parent report **3.255113s** are separate overlapping scopes, including their documented startup-clock accounting; none are summed. Root alone charges request-to-final once and releases the reserve remainder; this delivery edits no ledger.

Fixed shell: `C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe`, login false, cwd repository. Command: accepted `C-s53-cold-output-wrapper-01-attempt-01.watchdog.ps1` with only `-AllocationSha256 753b55e47d04dff84416868be15936021318e677af4f9e8064c055844e02a51b`. Shell/compiler/helper work occurred only inside that invocation.

Complete native chunk (stdout CRLF preserved as a JSON escape; no truncation):
```json
{"chunk_id":"62eac7","wall_time_seconds":3.1882073,"exit_code":0,"original_token_count":19,"output":"C2-S focused wrapper cases returned; root actual-return review required.\r\n"}
```

Parent PID46840; worker20644, started UTC `2026-10-31T07:03:03.2269071Z`. App startup UTC recorded in started.json `2026-10-31T07:03:02.6404762Z`. Common start QPC2802798690253/frequency10000000; absolute work deadline2802998690253.

## Raw public report pins

All three reports retained unchanged; total **19228B**, below196608B. Each below65536B.

| Attempt-prefixed file | Bytes | SHA256 |
| --- | ---: | --- |
| started.json | 1021 | `9b2bb821d28b8c7430aa2b54cd54e8a2345c819695ac1ab7d8ea20f1ba17f1da` |
| cases.json | 13349 | `4c6c7e864f409703a3f440bed50b9536573d667b584908561f7c0da183552283` |
| watchdog.json | 4858 | `549df7a1837f52027c03f5ff27098b656f8e32fd9b4ef0e6e970f2a89763b604` |

## Actual witnesses

Healthy: exact real owner/delegates through changed legacy wrapper; E created/transferred, binding success true. Owner7 beforeTake→6 afterBind→0 after caller cleanup, ancestry6. Core owned1 after return→0 after Dispose, peak1/known cleanup, no error. E empty before/after; ACL digest unchanged.

ACL: E created/transferred true, **binding success false**, expected real verifier refusal. Setter1/result0, present/non-null/valid protected narrowed DACL; current-user/SYSTEM ACE bytes and owner/group preserved. Exact original marks `colda_code=output_acl`, `colda_phase=output_binding`, `colda_role=output`, `cold_output_code=private_acl`, `rootcopies_code=private_acl`; native0/HResult-2146233079/cause unexpected. Actual PowerShell chain:
`MethodInvocationException(-2146233087) -> normalized InvalidOperationException(-2146233079) -> original private_acl InvalidOperationException(-2146233079)`.
There is no visible TargetInvocationException; entry was not inferred from it. No separately instrumented verifier-entry count is claimed.

Wrapper catch QPC2802825413716 precedes Core.Dispose entry2802825469381 and wrapper return2802825801172. Catch witnesses: pending IsClosed=true, **IsInvalid=false**, Core owned0/CleanupUnknown=false/native0 before wrapper Dispose. Core.Dispose succeeded; owned0/known cleanup afterward. Fallback needed/used/skipped-unknown allfalse; pending closed afterward; no secondary error, cleanup/native-close unknown false, diagnostics commit marker true. Closed managed state and observed Core native-close accounting are reported separately; no second-native-close claim.

Owner ACL case7→6→0, ancestry6, receipt mask/transfers0. Helper retained roots7; healthy helper7 around binding, ACL helper8 including editor→7 after case. Final helper closes10/10, searches4/4, empty checks4, editor opens3, peak9 including search, remaining0/known cleanup, disposed. Descriptor pin1/free1, no owned descriptor/search/editor. Both Core instances: path charge0, record12073840B, scratch524288B, filesystem peak1; hash/capture/inventory/receipt counters0. Worker fresh types4/initializers4; compiler errorfalse, diagnostics empty. Public inputs9/9 closed in worker/parent; worker job finished/owned children0; parent registered child released/stdio disposed, no force/kill/unknown.

| Witness | Healthy | ACL refusal |
| --- | --- | --- |
| E identity | `820acd79:000200000005eb6f` | `820acd79:000200000005eb70` |
| ACL before SHA256 | `45fe1be9f04c0ec6d95da3a47dfb85a7adde66fb0a9fb508a20882106fe0a46a` | `45fe1be9f04c0ec6d95da3a47dfb85a7adde66fb0a9fb508a20882106fe0a46a` |
| ACL after SHA256 | `45fe1be9f04c0ec6d95da3a47dfb85a7adde66fb0a9fb508a20882106fe0a46a` | `de29a6a9b735ed29e0cf7cd65a25d9a831568ebd5b1330da96ad55b8c4b65169` |
| In-memory metadata bytes | 2013 | 2025 |
| Metadata SHA256 | `3de9737b02c04660f89b36eda638267f846a2c66e8ba3286022274791620a136` | `ea7a2bdab58553178221136f7fd467e24f29199f506529bb0aaf05a8a1ccd4e1` |

ACL fingerprints are descriptor snapshots, not preservation payload digests. Producer-shaped metadata stayed in memory; no inventory, input trees, lock files, payload capture/hash or receipts. Both cases explicitly report positive_preservation_proof=false and whole_attempt_verdict=not_established.

## Completion and resources

Normal wait attempts69: false68/true1; cleanup waits0. One successful retained-Process WaitForExit set reap in normal phase, exit0 read afterward. Both EOF QPC2802828861094; exit indication2802828874992; actual reap2802828977811. All before common20 deadline. Normal completion gate passed; pre-cleanup/first failure reasons null; stderr/stdout empty in worker, no output cap, forcedfalse/cleanup-killfalse/unknownfalse. Successful reap was not inferred from HasExited or EOF; wait trace2 entries retained.

Observer21 attempts/21 live samples, no exit race/failure. Sampled OS handle peak622 (cap1024); observed Process lifetime working-set high-water209985536B (cap1073741824B); maximum sample duration0.020989s, maximum measurement-gap upper0.1464027s, unobserved exit-tail upper0.082966s. These are sampled Process observations, not exact OS handle high-water or a hard memory-limit proof. Explicit helper peak9 and source-constrained combined filesystem/search bound16 remain separate from OS counts.

## Source preservation and limits

Seven source/dependency hashes agree across started marker, worker before/after, parent after and independent postrun disk reads; allocation hash unchanged. No source repair/mutation after execution. Public reports and source pins were the only postrun workspace reads; no fixture/private/original inspection or network.

| Pin | Bytes | Postrun SHA256 |
| --- | ---: | --- |
| preflight | 40694 | `bbd62d628ef9f8ac7d93ef1a9bb6ee45336739445e5e0e7fc5a0d89db9ffb8b1` |
| watchdog | 45206 | `c08085db048bd80d82abe227d72b4ac35ff95797a8f8250de0189fd9153ab535` |
| core | 82836 | `4288f66bbc634f336fb962624fe15aaf6561f760f7d2c096579bb94e31711612` |
| output | 21296 | `9d4b2a4fed3ffd76a1393514dff69f77baa5e20567e25c171be1c4203ec3a4b9` |
| deadline | 6038 | `0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45` |
| fixture_support | 21111 | `4838fa35fe41551191e1bccf33b9c54115e53587a46f1634b7804e807698b7c8` |
| shell | 301368 | `362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139` |
| allocation | 1261 | `753b55e47d04dff84416868be15936021318e677af4f9e8064c055844e02a51b` |

Independent postrun read-only verification: **44 assertions** across source pins, outcomes, original marks, managed state, cleanup ordering/counts, ACL/metadata fingerprints, zero capture/receipts, compiler diagnostics and actual completion. These are public-report checks, not another native case.

No cleanup-fault/native-close-failure/pre-entry rejection/concurrency case was executed. No full matrix/producer rerun, private typed-open, G1–G4, product/Rust/network tests or S5.4. Whole S5.3 remains open. No ledger/navigation edit, commit or push. Callback root and STOP for actual review.
