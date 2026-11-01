# P10A — same-owner C2 handoff, inactive implementation

2026-11-02. Implemented on accepted P9B `5006de9c6aafcbf3fab362d248b14eb9c167f190`, tree `981bbca9ee56c40d9777575e3bb6d3b7f48e0968`. Task6987B/SHA256 `b67f57c33ec2398792878f706cfde54719f4a8a5b7eee7fcd469416b121ebfa2`; accepted contract14515B/`ff76125edb7251a774acf6fc5708a891247abfc1c286152507dbfea29f193f11` is unchanged.

The inactive `handoff::p10a(catchup::Continuation)` returns typed owned continuation with both original contexts Active/DirectOnly, C2 cuts33/33, unchanged C1/B640 and observed zero packet/constructor deltas. Phase.next remains10; no executable partial PASS. **No real C2 transition/scenario, relay, profile route mutation, B640 release, enrollment/private fixture/F/P/field action, P10B/P11 or commit/push was performed.** Descriptions of future runtime observations below explain enforced predicates, not measured runtime success.

## Exact changes

Eleven source paths/raw434855B, with full SHA256/Git blob pins in `.tmp/s54-block3d-p10a-01/review.json` and archived `long-lived-dialog-a3-migration-s5-final-block3d-p10a-evidence/executor-review.json`. All are under `apps/kilogram-cli/tests/support/`:

| File | Bytes | Change |
| --- | --- | --- |
| migration_s54_borrowed.rs |8685| Single shared ceremony implementation, context/account/role binding, actual command builders and guarded upgrades. |
| migration_s54_borrowed_tests.rs |5855| Four pure shared-helper/C1-compatibility/C2-slot/binding checks. |
| migration_s54_handoff.rs |4597| Consuming P10A order and owned continuation. |
| migration_s54_handoff_live.rs |8507| Phase sampler, original process/identity/holds/request/work checks, idle histories/current readiness. |
| migration_s54_handoff_proofs.rs |10184| Transition, selected-control, history, intent-freshness and zero-work predicates. |
| migration_s54_handoff_tests.rs |13391| Eleven pure controller/Ready/decoder-negative checks. |
| migration_s53_handoff.rs |330116| One helper declaration; existing command/upgrade/ceremony wrappers delegate, no duplicated ceremony. |
| migration_s54_control.rs |17917| Bounded context-aware import2_for; import2 remains C1 wrapper. |
| migration_s54_observations.rs |15545| Read-only live decoder count accessor; parsing/ledger bodies untouched. |
| migration_s54_p5.rs |4684| One inactive P10A module declaration. |
| migration_s54_transition.rs |15374| Record the exact earlier successful C1 upgrade IDs for later freshness proof; same B71/A72 commands/order. |

No product/runtime/protocol/storage/crypto changes. Earlier P7/P8/P9 guard bodies and all P9 controller/source witnesses are unchanged. The P3 intent addition is metadata from the actual issued arguments after successful commands, not a fabricated new intent or another registry/framework. It is needed to distinguish C2 IDs from original control IDs using retained facts. Navigation3/root Current handoff and historical files remain outside this change; index is empty.

## Borrowed commands and ceremony

`Pair::borrowed_for` validates canonical selected context, owner-index/account/device/claim, same profile/IPC/locator/state/config/base paths, original indexed mode and bounded physical slots. It copies only public metadata views/root/base paths into `ContextCommands`. There are no child handles, Drop/stop methods, state opens, new accounts or substitute Pair in that descriptor; the original Controller retains all lifetime ownership. This permits each ceremony/upgrade subprocess to call back into the same mutable Controller rather than borrow or transfer its process handles.

One shared implementation performs public original-scope export → separate original-root authority snapshots → Consent2 propose/cosign → bounded public scope/consent read and signature/scope/role/account/device verification → both actual consent2-import calls. The actual signed consent reference is passed to the guard before the first import, allowing partial import readiness to be checked against that exact reference. Guards run before/after subprocess work and authority steps with the original absolute Phase bound, known-child liveness/decoder polling, current identity/policy, unchanged holds/requests/frontiers and actual work checks. No duplicate ceremony body or private-source export was introduced.

