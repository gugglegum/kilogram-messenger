# AQ4: P2 live boundary and P3 stopped warm-A proof

2026-11-10. Review candidate on accepted/published AQ3 `fc3682707c5bedca297b7c80db2ebc463863803d`. Six support source/test files plus this report/pins. No commit/push, new allocation, Prepare/Run/WholeP, production image/preparation edits or historical private/raw evidence access. Preconditions, call order and exact selectors were recorded before implementation in .tmp/s54-block4aq4-mixed-boundaries-01/preconditions.md.

## Result

P2 retains original traffic, ACK controls, waits and milestones. The existing final wait now uses four actual IPC rows: current A C1 next25/16, queue0/debt1/pending8; frozen B C1 send/receive25/25, unacked9/pending0; both C2 next1/1 with current queue/debt/pending0 and legacy unacked/pending0. It checks actual schema context/phase/current role/mode without demanding absent legacy queue fields. A24 delivered=false and frozen HVI1 evidence remain. The final retained rows include C2 and original A migration source.

p2_sources uses unchanged AN checked lookup, asserts retained A endpoint still17/16 and C2=1/1, and explicitly requires no B witness, Ready, registered migration endpoint or successful AN source. A small predicate reads existing memory diagnostic actual_lifetimes: all registered/non-aborted A lives bind the AN source, every legacy B life remains unbound. It does not call final coverage, alter observer algorithms or add a registry. After B conversion the same predicate requires both sets bound to their respective AN sources.

P3 preserves existing stop B/stop A and explicitly requires stopped_reconciled before new private access. It captures B once via admitted capture_boundary, prepares LateB, validates original B identity, and retains this same Checked B for conversion. Before conversion, mixed_boundary::collect validates original-directory admission/ownership, current A view, Profile3 account/device and opens the actual exclusive owned repo. Its borrowed repository verifier compares source/context/role/physical mapping against A's earlier AN endpoint, requires global initialized and strict snapshot for both contexts, normal Ready L1 Head/Native, and never initializes absent metadata. C2's normal initialized next1/1 follows existing L1 Initialize semantics, which derive next from authenticated lane roots; no new metadata convention or cold fallback.

The collector reads bounded actual queue/debt/pending pages and authenticated root counts, exact pending page/point records, source lane/projection roots, repair state, blocked-fork control and receipt presence. It requires A C1 next25/16, queue0, one exact A24 debt, eight ordinary pending B17–B24 with no Head/quarantine/blocked state; repair/root/page cardinalities agree and cannot hide capped-page extras. AQ1 verify_pending compares signed IDs/raw bytes against retained B, excludes held B16 and requires the full17–24 set. B16–B24 readable receipts are absent. Exact C1 receipt roots23/15 and C2 roots0/0 agree with these milestones. C2 has next1/1, sources2/projections0, empty queues/debt/pending/receipts and legitimate initialized Native state.

Old saved-R1 logic is narrowly factored to Legacy::r1_rows. It reuses AQ1 signature verifier, checks retained genuine R1(A24) against the same exact A24 packet/ID/debt and absence of local receipt, and returns the original event/receipt_blake3/source_blake3/lane/ordinal report shape. The collector invokes it without a second competing repo open. No repository/lease escapes the returned report.

Only after warm verification and existing current-old encrypt collection does P3 consume the same Checked B through mandatory AQ2 conversion/strict cold Exact. Original retained A source and B's newly migrated source are checked through AN pair and memory bindings; A's migration endpoint is never replaced by its live25/16 state. Existing old-overlap/encrypt reconciliation, typed95 accounting and saved-r1 report precede unchanged ceremony/start/import order. The obsolete capture-and-convert migrate_p convenience wrapper is removed after both P paths now require explicit Checked handoff; migrate_p_checked visibility is widened only to its sibling P3. The closed retained P branch/common conversion/cold verification remain unchanged. F migrate and P1 are unchanged.

## Coverage and limits

Three exact stable selectors passed. New supplied IPC/endpoint/lifetime test has12 status,2 endpoint and7 memory-binding refusals. These are injected observations, not owned children or actual mixed transport. It checks wrong/missing context/role/mode/frontiers/queue/debt/pending, changed source/baseline and premature/missing/changed deferred bindings without repeating unchanged AN/AO matrices.

AP FirstA uses actual fresh real crypto/storage, both physical C1 slots, and now exercises the borrowed collector's strict rejection of an actual uninitialized cold repository. It also checks4 source/role/context/physical mapping refusals before the initialized gate (first slot). Existing AP phase/queue/cold assertions remain; generation/commit/backend-write/native-call checks show the new refusals do not modify cold storage.

