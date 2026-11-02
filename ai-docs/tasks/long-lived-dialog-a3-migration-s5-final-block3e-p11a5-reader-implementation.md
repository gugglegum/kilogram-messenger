# P11A5: dormant stopped source and receipt repository adapter

2026-11-02. Baseline `1f2e950ad991ef353a4b0ecdfa2976fe1a97e872`, parent `6349b48e62d67faedc93a1ec99694cc12ff03049`, tree `150304f2eb15e5221ed8920a9ee11ae5c96173e3`. Task10953B/SHA256 `d5fbfd60162e37ed7fdb9e0fa57a8d130681bc62893c814ca5caf24d5f6ae293`. P11 contract and phase boundary unchanged.

Implemented the dormant same-controller source/receipt cut reader. It binds A1's retained actual latest normal pair, original Legacy identity/Ready mapping, stored authenticated selected scopes, actual A3 records, complete A4 graph, retained final history and immutable Legacy/P7/B640/R1/ACK witnesses. **The actual repository adapter, Profile3 loading/opening, full authenticated origin constructor and integrated traversal have not been executed.** Focused tests exercise real new validation functions with labelled synthetic metadata and reuse signed A3/A4 seams. No actual repository/state/profile/private fixture, Native restore/constructors/packet crypto, node/CLI/relay/network/enrollment/F/P/field scenario, phase advancement, Operations population, durable whole PASS or commit/push occurred.

## Exact change

15 source paths/raw159844B. Five reader/support test files plus two more test/count files are new; eight existing files add narrow registration/normal-closure retention/immutable comparison seams. Existing A3 verifier, A4 graph, P7/P9 checks, killed reconciliation, product writers/protocol/config/status schema remain unchanged. The sole product API addition is a separate numeric read-only L1SourceCounts accessor. Exact SHA256 and normalized Git blobs are in executor-review.json; no unrelated paths are candidates.

| Source | Bytes | SHA256 |
| --- | --- | --- |
| apps/kilogram-cli/tests/support/migration_s54_control.rs | 19869 | 77525e0bf290c488b825d18ff4b243514a8fae57bd91f01c721dc88b1b4420c3 |
| apps/kilogram-cli/tests/support/migration_s54_normal.rs | 14418 | 9ee7695ccb667d80e5395a17178c07d501aff8d8c9cadd579129c20fdf99a689 |
| apps/kilogram-cli/tests/support/migration_s54_normal_tests.rs | 5878 | 4cbbf6a7ec7b534314a342eda9ba4a9899a01025b898b95b056931b078e4247a |
| apps/kilogram-cli/tests/support/migration_s54_offline.rs | 10215 | 518793ba697cb42072c38f6b7d02861aaf34ac827721e0b1d4f16c55364c9964 |
| apps/kilogram-cli/tests/support/migration_s54_p5.rs | 5011 | 5fdc05a359e4659094fd4cb49def143544f48153de8fbda22e13f87cd28a618e |
| apps/kilogram-cli/tests/support/migration_s54_reader.rs | 5855 | 2b8ae73ea65d61aecd09bfd945725c857823e3a9ce8273edafa390d769eb9b18 |
| apps/kilogram-cli/tests/support/migration_s54_reader_gates.rs | 5376 | e24ff2a40995e3119504dd449647532f2ff1032fc31bf7c23817134a38913f2d |
| apps/kilogram-cli/tests/support/migration_s54_reader_owner.rs | 20546 | fd840d22517fb4706fd43e6a7ab87c209c579dcdc7d5123687046e301d9c5bf0 |
| apps/kilogram-cli/tests/support/migration_s54_reader_tests.rs | 9217 | a77f2e7de99c8d9d5b7320789ff3f9b283b9889212017aee58dfe8f201928c93 |
| apps/kilogram-cli/tests/support/migration_s54_retained.rs | 17031 | 24195fa9f30ed13decbd4901bd88f39010d6238913522c23112fd2c6549d7e39 |
| apps/kilogram-cli/tests/support/migration_s54_retained_terminal_tests.rs | 2181 | 3f63f5c34a373786d23271ddd18555b26f92a0929641aeca2dacfa44d2685346 |
| apps/kilogram-cli/tests/support/migration_s54_source.rs | 25445 | 895f98ab7706f3489c6431321056abe26d986e4d2e70ebe3ddb963c99b4f45d6 |
| apps/kilogram-cli/tests/support/migration_s54_source_terminal_tests.rs | 1406 | 16dd53ec471d6bbf1beebef7f31b61e1da6ce8b05be51ed10fd972cd83d1c2f4 |
| crates/kilogram-state/src/vault/indexed_dialog/migration7/live1.rs | 15167 | 2e28de7e85ccd4d5eb4160ae5e91b78e7465807b5d5c4015eb6fa791fbe7aafa |
| crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/source_counts.rs | 2229 | 1c745c32d0b7bfbcfc1abd623d661ca8a93ca07e3e0410820d964c49b1038d6c |

