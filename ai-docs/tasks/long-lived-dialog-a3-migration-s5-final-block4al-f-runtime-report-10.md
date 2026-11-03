# Block4AL: sole F10 Run did not pass feasibility checks

2026-11-03. The one externally activated published FeasibilityF Run on HEAD `f06dcd7e0d372dca2b7f5cb29d9c945961dd41cf` returned child/administration/tool exits **1/1/1** after **171.8942818 seconds** of supervised execution. `F_feasibility_checks_passed=false` is the conjunction established by the actual public postcheck, not an inference from an inner marker. F10 is consumed and was not retried. Whole S5.4 remains unaccepted. No source repair, compilation/staging, extra full Fresh, closed matrices, WholeP or commit/push occurred.

The concrete blocker is `harness_nonzero`: the single selected native test returned101 and its launcher returned124. There is one named failed test/failed summary, no assertions file and no native functional/capture or wrapper-completed marker. Runtime cleanup was forced (before-list PIDs69388,67428; after-list empty);69388 is the recorded launcher PID,67428 executable identity remains unknown. Supervisor cleanup was not forced. This is a functional failure before the450/480/510cutoffs, not a supervisor timeout. No structured admission stage/reason diagnostic was emitted. The bounded redacted public evidence does not establish the exact underlying functional error; private command logs were not opened to recover it.

## Exact root binding and original invocation

Purpose FeasibilityF; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-10`; selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`; root run ID `1d99f8b46e56fd6daf0c36ea8df507217a2e8614804a6e810612bf210390dc87`.

- Inventory `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-10\inputs.json`: 117966B, SHA256 `645669bc61a2bb23d0d916f7019cbd70002d5e26f118a82b3b78f4f1ef146347`.
- Root-issued activation `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-10\activation.json`: 1003B, SHA256 `8c809683906d7d8e475f5ed214d74a05651ebca97cbe27fc28b246846e23a138`.
- Validity `[1793696232627,1793703432627)` Unix ms permits this dispatch only; it does not alter runtime deadlines or allow a retry.
- Accepted preparation/review/activation receipt are under `.tmp/s54-block4ak-f-preparation-10`; original report/manifest/root receipt/review pins are retained.

Actual child invocation, executed once through the published outer/frozen stable launcher:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s54-whole.ps1' '-Action' 'Run' '-Purpose' 'FeasibilityF' '-InventorySha256' '645669bc61a2bb23d0d916f7019cbd70002d5e26f118a82b3b78f4f1ef146347' '-ActivationSha256' '8c809683906d7d8e475f5ed214d74a05651ebca97cbe27fc28b246846e23a138'
```

Fresh `.tmp/s54-block4al-f-runtime-10` reuses reviewed AC/Y owner/preflight/postcheck/finalization administration with new exact bindings,2157 reviewed flat history pins and F08/F09 shallow-name preservation. Original AC/Y files remain unchanged. The owner records actual exit/EOF observation ticks in its live wait loop before cleanup, pin reads or supplementary diagnosis. Invocation records were written once before Process.Start. Supervisory570/600/30 only bounds return/EOF/containment; it does not reset or extend original F450/480/510. No Run retry, new activation or output rewriting occurred.

| Step | Actual app results | Child/owner exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `609c02 → a4469a` | 0/0 | 19.0099149 | 270/300/30 |
| run-01 | `70f2cb → a3902d` | 1/1 | 171.8942818 | 570/600/30 |
| postcheck-01 | `ba0dc1 → c09bbe` | 0/0 | 17.4255305 | 570/600/30 |

All three command receipts establish child handle/Job membership, child wait, bounded stdout/stderr EOF and verified empty supervisor cleanup. Exact flags and forced-cleanup facts remain in original receipts below; executable identity of any residual PID not captured by the owner is unknown and is not inferred. Actual Run owner PID50888,child PID70336,child started `2026-11-03T09:00:19.5130029Z`. Exit observed `2026-11-03T09:03:11.3667131+00:00`; EOF observed `2026-11-03T09:03:11.3690205+00:00`; observations-before-cleanup=true,supervisor_work_expired=false,reader_failed=false.

## Cheap fresh-child preflight and original clock

Preflight checked exact root inputs/activation and current HEAD/shared resolver,empty index,exact inputs.json+activation.json entries,WholeP absence,524source/nineimage/tool pins,successful native process inventory with zero exact-image conflicts and2157 flat public/history pins. Activation remaining at sample was7052129ms. No extra full Fresh/profiling/compiler/list/matrix work moved outside production admission.

All19 bound environment rows matched actual fresh child state without printing values. Regular Rust1.98 bin was prepended once to original PATH; process CARGO_NET_OFFLINE=true; RUST_LOG exact hash; other16 variables absent. TOKIO_WORKER_THREADS absent,available/effective24. Pinned installed pwsh,NativeToken/x64/BelowNormal/Jobs2/offline/locked/skip-cache unchanged. Original owner PATH SHA256 `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`,configured owner `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`,actual fresh child `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. No persistent tool/environment/ACL/system changes.