Original C1 command order, upgrade ID encoding, ceremony roots/base/TTL/expected-current and logical context behavior remain. The four shared-helper tests exercise exact old command/import arrays, actual creation arguments for propose/cosign, original roots/account bindings, C2 non-default physical slot and swapped/foreign metadata refusal. New bounded role/slot errors are errors rather than indexing panics. Controller.import2_for checks logical context bounds before indexing, waits current selected-context locator2 publication, resolves both slots from existing Ready and checks the same children/deadline around imports. C1 import2 delegates to context0. P10A itself guards the C2 publication wait before importing, so no unobserved long admission wait is used to advance intent.

Fresh C2 intent IDs are distinct from the recorded successful original C1 control requests and every one of the original1344 Text request IDs. Original Text map cardinality and a digest of actual request/accepted-reply associations are preserved. Context upgrade captures the CLI's actual JSON reply value (currently a hex-encoded status payload); authoritative state assertions come from the separate real status sampler, not an assumed decoded upgrade reply.

## Same-owner transition and fact sources

Call graph: `same P9 continuation → entry pressure/identity/control/history witness → original borrowed C2 ceremony → actual C2 publication/import → explicit B/A upgrade → bounded transition samples → both-C stable family2/direct Heads + C2 Active/controls/cuts → idle original history/frozen-cursor checks → owned phase.next10 continuation`.

Entry requires both actual live processes/source/nonce keys, phase10 and original P9 input. Existing pressure::full is used only for its valid C2-L1 entry condition, including original Consent1/identity/direct route. Actual files must match Controller controls; both OD1 and S54 values are then held equal as parsed JSON across all P10A samples. This retains both ACK/prefix/live suppressions, A385/B640 ordinal holds and cleared source64/ACK/page/retry selectors. Entry current counters and decoded commit counters must be the actual fresh zero P9 observations; no values are installed into the ledger.

The new sampler rechecks physical Ready slot/context/role, original D/E/SID/public Native keys with Legacy.identity, actual window/global resource bounds, same running PID/source/nonce, unchanged Text requests and current DirectOnly policy. C1 actual entry Q/cuts/P/A/K/Committed/current Consent2/frontiers/control debt are retained. A remains[641,640]/debt0, B[641,641]/debt1, no queued/pending/funded head. C2 stays[33,33],66 total sources including two genesis,64 original Text, no queued/pending/debt/head. Consent2 and then Offering/Prepared/Selected/Active are permitted without changing sources or public keys.

Before intent, actual C2 remains L1/NoLocalIntent with no P/Q/control refs. Partial consent import accepts only original Consent1 or the actual new ceremony reference; an unrelated current Consent2 fails. Both acknowledged imports and actual C2 publication are required before explicit intent. After intent, current Consent2/permission must match the ceremony. The first nonnull selected Q/control references are retained from actual observations; later contradictory refs fail. A follower may legitimately lag. Stable Active completion compares both owners to that retained selected-control witness and exact imported consent, with cuts33/33 and no control debt. Merely agreeing with another arbitrary row is insufficient.

During lease replacement, absent current lease/selected path returns not-ready. A still-current family1 direct lease is also a valid transient. Slot/identity/policy/holds/frontier/work violations or a relay path remain errors. Stable completion requires all four current family2 leases, actual matching decoded Head generations, valid publication/origin readiness, current permissions and actual selected direct paths; missing lease/path or stale Head cannot satisfy completion. No old pressure/catchup guard was relaxed or reused after Consent2.

Before each complete history read and frozen-cursor verification, the sampler waits actual idle selected shares (history reserves34). Per-lane order/frontiers and complete normalized source/EventID/request/body/format maps must match P9/P8 exactly: C1 A1279/B1280 with original B640 absent at A and retained at B; C2 exact64 format1 Text. Frozen original23 suffix remains verified. Live EventID establishes retained content identity, not a new raw ciphertext read. P10B's required stopped route cut rechecks held raw/cipher commitments; final received-source/signed-receipt equality belongs to P11. P10A adds no private-state cut and never invokes the P9 C2-L1 terminal reader after conversion.

Public observed Stats deltas require zero encrypt/decrypt/attempt/constructor/abort/ambiguity work. A read-only Decoder.observed_counts additionally checks actual accepted typed commit records before/after the phase, including trailing poll, so expected zero cannot mask a committed record after a status sample. No Operations.complete, finish, counter reset, expected final insertion or stopped-source reconciliation occurs while these owners are live. The returned typed baseline/current Stats and typed packet observations remain candidates for P10B's mandatory stopped-source reconciliation, not completed-incarnation claims.

Continuation retains the entire P9 object, original handles/state/roots/request map/ledgers/page baseline/all prior source64/pending/page/ACK/B640/C2/history/cursor/outcome witnesses and original deadline, plus actual current process keys, before/after status, actual consent/intent/reply/selected-control evidence and both work baselines/observations. It neither advances Phase::Relay nor starts relay/release/final shutdown.

