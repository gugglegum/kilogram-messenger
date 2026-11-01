# S5.4 block3b — P8 successor64 and same-owner C2

2026-11-01. Source/static/targeted-pure submission on published block3a `f90bb7e3e4c9d4dfa959ec86baa6c2715349c5f0`. Implements the accepted contract in `long-lived-dialog-a3-migration-s5-final-block3b-p8-task-01.md` (9803B/SHA256 `8959fcdf928d96b914ca287e6992902230c043bd72f9bfc848287f5a1e3b053a`). No new general proposal or unaccepted capability is required by this implementation.

**Real enrollment, product/process/P0–P8/F/P runs, old private fixtures, fixed-field staging, P9 and later phases were not activated or executed. No runtime fairness/deadline fit or whole/partial scenario PASS is claimed.** No commit/push. Published P7 source/witness/test files and S53 bodies are unchanged. Three unrelated tracked navigation changes and historical files remain outside this increment; index is empty.

## Exact source scope

All paths are relative to `E:/AI-Projects/kilogram-p2p-messenger/`. Final review manifest `.tmp/s54-block3b-03/review-02.json` pins raw bytes/SHA256 and normalized Git blob for exactly12 Rust paths, this report, evidence, helpers, artifacts and preimages. Its final hash is delivered in the callback to avoid a circular report/manifest hash.