## Source premises and actual completeness argument

Traced Profile3::load/open in runtime7/config.rs; L1Repository read.rs/existing.rs/history.rs and source/migration/generation/last_commit APIs; namespace Key::slot and Summary root layout in indexed_dialog/types.rs; migration7/copy.rs kinds11/12/16/17/20; live1/data.rs Text projection/own request materialization; live1/ops.rs queue-index preparation; legacy outbox.rs own-send request writing; credits.rs pending/grant retirement; existing VerifiedExistingScope1 and selected protocol proofs. No real store was opened to obtain expectations.

Namespace semantics per logical context, at either owner:

| Namespace | C1 | C2 | Source reason |
| --- | --- | --- | --- |
| Event | 1282 | 66 | both Text lanes plus two actual genesis |
| LaneA / LaneB | 641 each | 33 each | ordinal0 genesis plus complete Text positions |
| Projection | 1280 | 64 | Text only, own and received; data.rs kind3 and imported verified projection lowering |
| Request | 640 | 32 | own sender Text only; legacy own-send and current own materialization, not received Text or upgrade intents |
| ReceiptA / ReceiptB | 640 each | 32 each | saved opposite-owner/accepted own receipts for Text; no genesis receipts |
| Queue / Undelivered / Pending | 0 | 0 | drained final cut, independently counted and bounded EOF checked |

Existing counts() already reads authenticated Event/Queue/Undelivered/Pending/Receipt summaries. New source_counts() returns only LaneA/B, Projection and Request numeric counts through the same authenticated Root summaries and actual Key::slot mapping. It checks occupied slot, exposes no keys/values/raw database handle and does not alter RuntimeCounts/status. The new pure state test seeds real Summary values at independent fixed format5 root positions1/2/3/4 and25/26/27/28, with irrelevant Event counts, then tests the actual reader/mapping and invalid indices; it is not a database-read claim.

For each namespace, the adapter verifies all expected distinct actual entries and requires its independent authenticated cardinality. Thus a far suffix, extra hash key, missing/replaced index or unrelated sender request cannot hide beyond an immediate next-ordinal probe. Actual event IDs and requests are unique through A4. Both original genesis are read/compared as actual sources. Immediate suffix/genesis-receipt absence and empty work pages are additional bounded checks, not the completeness basis. No aggregate accounting total substitutes for per-namespace counts.

## Closure, original owner and one-lock traversal

A1 now retains a private pair of typed Closure facts only after both actual normal waits and both final/EOF/source reconciliations succeed. Each retains actual source/PID/nonce, final sequence/counts, EOF, stopped generation/last_commit/source point and actual outcome index. Any start or kill invalidates it; killed behavior is otherwise unchanged. normal_pair() requires stopped/reconciled original current/frozen handles, a live unfinalized owned relay, latest per-owner nonempty normal exit0/wait/ack/resolved outcomes matching the typed pair, exact closed Operations streams and resolved closed summary. There is no caller-provided success flag or empty-outcome acceptance. Synthetic tests call these real pair/outcome gates; the OS/Controller accessor path remains source-reviewed.

