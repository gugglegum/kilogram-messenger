# S5.4 block2 root review 01 — changes requested

2026-11-01. Baseline `8bac8da7984ab12026e9b2aeab4bcae9a90c02cc`; index empty. This is source/static/pure review, not execution of P0–P6. No publication or block3 activation is accepted yet.

Reviewed the 12 submitted Rust paths, their affected production/helper call paths, the implementation report, and actual executor command results. Submission: `.tmp/s54-block2-01/review.json`, 28299 bytes, SHA256 `76978d655bb00ad6972b28bc5c7f3516ab7f8af4f7670eae11cee194eb4d3662`; report SHA256 `98f22ce0277d4559513274855edd039de3627359207b8ff1f709e070b9ef41b7`.

## Verified checks

Root command `9a6aa5` independently verified 57 pinned source/evidence/artifact files, all 535 compiler source inputs against current bytes, identical compile/run before/after inputs, 501 compiler units with all run units fresh, frozen guard success and exactly the named 35 passing tests. HEAD and empty index agree. Actual executor app items corroborate successful strict Clippy (`exec-dd3c6e30-0f1f-4f3e-bf8e-852b7525877e`), compile-only (`exec-a05e0d47-38af-4e6d-83fa-616cc07fe701`), formatting (`exec-330647c2-5e3c-4227-ac70-2ef44371a9bd`) and stable pure run (`exec-826dab79-4be1-4f76-8cb5-6fe779a29119`). Earlier failed compilation remains failed, preserved and excluded. No need to repeat unchanged checks just to reconfirm this review.

## R1 — P4 old-tail repair has no available delivery path on the pinned connection

Blocking source-level liveness issue, not an observed product regression. `migration_s54_transition.rs:201–219` waits for pending9 including real B25, then clears the B16 hold and A prefix suspension, expecting B25 readable while the same Selected/Active lease generations and withheld Committed remain.

However, `runtime7/network2.rs:537–541` has A's Selected worker awaiting saved K / Committed. That worker cannot reach its prefix request at line607 until Committed returns. Independently B's worker has already successfully sent B25 to satisfy pending9. `protocol2.rs:663` skips held B16, `:691` sets the outgoing next position after the last included source, and `network2.rs:618–629` retains that next position. After B25 it is at least26. Clearing hold16 does not rewind it; peer receive frontier16 is combined with that cursor by max. B has no new source after25 at this point. Its prefix requests address A's lane, not its own missing B16. A cannot drain B25 without B16.

Waiting longer does not repair this graph. Eventually the existing RPC15 can force a reconnect, but the scenario requires an unchanged current lease bracket and a bounded10s early interval, so that is not a valid completion path. The earlier PREQ controller repaired the old tail before releasing K; it does not prove the new sequence that first requires B25 pending behind that tail. The accepted S54 plan inherited this feasibility assumption; correct the assumption explicitly rather than weakening the assertion silently.

Required: establish an actual path for replay of the original B16 source after B25 is observed pending, preserving original sources/IDs, real pending-to-readable transition, Selected receive-only A, saved controls and the stated current-lease brackets. Reuse supported scheduling/lifecycle capabilities if possible. Do not change normal protocol behavior, manufacture packets/receipts/frontiers, remove pending9/readability assertions, or accept timeout-induced reconnect as success. If satisfying this requires a new debug scheduling capability or a change in the declared lifecycle/lease bracket, submit a small concrete contract addendum and exact proposed call-site before implementing that change. Root will review that bounded choice; no new general planning round is needed.

## R2 — the early deadline begins after the RPC whose timeout it is meant to respect

`migration_s54_transition.rs:169–191` imports fresh locators, waits up to15s for the Committed publication marker and gathers status/facts before opening a new10s early Phase. A's K RPC is already outstanding when the marker is written. Giving the subsequent interval10s does not ensure it fits the remaining RPC15. The prior PREQ implementation starts its absolute early bound before the command releasing K.

Required: place one absolute bounded interval at the relevant causal trigger, including trigger/import, observation, B25/pending/repair and prompt release, or use another source-justified ordering with the same original RPC timeout. Do not renew the phase at the marker or extend production timers. Address this together with R1 so the repaired graph has an explicit timing boundary.

## Next ownership and scope

Executor owns resolving R1/R2 after successful dispatch. First inspect the existing capabilities and choose the smallest correct graph. If an existing-capability correction preserves the accepted contract, implement it and run only affected static/pure checks. If a contract or debug-hook change is necessary, return the narrowly scoped addendum for root review before implementing it. Preserve this submission and its failed/successful evidence; use a fresh numbered evidence location for changed bytes. No real enrollment/product/F/P runs, no commit/push, and no block3 until acceptance. Keep High/noFast, ordinary autonomous source repairs and generous finite command watchdogs. Send one substantive callback and then stop; no polling.
