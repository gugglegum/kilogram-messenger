# C2-T focused public controls — attempt 01 actual return

Status: **observed; submitted for root actual-return review**. Whole-attempt verdict remains `not_established`; this is not whole-S5.3 acceptance.

## Authorization and single invocation

Adjacent `C-s53-cold-production-control-01-attempt-01.source-accepted.md` and allocation SHA256 `2a7b5b2f449b7b24913b714ebcf51283ee5efe818c773298f7f80a8172d38770` authorized exactly one reviewed watchdog invocation: work 20 s, whole 30 s, lane 1, retry 0. Public source/allocation pins and absence of all five report outputs were checked before invocation (native chunk `b890bf`, exit 0).

Invoked the accepted watchdog with `-AllocationSha256 2a7b5b2f449b7b24913b714ebcf51283ee5efe818c773298f7f80a8172d38770`, using the fixed bundled shell `C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe`, profile disabled, repository cwd, NativeToken. No session, poll, retry, repair, production entrypoint, or private-role invocation followed.

Request UTC **2026-10-31T08:53:38.639Z**; final-return UTC **2026-10-31T08:53:47.044Z**; request-to-final **8.405 s**. Complete native result (one chunk):

```json
{
  "chunk_id": "a30eca",
  "wall_time_seconds": 5.5955414999999995,
  "exit_code": 0,
  "original_token_count": 19,
  "output": "C2-T focused public controls returned; root actual-return review required.\r\n"
}
```

App commandExecution `exec-f7a11619-ed02-4147-a449-cd55ebfdf9b8`: completed, exitCode 0, durationMs **5595**, output.truncated false. Native wall **5.5955414999999995 s** is a separate tool measurement. Worker **5.3120954 s** and supervisor **5.6534733 s** are overlapping scopes; do not add them to each other or to request-to-final. Root owns ledger charging/release; no ledger was edited.

## Retained public reports and unchanged pins

Five reports, **14840 bytes total**, below the 204800-byte cap; raw files remain adjacent. SHA256:

| Report suffix | Bytes | SHA256 |
| --- | ---: | --- |
| started.json | 1031 | 82bbfd3d0b95ac2c480003850e5712e29d556646ad013ce521b6192892b6e50c |
| cases.json | 7530 | 48e15f2574fea3e767e6d81b743fe3a5424ece66aa8fd1442e3d51d9dd1c8483 |
| watchdog.json | 4873 | 3eba79e828fbaf8db67aaad63f023acb8f7bf82eff18e071b8b31c7e4baf7cdf |
| protocol-match.started.json | 703 | f082bc1516c0ad2bbf04cdd7eb89bac05487bf3bd98b50e42815933c0558d33f |
| protocol-wrong-pid.started.json | 703 | 9bc1ab1c8c1ee0a25ad74afd88ca3810c5838fd2de0b6147068fd20c4fefe370 |

Post-return public source/allocation pin reads:

| Role | Bytes | SHA256 |
| --- | ---: | --- |
| preflight | 35688 | 13f9408a537f04895c5ad14d598ba04ac62a9103e258fe49fab10d8b4b70450b |
| watchdog | 43454 | 1c8c5c617fd379965041eb4d4d54f0bdb1eee0287d2b05e797ed2e612a283166 |
| production | 71277 | 12661c746bfad302d5dabf0acdb5ea621005f5650120300a5e971d7507e6d976 |
| output | 21721 | 331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98 |
| root_native | 25596 | a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382 |
| deadline | 6038 | 0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45 |
| shell | 301368 | 362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139 |
| allocation | 1555 | 2a7b5b2f449b7b24913b714ebcf51283ee5efe818c773298f7f80a8172d38770 |

All seven source SHA256 values agree across the start marker, worker before/after, supervisor after, and independent post-return pin read. Allocation agrees with the dispatch and marker. An independent comparison of retained public results completed **73 assertions**; it invoked no project functions.

## Actual observations

Supervisor PID **37428**, native start **2026-10-31T08:53:41.2759406Z**; worker PID **34496**, start **2026-10-31T08:53:41.8664835Z**, retained handle **2288**. Common start QPC **2869185051773**, frequency **10000000**. Both controllers report `observed`, with no failure or cleanup cause.

