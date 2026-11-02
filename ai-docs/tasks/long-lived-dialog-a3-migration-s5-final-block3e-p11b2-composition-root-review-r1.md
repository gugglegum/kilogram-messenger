# P11B2 review R1 — preserve historical pending observations

2026-11-02. Baseline `7beba8264464550bc865596308d889ccb8af48d9`; candidate manifest `.tmp/s54-block3e-p11b2-01/handoff-manifest.json` SHA256 `6dae42325834c1ed33c3ed6505c167eb06b429744cc3ea0fa14601b5fadddab1`. Implementation is not accepted for publication. One concrete blocking adapter defect found; no request for a new contract or broad redesign.

## R1: historical saved_pending is not current pending debt

`migration_s54_final_live.rs:119` requires `run.decoder.pending_rows.is_empty()` on both original P10 lifetimes and the new restart. This map is an append-only observation history, not the current pending-store contents. `migration_s54_pause_observations.rs:47–82` inserts saved_pending rows and never removes them when materialization succeeds. `runtime7/data2.rs:347–350` emits such an observation when the original B640 is committed as Pending; materialization occurs later. The original P10 live adapter explicitly permits the retained exact A/C1/peer-role1/ordinal640/EventID witness through Stage::Final (`migration_s54_relay_live.rs:151` onward).

Consequently a valid P10B completion retains its B640 pending observation even though fresh Stats and final source show no pending work. The first P11 `live::sample` rejects that valid continuation before Cut1. Existing new tests check counts/packet equality but do not exercise this actual decoder-history seam.

Fix only this distinction:

- At P11 entry validate and retain the actual original pending observation history against the retained original B640 commitment and correct executing owner/context/role/ordinal/source/incarnation. Use actual captured decoder facts, not expected manufactured rows. No other historical pending source is allowed in these fresh P10 relay lifetimes; preserve the P10 packet/count/identity checks.
- During the original-lifetime P11 observations require that this validated history remains unchanged. Keep fresh Stats/source/terminal checks enforcing zero current pending/debt/head; do not use historical map emptiness for that claim.
- For genuinely new zero-work restart lifetimes require no pending observations as well as zero current obligations/counters/packets. Do not clear/rewrite the decoder history or weaken earlier P10 predicates to satisfy P11.
- If actual adapter semantics establish a stricter exact B640 membership requirement, derive it from the real capture path. Do not infer a new success from a missing/unchecked history entry.

Regression: feed actual existing Decoder start -> saved_pending(B640) -> native_commit(materialized) envelopes and prove the historical entry survives. The extracted production P11 history predicate must accept the correct retained old-lifetime witness with zero current pending state, reject foreign EventID/context/role/owner, changed/extra observation after entry, and reject that same history in a fresh zero-work restart. Keep live current pending>0 rejection in the reused status predicate. No actual messenger or private fixture is required. This tests the concrete seam rather than a synthetic assumption that the map is empty after delivery.

## Review status and next action

Root read the composition driver, actual adapters, reader cut-local binding, final/relay/memory ordering and tests. The post-memory-seal restriction is handled correctly; no additional blocking finding recorded in this pass. Final comprehensive artifact/test receipts will be verified against the corrected source, avoiding repeated full evidence audit of an already rejected candidate. The claimed25 passing tests are not approval of the incorrect pending-history premise.

Preserve this candidate and its failed/pre-test evidence. Implement the narrow fix, add meaningful regressions, and run format -> strict Clippy -> compile-only -> exact affected stable tests with existing jobs2/offline/locked/watchdogs. Retain prior unaffected evidence accurately and pin corrected final source/image. Return updated readiness/review/exact proposal. No commit/push or B3/whole activation. Continue High/noFast; ordinary fix/check iterations do not need intermediate callbacks. Callback root then end without polling.
