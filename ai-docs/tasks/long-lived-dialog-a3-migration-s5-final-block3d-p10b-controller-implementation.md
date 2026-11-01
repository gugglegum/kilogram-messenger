# P10B — owned relay route cut and original B640, inactive implementation

2026-11-02. Baseline `2dce753d8ecf5cc9455ef25679c5338fb0aa81b9`, parent `5006de9c6aafcbf3fab362d248b14eb9c167f190`, tree `775c179e6a4522e0ab24fa0fa3c5ed2d3e2925b0`. Task8739B/SHA256 `c987bbcee3ae2718dd7c4980470c959081ca68562db6375f0a6dbbf9e7092c81`; accepted P10 contract14515B/`ff76125edb7251a774acf6fc5708a891247abfc1c286152507dbfea29f193f11` unchanged.

Dormant `relay::p10b(handoff::Continuation)` consumes the entire P10A continuation and returns owned incomplete11/12 with both live owner streams and one owned relay. Phase::Relay is accepted only after all route/source/delivery/history/work gates. **No actual P10A/P10B scenario, relay startup, enrollment, old private fixture/F/P/field action, P11 finalization or commit/push was performed.** Runtime descriptions below are enforced future assertions; feasibility remains unmeasured.

## Exact source and ownership changes

Ten source paths/raw104991B under `apps/kilogram-cli/tests/support/`; exact SHA256 and normalized Git blobs are in archived `long-lived-dialog-a3-migration-s5-final-block3d-p10b-evidence/executor-review.json`.

| Source | Bytes |
| --- | --- |
| migration_s54_control.rs | 18620 |
| migration_s54_observations.rs | 16199 |
| migration_s54_p5.rs | 4788 |
| migration_s54_relay.rs | 10893 |
| migration_s54_relay_live.rs | 10883 |
| migration_s54_relay_locator_tests.rs | 9925 |
| migration_s54_relay_locators.rs | 5506 |
| migration_s54_relay_owner.rs | 5357 |
| migration_s54_relay_proofs.rs | 9494 |
| migration_s54_relay_tests.rs | 13326 |

Seven new modules implement consuming order, relay child ownership, relay-phase sampling, public authenticated locators, pure predicates and two focused test modules. Three existing files change: p5 declares inactive modules; Controller gains optional relay, liveness check and both-stopped/reconciled guard; Decoder retains read-only metadata of the last accepted actual native_commit (context/delta/operation/generation/sequence). Existing stop/reconcile/Drop/ledger branches retain their behavior. The commit branch already validated the context before ledger commit; new metadata records that same accepted context. No product/runtime/protocol/crypto/storage change and no old DirectOnly or C2-L1 guard relaxation.

The same Controller owns original roots/state, two node handles, optional relay, original absolute deadline, Operations/Pages/request map/frozen23, stopped outcomes and every prior witness. No second Owners/Pair/process framework or copied lifetime owner is created. Continuation retains the entire previous object plus observed direct/relay/closed/source/delivery/history/work evidence, fresh actual node incarnation keys/counts/packet records and relay PID. Both relay-incarnation packet streams stay open; no Operations.complete or whole PASS.

## Required stopped cut and relay lifetime

Entry requires phase10, both live nodes, no existing relay, unchanged actual P10A consent/selection and C2 cuts33/33. Exact actual control files must equal retained cache and original holds; all1344 original Text request/reply associations keep the P10A digest. Current direct locator2 for both contexts and owners is authenticated and joined to actual current family2 Heads/leases/revisions/selected direct paths. Original identities/scopes/native public keys/C1 selection, C2 ceremony and complete maps/frozen23 are rechecked under actual idle shares.

Both known kill/waits finish before reconcile_both; stopped_reconciled must then succeed before relay creation or either profile write. Actual waited non-success outcomes must match both original PID keys and source/counter-resolved closure. Actual stopped-source reconciliation checks the ledger against source frontiers/generation/last operation; actual counters must remain zero through the pre-route incarnation. Last live status alone cannot close it. The required stopped read of original B640 debt must equal the retained entire witness: original request/EventID/raw source/ciphertext commitments and absent receipt. No obsolete C2-L1 terminal reader is used after conversion.

