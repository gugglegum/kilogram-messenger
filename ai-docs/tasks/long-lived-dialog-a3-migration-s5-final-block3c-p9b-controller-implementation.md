# P9B — consuming catch-up controller and terminal immutable proof

2026-11-02. Source/static/pure implementation on accepted P9A `365468dc5046475ff923e1f5cdc68edacf649ff8`, parent `9e2b1ced63a90d0e8bcae9978942e4c573d78f8d`, tree `614f608402776b988ddb2cb884277742c3ecdeae`. Task `long-lived-dialog-a3-migration-s5-final-block3c-p9b-controller-task-01.md`, 9832B/SHA256 `e59c8615074814ee88dd14b34df09127ec2be2a2d54aa27b689a1db2faa66a65`; accepted P9 contract/root clarifications remain unchanged.

The inactive `catchup::p9(pressure::Continuation)` is implemented. It consumes the same P8 ownership and returns an owned incomplete10/12 continuation for P10. **No continuous P9, product/process test, enrollment, old private fixture, field staging, P10/P11, commit or push was executed.** Pure checks exercise extracted predicates and typed decoder input, not an actual interrupted connection or terminal database. Runtime observations described below are requirements enforced by the future controller, not facts obtained in this source-only increment.

## Exact scope and source pins

Ten source paths, raw101326B, are pinned with full SHA256/Git blob IDs in `.tmp/s54-block3c-p9b-01/review.json` and archived `long-lived-dialog-a3-migration-s5-final-block3c-p9b-evidence/executor-review.json`:

| Source under `apps/kilogram-cli/tests/support/` | Role | Bytes / SHA256 |
| --- | --- | --- |
| migration_s54_catchup.rs | Consuming P9 order and owned P10 input | 9402 / `6a13df493afb1cc60a0ab57d3a838fbb82e935cbb6063218ef4e55f009d21601` |
| migration_s54_catchup_live.rs | Existing lifecycle, current observations, page/ACK arms, public histories | 9524 / `a757d418c2c704e43c9ea649f8f2375a37e65b8edd839736e9ac54062ab7b4d4` |
| migration_s54_catchup_proofs.rs | Actual transition/source/history/work predicates | 9145 / `8909372e02a1a98dcefbafca70b1f8589b006db08148caf4024b1cf1ddd1d2ca` |
| migration_s54_catchup_tests.rs | Eight pure controller-negative checks | 11046 / `2d357d375b8ba97b914fa02e3fb92dbaf2a1273faff5b82c8f98333fad302ac5` |
| migration_s54_retained.rs | Stopped-state immutable ordinal/receipt/C2 reader | 15289 / `a90ceb585730248264c7324d7bcdb52a1afcfa6bec337384354ab49444d54e30` |
| migration_s54_retained_tests.rs | Three immutable/receipt/mutable-separation checks | 4743 / `4ce5f6db2e88d8d0a66e992b3327c000ba397a226d0cb3b5c659a789880e294b` |
| migration_s54_p5.rs | One module declaration | 4634 / `79d0ca7811fbb8cb54a1df00fe8059027bef0b64a05f5add85f339bd4d21dc92` |
| migration_s54_offline.rs | Minimal validated immutable metadata accessor and child declaration | 9954 / `14da4310a0f11ecc997743d10af875e4a33c3622d6f3bfb4a7366745179a1ce0` |
| migration_s54_offline_witness.rs | Existing signed-origin helper visibility only | 10663 / `cc96582523480cec8bf7fb141dc1f51671655deb42971bf945fa8b2974ae26d4` |
| migration_s54_pressure.rs | Existing full/bracket/resource helper visibility only | 16926 / `fc7c68756a34f7b209905ac4573564437a2ae9e1a0477d6158eb26dafbe1fe9e` |

The original P7 capture's debt256, C2-zero and Native-content guards and Snapshot.same body are untouched. Published P9A decoder/page/ACK implementation and ordinary runtime/protocol/storage/crypto sources are untouched. The three unrelated navigation files and historical untracked work remain outside this change. Index is empty.

