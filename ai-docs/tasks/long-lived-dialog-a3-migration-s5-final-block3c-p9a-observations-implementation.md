# P9A — bounded page and receipt observations

2026-11-02. Review-ready supporting increment on `9e2b1ced63a90d0e8bcae9978942e4c573d78f8d`. Implements only `long-lived-dialog-a3-migration-s5-final-block3c-p9a-observations-task-01.md` (5777B/SHA256 `b081ce7a13d086ca656974c4a8582448fcb5109b036b0218e1b2cdb7c500736a`). The accepted contract13913B/SHA256 `5639214e6aca95070be42655f61b24d30033e0de1f4d87054928b69ad39dfa7b` and root clarification3702B/SHA256 `aa9cf631e62e9a288ff2c287cfb32a3b01348843265af514766c72d1ce338cf8` are unchanged. The clarification supersedes the original transient next450 polling suggestion; no head pause was added.

**P9B/controller/terminal reader, real P0–P9/product/F/P/enrollment/private fixtures, field staging, P10/P11 and commit/push remain unimplemented or unexecuted in this increment.** No real ACK loss, connection cancellation, retransmission, fairness or deadline fit is claimed. Only fresh pure metadata/control-file tests ran.

## Exact source scope

Paths below are under `E:/AI-Projects/kilogram-p2p-messenger/`. Final `.tmp/s54-block3c-p9a-02/review.json` pins exactly12 Rust paths (raw SHA256/bytes and normalized Git blobs), this report, accepted contracts, captures/receipts/helpers/logs, preimages and all final artifacts. The callback provides its immutable final hash.

| Path | Result |
| --- | --- |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/control.rs` | Optional hold_ack_from; closed CatchUp/hold/B/ordinal constraints. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/receipts.rs` | Pure parser for actual built AckBatch receipts, selector and compact actual fact serializer. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification.rs` | Narrow integration into the existing ACK hook; early default-off/no-hold return and original pause ownership. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/receipts_tests.rs` | Four new parser/selector/schema/max16 envelope tests. |
| `apps/kilogram-cli/src/ordered_dialog/runtime7/verification/tests.rs` | Existing ACK test now uses the actual signed Receipt1 representation and valid43-byte SID. |
| `apps/kilogram-cli/tests/support/migration_s54_oracle.rs` | Owner/source totals, actual incarnation registration/current view, opaque phase baseline and checked local-target query; old global API preserved. |
| `apps/kilogram-cli/tests/support/migration_s54_observation_types.rs` | Bounded typed process stamp/page/receipt/ACK records and exact field/tuple parsing. |
| `apps/kilogram-cli/tests/support/migration_s54_pause_observations.rs` | Validated page/head/tail accessors, ACK arming/current record/markers; no extra journal. |
| `apps/kilogram-cli/tests/support/migration_s54_observations.rs` | Register the real start with Pages; pass actual native sequence into typed observations. |
| `apps/kilogram-cli/tests/support/migration_s54_pressure_tests.rs` | Update the affected P8 ACK fixture to the actual new ten-field/six-element-tuple schema. |
| `apps/kilogram-cli/tests/support/migration_s54_p9a_tests.rs` | Six new owner/baseline/typed-page/ACK interval and stale-marker pure tests. |
| `apps/kilogram-cli/tests/support/migration_s54_p5.rs` | Declare the pure P9A test module only. |

No network2/protocol2/storage/crypto/crates/Cargo/scripts changes. No P7 snapshot/guards, P8 consuming controller or S53 body changes. No normal wire/quota/timeout/pacing change. Three unrelated navigation files and historical untracked files are preserved; index is empty.

## Page identity, baseline and typed evidence

Decoder's actual zero `start` now registers its already-validated Incarnation with Pages after Operations.start. Registration records actual owner/source ID and PID/nonce, initializes only the real new local counter, and rejects zero identities, reused starts/nonces and contradictory mappings under the existing32-incarnation bound. Owner totals survive PID/nonce changes. Queries require the latest registered incarnation of that same source, rejecting unknown/stale/mismatched keys. Existing global total, operation dedup and legacy local_target behavior are retained; direct legacy accepted callers still register on a genuine normal-page acceptance.

An opaque PageBaseline captures the current observed owner total. `phase_total(key,base)` subtracts that same owner's baseline; `phase_local_target(key,base,target)` derives remaining phase pages plus the actual current local counter using checked arithmetic and existing bounds. No reset or expected-count insertion occurs. Earlier B pages, another owner's pages and head/replayed pages cannot shift P9 targets. The new regression starts from nonzero B history, adds A noise, excludes head/replay, reaches phase2 and verifies that a registered fresh B incarnation derives local3 for phase5. The unchanged global page2/reopen3 test also passes.