Relay image is verified by bounded streaming SHA256 at fixed path `target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe`:10460160B/`cf9d029cebf1d04745c90a9712401d70e823ecf4af5a082fe7269649df358e5e`. It was not rebuilt, replaced or run. Future startup creates a fresh attempt-local p10-relay directory with ready/stop/pending/stderr paths, immediately stores the spawned OwnedChild in Controller, retains actual PID, bounds readiness to min15s/original remaining deadline, caps ready bytes2048 and accepts only exact canonical `http://127.0.0.1:nonzero-port/`. Liveness is checked through readiness and every Controller wait; a file alone cannot establish it. Original outer deadline and relay image lifetime remain unchanged.

On error or deadline unwind the Controller cleans its known node handles; relay Drop independently try_waits or kills/waits that one known relay and writes its actual outcome to fresh relay-cleanup.json without a deadline check. Relay outcomes never enter node packet accounting. Normal return retains the live relay for P11. Drop evidence writing is best effort, with unresolved wait/error represented when available; pure tests do not establish actual OS cleanup/liveness. No name-based process kill, detached relay or system setting change.

## Profile, locator and current admission

With both nodes stopped/reconciled, existing handoff::route updates each original public profile. Full parsed JSON must equal the prior document with only route set to relay-only/owned URL/bind null. All other current and future fields are preserved. Actual Profile3 effective policy is checked. Same two owners restart, Controller.start removes stale both-context publications, and genuine new incarnation keys must preserve source owner identity (PID rotation is not assumed).

Original public existing-origin.cbor verifies context/root credential/account/device/role scope. Actual fresh signed locator2 for all four views must verify the expected original peers/scope, current wall interval, exact owned URL, zero IP addresses, positive revision and strict revision/content change from direct publication. Same-owner endpoint IDs must agree between contexts; restart need not rotate endpoint IDs. Both contexts use import2_for. Runtime import/current policy checks provide current authority/consent validation in addition to public signature verification.

The phase sampler joins actual Ready-derived physical slot/context/role, route policy, current lease, family2, publication/origin readiness/errors, signed own/imported revisions and decoded current Head generation. Actual selected path must be relay; configuration text cannot satisfy admission. Missing lease/path or incomplete current publication waits under the same deadline; contradictory identity, policy, route, source, hold or packet facts fail. All four views must be current before ACK/release, and current admission is checked again after enabling A ACK and through delivery/final sampling. No unchanged G3 rejection scenario is rerun.

## Ordered autonomous B640 release

The exact staged schedule is derived from retained entry controls. Held retains original A385/B640 and ACK/prefix/live holds. Ack enables only A OD1 ACK and A Relay phase while its prefix and B640/live remain held. Released changes B S54 to Relay and clears hold640 plus suspend_live in the same S54 update; A prefix remains held and B OD1 ACK remains disabled. Controls.write retains OD1-before-S54 order, guarded before/after each owner update with readback equality. OD1 hold list stays empty (640 never enters its <512 schema); source64/ACK selectors/page/retry must stay cleared.

Before release the actual public request-status3 for original B request640 must resolve the retained EventID with delivered0. Delivery waits on the same live A incarnation, actual all-four current relay admission, A frontier[641,641]/pending0 and B original request delivered1/debt0. Request queue/source associations must remain identical. Actual A public history row must match original B640 request/EventID/body/format before A prefix clears. There is no new A Text/send/open/restart trigger. Only then Final clears remaining ordinal/live/prefix/ACK suppressions in both files using valid Relay controls.

The stopped route cut proves original raw/cipher commitments. The live original EventID/history and receipt-derived RequestStatus3 prove subsequent content/delivery identity. This is **not** a live ciphertext export or complete signed-receipt byte comparison; P11's stopped final read owns those stronger assertions. Continuation retains original source/debt/request witnesses, both actual request statuses and materialized row for that later comparison.

## Final observable obligations