AP LateB retains actual real identity/Engine/Native legacy packets, saved signed R1 and eight pending source rows for both slots. On its first slot, the same verify_rows predicate used by the collector passes using real AP packet/receipt data with explicitly supplied numeric metadata and pending flags, and refuses18 missing/extra/wrong packet/debt/queue/pending/Head/quarantine/blocked/receipt/C2/repair observations. These supplied rows are not a warm L1 repository or an earlier A17/16 migration source. Existing shared signed pending, phase, debt/receipt, cold mapping and source/binding checks also pass.

No ready legitimate fixture was found for actual migrated A17/16 advancing to warm live25/16 with this checkpoint. AP late-A is still a legacy Engine and its migration source would be25/16; the existing AF global initializer supplies other frontiers. Neither substitutes for the required warm checkpoint. No new runner or durable target state was constructed. The full owned Profile3 collector wrapper and its positive initialized warm branch are compiled/static-reviewed but not positively executed. Actual P2 IPC, P3 full collector, mixed current-A/legacy-B transport and complete Controller/WholeP remain future authorized full-run obligations. Cold refusal, supplied-row positives and real legacy packet authority do not establish that coverage.

Static-order.json verifies actual P2 unchanged traffic/controls/milestones, P3 stopped/admitted single B capture/prepare/warm return/encrypt collection/same Checked conversion/AN+memory/typed95/ceremony order, readonly collector API calls, initialized-before-snapshot and no handle overlap by scope return. P0/P1/P4/P5/P6, common migration, current-old encrypt function, AQ1/AQ2 comparator/cold code and lifecycle/memory/interval files match the parent. Planned24 lifetimes and original deadlines/Guards remain unchanged; these are static/planned facts, not observed runtime counts.

## Verification and delivery

Fresh administration .tmp/s54-block4aq4-mixed-boundaries-01, pinned Rust1.98.0/pwsh, offline/locked verification profile, Jobs2/BelowNormal; no-run compile then scripts/run-cargo-tests-stable.ps1 exact selectors. Fresh AP fixture instances use the existing factory's unique fresh directories and remain retained; no historical fixture was opened. Final stable harness 36059648B SHA256 e850699ddb6655dba66c853e648475846fd1115286e959b3fdb8e5446a02a86d at E:\AI-Projects\kilogram-p2p-messenger\target\s54-diagnostics-source-checks\verification\deps\kilogram-cli-test-migration_s5_process-tests.exe. Strict CLI all-target Clippy passes literal -- -D warnings. Scoped formatting/diff/final successful diagnostic process inventory pass. Final source/tool/image pins remain unchanged through focused execution.

| Step | Owner/child exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- |
| format-01 | 1/1 | 0.916 | 570/600/30 |
| format-02 | 0/0 | 0.856 | 570/600/30 |
| compile-01 | 0/0 | 40.498 | 3600/3660/60 |
| clippy-01 | 0/0 | 17.054 | 3600/3660/60 |
| rust-negatives-01 | 0/0 | 4.111 | 570/600/30 |
| rust-early-01 | 0/0 | 140.844 | 570/600/30 |
| rust-late-01 | 0/0 | 224.437 | 570/600/30 |
| seal-01 | 0/0 | 1.316 | 570/600/30 |

format-01 failed on two missing closing delimiters; source corrected before format-02/compile, failed receipt/logs preserved. No compile/test/Clippy failure or focused test retry occurred. All application owner receipts have actual Job membership, child wait/exit, bounded stdout/stderr EOF, no expiry and empty post-cleanup memberships. Forced owned residual cleanup: {'compile-01': [48344]}; residual application identity is unknown, verified membership/cleanup after=[] are recorded. Other cleanup was empty/unforced. Actual application chunks, Cargo argv, source/image pins, exact selectors and raw diagnostic logs accompany this report.

Candidate: migration_s54_source.rs (module/shared R1 row check), early.rs (P2 predicates/checked visibility), transition.rs (P3 collector/retained B), p_storage_tests.rs (cold/supplied-row checks), new mixed_boundary.rs and mixed_boundary_tests.rs, plus report/pins. source.patch includes both new files and passes reverse-apply --check against the worktree. HEAD/index/navigation stay at their fresh baseline; unrelated dirty/root task/review files excluded. No F/P1/adapter or broad observer matrix rerun. Stop for root review before publication or allocation.