| Path | Change |
| --- | --- |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/control.rs` | Optional debug `source64_after`; strict Pressure/hold/A-role/checked ordinal bounds. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/selection.rs` | Bounded genuinely accepted Head metadata and same-context/binding/generation/frontier predicate. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification.rs` | Two per-slot accepted-Head records only when enabled; successor selection and actual selection/Head/page/ACK facts. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/protocol2.rs` | Pass actual already-accepted peer receive frontier to observer; bind Page metadata to actual current lease. No wire/authority change. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/tests.rs` | Update affected existing hook tests; four new schema/successor/current-Head negatives. |
| `apps/kilogram-cli/tests/support/migration_s54_p5.rs` | Declare inactive P8 module only. |
| `apps/kilogram-cli/tests/support/migration_s54_control.rs` | Small context-aware request/send path; existing C1 send delegates with context0. |
| `apps/kilogram-cli/tests/support/migration_s54_observations.rs` | Integrate accepted Head facts, typed pending/pause decoder and observed accepted Native batch peak. |
| `apps/kilogram-cli/tests/support/migration_s54_pause_observations.rs` | Bounded S54-specific source64/page/ACK/pending validation; no separate journal/parser framework. |
| `apps/kilogram-cli/tests/support/migration_s54_block2_tests.rs` | Update the two existing Head fixtures with actual received frontier. |
| `apps/kilogram-cli/tests/support/migration_s54_pressure.rs` | Consuming P8 controller, same-hold/status/permission/quota brackets, actual C2 request/history witnesses, owned9/12 result. |
| `apps/kilogram-cli/tests/support/migration_s54_pressure_tests.rs` | Eleven new non-network metadata/decoder/permission/history/request/resource tests. |

No crates, Cargo files, scripts, normal quota/timer/crypto/route algorithms or protocol encodings changed. The five runtime paths above add only narrow opt-in observation/selection behavior. No alternate executable scenario or P9 controller was registered.

## Accepted Head → real successor64

`protocol2::accept_head2` still performs its existing current lease, signature/scope, receive-cursor/source-tail and mode validation. Inside its existing observer-enabled branch, it now passes actual parsed body[5].received, physical slot, context and current local binding/generation to `Observer.head`. Two optional in-memory records are the entire new Head cache. The emitted `validated_head` gains `received`; the decoder checks its exact five fields, positive bounded frontier and rejects stale generation, conflicting same-generation binding or regressing frontier. These are debug facts, never protocol authority or configured expected state.

`source64_after` absence/null preserves the published first-debt selection behavior with initial after=None. A present value requires Pressure + hold_source64 + lane0 and1..=4032 with checked successor arithmetic under the existing4096 debug ordinal bound. Existing closed schema, release-build refusal,16KiB cap, canonical context, outside-state file and parent pinning remain. No additional work is required with the observer absent.

For Some(385), `Owner7::verification_selection` first revalidates its actual family2 lease. The saved genuine accepted Head must match current context/binding/generation and peer received385. This uses a prior accepted Head because the callback precedes the **next** worker Head; it does not manufacture a Head or add an RPC while the share is held.

`select64` acquires the real reserve(C1,64) before either debt/source read. In the first read callback only, a scoped `debt_page(None,1)` metadata result must identify actual first outstanding ordinal385. That temporary is dropped before the successor page is retained; no65th source or permit escapes. The two real successor reads are debt_page(Some385,32) and debt_page(Some417,32). Selection enforces exactly64 distinct verified sources in consecutive order386..449. Every source still passes the existing immutable source verifier for actual scope/role/ordinal/kind/ID. Default None keeps the previous monotonic first-debt behavior.

After selection, the current lease/generation is checked again. The observation contains actual context/binding/lease_generation/after/first/last/count/IDs digest. First/last/count come from the selected records; the digest serializes actual (ordinal,EventId) pairs, not a generated expected vector. The controller arms against its actual P7 vector and compares the emitted facts to that vector and B's committed pending associations.

Selection64 still owns its same RAII permit outside the Owner callback, across the existing asynchronous bounded Pause. IDs drop before the permit. Read/source verification failure, cancelled owner reply, async worker cancellation, connection close, timeout and ordinary release retain the same ownership/drop path. The existing worker has no outstanding dispatch/RPC at this boundary, pauses before its next Head outside Owner7, and rechecks the current lease after release. Incoming Head may continue; no synchronous Owner wait or quota-busy retry loop was added.

## One owned P8 call graph

`pressure::p8(offline::Continuation)` consumes the same P7 object. It checks phase8/12 incomplete, A owned/running, B owned/stopped, A next[641,640]/debt256/no lease, and the stopped original B640 source/ciphertext/debt witness. P7's actual ordered256 metadata supplies successors386..449; there is no replacement state/checkpoint/enrollment.

Before restarting B, both owners' C1-scoped OD1 disable_ack is set. A current schedule holds385; B holds640. Both suspend C1 prefix, and B additionally suspends outgoing live work. Completed retry_live is cleared. Initial hold_source64=false/source64_after=null allows ordinary delivery. The stale source64 marker is removed before arming. The same B restarts; current actual C1 ALPN2 and C2 ALPN1 locators are imported through the existing helpers.

Ordinary Head/credit/Deliver reaches B pending64 while receive frontier stays385. Held385 causes the normal range385..400 to send386..400 (15); subsequent16/16/16/1 successors finish at449. B prefix suspension prevents funded lowest-head repair. Status must show A next[641,640]/debt256, B next[385,641]/debt1/pending64, queues0, A pending0 and both funded heads absent. The typed B pending map must contain exactly64 C1/role0 associations, each matching a real P7 (ordinal,EventId), actual positive durable generation and actual operation ID. An ID-set cardinality alone cannot satisfy this.

The controller waits for C2's actual L1/family1 leases and zero-Text start. It captures the actual current C1 generations and A accepted peer Head received385. The original frozen HVI1 suffix23 is re-read **before** acquiring the full C1 hold: history3 reserves34 references on the selected context and must not be requested against a full C1 share. The original Frozen object remains owned throughout P8 for later P9 use.

The decoder is then armed with the actual P7 successor vector and the observed A binding/generation. Only then does A receive hold_source64=true/source64_after385. The controller waits for the genuine decoded Source64 pause, reads its bounded marker, and binds it to the actual decoded PID/source/incarnation/sequence/facts. Current A C1 limit/current64/64 and current C1 lease generations are checked; a remembered old marker alone never proves the hold.

Every `bracket` verifies both known live process PIDs, same C1 generations, same C2 generations/family1, original two-context D/E/SID/public Native identity, current permissions, unchanged C1 frontiers/debts/pending/head absence, retained controls, original marker and actual A64 window. Rebind, timeout, vanished quota or replacement pause cannot silently become another successful hold. The marker's sequence may precede later C2 Native commits within the same process, while PID/source/nonce and exact original facts must remain identical.

Hold.until is conservatively fixed from the existing max_pause_ms **before writing the arm**, capped by the same Controller.until. It is not derived or renewed from marker observation. An additional local Phase can only shorten the already-owned absolute bound; nested waits cannot extend it. The node's existing max120000ms Pause timer continues independently, unchanged. P9 receives these original clocks and must release the actual hold promptly in the future continuous run.

## Same-process C2 and current permissions

C2 uses `Controller.send_context(owner,1,n)` and the existing original Request ID/body mapping. Physical slots come from actual Ready context lookup; no slot0 assumption, new runtime, participant or private writer is introduced. Existing C1 `send` behavior delegates to context0 unchanged.

Each owner sends C2 requests1..32 in eight bounded rounds of four per role. After each round, actual C2 status must reach the corresponding equal readable/own frontier with queued/pending/debt0. Every round is bracketed by the same live C1 hold, original processes/current leases and actual resource observations. Final C2 must be next[33,33], queued/pending/debt0 on both sides; accepted receipts are established by actual durable debt retirement alongside exact readable sources, not by an ACK pause annotation.

The original C2 Consent1 reference is compared to the real P7 final-reopen status witness, with C2 modeL1/family1/current lease and policy_error=null. `current_consent2` must remain null. `handoff_policy_error` can legitimately report missing Consent2 for this still-L1 C2; it is **not** substituted for the actual Consent1 permission gate. C1 continues requiring its actual current existing/Consent2 gate. Current authority/device authorization, signed Consent1 lifetime, wall high-water, native1 gates and DirectOnly connection policy remain the existing production checks; no clock override, renewal or synthetic consent is introduced. A focused negative test covers this distinction.

Full status checks actual per-slot routes policy=direct-only/selected_path=direct for both contexts. Actual window limits are[64,64], each current/peak<=64, and A C1 current=64. Actual resources connections/peak_connections<=2, mailbox/peak_mailbox<=16, connection_limit2/mailbox_limit16/frame_limit1MiB/bidi4/uni0 are recorded. Native batching is already guarded by Operations.commit's accepted per-commit delta<=4; the decoder records the peak **after** that guard succeeds. No Stats7 max-native-batch field is invented. Actual observed final per-owner C2 delta must be encrypt32/decrypt32/attempts64 with no aborted/ambiguous or constructor deltas.

After the final causal/readable/receipted wait, the controller waits for the actual C2 window to be idle before its ordinary history3 reserve34; it does not borrow C1's held share or spin on a failed quota request. It reads both actual bounded C2 histories and requires64 identical immutable format1 sources with exact original role/ordinal/request/body associations. Another bracket proves the original C1 hold survived the reads. C1 history is not read during its full64 reservation.

## Non-Native observation integration

The decoder retains existing owned PID/source/incarnation/context/sequence checks and now carries bounded typed saved_pending records keyed by logical context/role/ordinal. It rejects conflicting alias/ID, foreign role/context, missing fields, stale durable generation and conflicting operation at one generation. An exact repeat does not advance anything.

`source64` requires an armed actual P7 vector and the same genuinely accepted current Head, with exact bounded facts/digest; it is not a Native commit. `page` requires the exact preceding forward normal_page facts on the same accepted binding/generation and may occur only once. It never calls Pages.accepted again. Normal/head/replayed classifications and actual novelty/bounds remain explicit; global/PID-local `Pages.local_target` remains unchanged for future cumulative page2/5. `ack` validates actual context/current binding/generation,16-byte request, explicit push/query-reply classification, nonempty receipt count<=16 and saved-receipt digest. It records a pause, not receipt retirement or packet work. No P9 interruption/ACK-loss controller was implemented.

## Explicit P9 input

On a future successful real run, the returned `pressure::Continuation` owns:

- the original entire P7 continuation/Controller, all dirs/roots/witnesses/request map/Frozen cursor/Operations/Pages/counters/process handles/outcomes and original absolute deadline;
- original heldB640, both live processes, same current C1/C2 generations, exact pause marker and conservative pre-arm hold deadline;
- exact real64 successor metadata, B's typed committed pending witnesses and both actual C2 source/history maps.

Only Overlap is accepted, giving9/12 and explicitly incomplete. The original A64 hold remains accounted for until P9 explicitly releases it. Errors retain the existing known-handle cleanup/unresolved-outcome path, never a manufactured final counter or PASS. **No live P8 hold or checkpoint exists now: this submission only compiled and tested pure seams.**

## Actual checks, failures and immutable pins

Final evidence: `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3b-03/`; review-02.json carries the exact source/evidence/artifact/preimage tuples and all35 accepted full test names. All checks use locked/offline verification-scale/x64, Jobs2/BelowNormal, stable Windows executable paths and one build/staging lane. Native jobs have finite limits1190/1200 for compile,590/600 for Clippy,230/240 for pure/fmt; all final receipts confirm child cleanup. Sources stayed frozen throughout every build; no provenance mismatch was ignored.

| Check | Actual result | Evidence |
| --- | --- | --- |
| Final strict CLI/all-target Clippy |exit0 /9.0542725s| `.tmp/s54-block3b-03/clippy-01.json` and stderr; literal `-- -D warnings`. |
| Final compile-only |exit0 /82.7040715s| `compile-01.capture.json`,501 units, equal before/after source/tool/config inputs; normal CLI test=false/opt3/debug assertions+overflow checks. |
| Final affected controller/decoder group |exit0 /14.7950675s| `pure-01.stdout.log`, exactly19 passed/0 failed/ignored; frozen guard true,501/501 units fresh. |
| Unchanged runtime hook/selection group |16 passed/0 failed, reused| Exact earlier `.tmp/s54-block3b-02/pure-01.stdout.log` runtime section; same final library EXE SHA/bytes and all five runtime source bytes independently asserted by review-02.py. No rerun of this passed unaffected group. |
| Final workspace format check |exit0 /4.23541s| `fmt-01.json`, read-only `cargo fmt --all -- --check`, no diff output. |
| Scope/evidence review |exit0| review-02.py checks12 source paths/Git blobs,35 exact accepted test names, reused runtime binary/source identity, preserved preimages, unchanged P7/S53/Cargo/crates/scripts/four field images and empty index. |

The35 accepted tests are15 new (four runtime +eleven controller) and20 affected existing (twelve hook/selection +eight decoder). Their exact names and selection/skip lists are in test-selection.json and test-selection-run.json. They cover default/schema/release/namespace behavior; reserve-before-read, exact386..449, gaps/distinct/verifier/read failure, cancellation/release and independent shares; genuine Head context/binding/generation; typed pending/source digest/current-marker failures; page barrier/no double count; ACK non-Native classification; actual batch deltas/oversize rejection; original Consent1 versus future Consent2; C2 request mapping/history and physical-slot/resource bounds. They use public synthetic metadata and tiny fresh control files; none invokes enrollment, P8, product children or old private fixtures.

Earlier outcomes remain immutable and are not relabelled as aggregate success. In block3b-01, the initial formatter caught a Rust2024 reserved variable name (fixed), Clippy-01 exit101/33.2457864s rejected needless Ok/? (fixed without lint relaxation); Clippy-02/03 passed9.4799034/8.5231126s, and compile-01 passed566.9213542s with equal inputs. Source review then corrected the C2 Consent1 predicate and moved C1 cursor reading before the full reservation; those intermediate12 sources/seven EXEs were saved before rebuilding. Block3b-02 Clippy/compile passed8.4886389/80.4601838s; pure-01 aggregate exited1/13.1260965s because one new synthetic batch test wrongly expected a five-call commit to pass the existing<=4 ledger. Its runtime group16 passed and integration group18 passed/one failed. The test now expects refusal, and the diagnostic peak is updated only after ledger acceptance. Final19 integration tests passed above. Only the proven unchanged16 runtime PASS section is carried forward; the failed aggregate is never claimed as passed. Both old stdout/stderr/captures and their same-library/source proof are pinned by the final manifest.

Accepted block3a preimages were preserved before any rebuild under block3b-01/preimages. Subsequent intermediate/failed-stage sources and seven EXEs are preserved under block3b-02/preimages and block3b-03/preimages; all corresponding manifests/hashes are rechecked by review-02.py. The unchanged four field EXEs were not replaced or executed. No published P7 tests,100k/G1–G4/cut sweeps or other unaffected suites were repeated.

Final normal CLI:50403840B/SHA256 `7661c22fd41221810ba17f7070300a1a8406ffd0923d4b48a04ebb36577f33d6`. Stable CLI library:32633344B/`0b0f4f92470a0141f0bf92c76f8ba5580d6624a8c7262f56e853f6a5550ea1af`. Stable migration harness:16432640B/`816121938bce13411873778b831f598ed36fd0d8be716f540b9e1b09548178f4`. Full compile capture SHA256 `b48160adb7981da976ebee955f47ffad6c557597b5a7e3d1f0baec96ce96ac0c`; all six stable artifacts, guards, helpers and logs are pinned by review-02.json.

For reproduction use a fresh evidence directory and the final bounded check/compile/Clippy/pure/fmt helpers. Compile-only first; derive the independent frozen expected manifest from that exact capture and refuse changed inputs/artifacts. test-selection-run.json intentionally selects only the final19; reproduce the additional16 runtime tests from test-selection.json if their source/library identity changes, rather than claiming an unverified reuse. This reproduces source/pure behavior only. Actual C2 progress under one live A64 hold, no starvation/competing RPC race, history idle scheduling and pause/whole-deadline feasibility remain for the separately authorized continuous measurement after review/publication.

One review-ready callback concludes this stage. P9/P10/P11, runtime/F/P activation and commit/push await root dispatch.