| Clock event | Windows tick ms | Elapsed from original T0 ms |
| --- | --- | --- |
| Original owned T0 | 546512421 | 0 |
| Controller cutoff | 546962421 | 450000 |
| Work cutoff | 546992421 | 480000 |
| Outer cutoff | 547022421 | 510000 |
| Outer checks observed | 546683718 | 171297 |
| Actual child exit observed | 546683875 | 171454 |
| Actual stdout/stderr EOF observed | 546683875 | 171454 |

Controller/work/outer remain450000/480000/510000ms from the same owned T0. The durable `actual_desktop_return` record is `pending`; it was not rewritten to claim the separately observed desktop command return. Original receipt exit/EOF times above are the external observation.

## Actual native/functional/output evidence

Outer result: 3758B, SHA256 `c498dd0ecb5e86d603dbffb828b814470f1dbaa4bbf5c12dc1bf340bd3eb631a`; outer_checks=`failed`,public failure=`harness_nonzero`. Assertion file: not produced; assertion envelope valid=false,stored full output contract/layout valid=false. Join count1,permit count1. No missing assertion,marker,claim or clock evidence is inferred.

Actual five binding checks (outer claim,launch/capture,native claim,original clock,join/permit):

```json
{
  "launch_capture": true,
  "join_permit": true,
  "native_claim": true,
  "original_clock": true,
  "outer_claim": true
}
```

Original harness/launcher process outcomes,including waits/exit codes when recorded:

```json
{
  "harness": {
    "exited": true,
    "waited": true,
    "pid": 46468,
    "code": 101
  },
  "launcher": {
    "exited": true,
    "waited": true,
    "pid": 69388,
    "code": 124
  }
}
```

Exact bounded protocol cardinalities:

```json
{
  "standalone_ok": 0,
  "native_functional": 0,
  "native_capture": 0,
  "wrapper_completed": 0,
  "named_invocations": 1,
  "named_failed": 1,
  "failed_summary": 1,
  "named_ok": 0,
  "success_summary": 0,
  "running_one": 1
}
```

| Stream | Stored protocol bytes | Captured original prefix bytes | Prefix SHA256 | Redacted lines | Partial tail bytes | Admission diagnostics count/dropped |
| --- | --- | --- | --- | --- | --- | --- |
| stdout | 215 | 326 | `a86e3c5a77366088bcb3b564511df16a629be3c2266df19871a0aa52006bcf03` | 8 | 0 | 0/0 |
| stderr | 0 | 38 | `a8c27b3d394cf3d63bb2ebcff72cc55aa6dc8cbf38ac3523dfd47d67230ee562` | 1 | 0 | 0/0 |

Safe admission diagnostics are structured stage/reason-only advisories with no runtime authority: `[]`. Arbitrary raw errors/private command-log text were not recovered or exported. On failure, retained public metadata alone does not establish the exact functional cause. Original public outer/protocol/assertion artifacts remain authoritative; an inner success marker cannot establish the full result.

Runtime cleanup verified=True,forced=True,remaining owned after cleanup=`[]`. Forced runtime cleanup is not success. Supervisor cleanup facts:

```json
{
  "preflight-01": {
    "failed": false,
    "before": [],
    "verified": true,
    "after": [],
    "forced": false
  },
  "run-01": {
    "forced": false,
    "after": [],
    "verified": true,
    "failed": false,
    "before": []
  },
  "postcheck-01": {
    "failed": false,
    "after": [],
    "before": [],
    "forced": false,
    "verified": true
  }
}
```

Actual postcheck passed source524/image9/input/navigation pin preservation and empty index; successful native process enumeration found zero remaining exact-image processes. Published outer final checks and original assertion/output validation are used at their production clock positions. The postcheck did not rerun full Fresh or reinterpret private errors.

## Scope and preservation

F01/F02/F09 remain empty; F03/F04 retain original eight failure files; F05/F06/F07/F08 preserve89/397/337/339 shallow names.2157 approved flat immutable pins,accepted preparation public artifacts and report/manifest documents,all seven final origins and seven dep-info pins remain unchanged. No old raw command/runtime logs or historical f-origin/profile/key/DB contents were read. WholeP directories remain absent; P1 passive30-second Backoff risk remains for its eventual contract review.

The published F10 scenario was authorized to create/use its own synthetic test profiles/keys and local peer/relay/restart/offline/migration processes. Reporting only reads the explicitly public bounded evidence set and shallow names; it never traverses or exports profile/key/DB or per-command-log contents. Current F10 has349 shallow top-level names,including334 command stdout/stderr log names; only14 explicit public files are pinned. Large originals are referenced,not copied into this report.

Outside-report Git status and navigation are checked against baseline. No source/workload/hook/validator/deadline changes,claimed-state deletion,automatic retry/rollback,next purpose transition or commit/push occurred. Stop after one substantive callback for root review; successful F feasibility alone does not establish whole S5.4 acceptance.