PageObservation carries actual incarnation, native sequence, decoder line ordinal, logical context, binding/lease generation, local page/span/novelty/head, committed operation/generation and original bounded facts. Exact eleven-field parsing requires even the explicit nullable grant field; a missing field cannot be replaced by an unknown key of equal object size. A barrier must match the preceding actual forward normal_page's facts/incarnation/native sequence and occur after it, once. It never increments Pages. Binding checks use the genuinely accepted Head; markers additionally require the same current process/source/nonce, exact stored sequence/facts, current binding/generation and <=4096B full envelope.

One latest C1 novel funded-head record and its first subsequent normal-tail record remain available. They preserve actual order and spans, so P9B can check head385 and first tail450 at the stable page2 barrier without trying to poll a transient Bnext450/pending0 status. Neither a new runtime pause nor an expected head/tail list is introduced. These are observation records, not Native commits.

## ACK schemas, selector and gate order

`hold_ack_from: Option<u64>` defaults to absent/null. Some requires CatchUp + hold_ack + lane1, ordinal1..4096. Partial release with hold_ack=false and a remaining selector is invalid; callers clear both in one complete atomic schedule update. Existing debug/release refusal, deny-unknown/cap, namespace/parent/file pinning, source64 rules and finite pause timers remain.

Runtime hook order: existing observer-enabled/AckBatch check → current context/role schedule → return if not holding → parse the actual already-built batch → optional selector → existing actual lease check → existing outside-Owner ACK pause. No new receipt/source storage read, signing, account/session construction or packet operation manufactures these facts. Production already verifies saved receipts against their sources before building the dispatch; this helper observes that result, rather than becoming another signature authority.

The parser uses ParsedReceipt1/ParsedReceipt2 and checks supported encoding, actual batch C/D/E/SID where represented, consistent Receipt2 Q, actual lane, positive bounded ordinal, unique lane/ordinal and EventId. It rejects tag/raw-format disagreement, malformed data, count>16, wrong scope/lane and conflicting tuples. **Receipt encoding and source format are separate:** a Receipt2 can acknowledge a format1 source. Mixed old/current batches remain valid. Receipt digests hash actual raw saved receipt bytes; the batch digest is the existing hash of the actual encoded tagged receipt list.

Actual ACK facts now have exactly ten fields:

`context, binding, lease_generation, request, query_reply, receipt_count, saved_receipts_digest, lane, receipts, hold_ack_from`.

Each compact receipt tuple is:

`[receipt_encoding, source_format, lane, ordinal, EventId_hex, receipt_bytes_digest_hex]`.

All IDs/digests have canonical32-byte hex representation and request is16-byte hex. Count must equal the nonempty tuple list length<=16; unsupported/duplicate/conflicting associations are rejected. The tuple list contains only public source metadata/digests, never private keys, plaintext or raw receipt signatures.

Absent/null selector preserves holding any supported nonempty push/query reply. A present selector matches only actual B push, lane0, query_reply=false, with at least one parsed receipt at/after the threshold. Old-only duplicates, opposite-lane B640 queries/replies and empty batches do not match. It delays a valid dispatch only; cursor traversal, AckProgress persistence, receipt acceptance and15s production RPC/incoming bounds are unchanged.

The maximal-schema test constructs actual signed Receipt2 representations for16 distinct sources4081..4096, parses them through the production helper, serializes the production facts and complete Observer envelope with full-width IDs, maximum PID/generation/sequence widths, and writes/reads it through the real pinned Location.marker. The full envelope fits the unchanged4096B cap. The17-item case fails. Other tests use actual signed legacy/current representations and exercise mixed receipt/source encodings, old-only/target/query/wrong-lane/default behavior. This is parser/serialization evidence, not a full authority-chain or network test.

## Decoder arming and P9B API

AckObservation retains actual process stamp, context, binding/generation, request, push/query classification, selector, typed receipt tuples, batch digest and original exact facts. `arm_ack` requires the actual latest accepted Head's context/binding/generation and a live decoded incarnation; it clears the old current ACK record and records the decoder interval. Targeted observations require an arm, matching interval/configuration, a later actual log line, eligible B push and one current targeted record. The prior request cannot satisfy a new observation. Marker validation binds to that exact new record/current Head; a cancelled old binding/request is not relabelled as a regenerated dispatch.

| API for future P9B | Use |
| --- | --- |
| `Decoder.incarnation()` | Actual started/live decoder identity; do not construct expected PID/nonce keys. |
| `Pages.owner_total(key)`, `baseline(key)` | Capture the actual B source baseline while prefix is suspended. |
| `Pages.phase_total(key,base)`, `phase_local_target(key,base,2/5)` | Owner-scoped progress/next local target; use each fresh decoder's real key. Legacy global local_target is not P9's phase target. |
| `accepted_head_page()`, `first_tail_after_head()` | Ordered genuine page evidence for head385/first tail, independent of transient status polling. |
| `paused_page()`, `page_marker(marker)` | Typed committed barrier and exact current envelope validation, without double count. |
| `arm_ack(context,binding,generation,from)` | Arm while the relevant ACK paths remain disabled; clear/rearm after fresh accepted Head readiness. |
| `paused_ack()`, `ack_marker(marker)` | Capture actual mixed receipts/payload and validate current pause; compare old/new payloads separately from binding/request. |