## Call, control and ownership graph

`P8 Continuation → immutable P7 metadata view → same Controller → pressure.bracket → release64 → page2 → stop/reconcile B → fresh B/imports → page5 → stop/reconcile B → fresh B/imports → final tail → targeted held ACK → stop/reconcile A → old-binding release → fresh A/imports → rearm/resend → ordinary debt retirement → both kill/wait → reconcile_both → immutable terminal read → reopen both/imports → incomplete10/12`.

`Controller` retains the original origins/state directories, same process handles, request map, Operations/Pages ledgers, outcomes, frozen cursor, original identity witnesses and whole absolute deadline. No replacement Controller, checkpoint, account or partial-PASS registration exists. Five owned stop/start transitions are expressed through existing helpers; the terminal two waits precede either reconciliation/read. Existing Drop remains error cleanup for known handles, with unresolved outcomes preserved rather than converted into success.

Entry checks phase9, both live handles, original1344 request entries, P7's validated256 immutable source associations and saved64 pending386..449. Existing pressure.bracket rechecks actual pending rows, original marker/PIDs/generations, identities/permissions/routes/resources and pre-arm hold bound. A and B then receive complete valid CatchUp schedules while both ACK paths and prefix remain disabled. A's update clears hold_source64/source64_after together and suspends live delivery; the old hold's original deadline remains the release observation bound. The controller waits for actual A selected-share current0 under its original PID and lease before enabling B prefix. It does not infer permit release from publication.

Both S54 and OD1 prefix controls are updated. A prefix/live remain suspended; B live remains suspended and original640 held. Existing DirectOnly/current original Consent1 checks and global/window resource checks are reused throughout live observations. The helper waits through temporary admission states before applying strict original-identity/permission checks; C2's legitimate absent Consent2 is not treated as a failed Consent1 lease.

## Page interruptions and real fact sources

The actual live B decoder key captures the owner-specific PageBaseline while B prefix is held. `phase_local_target` derives each local target from that baseline and real new incarnation. A stale marker is removed before arming. Markers are read with the existing4096B public-file limit and checked by Decoder.page_marker against the actual committed typed record, PID/source/nonce/native sequence/context/binding/lease/operation facts. The extracted controller barrier predicate additionally checks phase/local target and current process identity. Only genuine forward normal pages contribute to the retained Pages ledger.

At phase2 the controller saves the original typed funded-head385 and first-tail450 observations before B Decoder is destroyed. Their binding/lease and log order must agree; a public history source row for A385 must equal the original P7 EventID. The original committed64 pending associations remain in the owned input/report. No transient next450/pending0 poll or head pause is added.

The page marker is followed by a stable drain observation: B must reach that actual accepted page's end, pending0, queued0 and no funded head; overshoot fails. Actual A debt256/frontier and both idle selected shares are checked, with actual status/stats saved before the existing waited stop and source/ledger reconciliation. The barrier stays armed through the stop.

On the first new B incarnation prefix/ACK remain disabled and the old page target is cleared before start. Fresh actual C1 locator2 and C2 locator1 are exchanged. Original identities, policies, direct routes and current accepted Head/lease are checked. Real phase progress must still be2; querying the new incarnation for phase5 must derive local3. The same barrier/drain/save/wait/reconcile sequence applies at5. The third B incarnation completes the remaining ordinary prefix tail; no sender Text or direct delivery bypass is introduced. Final status requires A next[641,640]/debt256, B next[641,641]/debt1, empty queues/pending/head and at least8 real phase normal pages. B prefix is then held again.

## Held ACK loss and regenerated dispatch

Public A history supplies actual associations for every ACK tuple, including historical ordinals below385. Eligible385..640 tuples additionally match P7. Receipt encoding and source format remain separate. A queries remain disabled throughout.

