# Block4BF: F13 consumed; feasibility not established

2026-11-10. Exactly one root-activated published FeasibilityF Run on HEAD `81a4472805302a6e4fd778f21f3922c7d4cbc03c`. Actual app/owner/tool exits **1/1/1**, native harness **101**, frozen launcher **124**. Outer category `harness_nonzero`, outer checks failed; F feasibility and WholeP remain unproved. The current public stderr prefix pin matches the finite current-source message `Error: absolute S5.3 process controller deadline\n` (49B/SHA256 `9429bd4b41beb1dedd46ec97ab2da6f62aa11618f2ccd6a2c8ce4c8e82a08408`), without reading raw stderr. This establishes the bounded deadline error, not an individual operation's cost or a product root cause. F13 is consumed; no retry, repair, new activation or WholeP was attempted.

## Exact binding and preservation

- Case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-13`.
- Selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`.
- Run ID `f18c5714ee30faf7e84c6f130f58fe93a756b2bc43c4234fea5dbde0d3ad6617`.
- Inventory 120527B, SHA256 `b407343203b09408e1634cb8450a2a27b843f04c3200f80bd2057a76f5ac9ff5`.
- Root activation 1002B, SHA256 `5fd8ba654f459bfb424d4621ddecca85ad7366d8ce85052efe608c0ee6c7181b`; validity [1794287540246,1794294740246) Unix ms. Validity adds no scenario time.
- Accepted proposal 11075B, SHA256 `822ad3edc131d0c2a9362a148107731ffc5f70a622922b9d3432365bdeb263bd`.