P9B must keep ACK suppressed while establishing fresh admission/accepted Head, arm decoder and S54 hold/selector **before** enabling the OD1 ACK flag, and re-suppress ACK during A restart until the fresh binding is observed/rearmed. This avoids an unarmed fresh ACK racing ahead of controller polling. The typed test models old-record clearing, stale same-request refusal and a genuinely distinct current Head/binding/request with the same receipt tuple/digest metadata. It does **not** simulate Iroh cancellation. Actual connection-close/dispatch/permit release, persisted-receipt resend, terminal source comparisons, page interruptions and all consuming-controller ownership remain explicitly for P9B.

Records remain bounded: existing last normal page per context, one current barrier, one C1 novel head/first tail, one current ACK/arm/last request. No independent event journal or generic parser framework was added. All observations remain non-Native and cannot retire debt or advance Operations; pure final zero-counter checks confirm this distinction.

## Actual checks and immutable evidence

Final evidence directory: `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3c-p9a-02/`. All final native receipts verify child cleanup. Commands use locked/offline verification-scale/x64, Jobs2/BelowNormal, stable executable paths and one staging/build lane. Watchdogs: compile1190/1200s, Clippy590/600s, pure/fmt230/240s. Sources stayed frozen during each compile; no provenance mismatch was ignored.

| Check | Actual outcome |
| --- | --- |
| Final CLI/all-target strict Clippy |exit0 /8.7541109s, literal `-- -D warnings`. |
| Final stable compile-only |exit0 /74.1414892s;501 units, equal before/after inputs; normal CLI test=false/opt3/debug assertions/overflow checks. |
| Final decoder/oracle tests |exit0 /14.4894846s;18 passed/0 failed/ignored, independent frozen guard true,501/501 units fresh. |
| Carried unchanged runtime tests |9 passed/0 failed in initial `.tmp/s54-block3c-p9a-01/pure-01.stdout.log`; exact final library bytes/SHA and five runtime source files rechecked by review.py. Not redundantly rerun. |
| Final workspace format check |exit0 /4.290532s; read-only `cargo fmt --all -- --check`, no diff. |
| Source/evidence review |exit0;12 source paths/Git blobs,27 exact names, contract/task hashes, prior9-PASS identity proof, all preimages/final artifacts/four field images, restricted diff and empty index. |

The27 accepted names are10 new (four runtime receipt/schema/max-marker +six P9A owner/page/ACK APIs) and17 affected existing (five runtime defaults/schema/ACK, five P8 decoder tests, six earlier decoder regressions and the global page/reopen regression). test-selection-run.json records all27; test-selection-final.json selects only the final18 and names the carried9. Other52 S54 tests, unchanged P7/P8 controller suites, broad G1–G4/100k/cut tests and real scenarios were excluded.

Initial P9A Clippy-01/02 passed31.2173729/8.6639981s; compile-01 passed545.8695841s with equal inputs. Initial pure aggregate exited1/12.8984959s: runtime9 passed; integration17 passed/one failed. The new stale-ACK marker test incorrectly tried to replace batch digest `[9;32]` with the same `[9;32]`, so the correctly unchanged marker was accepted. Only that negative test replacement changed to a distinct digest. Final18 passed above. The earlier aggregate remains **failed**, never a27-test aggregate PASS; only the exact unchanged9 runtime section is reused after library/source identity proof. All initial logs/captures/receipts and intermediate sources/EXEs remain pinned.

Accepted P8 artifacts/eight source preimages were saved before any rebuild in block3c-p9a-01/preimages. The initial P9A twelve sources/seven EXEs were saved before the test fix in block3c-p9a-02/preimages; review.py verifies both manifests and archived hashes. No accepted field EXE was staged, replaced or run. No Rust/source changes followed final compile/pure/fmt.

Final normal CLI50473984B/SHA256 `f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`; stable library32656384B/`2fcbfb6529fad0f156cbed869ea0455e864119dcecef94eba508aa06e588762f`; stable migration harness16601600B/`4fedca5de5cf0139f9ea897cf720cf5b25236ffab14537b8dcc2eab747b4a3ca`. Compile capture SHA256 `32de41a54e3db3e5dcde7bd10a90c2feddc5288c38036f389bcf4178aea0ee9f`; final pure capture `220317a64e3aa3d6751ef9bf92321b94c312005e14258cf625345a461642986b`. All six stable artifacts and exact helpers/guards are in review.json.

To reproduce, copy bounded helpers into a fresh immutable directory, update base paths, compile without execution, derive the independent frozen manifest from that exact capture, then run the selected pure group(s). Reproduce the carried runtime9 if their artifact/source identity changes; do not claim reuse without that proof. This is source/pure acceptance only. Real cancellation/resend, page/ACK races, fairness, persistent end-state and whole/pause feasibility remain unexecuted until separate dispatch. One review-ready callback ends P9A; no commit/push or P9B work follows automatically.
