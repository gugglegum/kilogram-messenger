# A3-S5.2 first-long readiness review 2 — origin go-ahead

2026-10-26. Root reviewed the controller correction requested in
[readiness review 1](long-lived-dialog-a3-migration-s5-l1-long-readiness-review-1.md).
The user's standing authorization covers this review and the assigned test stage.

## Readiness accepted; one origin authorized

Accept the concrete revision-2 readiness for one genuine frozen A511/B16 origin.
This is a source/static readiness decision, not acceptance of an executed origin,
boundary test, complete S5.2 implementation, performance claim or publication.
The origin requires ordinary native execution approval in addition to the
separate root go-ahead JSON. Stop and callback after this one attempt, including
on failure or cutoff. Do not launch boundary or another origin automatically.

Accepted revision:

- Readiness revision2, 7007 bytes, SHA256
  `2e844d34c089e55be17ad84a48cfafef8dcd51c8400c9d167abc493dd4867876`.
- S-long-readiness-revision-2-binding.json, 23657 bytes, SHA256
  `4116a5c3fc17ffb0c8b2efca0e71db98a4f22ff42ff091de3eb73ac94b0a3f67`.
- Exhausted controller, 26779 bytes, SHA256
  `a8077b49ba2e02956a94ca352158190854f47d3a6b37b2e624b2bb81743e71f4`.
- Stable migration_s5_process harness, 13683712 bytes, SHA256
  `5ccc3a8a3e4bc76ed811f6d0aad5c93b95381bbab8d6a2c1284b3d66c8eb5ce2`.
- Runner, 23612 bytes, SHA256
  `c9c6841213181af75f3364128cf7d0697d45db5a7a8bf5bfda1b58986c3211ed`.
- Current ledger, 363751 bytes, SHA256
  `5218356ef3d08914f1ce1c839da558a519a6ff33455f2635b7345f36a7b725c5`.

## Correction and static verification

Root inspected the actual controller delta and relevant runtime sequencing.
The final A log must be new. Context-scoped trace_rpc changes diagnostic output
only. A single bounded wait requires both actual processes alive, current leases,
fresh admissions and cross-matched current Locator revisions. It observes a
later exact successful A Resume completion while the lease/admission token is
stable. A's cached bootstrap handshake precedes its Resume worker, closing the
previous premature-zero-replay gap. Fresh post-barrier status rechecks the same
pair/token, frontiers/debt/queue, Session and zero packet/constructor counters.
The future result includes the concrete log snapshot and public barrier evidence.
This does not claim that the field reconnect has already happened.

Root verified the bound current inputs, including all 52 compile inputs, all 33
ordinary production inputs, all eight fixed/archived images, the new stable
harness, authority, static records, runner/deadline helper and ledger. Original
readiness/binding and both preserved controller/harness snapshots match their
original hashes. Only the exhausted controller changed among compiled inputs.
Clippy23, compile19 and fmt25 records/logs report successful static checks under
the existing locked/offline Jobs12 verification workflow. git diff --check passes
apart from the existing informational LF/CRLF notice. No product rebuild or test
execution is inferred from these static checks.

HEAD remains 73c499818ba9b699c331aa842e64e8432e90c59a; index is empty.
Ledger sums independently reproduce C147.8846192 and F1444.3918456, with P0.
Origin directory, started markers and origin contributors are absent at review.
Earlier Ready02/capacity01 whole FAIL records retain their limited acceptance;
neither record is relabelled or refunded.

## Exact authorized attempt and stop boundary

Run only:

```powershell
& scripts/invoke-migration-s5-l1-case.ps1 `
  -Case F-exhausted-origin-01 -Set F -Action ExhaustedOrigin `
  -LongApprovalFile ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-origin-01-root-go-ahead.json `
  -ExecutionMode NativeToken
```

Use the ordinary require_escalated execution route for local networking; this
document is not permission to bypass an approval rejection. Keep BelowNormal
priority and the reviewed non-breakaway job/watchdog mechanism. Origin work850 /
whole900 includes its setup, actual traffic/history/refusal/retry checks and
cleanup. The current F ceiling2900 and remaining1455.6081544 are unchanged;
the required long-entry reserve1215 is available. No C/F allocation increase,
refund, P borrowing or automatic retry is granted.

Both producers must be the fixed frozen d9b image and the output the fixed
F-exhausted-origin-01 directory. Preserve the real stopped original and inventory;
never typed-open it afterward. Preserve all failed attempts and the disclosed
terminal-publication timing boundary. Source-bound Rust/PowerShell inputs and
all runtime images must remain unchanged during the contributing attempt; do
not prepare or restage another controller in parallel.

After completion send the actual case/log/result/provenance hashes, whole verdict,
deadline/cleanup outcome, exact counters/history/refusal evidence, updated ledger
and remaining allowance. Root will review the real result before issuing a
separate boundary approval bound to the then-current ledger and origin case.
No whole-P, commit/push, cut3 or S5.3 implementation is authorized by this decision.