Executable remained a STRING with a separate pin: pinned x64 pwsh, 301360B, SHA256 `2a6769a3330dc1084d0154bfde7d164ab750ca7f700f313f9d417c1be78b2233`. Exact accepted argv, only activation placeholder substituted:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s54-whole.ps1' '-Action' 'Run' '-Purpose' 'FeasibilityF' '-InventorySha256' 'b407343203b09408e1634cb8450a2a27b843f04c3200f80bd2057a76f5ac9ff5' '-ActivationSha256' '5fd8ba654f459bfb424d4621ddecca85ad7366d8ce85052efe608c0ee6c7181b'
```

Fresh `.tmp/s54-block4bf-f-runtime-13` administration reuses AZ code with explicit current bindings, not old evidence. The native fresh-child probe matched all 19 presence/hashes without printing values: original/configured-owner/actual-child PATH SHA256 `209a7722263b771eaec5539b45aa49f62c1ee9adfa9d829588ce28a20da41d53` / `dbc38fda0e6edb66dc2e175343841fbbe7e2a6867d7d6e7488e099bf2f377b4c` / `46ea263df82a55356269fbf3d8d759a137930675e7c9882414d698378834e656`. Rust1.98 bin prepended once; offline process setting, BelowNormal/NativeToken/x64, Jobs2, inherited RUST_LOG, TOKIO_WORKER_THREADS absent, available/effective24. No persistent environment/system changes.

Preflight verified HEAD/index, 536 sources/nine images, input pins, exactly inputs.json+activation.json before Run, no WholeP/claims/runtime leftovers, successful native exact-image enumeration with zero conflicts, and sufficient validity. No extra full Fresh/profiling/matrix, compilation or image staging. Postcheck and delivery reverified source/image/input/proposal/navigation/foreign tracked baseline. BE report 8159B/SHA256 `e2ccc43ba56b8097a98baf60c075b6371c5cb6d2236d5fd8eeb212fe80e186ae` and BE pins 156641B/SHA256 `22d7cab63e044f5a6a559f4ba3d55d526b31bbcfdd5e076ae76b97a86d9940d0` remain byte-identical. HEAD unchanged; index empty. No commit/push.

## Actual returns, ownership and clocks

|Step|Actual app/owner exit|Seconds|Work/whole/cleanup seconds|
|---|---|---|---|
|preflight-01|0/0|5.6751519|270/300/30|
|run-01|1/1|463.1537119|570/600/30|
|postcheck-01|0/0|4.2926858|570/600/30|

Actual desktop receipts: preflight `4ef8c4` exit0; Run dispatch `0f0090`, session82805, final `c05a99` exit1; postcheck `5dfe2e` exit0. Run owner PID39660, actual app PID49980; native harness PID49092 exited101/waitedtrue, frozen launcher PID7296 exited124/waitedtrue. All owner steps verified actual handle/Job membership, waited, observed bounded stdout/stderr EOF, had no reader failure, and ended with empty supervisor-owned lists. Run work_expired=false; supervisor original tick282827218 is not the scenario clock. Its 570/600/30 bounds observation/containment only.

Original owned-launch T0 governs controller450/work480/outer510 seconds including admission/startup/function/final work. No reset or additional scenario budget. T0 UTCms 1794287813422.

|Event|Windows tick ms|Elapsed from T0 ms|
|---|---|---|
|T0|282828312|0|
|Controller|283278312|450000|
|Work|283308312|480000|
|Outer|283338312|510000|
|Checks observed|283290875|462563|
|Actual app exit observed|283291031|462719|
|EOF observed|283291000|462688|

App exit UTC `2026-11-10T05:24:36.1374509+00:00`; EOF UTC `2026-11-10T05:24:36.1078367+00:00`. Both ticks were recorded in the owner's live loop **before** cleanup/pin reads/diagnosis. Public outer `actual_desktop_return="pending"` and verdict `not_established` remain durable original fields; later app exit1 is recorded separately. Compatibility postcheck `actual_native_exit=1` means the Run app child; it is not the actual native harness code101.

All five public bindings passed: outer_claim, launch_capture, native_claim, original_clock, join_permit. Exactly one join and one permit, with current owner/harness identity. Protocol counts: `{"failed_summary": 1, "named_failed": 1, "named_invocations": 1, "named_ok": 0, "native_capture": 0, "native_functional": 0, "running_one": 1, "standalone_ok": 0, "success_summary": 0, "wrapper_completed": 0}`. One named invocation/FAILED/failed summary; no native-functional, wrapper-completed, native-capture or success markers. Stored protocol layout invalid. Saved admission diagnostics: `[]`; none of the five f_ack_release reasons was emitted. Absence is not a positive stage result.

## Public assertions and bounded failure diagnosis

Original assertions.json exists: 368082B/SHA256 `34c1e5a9bd4895849bc968219598250e43bed11493461a9d967abca76c7dcbcd`. It records `functional_assertions_completed`, observation class `F feasibility functional only`, verdict `not_established`. This is a durable functional snapshot **before final tail**, not successful completion. `attempt_result.rs` writes/flushes/syncs the snapshot before its tail closure. `migration_s54_admission_runtime.rs::persist_facts` revalidates before the snapshot, revalidates in the post-write closure, pins the output, then checks the deadline before the native-functional marker. No such marker was observed. Assertions record controller_elapsed_ms=376514, scenario_elapsed_ms=264733. Stored controller observation-to-observed-app-exit difference is 86205ms; the observation precedes persistence. This gap includes unmeasured pre-snapshot revalidation, persistence, post-snapshot revalidation, final/launcher/outer work. There is no per-operation duration or precise failing revalidation call in the permitted public evidence.

An independent, bounded public envelope inspection also found a path-representation mismatch: Rust canonicalization emitted an extended drive prefix `\\?\`; the provenance path equals the expected inventory path after removing only this prefix, while .NET GetFullPath plus OrdinalIgnoreCase comparison still differs. Published `Assert-S54SamePath` uses the latter comparison. Other fixed envelope fields matched. Thus postcheck's assertion_envelope_valid=false has a concrete path-comparison explanation; it is separate from the native controller error. The initial literal comparison alone was insufficient and was refined in public-refinement.json. No source or assertion was rewritten, no published admission was rerun, and no full F success is inferred from the inner snapshot.

BC scalar observations (original public assertion, physical B slot **0**):

|Observation|Scenario/controller-local ms as stored|
|---|---|
|arm lower bound / earliest pause expiry|295220 / 415220|
|kill start / kill-wait reconciliation end|295913 / 296332|
|release request / response end / observed|326263 / 326361 / 326367|
|release phase deadline|355913|
|controller / pause remaining before kill|154086 / 119307|
|controller / pause remaining at release|123632 / 88852|
|release to memory completion|50145|

Inclusive old-ACK release class duration 30454.479ms (kill start295913196us to observed326367675us), before phase deadline355913ms. Tail planning floor90000ms; tail_measured_guarantee=false. **late_open_witness=false**, late-open request/response null. Even though release followed stop-wait by about30035ms, this does not establish a fresh >=30s late-open request with the same lease/generation/reservation32. No >30s closure reproduction claim. Other fixed numeric class facts remain in public-assertion-facts.json; raw rows/status bodies were not exported.

## Cleanup, evidence and stop boundary

Runtime forced cleanup=true/verified=true, owned_before=[7296,37700], owned_after=[]. PID7296 is the recorded launcher; PID37700 has no individual executable identity/termination receipt in this permitted public metadata. Do not infer its role. Supervisor cleanup separately was nonforced/verified with before=[]/after=[]. Successful read-only native exact-image enumeration after Run found zero surviving current images; no foreign process was terminated. Forced cleanup is not success.

Original outer-result.json is 3756B/SHA256 `21195987175153fc07059b9ef35c7df0bd6a08b41acd5b5c16fdfa2392b04fc5`. Stored stdout protocol is 215B/SHA256 `a5bb76b82601fb5c66e166759d071fd1ea3a02f46c14aa84ef74aee6cc4c1326`; stored stderr protocol is empty. Both captured streams reached EOF without overflow/invalid UTF8/truncation; nonprotocol redaction counts stdout8/stderr1. Captured stderr prefix was compared only against 16 finite current-source message/prefix/newline candidates using the public length/hash; raw stderr and arbitrary command logs were not opened. The public output remains complete=false; capture marker absent.

Only exact current allowlisted public artifacts were inspected/pinned: inputs/activation, claims/launch/capture/clock/join/permit, assertions/outer-result/protocol, and public memory metadata if present. No command-log/f-origin/profile/key/DB reads or private export; F01-F12 and BA/AU/AM16/AD16 raw/private scopes remain closed, with no historical graph expansion. One Python administrative inspector initially failed to parse before any reads; the original code was preserved, spacing corrected, then the bounded read ran once. This was not a runtime retry.

Companion pins and .tmp/s54-block4bf-f-runtime-13/candidate.json identify the two BF review documents and original evidence. BE documents remain separate and unchanged for root's combined publication after review. S5.3 remains closed; S5.4 open. Stop after callback: no polling, repair, retry, next activation, WholeP or commit/push.