reader::read accepts the actual retained P10B Continuation, derives final original C1 maps from P9 plus the sole original B640 delivery and unchanged original C2 maps, and verifies their retained P10 final digests. It binds both lost/replayed ACK tuple/batch digests. It checks the actual last A1 pair before any owner open, retains the same relay PID, brackets each owner with normal_pair()/Controller checks and keeps the inherited original deadline. Each owner is opened exactly once and dropped before the other. This same per-owner function is the seam for the later in-memory Native slice; no third stop/checkpoint, process manager or extra read checkpoint is added.

Before opening, the adapter restricts the node through source::owned, checks the full effective profile/config Value hash against retained P10B facts and exact node/profile paths/account/device/relay URL. Profile3::open enforces lock, root account/claim/device/config/source/migration association. Original Legacy helpers compare canonical original state and zeroizing device secrets without exposing fields or loading/restoring Native. Actual source IDs match original/migration witnesses and A1 closure; original Ready binds migration ID and logical-to-physical context/role slots. Both enabled contexts must equal the original IDs. Profile3.route is used for relay-only; legacy route_policy remains unchanged/direct-only.

Each context loads spec/head/metadata, not snapshot/opaque Native. Original Spec, D/E/SID/Offer/Grant and genesis sources are compared to original Legacy witnesses. Existing helper authenticates actual Spec/Consent1/HelloProof/Offer/Grant/pair/genesis/Ready export. Actual stored Q proof is verified against that origin; both modes must be Active with original cuts25/25 or33/33 and retained current consent/control refs. Actual current Consent2 is independently verified and must equal the consent bytes embedded in Q. Stored P/A/K bytes must exactly equal Q's authenticated chain; extra Committed is verified with the actual selected scope/B signer. Stored mode is not claimed as live admission authority.

## Actual source, receipt, history and retained witnesses

One source/projection/receipt at a time, <=32 for existing pages/ACK batch. Each record checks original deadline and the same live relay; actual history pages also call Controller.check. Author and receiver bundles are obtained from actual authenticated auth records and verified at root. Source format follows verified cuts; receipt version/auth is decoded independently. Actual projection migration/source/request binding is read; actual sender request points to the verified event/body, while received Text must lack a sender-only request. A3 verifies signed source and independently signed receipt against retained original EventID/request/body/scope, then its actual normalized Record enters A4. No expected EventID/body association is copied from the unverified source being tested.

All original Legacy source bytes and saved receipts present in either original or migration witness are checked during the same traversal, including genesis. P7's narrow new terminal seam checks original immutable scope/source/migration and actual anchor384/A385..640 ordinal/request/previous/raw/cipher/body commitments; it does not call its old mutable Native/C2 comparison. Original C2 legacy64 associations come from retained maps plus A3 verification. Original B640 request/EventID/raw/cipher commitment is checked with a now-verified receipt at both owners; source::debt is never called. Original saved R1(A24) bytes/hash and accepted-R1 retained facts must match actual final receipt. Original ACK tuples match verified source/receipt facts; existing saved_ack validates every actual retained receipt encoding/raw-byte digest and ordered batch digest.

Exact terminal counts, next/tails/receipts, zero work/control debt and no pending Head/repair obligation are required. A completed request-index preparation may retain its after watermark; an exhausted grant with zero available bitmap is harmless, while remaining available repair reservations fail. These follow actual writer semantics, not invented empty-field expectations. Stored next/tails are checked by A4 against derived actual tails. Actual repository history uses <=32 pages, stable view/frontiers, bounded cursor progress/EOF, original association checks and exact A4 ordered identities. Frozen23 is deliberately left to original-cursor P11 composition.

Typed returned Owner facts preserve original source/migration/account/claim/device/config/profile/slot identities, closure, actual root generation/last_commit, complete graph/receipt commitments, history and witness commitments, with no retained repository/lock/raw/projection/secret. Cross-owner equality compares immutable graph/history/witness content; owner-specific identities/generations are retained separately. same_content compares each original owner across cuts without treating new PID/nonce/root generation as shared immutable content. Actual stopped root/last_commit/source point must equal A1 reconciliation. Plain projection body and raw history pages use Zeroizing; parsed history text is wiped on drop, and comparisons borrow history fields rather than cloning actual plaintext. Entire read, including failure, brackets actual native_call_counts equality; there is no Native constructor/restore/packet call in this adapter.