With B ACK disabled, the controller observes genuine current accepted Head, arms Decoder, removes the stale ACK marker and publishes CatchUp/hold_ack=true/hold_ack_from385. Only a second publication enables B ACK: Controls.write publishes OD1 before S54, so a single combined enable would be unsafe. Old-only duplicate batches may complete normally.

The targeted marker must correspond to a nonempty actual B lane0 push containing original outstanding offline source(s), with all typed tuples and digest retained. Actual A debt must remain256 and actual B selected-share current32, next[641,641]/debt1 are required while the dispatch is held. B ACK is suppressed again without releasing its hold before A's waited stop/reconciliation. The controller then requires actual B old-lease loss and selected-share current0 before reopening A. Thus a held old frame cannot be silently labelled a fresh transmission.

Fresh A imports both contexts' current locators and waits actual permission/identity/current Head readiness. B ACK remains disabled through the new arm. The subsequent held push must have a distinct binding and request, later decoded line, same actual B incarnation, identical saved receipt tuples and batch digest, and still-outstanding A debt256. Clearing hold_ack and hold_ack_from together permits ordinary retransmission. The controller waits for actual A debt0, suppresses B ACK, rechecks quiescence and unchanged histories, and retains both typed old/new ACK records. No production15s RPC limit,120s pause cap or schedule parser is changed.

## Public and stopped immutable end proof

Public HVI1 reads occur only after actual selected-share current0; existing history3 reserves34. Complete C1 Text histories must be A1279/B1280, with contiguous per-lane ordinal order, exact request/body/format, P7 offline EventIDs, and exactly original B640 as their difference. Both1280 would violate this phase. The same frozen original23 suffix is verified. C2 public maps must exactly equal the original P8 maps of64 Text, with original L1/current Consent1/DirectOnly retained by live identity checks.

The accepted terminal cut first kills/waits both same owned processes, then reconciles both actual ledgers/outcomes. The new reader checks stopped ownership and canonical state/profile/source/role/context mapping, actual selected original C/D/E/SID/Q, and original A migration/public identity. It reads immutable ordinals385..640, **not the now-empty debt index**, verifies actual Event2 signatures through stored root-verified credentials/selection, and compares every original request/EventID/previous/raw-source/ciphertext/body commitment against P7 at both A and B. A's original durable request association must still name that source.

Every offline receipt must exist at A and B, verify against its actual source/receiver/selection and have identical saved/accepted byte digest. The saved targeted batch is independently rebuilt from actual receipts on each side, including mixed historical entries, verifying their source/receipt signatures and exact observed individual/batch digests. Exact original B640 request/source/ciphertext/debt1/no receipt is checked with the existing bounded reader. A debt0 and absent B640, exact source counts/frontiers, empty pending/queue and C2 L1 next33/33/exact64 signed Text with zero debt are required.

The immutable view contains only original state/scope/anchor/source metadata. It excludes P7 Native-content/seal and C2-zero identity. The terminal reader does not restore Native account/session or export private material; constructor/packet call counts must remain unchanged around the entire read. Public projection text is checked in memory, while the proof reports metadata/counts/digests. Original P7 mutable equality remains strict for its original phase and is never reused here.

Actual P9 work is accumulated from per-incarnation observed counter deltas only after successful existing stopped-source/Operations reconciliation. Double close/reopen, regression, constructors, A packet work, encrypt, abort/ambiguity and attempts/decrypt mismatch fail. The asserted total B decrypt256 is compared to the sum of real closed pieces; it is never inserted into final observations. The live ledger is retained; Operations.complete is not misused while processes are live.

Finally both original owners reopen with held schedules, fresh current locators, original identities/permissions and genuine new decoder keys. Their actual counters must be zero before and after complete public/frozen/C2 verification. P10 receives the entire prior owned continuation, opaque page baseline, page2/page5 plus head/first-tail records, lost/regenerated ACKs, exact public maps, terminal receipt/source proof, actual P9 work and all original outcomes/deadline. C1 remains Active, C2 L1; both live handles remain owned at incomplete10/12.

