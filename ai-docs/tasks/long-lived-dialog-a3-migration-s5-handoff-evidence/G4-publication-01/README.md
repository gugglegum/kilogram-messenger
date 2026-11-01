# G4 accepted fresh-path regressions

2026-11-01. Root accepts the seven G4 preparation/runner script changes and both existing fresh runtime regressions against baseline `8b3237a593c529d55af9a556fa44ad3f7a704a7c`. Production Rust and the existing test assertions were unchanged. This closes row 14 / G4 of the S5.3 completion matrix; whole-S5.3 acceptance awaits final reconciliation and verified publication.

| Scenario | Observed result | Actual final / outer elapsed |
| --- | --- | --- |
| Fresh two-process offline/restart dialogue | 24 text, 4 genesis, 8 offline text; both nodes restart; readable histories; exact native marker and named PASS | `857d5b` exit 0 / 49.5605646 s |
| Escaped IPC history pagination | Six 4096-byte NUL texts, 49152-byte JSON page cap, six pages per peer, exact request coverage without skip or duplicate | `d5e502` exit 0 / 29.0309233 s |

Root inspected completed app execution records `exec-b5cc77b1-4605-468c-87d7-08887c4bf280` and `exec-2e9a11b9-3494-4984-865a-3b6e10054c99`, native logs and case/outer records. Root check `f72b3c` revalidated the pinned bindings, all current source/image/dependency tuples, evidence hashes, success counters, inventory admission and cleanup. Root process observation `fe0063` found none of the six known Node PIDs or any kilogram-* process. Short-lived descendants are covered by non-breakaway job cleanup, not individual PID observations.

[Runtime review](runtime-root-review.json), [preparation review](preparation-root-review.json), [source and evidence mapping](source-and-evidence.json), [smoke log](smoke.log), [history log](history.log), [successor gate](successor-gate.json) and [PID cleanup](pid-cleanup.json) retain scoped evidence. Preparation passed 25 G4 policy/inventory checks and 12 existing policy checks. The memory-shell probe confirms actual executable resolution and inherited job membership. The compile-only log precedes freeze; the runtime logs confirm frozen verification.

The two existing tests use native markers and logs, with `assertion_envelope=false`; null assertions SHA is expected. Their unchanged assertions establish history/message content; root did not independently reopen private stores. Initial smoke child exits 1 are intentional forced stops, followed by successful reopened nodes and final exit 0. Observed memory samples are not unsampled peaks. Synchronous CLI/memory calls are bounded by independent case/outer jobs, not solely the asynchronous test timeout. No stress, arbitrary-network, physical power-loss or universal latency guarantee is inferred.

All published copies are exact bytes, including original local paths; source-and-evidence distinguishes public copies from local-only inventories, compiler captures, executables and fixtures. No private keys, account files or stores are bundled. Artifact path references inside original records are provenance, not an assertion that the original local directories exist in a fresh checkout.

Accepted G4 F time is **78.5914879 s**, charged once here: S5.3 F **2845.3231047 s**, combined F **7561.6432536 s**. Test, compiler, app-tool and nested durations are not added again. Earlier failed attempts elsewhere remain failed and immutable. No new full suite or S5.4 run was performed.