## Actual verification and reproducibility

Evidence base `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3d-p10a-01/`. Initial immutable manifest preserved seven accepted EXEs/four source preimages before edits; an additional untouched decoder preimage was saved before adding its accessor. Both manifests and all12 archived preimages hash-verified. Existing failed evidence was never overwritten.

| Actual check | Result |
| --- | --- |
| Final CLI/all-target strict Clippy-05 |exit0 /9.6431625s, literal `-- -D warnings` |
| Stable compile-only-01 |exit0 /86.7047846s;501 units,560 source inputs equal before/after |
| Targeted stable pure-01 |exit0 /15.0631493s;15 passed/0 failed/0 ignored; all501 units fresh, independent frozen guard true |
| Format-01 |exit0 /4.3155333s, `cargo fmt --all -- --check` |
| Exact source/evidence review |exit0;11 source/Git blob pins,15 exact PASS identities,560 current compiler input hashes,12 preimages,7 current artifacts,4 unchanged field images, restricted dirty paths/empty index |

Native watchdogs: compile1190/1200s, Clippy590/600s, pure/format230/240s. Jobs2/BelowNormal, locked/offline verification-scale/x64, one staging lane and stable executable paths. No hash-named harness execution. Sources were frozen for the single compile and targeted run and remain unchanged afterwards. Compile before/after and pure before/after inputs are identical; other harnesses selected zero tests. No unchanged G3/P7/P8/P9 suite was replayed.

Clippy-01 failed on a new helper type shadowing the imported Context trait and one pure test moving a field out of a Drop type. Those logs/receipt remain failed. The helper was renamed ContextCommands and the metadata field cloned. Clippy-02/03/04 passed9.307482/9.5117898/8.9849976s during subsequent source/premise tightening. Clippy-05 above checked the final guarded-publication source before compile. One compile revision and one targeted pure run were needed; no intermediate failed Rust test run exists.

The15 actual names are pinned in test-selection.json/review.json and pure.stdout.log. Coverage: exact C1 command/upgrade/import compatibility; real ceremony argument context/root/account/cosign; C2 slot0 through actual Ready resolver/import builder; foreign roles/context/account/device/profile/state/claim/slot; input phase/ownership/request-map substitution; real prior control/Text ID collisions; current Consent1→Consent2 identity/source/no-intent guards; transient-vs-stable lease/path/Head; actual retained selection/consent/cut/control conflicts; original C1 Q/control/B640 frontier; each packet/constructor/outcome counter/regression; published hold changes; source/request/body/format/order changes; and actual typed decoder counts rejecting an unexpected commit. All use synthetic public metadata, no enrolled state or child runtime.

Final normal CLI remains50473984B/SHA256 `f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`; other five stable EXEs unchanged. Migration harness16982528B/`43d10254d0ae51fddfc9ae11bbc506cf95c9f209d9e310d71ac137d80a85047e`. Four frozen field images remain unchanged/unexecuted. Raw compile capture2709107B/`9d4bbd20ae946b400cd09323b47fca47703439d6857e6ffed65a89c8be96a3e3`; pure capture2709166B/`791382d909457e3a8f377915d3962eb344a3e71878ab0778b5eea088cd8b4d96`. Exact review25640B/SHA256 `0c716a567543a89b25735c9cd48223fbc1661992aa8ccf0097717cdc3054e84f`.

Compact durable evidence includes actual final and failed receipts/logs, full compiler source-input hashes, exact review/provenance and bounded helper/selection scripts. Raw full captures, independent expected manifest and EXE/source archives remain at the immutable evidence base. To reproduce, use a fresh directory, update helper base paths, compile without execution, derive the independent manifest from that capture, then select s54_p10a_ via the stable runner. Preserve earlier receipts; the original review.py audits the historical step identities of this attempt.

## Remaining boundary

Actual public ceremony/admission/connection replacement, zero-work conversion, persistent identity/source/cursor preservation and whole/pause timing remain unexecuted runtime assertions. Extra status/history/authority work consumes the same future deadline; no feasibility claim is made. P10B must consume this same continuation, reconcile these live streams at its mandatory stopped route cut, own relay/current signed routing and release original B640. P11 later owns graceful final reads/receipt/counter/memory/cleanup. Ready for root source review; no commit/push or next increment until explicit dispatch.