## Actual checks and reproducibility

Evidence base: `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3c-p9b-01/`. Eleven accepted preimages (seven EXEs and four existing sources) were archived and hash-verified before edits. Bounded native helper commands use Jobs2/BelowNormal, locked/offline verification-scale/x64 and the stable runner; no hash-named harness was executed. Sources stayed frozen during compilation and afterwards. The compiled554 source inputs all still match, compile before/after equals pure before/after, and all501 units are fresh in the pure capture with independent frozen guard true.

| Check | Actual result |
| --- | --- |
| CLI/all-target strict Clippy-03 | exit0,8.9498636s, literal `-- -D warnings` |
| Stable compile-only-01 | exit0,86.9319802s;501 units; equal source inputs |
| Targeted pure-01 | exit0,15.1840563s;11 passed/0 failed/0 ignored; only `s54_p9b_` names executed |
| Workspace format-01 | exit0,4.4574948s; read-only `cargo fmt --all -- --check` |
| Exact source/evidence review | exit0;10 source pins,11 exact test names, all preimages/7 current artifacts/4 field images checked; empty index/scoped worktree |

All final receipts verify owned child cleanup. Watchdogs are1190/1200s for compile,590/600s for Clippy,230/240s for pure/format. Clippy-01 failed on u8→u64 conversion in the new tagged receipt encoding helper; Clippy-02 failed on two collection-range warnings. Those failed receipts/logs remain immutable. The fixes passed Clippy-03 before the single compile/pure run. No failure was reclassified as success or repaired in place. No unrelated accepted P9A/P7 suite was repeated: shared helper changes are visibility-only and the original guards' bodies are untouched.

Normal CLI remains50473984B/SHA256 `f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`. Stable migration harness is16784896B/`fa7f026110e21621bd2528b0fefd48ebf569c38b7d7a0c6a4870746731d04a6e`; the other five stable EXEs are unchanged. The four fixed field images are unchanged and unexecuted. Executor review is21861B/SHA256 `c9ae531b6bb4f8a0d246e7c9631c5570f14eddc941feb93df2e077329d5c47ee`; archive provenance pins raw compile/pure captures, expected manifest, full compiler source inputs and reproducible bounded helper scripts.

The11 tests cover owned entry prerequisites/pending substitutions; accepted-page drain/overshoot; phase/process/source/nonce/local barrier mismatch; atomic source64 selector clearing and ACK suppression; actual work closure/reopen/regression; A1279/B1280 lane/request/body/source preservation; every historical/offline ACK association; new dispatch binding/request/same payload; immutable ordinal/request/source/cipher/body/chain mismatch; saved receipt bytes/encoding/batch mismatch; and separation of terminal immutable metadata from legitimate Native/C2 mutation. They use synthetic public metadata only, never actual enrolled state.

To reproduce, copy the archived helpers to a fresh evidence directory and change their base path. Run bounded `check.ps1 -Step clippy-01`, compile-only, derive pure's independent manifest from that exact compile capture, run the `s54_p9b_` selection through the stable runner, then format-check and review current source/artifact pins. Use fresh numbered receipts; preserve any failed attempt.

## Remaining review/runtime boundary

Actual page/ACK scheduling, cancellation of a held Iroh dispatch, old permit release, genuine resend payload equality, durable reader results, idle-history access and five lifecycle transitions remain runtime-only assertions. Neither pure synthetic metadata nor successful compilation proves them. The terminal both-stop/read/reopen cut and repeated bounded public histories consume the original future whole deadline; their elapsed cost and pause/whole feasibility have **not** been measured. No new timeout/deadline, workload reduction or lower acceptance standard is inferred from these checks.

Ready for root source review. Commit/push requires the architect's explicit accepted-diff publication dispatch. P10/P11 and real continuous execution require separate assignment.