One synthetic parent supported separate create/existing output children. Create: guards peak 5, open attempts 6 (including missing-parent probe), native close attempts/closed **5/5**. Existing: peak 5, opens 5, closes **5/5**. Both report guard remaining 0, no unknown/native error; owner ancestry/prepared/after handoff **5/6/6**, remaining 0, no unknown/native error, disposed. This is guard-to-owner handoff only: owner transferred false, receipt mask/transfers 0; no TakeDirectory/Core handoff. Aggregate native guard closes **10**, owner lifecycles **2**, explicit helper peak bound **11**, combined controlled bound **32**.

ACL before/after and between cases agrees: `45fe1be9f04c0ec6d95da3a47dfb85a7adde66fb0a9fb508a20882106fe0a46a`. These are controller observations; no fixture/ACL path was read after return.

Three accepted mappings report byte-identical inverse SHA256: parent guards source/inverse `1f65b9908b6ddbbcba16d52a94a937a9152dc3e6ce842b48b74b42d4ba3fb51a`, mapped `78abfa86692ae497cd3dd3885aed91b884140b9e7d0fe87fb04ac771120e9552` (namespace/path replacements 0/1). Output source/inverse `054cedbea10f768807b9e87ae94122e73bfd30e0f278195d3545ce196e566fe4`; create mapped `4b3f09e26801c495b02adf181255cfce00a8a06a4ec846d378441ac6ac22702f`, existing mapped `f09eae07e0d7c0bd647f93906e726d5be6a622647e29b87a33acd7a773fef48b` (each 2/1). Fixed synthetic targets are recorded in cases.json.

Both protocol cases used the actual **26 definitions / 21095 bytes**, definitions SHA256 `ea95c30fa17ba918391708d70aa83a3c887426546f9f76d6a1e897b0ceb2a878`. Matching retained 703-byte record returned true, null cause, and the matching raw started SHA256. Wrong-PID retained 703-byte record used parent PID **1**, returned false, null started SHA256, `System.InvalidOperationException`, HResult **-2146233079**, native_error 0, mark `cold_launch_code=schema_value` (one-element same-type/HResult chain). Matching parent/supervisor identity and common clock agree with controllers.

Each protocol case reports public peak 1, remaining 0 and managed disposal; **managed native close was not observed**. No claim/private work occurred. The records' 45/60 fields and permission_consumed are validator fixtures, not execution grants; the host enforced 20/30. These are function tests, not full Parent/job/claim/private end-to-end or concurrent publication tests.

Initialization occurred once for each of five helpers; diagnostics empty, compiler_error/diagnostic_cap false. Worker/public parent inputs closed **9/11** respectively. Worker self disposed/job finished/owned children 0; parent stdio disposed and registered child released. Fixture retained; Core type absent; capture/inventory/receipt/private access false.

## Completion and resource evidence

Child exit 0, reaped normally; stdout/stderr empty and both EOF; no output cap, forced action, cleanup kill or unknown. Normal waits **139 = 138 false + 1 true**, cleanup waits **0**. The successful 20 ms wait is explicit evidence, separate from HasExited/EOF.

QPC: both EOF **2869239891292**; exit indication **2869239902650**; successful reap **2869239960944**; normal gate **2869239977354**; pre-cleanup gate **2869241463382**. Both gates passed without first failure; all were before the common 20 s deadline.

Observer: **43 attempts/43 samples**, no race/failure; sampled OS handle peak **611** (cap 1024), observed lifetime peak working set **230563840 B** (cap 1 GiB), attempt cap 301. Maximum successful sample gap **0.1377627 s**, measurement-gap upper bound **0.1487185 s**, maximum sample duration **0.0221042 s**, total sample duration **0.3816050000000001 s**. Exit tail **0.0743067 s**, upper **0.0825383 s** remained unobserved. These support sampled bounds, not an exact OS handle peak or a hard RAM limit.

## Boundary and handoff

After return, reads were limited to the five public reports, public source/allocation pins, and native app execution metadata; this record is the only subsequent authored artifact. No private/fixture inspection, new runtime, source edit, repair, navigation update, ledger change, commit or push occurred. No cap-exhaustion test or full production bootstrap/comparison is established. D1/private roles remain inactive; whole S5.3 remains open. Root actual-return review is the next gate; executor stops here.

