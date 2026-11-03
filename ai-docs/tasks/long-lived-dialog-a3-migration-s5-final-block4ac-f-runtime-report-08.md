# Block4AC: sole F08 Run did not pass feasibility checks

2026-11-03. The one externally activated published FeasibilityF Run on HEAD `a5124dad010a24d4eb59bf01bfb2004178c1e5a8` returned child/administration/tool exits **1/1/1** after **166.3810315 seconds** of supervised execution. `F_feasibility_checks_passed=false` is the conjunction established by the actual public postcheck, not an inference from an inner marker. F08 is consumed and was not retried. Whole S5.4 remains unaccepted. No source repair, compilation/staging, extra full Fresh, closed matrices, WholeP or commit/push occurred.

## Exact root binding and original invocation

Purpose FeasibilityF; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-08`; selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`; root run ID `5c09c59f80cad3b644e70f42982216c45cbd5ab9b557e824ef84584db36ebcc2`.

- Inventory `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-08\inputs.json`: 117304B, SHA256 `14b346dd63657287598f01b116ef33496b187bec36b524c87b00c769b9ef43cf`.
- Root-issued activation `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-08\activation.json`: 1003B, SHA256 `41fb017293c5e84981c0edc06ad51c18bdc42b90cc820a9a16733011af1e2029`.
- Validity `[1793683421479,1793690621479)` Unix ms permits this dispatch only; it does not alter runtime deadlines or allow a retry.
- Accepted preparation/review/activation receipt are under `.tmp/s54-block4ab-f-preparation-08`; original report/manifest/root receipt/review pins are retained.

Actual child invocation, executed once through the published outer/frozen stable launcher:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s54-whole.ps1' '-Action' 'Run' '-Purpose' 'FeasibilityF' '-InventorySha256' '14b346dd63657287598f01b116ef33496b187bec36b524c87b00c769b9ef43cf' '-ActivationSha256' '41fb017293c5e84981c0edc06ad51c18bdc42b90cc820a9a16733011af1e2029'
```

Fresh `.tmp/s54-block4ac-f-runtime-08` reuses reviewed Y owner/preflight/postcheck/finalization administration with new exact bindings,1872 reviewed flat history pins and F07 shallow-name preservation. Original Y files remain unchanged. The owner records actual exit/EOF observation ticks in its live wait loop before cleanup, pin reads or supplementary diagnosis. Invocation records were written once before Process.Start. Supervisory570/600/30 only bounds return/EOF/containment; it does not reset or extend original F450/480/510. No Run retry, new activation or output rewriting occurred.

| Step | Actual app results | Child/owner exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `c36d06 → 58a55b` | 0/0 | 15.7573331 | 270/300/30 |
| run-01 | `c1cc0f → e01f9c` | 1/1 | 166.3810315 | 570/600/30 |
| postcheck-01 | `a0a6dc → 43ce31` | 0/0 | 13.6987026 | 570/600/30 |

All three command receipts establish child handle/Job membership, child wait, bounded stdout/stderr EOF and verified empty supervisor cleanup. Exact flags and forced-cleanup facts remain in original receipts below; executable identity of any residual PID not captured by the owner is unknown and is not inferred. Actual Run owner PID45804,child PID66944,child started `2026-11-03T05:26:17.7360415Z`. Exit observed `2026-11-03T05:29:04.0769903+00:00`; EOF observed `2026-11-03T05:29:04.0333704+00:00`; observations-before-cleanup=true,supervisor_work_expired=false,reader_failed=false.

## Cheap fresh-child preflight and original clock

Preflight checked exact root inputs/activation and current HEAD/shared resolver,empty index,exact inputs.json+activation.json entries,WholeP absence,521source/nineimage/tool pins,successful native process inventory with zero exact-image conflicts and1872 flat public/history pins. Activation remaining at sample was7101842ms. No extra full Fresh/profiling/compiler/list/matrix work moved outside production admission.

All19 bound environment rows matched actual fresh child state without printing values. Regular Rust1.98 bin was prepended once to original PATH; process CARGO_NET_OFFLINE=true; RUST_LOG exact hash; other16 variables absent. TOKIO_WORKER_THREADS absent,available/effective24. Pinned installed pwsh,NativeToken/x64/BelowNormal/Jobs2/offline/locked/skip-cache unchanged. Original owner PATH SHA256 `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`,configured owner `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`,actual fresh child `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. No persistent tool/environment/ACL/system changes.

