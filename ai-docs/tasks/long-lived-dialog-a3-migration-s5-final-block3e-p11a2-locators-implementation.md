# P11A2: authenticated same-route restart locator seam

2026-11-02. Baseline `5fef1926d532d2a5b926739d71af7680818550e8`, parent `f70aec67a304354e6130198d48020b448951376e`, tree `287d3ce335ae77c88f857827d7136462622bd23c`. Task6281B/SHA256 `89b649d4ef316c8e2344f6215c5141fed768fb9bb52ab45a6d759a7bdea08306`; accepted P11 contract/root grouping unchanged.

Dormant test support adds an explicit same-route locator verification/read seam for future restart composition. **No actual owner/restart/publication/import/relay/network/state/private-fixture/enrollment/F/P/field activity, phase composition, commit or push occurred.** Signed synthetic tests verify predicates only; real current admission/liveness/publication behavior remains unexecuted.

## Exact change and invariants

Only two source paths/raw25704B, both under `apps/kilogram-cli/tests/support/`. Full normalized Git blob pins are in executor-review.json.

| File | Bytes | SHA256 |
| --- | --- | --- |
| migration_s54_relay_locator_tests.rs | 17976 | 235e198b1ce1ae7df1ba9eaf0ab3ca56c1229728f2adaa831d50684fff3ebce1 |
| migration_s54_relay_locators.rs | 7728 | 621f686714508e6f2c2be0a6e9268fcb39ef613341679b7886a4c5ddbb9d41f2 |

`verify_same_route(raw,context,peers,owner,now,url,prior)` independently authenticates the prior signed bytes under the expected original context/owner/peer and exact canonical owned relay URL/no IPs. Its cached revision and endpoint must match those signed bytes. Thus a foreign row, wrong route, corrupt signature, oversized record or independently altered metadata cannot provide an unchecked comparison number.

Current bytes pass the same bounded authentication/route logic and existing current interval/revision verifier:8192-byte maximum, owner<2, actual protocol parse/signature/cert/peer/context verification, canonical exact URL-only relay, positive revision and not_before<=now<expires. Lower revision fails. Equal revision requires byte-identical signed content, including endpoint/interval/signature. Greater revision accepts valid same-route content with either the same endpoint or a genuinely changed endpoint. No endpoint rotation requirement and no new60s verifier headroom.

Prior is authenticated historical evidence of this run's preceding cut, not a new current-time publication. It may expire while owners stop. A valid newer current replacement is accepted without pretending old bytes remain current; equal-revision identical reuse still fails if those same current bytes have expired. Publisher's endpoint/address/relay match and expiry>publication_wall+60 reuse decision remains solely in unchanged runtime7/admission2.rs. Tests cover valid current bytes with one or ten seconds remaining rather than imposing that earlier publisher margin at observation time.

Shared `authenticated` factors the existing signature/bound/route predicates. Original P10 `verify` signature and strict optional-prior rule remain: greater revision and changed bytes required. Its current interval checks remain. `read` still explicitly selects Strict; new `read_same_route` requires both an explicit URL and complete prior Publications and selects private SameRoute. There is no permissive default or global replacement.

Both reads share the existing four-publication traversal and original public existing-origin scope/account/device/role checks, Ready-derived views and8192-byte file reads. Each prior is selected by exact owner/context indices before authentication. Each owner's two current endpoints must still agree, while prior/current endpoint equality is not required. Wall time is now sampled immediately for each record's verification instead of once before the whole traversal; the original Controller deadline/liveness checks remain before reads, with a final check after all four. No Controller/phase/ownership/deadline API changes.

The actual admission predicate is unchanged: Active/current policy, physical slot, lease/family2, publication/origin readiness/errors, authenticated own/imported revisions, current decoded Head generation and selected relay path remain independent gates. Authenticated same-route bytes alone do not authorize release or establish a live path. No product/runtime/protocol/crypto/storage writer changes or identity framework were introduced.

## Actual checks

Immutable raw base `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3e-p11a2-01/`. Nine preimages (seven accepted EXEs/two source files) were preserved before edits and hash-verified. One final source revision was compiled and reused. No failed Clippy/compile/test or overwritten receipt exists.

| Actual check | Child exit | Seconds |
| --- | --- | --- |
| clippy-01 | 0 | 26.5101677 |
| compile-01 | 0 | 98.4662964 |
| pure-01 | 0 | 15.2326012 |
| fmt-01 | 0 | 4.721426 |

Strict CLI/all-target Clippy uses literal offline/locked arguments and `-- -D warnings`. Compile-only and pure run use scripts/run-cargo-tests-stable.ps1, Jobs2/BelowNormal, verification-scale/x64 and fixed EXE paths. Native watchdogs are Clippy590/600, compile1190/1200 and pure/fmt230/240 seconds. No direct hash-named Rust test execution or broad decoder/lifecycle/G3/P7/P8/P9 replay.

Targeted run:10 passed/0 failed/0 ignored =6 new signed-fixture tests plus4 existing P10 locator/admission tests. Exact full names are in test-selection.json/pure.stdout.log. New cases invoke actual shared/new verifiers with the existing protocol signing/root-credential fixtures: unchanged bytes/revision for both contexts and both owners; newer unchanged/rotated endpoint; lower/equal signed endpoint/interval conflict; valid newer replacement of expired prior while expired-equal/future/expired current fail; current signature/context/owner/peer/URL/IP/bounds; prior signature/foreign context/owner/peer/URL/bounds and forged revision/endpoint metadata. Existing strict route-change and actual-admission negatives remain selected. No separate comparison model or executable enrollment.

All501 compiler units and571 source-input hashes match before/after compile/pure; all501 units fresh during pure and independent frozen guard true. Current input/binary hashes, empty index and preserved navigation pins were verified. Scope/account/device/four-read filesystem behavior is source-reviewed and dormant; the pure cases do not claim a real read/restart/import.

Normal non-test CLI remains50473984B/`f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`; other five stable EXEs/four field images unchanged. Migration harness17138688B/`7955960b4ff265c7402951d94e3a5916ce86e60639d7fc324cb252f8b4f44cbd`. Exact review17718B/SHA256 `f03a55e4993a3e10da4d4cd8af2550cb7ad60b1801ee3ca7d1909d0bf70a84a2`.

Durable `long-lived-dialog-a3-migration-s5-final-block3e-p11a2-evidence/` contains actual receipts/logs, compiler-input hashes, exact review/provenance and reproduction helpers. Full raw captures/independent expected manifest and nine EXE/source preimages remain at the immutable base. Reproduce in a fresh directory with updated helper paths: compile without Run, derive an independent frozen manifest from compile-01.capture.json, then execute only the selected stable pure checks. Original review.py audits this historical immutable attempt.

## Review boundary

Navigation3/root Current paragraph and historical files were not edited or included. Index empty; no commit/push. No actual runtime feasibility/current publication/admission result. Full terminal reader, whole restart/final composition, memory observation, sanitized capture, final accounting/result and runner/admission remain mandatory later work. Ready for root source/static/pure review; stop at P11A2.