## Actual checks and preserved failures

Immutable raw base `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3e-p11a5-01/`. Eight pre-existing compiler images including the state unit harness and nine source preimages were preserved before relevant edits/overwrites. Complete15-source revisions of failed Clippy01/02 are preserved with immutable receipts/logs. No failed Rust test execution or overwritten receipt.

| Final check | Child exit | Seconds |
| --- | --- | --- |
| fmt-05 | 0 | 5.1046134 |
| clippy-07 | 0 | 15.0727183 |
| compile-02 | 0 | 131.9982803 |
| pure-01 | 0 | 21.0066855 |

Clippy01 stopped on json! array-repeat macro syntax. Clippy02 found two Value-vs-Option comparisons and seven incorrect Continuation-depth field accesses. Corrected without weakening checks. Clippy03 passed; then the root-summary test was made independent, Clippy04 passed; per-record relay/deadline and per-page Controller checks were added, Clippy05 passed; borrowed history comparisons removed an avoidable plaintext clone, fmt04/Clippy06 passed. Initial compile01 succeeded in708.9367143s, with all15 compiled sources preserved. Explicit actual relay-child liveness and early Zeroizing ownership on projection binding errors were then strengthened; final fmt05/Clippy07 passed before final compile02. All five format checks passed. An attempted no-op patch with a nonexistent context was rejected before mutation; no source/test state was lost. No pure wrapper ran before the successful final compile capture existed.

Literal strict Clippy checks CLI+state/all targets with locked/offline `-- -D warnings`. Stable compile/pure include CLI+transport+state, x64/verification-scale, Jobs2/BelowNormal/fixed paths. Finite watchdogs compile1190/1200,Clippy590/600,pure/fmt230/240 seconds. Exact30 selected PASS/0fail/0ignored:29 CLI and1 state.10 new tests cover actual normal-pair/outcome, Ready mapping, numeric root-summary mapping, counts/metadata/Active controls, own request/projection/B640, history, Legacy/P7 immutable gates.20 affected tests cover7 A1 normal lifecycle/decoder,8 A4 graph,3 P9 immutable/ACK and2 A3 signed source/independent receipt seams. No unrelated relay/P9 phase replay. Full names/arguments in test-selection.json/pure.stdout.log.

All504 compiler units and583 source inputs match before/after final compile/pure and current source; all pure units fresh and independent frozen guard verified. All10 current compiler images are pinned in executor review/provenance. Among the eight archived image preimages, outputs changed:6; unchanged:2. The two additional state integration harness paths were compiled and selected zero tests; their current hashes are pinned, but no claim is made that their previous cache contents were archived before stable publication. No field-runtime evidence image was overwritten. New public read-only accessor can affect linked compiler outputs; no unchanged-binary claim substitutes for actual hashes. Four frozen field images remain unchanged/unexecuted. Exact review56508B/SHA256 `20319e2ccf6f2ee4a7f04e4a2f0f61938d7acadd907fe73d1af6a74a3821355f`.

Durable p11a5-evidence retains receipts/logs, full compiler source pins, executor review/provenance and bounded reproduction helpers. Full initial/final raw captures/independent frozen manifest/EXE-source preimages/failed and first-compiled revisions stay at immutable raw base. Reproduce with a fresh base and updated immutable receipt paths: project-edition format, strict lint, compile without Run, wait for successful capture, derive independent frozen manifest, then run selected stable pure tests. Historical review.py audits this attempt's IDs.

## Remaining work and stop

The complete adapter/open/actual-scope/actual-repository traversal paths are source-reviewed and compiled only; synthetic gate and signing/graph checks do not establish a real database was read. Later integration must execute both planned cuts under original lifecycle ownership, extend the same per-owner opening with the Native checkpoint, verify current live admission and original Frozen23 across restart, then compose Cut1/restart/Cut2, memory/sanitization and final runner/provenance/accounting. No phase12, relay stop, third checkpoint or whole S5.4 feasibility result. Navigation3/root Current and unrelated historical work preserved; index empty, no commit/push. Stop for root implementation review.
