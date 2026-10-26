# A3-S5.2 instrumented boundary02 — readiness accepted

2026-10-26. Root source/policy review of the static preparation requested by
[boundary continuation review 1](long-lived-dialog-a3-migration-s5-l1-boundary-continuation-review-1.md).
The existing human authorization covers this technical review and one continuation.

## Accepted exact preparation

- boundary02-readiness.md12755B SHA256
  `0d626122b5a4fcd3b4b3fb4ee7cb9549fa53b2e1972ac1ce95e8a98bb6bc3bc0`.
- S-boundary02-readiness-binding.json35119B SHA256
  `fdb1682015099e26bcaee3e710413dbb8f62677fc30a74f5eaf78a4727addc68`.
- Exhausted controller40620B SHA256
  `dd3a4b9e6c76daa1056b1bb3d765bcb251c54092ac14f84db3178cc7c5c510bd`.
- Runner24964B SHA256
  `22dcaa81179e39986199664adea130c01c22749b3ea69a34664a60b2d66ca79e`.
- Stable migration_s5_process harness13935104B SHA256
  `5a003864af885c09d0274d84f54b2000a774908382169085b42314ee546ec161`.
- Ledger390894B SHA256
  `b9dad4528aecff9a7084492a52e2f411f67cba89cc1af4cc3ea8196cf9bd1462`.

Root independently verified all142 current bound references, including the52
compile inputs,33 unchanged product inputs,eight fixed/archived product images,
static records/logs and protected historical artifacts/snapshots. The two
documented historical before-hashes at the original source paths are superseded
versions, not current-file expectations; their preserved snapshots match.
Comparison with compile19 confirms exactly the controller and runner changed.
HEAD remains73c499818ba9b699c331aa842e64e8432e90c59a; index is empty.

Compile20 log shows compiled-only through the fixed-path wrapper; Clippy24 and
fmt27 report successful static checks. The changed PowerShell runner independently
parses with zero errors. git diff --check passes with only the existing LF/CRLF
notice. There was no product build/restage or contributing execution in preparation.

Root and the independent read-only controller reviewer found no blocking issue
in the observed-migration instrumentation. Each exact hook hit is recorded and
removed before complete atomic control replacement. On errors the owned child
is killed/waited before control release; declaration order and OwnedChild provide
the unwind fallback. The560s controller deadline is absolute and does not renew
per hook. The final reconnect25s barrier, all prior assertions, normal migration
arguments/Node transitions and genuine fresh-copy lineage are retained.

These are static review findings, not a field PASS. Blocking operations still
require the independent outer watchdog. Controlled pauses and synchronized logs
contribute to measured time. Missing fence remains a coarse interval, not proof
of one internal bottleneck. No validation removal, optimization, performance
improvement or corruption diagnosis is accepted by this decision.

## One instrumented attempt authorized

Use the separate root-created approval file and ordinary native-token tool
approval for exactly:

```powershell
& scripts/invoke-migration-s5-l1-case.ps1 `
  -Case F-exhausted-boundary-02 -Set F -Action Tests -Package kilogram-cli `
  -Filter migration_s5_l1_genuine_exhausted512_refusal_receive_history_checkpoint `
  -FixtureDirectory E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-l1/F-exhausted-origin-01 `
  -LongApprovalFile ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-boundary-02-root-go-ahead.json `
  -ExecutionMode NativeToken
```

Work580/whole600, Jobs12/BelowNormal, existing non-breakaway job cleanup and fixed
EXE paths remain mandatory. No Fast. Keep all source/script/image inputs frozen
through the contributing attempt; no concurrent builds or future-controller work.
Use the new isolated branch from the same immutable origin, with one active copy
per Native domain. Do not open or repair the failed01 branch, typed-open the
original, regenerate the origin or retarget a pinned profile/checkpoint.

The approval binds the current runner/ledger and actual origin case SHA256
e1acc55360a45c386dde029c2f9be35cbbdacc08bd2a076a1812ac6c692d5135.
Runner checks the retained failed01 case072b5228... and logb7b5efa0... as well.
Their120.1641445s cutoff remains whole FAIL, with no partial functional acceptance.
The origin481.4608671s whole PASS and earlier Ready02/capacity01 split outcomes
also remain unchanged. Neither historical attempt is overwritten or refunded.

Current C147.8846192/F2046.0168572/P0 under ceilings500/2900/0 leaves853.9831428F.
The entry reserve825 is met. Even the whole600 bound leaves253.9831428, above225
contingency. This is an allowance within the existing ceiling, not a prediction,
refund, P borrowing or permission for another attempt.

After any failure or cutoff, stop and callback with exact immutable evidence,
last completed/open phase, captured migration stdout/stderr, checkpoint hits,
child cleanup and updated accounting. No automatic retry or speculative product
change. After PASS, finish the previously assigned implementation report and
exact candidate allowlist without further C/F execution or source changes, then
return for full implementation review. Include original/checkpoint/barrier/phase
provenance, current static/source/image binding, all failed attempts and honest
cut3/whole-P/MVP limits. Root-owned S5.3 handoff remains excluded from S5.2 release.

No commit/push, whole-P, product optimization or cut3 is authorized yet. Publication
requires separate explicit acceptance of the final implementation candidate.
