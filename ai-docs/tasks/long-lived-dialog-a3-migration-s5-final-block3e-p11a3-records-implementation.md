# P11A3: one-record authenticated Text and independent receipt facts

2026-11-02. Baseline `bd13f33e05727dc971267bfb7dbc040a8504cdf7`, parent `5fef1926d532d2a5b926739d71af7680818550e8`, tree `e3b3539e0de80c27cf24713766f05bd146153647`. Task9177B/SHA256 `1a710872f3bf29943883a6e6a6cde297246ea42d6cbe6a8ba064aaf85c660f52`; accepted P11 contract/grouping unchanged.

Dormant test-support helper verifies one actual bounded signed Text/receipt pair and its supplied actual projection against retained original public association. It returns normalized immutable facts for the later terminal reader. **No real state/profile/repository/private fixture, Native restore/constructors/packet crypto, owner/CLI/relay/network/enrollment/F/P/field action, phase composition, commit/push or whole result occurred.** No full graph/traversal/Native proof is claimed.

## Exact source change

Four paths/raw34122B under `apps/kilogram-cli/tests/support/`; exact normalized Git blobs are in executor-review.json. Existing p5 adds only the dormant records module declaration. No earlier helper/body/product/runtime/protocol/crypto/state-writer change; P9 retained.read keeps its C2-L1/debt1/no-B640 predicates untouched.

| Source | Bytes | SHA256 |
| --- | --- | --- |
| migration_s54_p5.rs | 4838 | 9fda96f9a9c9c51366e53b732d7245089fa65749fd8f838c875563ec8cbab7db |
| migration_s54_records.rs | 6676 | 3f8bedd49a1b8391ad048e66bdaa269f82acdb5db86cdd26dcdaecba8f3e6e6f |
| migration_s54_records_fixture.rs | 9755 | c10fa112a1574137d3797521c312b47739d52b0597100864d8523cf92f22014a |
| migration_s54_records_tests.rs | 12853 | 043e7bd93c877f2bbc71d942087c56ef4e7cf80a0a29d439ac3df2ce27fa5e7c |

records.rs contains the per-record helper, borrowed input descriptors and normalized Record/same comparison. records_fixture.rs reuses the existing public protocol Actors/selection/signing test construction; records_tests.rs exercises the real new helper. No generic fixture or crypto framework.

## Protocol premises checked before fixture construction

Reviewed current ordered_dialog_v1.rs Event/Core/sign/parse and native header checks; ordered_dialog_v2.rs Core2/ParsedEvent2/sign_event; frame.rs TaggedItem.verify_event; authority.rs selected Receipt1/Receipt2 and independent TaggedItem.verify_receipt; mode.rs verified Existing proof/old genesis+Ready/cuts/selection; runtime7/data2.rs declared/facts/receipt2; P7/P9 retained source and saved-ACK primitives. Protocol fixtures in crates/kilogram-protocol/tests/support/ordered_dialog_v2.rs are already included by CLI component tests and are reused unchanged.

Actual premises: selected format comes from the verified cuts; Current source requires selected Q, actual author cert/AuthRecords and signed scope. Receipt2's internal source format can be1, independently of receipt encoding2. Selected Receipt1.verify supports a legacy source under the same original session and opposite receiver; it cannot acknowledge Event2. Existing-mode proof construction requires signed previous genesis/Ready, not an unvalidated selection DTO. Core2 requires the previous Text parent at n>1; parent<=2/order checks are real protocol checks. Source sequence remains author-local. Header parsing is separate from Native MAC/decryption.

Fixtures create real root-signed credentials/membership/Consent2/Propose/Accept/Select, signed public legacy genesis/Ready and verified StaticSelection2, with C1 cuts25/25 and C2 cuts33/33. Opaque Olm bytes use the public vodozemac0.10 Message codec vector and its documented PreKey protobuf wrapper; actual Packet.header/real Event parsers validate syntax and the derived SID. No OrderedAccount/OrderedSession constructor, Native restore or encrypt/decrypt is used. This fixture is signed protocol/codec evidence, **not** Native ciphertext/projection or enrollment provenance. The tests also compare actual native_call_counts before/after the successful matrix, unchanged.

## Actual helper binding and independent dispatch

Expected supplies the two original context IDs, canonical logical Request, retained actual EventID, D/E/SID and selected Q. Projection supplies actual event/request/text; Credentials supplies authenticated actual author/receiver bundles. Later traversal must obtain these actual stored associations; this helper does not open a repository or synthesize a stored projection.

Inputs are bounded first: source<=16KiB, receipt<=1024B, projection UTF-8 bytes<=4096. Revalidate Request context/role/ordinal (C1<=640/C2<=32) before indexing, require distinct original contexts and exact selected C/D/E/SID/Q/cuts. Decode the source's own encoding once and require C1<25/C2<=32 Event1 or C1>=25 Event2. TaggedItem.verify_event is the real signature/selection/author verifier; an error never retries another encoding.

Normalize verified source fields and require actual kind Text3/n>0, exact expected role/ordinal/EventID/request, previous present and parents<=2. Match supplied actual projection event/request/body to that source and the original deterministic request/body. The only helper-created plaintext body copy is Zeroizing; supplied projection text stays borrowed. No Native/key content is copied. Raw/cipher commitments are BLAKE3 of the actual verified source bytes and its actual signed packet field, matching the earlier retained commitment convention. Body commitment uses the accepted request-text tag on actual supplied projection text. **A matching projection is not cryptographic proof of ciphertext plaintext.** No verify_inner, decrypt or MAC claim is made.