| Clock event | Windows tick ms | Elapsed from original T0 ms |
| --- | --- | --- |
| Original owned T0 | 533670625 | 0 |
| Controller cutoff | 534120625 | 450000 |
| Work cutoff | 534150625 | 480000 |
| Outer cutoff | 534180625 | 510000 |
| Outer checks observed | 533836437 | 165812 |
| Actual child exit observed | 533836578 | 165953 |
| Actual stdout/stderr EOF observed | 533836531 | 165906 |

Controller/work/outer remain450000/480000/510000ms from the same owned T0. The durable `actual_desktop_return` record is `pending`; it was not rewritten to claim the separately observed desktop command return. Original receipt exit/EOF times above are the external observation.

## Actual native/functional/output evidence

Outer result: 3758B, SHA256 `d08f2343b97440b1c099af4fe6a86ea6795c135aa27940f44a1badd5e744322c`; outer_checks=`failed`,public failure=`harness_nonzero`. Assertion file: not produced; assertion envelope valid=false,stored full output contract/layout valid=false. Join count1,permit count1. No missing assertion,marker,claim or clock evidence is inferred.

Actual five binding checks (outer claim,launch/capture,native claim,original clock,join/permit):

```json
{
  "native_claim": true,
  "launch_capture": true,
  "join_permit": true,
  "original_clock": true,
  "outer_claim": true
}
```

Original harness/launcher process outcomes,including waits/exit codes when recorded:

```json
{
  "harness": {
    "waited": true,
    "exited": true,
    "code": 101,
    "pid": 63140
  },
  "launcher": {
    "waited": true,
    "exited": true,
    "code": 124,
    "pid": 50396
  }
}
```

Exact bounded protocol cardinalities:

```json
{
  "named_failed": 1,
  "success_summary": 0,
  "running_one": 1,
  "wrapper_completed": 0,
  "native_capture": 0,
  "named_ok": 0,
  "failed_summary": 1,
  "named_invocations": 1,
  "standalone_ok": 0,
  "native_functional": 0
}
```

| Stream | Stored protocol bytes | Captured original prefix bytes | Prefix SHA256 | Redacted lines | Partial tail bytes | Admission diagnostics count/dropped |
| --- | --- | --- | --- | --- | --- | --- |
| stdout | 214 | 325 | `d43cafeffa6bfea815542547d3ee2723d71cdfcb3b602aeae8d5fe7b4e830679` | 8 | 0 | 0/0 |
| stderr | 0 | 51 | `1550ce810f13055cc1603dd267fc8fe413cc39430a6f0ac98c52722798c567b3` | 1 | 0 | 0/0 |

Safe admission diagnostics are structured stage/reason-only advisories with no runtime authority: `[]`. Arbitrary raw errors/private command-log text were not recovered or exported. On failure, retained public metadata alone does not establish the exact functional cause. Original public outer/protocol/assertion artifacts remain authoritative; an inner success marker cannot establish the full result.

Runtime cleanup verified=True,forced=True,remaining owned after cleanup=`[]`. Forced runtime cleanup is not success. Supervisor cleanup facts:

```json
{
  "preflight-01": {
    "before": [],
    "failed": false,
    "forced": false,
    "verified": true,
    "after": []
  },
  "run-01": {
    "failed": false,
    "verified": true,
    "after": [],
    "before": [],
    "forced": false
  },
  "postcheck-01": {
    "after": [],
    "verified": true,
    "forced": false,
    "failed": false,
    "before": []
  }
}
```

Actual postcheck passed source521/image9/input/navigation pin preservation and empty index; successful native process enumeration found zero remaining exact-image processes. Published outer final checks and original assertion/output validation are used at their production clock positions. The postcheck did not rerun full Fresh or reinterpret private errors.

## Scope and preservation

F01/F02 remain empty; F03/F04 retain original eight failure files; F05/F06/F07 preserve89/397/337 shallow names.1872 approved flat immutable pins,accepted preparation public/admin artifacts,all seven final origins and seven dep-info pins remain unchanged. No old raw command/runtime logs or historical f-origin/profile/key/DB contents were read. WholeP directories remain absent; P1 passive30-second Backoff risk remains for its eventual contract review.

The published F08 scenario was authorized to create/use its own synthetic test profiles/keys and local peer/relay/restart/offline/migration processes. Reporting only reads the explicitly public bounded evidence set and shallow names; it never traverses or exports profile/key/DB or per-command-log contents. Current F08 has339 shallow top-level names,including324 command stdout/stderr log names; only14 explicit public files are pinned. Large originals are referenced,not copied into this report.

Outside-report Git status and navigation are checked against baseline. No source/workload/hook/validator/deadline changes,claimed-state deletion,automatic retry/rollback,next purpose transition or commit/push occurred. Stop after one substantive callback for root review; successful F feasibility alone does not establish whole S5.4 acceptance.