Every sample rejects source/identity/selection/policy/control drift, unexpected pending rows, queues/head/control debt and packet/constructor work. Fresh actual readable Stats and typed native_commit cumulative counts allow only A one decrypt/attempt after release; B has zero, encrypt/constructors/abort/ambiguity are zero. Final requires actual A decrypt1/attempt1 and actual decoder metadata context C1; B has no packet. A total one and its actual context-bound delta establish no C2 packet work without injecting expected counts. Trailing polls are checked as well.

Idle-share complete histories must have C1 both1280 original Text with only B640 added at A, and original C2 exact64. Complete normalized EventID/request/body/format maps and per-lane order must match retained originals; frozen23 and1344 Text requests remain unchanged. Final frontiers[641,641]/[33,33], receipts[640,640]/[32,32], no debt/pending/queue/head/control debt and all current relay permissions/paths are required. Phase moves10→11 only after these gates; same live handles/relay/deadline/ledgers/outcomes continue into P11.

## Actual checks and reproducibility

Immutable raw base `.tmp/s54-block3d-p10b-01/`. Ten initial preimages (seven accepted EXEs/three touched source files) are preserved and hash-verified. Failed Clippy logs/receipts remain unchanged. Native checks use finite watchdogs: Clippy590/600, compile1190/1200, pure/fmt230/240 seconds; Jobs2/BelowNormal, locked/offline verification-scale/x64, stable executable paths only.

| Actual final check | Child exit | Seconds |
| --- | --- | --- |
| clippy-04 | 0 | 9.4042919 |
| compile-02 | 0 | 89.4392504 |
| pure-01 | 0 | 14.73346 |
| fmt-01 | 0 | 4.420653 |

Final compile/pure before/after inputs are identical:567 compiler source inputs,501 units, all501 fresh during pure run; independent frozen guard true and all current source/binary hashes verified. Selected pure run:18 passed/0 failed/0 ignored, exactly13 new P10B tests plus5 affected Decoder tests; other harnesses select zero. Exact names and arguments are in test-selection.json/pure.stdout.log and source/EXE pins in executor-review.json. No broad unchanged G3/P7/P8/P9 replay.

Coverage includes canonical URL/unique relay/original deadline; both stopped+reconciled/actual outcome contradictions; full profile route overlay; staged release/stale suppression; only B640 source/frontier/receipt changes; same actual RequestStatus association; actual work and C1 packet context; exact history fields/order; real protocol signatures for both contexts/roles and wrong signature/scope/cert/peer/URL/IP/interval/revision; stale Head/revisions/configuration-only/Ready slot/selected path negatives. These are public synthetic in-memory protocol/metadata checks, with no executable enrollment or actual owner/relay process fixture.

Clippy01 failed on one missing delimiter in a new test JSON fixture;02 failed on four lint findings (two indexed loops, duplicate test branch, unnecessary clone). Those ordinary repairs passed03. Compile01 passed in90.4271786s. Subsequent static review found that one negative locator fixture had no address and would fail the protocol builder before testing route rejection; it now signs a valid direct IP publication and verifies relay rejection. Compile01's full source revision and migration EXE are archived before that correction. Clippy04 and compile02 above checked the final revision. No failed Rust test execution occurred. Sources were frozen throughout each compile/run; final source matches final capture.

Normal CLI remains50473984B/`f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`; other five stable EXEs unchanged. Migration harness17103872B/`12e5237276f38c33b4bf19bcec0cb5d1341982caf138ea694140a341f29982dc`. Four frozen field images remain unchanged/unexecuted. Raw captures/expected manifests/preimage EXEs remain at the immutable base; compact durable evidence contains final and failed receipts/logs, source hashes, exact review/provenance, historical review helper and bounded reproduction helpers. Use a fresh base, change helper paths, compile without Run, derive the independent expected manifest from that final capture, then execute only selected pure tests through stable runner. Never overwrite this attempt's receipts.

## Review boundary

No measured actual runtime admission, relay lifecycle/cleanup, autonomous delivery, source persistence/receipt bytes or timing/feasibility result exists. The mandatory route cut and later live assertions will run only in an authorized future scenario; P11 is not implemented here. No commit/push; navigation3/root handoff and historical untracked work are preserved. Ready for architect source/static/pure review; stop at this increment.