Receipt encoding is obtained from its **own** bounded encoded structure/version, then passed to TaggedItem.verify_receipt. It is never inferred from source encoding. The selected real verifier checks source/scope/Q as applicable, opposite-role receiver cert/AuthRecords/signature and exact referenced EventID/lane/ordinal/source format. There is no parse/verification fallback and no replay of pre-Q acceptance. Both legacy saved Receipt1 and new Receipt2 for Event1 work; Receipt1 for Event2 fails.

Record returns only bounded source format/ID/C/D/E/SID/selected Q, role/n/request, signed previous/parents/sequence and author ref, actual raw/cipher/body commitments, independent receipt encoding/byte commitment and receiver ref. No raw source/receipt/plaintext/log/JSON/file export. Record.same compares all normalized fields, including signed graph facts, author sequence and actual raw/cipher/receipt commitments. It cannot silently reduce equality to projection/ID.

## Actual verification

Immutable raw base `E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block3e-p11a3-01/`. Eight preimages (seven accepted EXEs/untouched p5 source) were archived before editing and hash-verified. Failed Clippy01/02 receipts/logs and their complete four-source snapshots remain preserved. Final source was frozen for one compile and one targeted run; no failed Rust test execution or overwritten receipt.

| Actual final check | Child exit | Seconds |
| --- | --- | --- |
| clippy-03 | 0 | 30.391699 |
| compile-01 | 0 | 120.870008 |
| pure-01 | 0 | 16.2575165 |
| fmt-01 | 0 | 5.2118177 |

Clippy01 failed27 test assertion type errors: the reused protocol TestResult uses Box<dyn Error>, while anyhow ensure returns anyhow::Error. Test assertions now use assert, matching the protocol test style. Clippy02 then rejected one fixture expect; projection construction now returns Result and propagates its actual error. Clippy03 checked final source. No production/helper verification condition was weakened for these repairs.

Strict CLI/all-target Clippy uses literal locked/offline `-- -D warnings`. Stable runner compiles without execution and runs fixed EXEs, verification-scale/x64, Jobs2/BelowNormal, independent frozen manifest. Native watchdogs: Clippy590/600, compile1190/1200, pure/fmt230/240 seconds. No direct hash-named harness execution. Only8 new tests selected: existing production/retained/lifecycle/locator bodies and protocol signing support were not changed, so no unrelated old suite replay was required. Other harnesses selected zero.

Actual8 passed/0 failed/0 ignored. Exact full names/arguments are in test-selection.json/pure.stdout.log. Matrix covers both authors and C1 n1/24/25/640/C2 n1/32 with Event1/Receipt1, Event1/Receipt2 and Event2/Receipt2. Negatives use validly signed wrong source encoding/cuts/scope/Q/request, corrupt signatures and wrong actual credentials; expected ID/request/role/context/D/E/SID/Q and actual projection ID/request/body mismatches; independently typed receipt source/receiver/signature/Q/source-format/lane failures; bounds/malformed declared receipt structures and signed genesis refusal. Actual normalized facts retain changed signed parents/sequence, equal author-local sequence on opposite authors, changed opaque packet bytes with unchanged projection, and new valid receipt bytes from a renewed receiver authority. Record.same rejects those distinct actual observations. A validly signed three-parent source is rejected by the real parser. Native call counters remain unchanged.

All501 units and574 current compiler source inputs agree before/after compile/pure; all501 pure units fresh, independent frozen guard true. Current source/binary hashes and index/navigation were checked. Normal CLI unchanged50473984B/`f5322cfc26b93ab7a431a474d7f33e9ff805a3e1a3df98fb1d0f84185794dcb9`; other five stable EXEs and four field images unchanged/unexecuted. Migration harness17239552B/`a4479715fbccdba4adc836645e699148642d3fc8e60bd5b527bdaedda73fe809`. Exact review19542B/SHA256 `b56ffd19740d9ef8addf675602b674fe16978d92e34d01db6b99341c9b512944`.

Durable long-lived-dialog-a3-migration-s5-final-block3e-p11a3-evidence contains actual successful/failed receipts/logs, source hashes, review/provenance and bounded reproduction helpers. Raw full captures/independent expected manifest/eight EXE-source preimages/failed source snapshots remain at the immutable base. Use a fresh base/update paths to reproduce: compile without Run, derive the frozen expected manifest from compile-01.capture.json, then run only selected stable pure tests. review.py audits this immutable historical attempt.

## Required later integration, not completed here

Later bounded repository traversal must authenticate original genesis/scope/Ready/profile/source/migration/actual selected controls and derive actual projection/request/credential inputs. It must verify complete lane predecessor/parents/acyclic causal graph and original history order, without treating author sequence as a shared clock. The test's second signed parent deliberately has no existence proof here; that fact is retained for the later graph oracle, not accepted as a completed graph.

Exact source/receipt absence/extras,1348 sources/four genesis/1344 Text per owner, both-owner/Cut1-Cut2 byte equality, original Legacy/P7 A385..640/P10 B640/R1/ACK witness integration, final Native checkpoint, memory/sanitization/phase/runner/whole accounting remain mandatory. No actual database/delivery/restart/Native correctness or whole feasibility result. Navigation3/root Current handoff/historical files preserved, index empty, no commit/push. Ready for root source/static/pure review; stop at P11A3.
