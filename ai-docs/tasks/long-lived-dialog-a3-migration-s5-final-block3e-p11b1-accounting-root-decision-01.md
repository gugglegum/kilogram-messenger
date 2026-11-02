# P11B1 contract review and B1a implementation assignment

2026-11-02. Root reviewed contract SHA256 `3370b4e7dfbe08367ec321986e9d1a9396c9014e2d56439a6c3df9f87bef507a` (17036 bytes), baseline `b2e1d7016ee7b23a9a646e41cbe15d0f5fba7398`, existing source endpoint, decoder closure and Operations reconciliation. HEAD matches; index empty; tracked differences only the three existing navigation documents. Contract accepted with the mandatory clarifications below. This accepts the design, not an implementation or whole-run result.

## Required clarifications

1. Cut2 must identify two genuinely new owned lifetimes with fresh nonces/full Incarnation keys, observed start and later normal closure, one per original owner. Windows may reuse a PID after exit. Same PID with a new nonce and established new lifetime is valid; a reused nonce/old lifetime is not. Remove the requirement for a distinct PID. Include both regression cases.
2. Interval endpoints are committed materialized `snapshot.meta.next` lanes from the existing source reads, mapped through authenticated context/role/Ready identity. Physical stored Event counts and pending unreadable sources are not endpoints. Capture both lanes/context in the same existing openings; derive aggregate values using checked arithmetic. Pending source without materialization must not advance an interval. Genesis remains outside Text-only endpoints.
3. `Operations::reconcile_final` currently adds one reconciliation entry with stopped root generation/last_commit when a killed stream has an unlogged successful tail. Its summary operation set therefore is not a set solely of observed native packets. Preserve existing closure semantics. Retain typed provenance for the observed decoder prefix versus the source-reconciled entry/root boundary, including observed versus closed sequence. Never label the latter as an observed native_commit or infer a per-EventID packet ID. Reconcile exactly once. Reject unexplained summary membership, negative per-context tails or totals that cannot be allocated to actual committed lane ranges. Normal closure admits no such synthetic tail.
4. Existing validated live observation envelopes and typed actual waited outcomes are the evidence. Fixtures are explicitly supplied models; they do not establish actual integrated-workload provenance. Preserve the original P10 decrypt's owner/lifetime. Whole-run acceptance remains gated.

## Implement B1a now: retain and validate current interval evidence

Split at a useful review boundary rather than implementing all proposed adapters together. B1a covers only the bounded current-lifetime input to accounting:

- Retain per-context/per-lane start and stopped endpoints and source identity through existing source reads, without an additional database traversal.
- Accumulate checked per-context directional native work and real observation boundaries in the existing decoder. Keep actual observation provenance separate from the existing killed reconciliation entry.
- Retain typed current closed intervals/outcomes in the existing controller/normal closure paths before consuming their pending state. Maximum32 lifetimes; exact owner/source/Incarnation, endpoints, counts, sequences, outcome kind and actual waited closure. Frozen starts are classified from their actual owned format; no caller-supplied bypass flag.
- Provide a small validation API for these retained intervals against the existing closed Operations summary: bijective stream/outcome coverage, contiguous owner/context lane intervals from migration start, bounded and checked capacities, observed prefix plus explicitly reconciled tail, zero constructors/aborts/ambiguity. Use actual frozen/current phase distinctions and zero-work intervals correctly. Do not require the terminal tap merely to validate this input layer.
- If a proposed contiguity or binding assumption fails against existing actual start/stop order, identify that precise seam and adjust the narrow adapter with evidence; do not manufacture expected frontiers or silently relax validation.

Likely scope: source, observations, control, normal, a focused interval/accounting input module, tests and necessary module wiring. Adapt supporting call sites only where these type changes require it. No product/protocol changes, new observation framework, lifecycle redesign, additional key access, workload changes, deadline changes or whole execution.

B1b remains next: typed old/genesis/overlap/B25 witnesses, terminal record tap, full directional union and Cut1/Cut2 comparison. Keep the reviewed parent contract for B1b; no additional contract-only round is required unless implementation reveals a substantive design conflict. Do not implement B1b in this assignment. PID-reuse acceptance belongs in B1a identity tests and later Cut2 tests as applicable.

## Validation and delivery

Before running tests verify their premises: actual context/role mapping; committed next rather than stored pending entries; mixed-direction batches; zero-work lifetimes; killed reconciliation's extra sequence/operation membership; actual normal final/EOF semantics. Focused tests must cover positive bounded intervals and refusals for missing/duplicate/foreign/open outcomes, owner/context/nonce mismatch, overlapping/gapped or regressing ranges, incorrect direction/tail allocation, normal fabricated tail and a root control commit mislabeled as an observed packet. Include PID reuse with fresh nonce, nonce reuse rejection, and pending-but-unmaterialized source where the existing fixture supports it. Retain affected existing decoder/normal/summary regressions. Avoid a duplicate full-workload fixture for B1a.

Use established format -> strict Clippy -> compile-only -> exact stable-runner sequence on Windows, offline/locked, jobs2/BelowNormal and per-command watchdogs. No direct network cargo test. Preserve failed evidence, distinguish old and final build images, rerun only checks invalidated by changes. Ordinary local fix/check cycles have no tiny aggregate time budget; complete the coherent assignment before returning. Escalate only an actual design/authorization blocker or a completed reviewable result.

Submit source diff, exact check commands/results, test scope and limitations, immutable evidence and proposed publication allowlist. Do not commit/push before root implementation acceptance. Preserve unrelated navigation/history files. Callback to root `01a164ec-589c-7b41-bc34-3ba158d1f22b`, then end without polling. The user's standing authorization covers this callback and accepted publication.

## Continuation rule

After compaction consult the current handoff and latest actual callback before answering historical questions. Root owns a submitted review until a concrete acceptance/fix/next-task message is successfully sent. Executor owns the implementation after successful dispatch. End the turn after dispatch, with no polling loop. Historical promises or an old final response are not evidence that a handoff occurred.
