# Block4AE: cold Ready storage read interface — review candidate

2026-11-03. Baseline `a5124dad010a24d4eb59bf01bfb2004178c1e5a8`. Library read interface and focused storage regression complete; **not yet accepted or published**. Six exact tests pass on the final formatted source, strict affected-package Clippy passes. AE does not establish that F08 is repaired: witness `Legacy::after_migration` and first `Controller::start -> source::point` remain untouched for AF. F08 remains consumed, S5.4 open; no F09, Prepare, Run, WholeP or full closed scenario/crypto matrix executed.

## Source contract and diff

Exactly three source paths: `crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/read.rs`, `types.rs`, and new `cold_read_tests.rs` in that directory. No Cargo, dependency, migration writer, Owner7, runtime or witness edits.

`head_native_snapshot` privately factors the existing occupied/Head7 decode/existing mode/q/cuts and exact Native pointer checks, including the selected-mode `MIXED_NATIVE_ID` domain. `L1ColdReadySnapshot` contains only Head and optional Native bytes, derives Clone, and has no metadata or serialization/export interface. Existing `snapshot` calls the shared reader then the original strict metadata read; missing metadata still returns exactly `Integrity("L1 owner not initialized")`. Shared validation and strict metadata tail are text-identical to the baseline; all other existing read functions and types are unchanged.

`cold_ready_snapshot` requires authenticated opened Ready storage, one or two occupied contexts, absent global initialization and absent metadata in **every** occupied context, and L1 existing state with valid Head/Native in every context. Invalid slots, partial initialization, any global-present state, non-L1, inconsistent pointers/mode/q/cuts, corrupt reads and poisoned core fail. Errors propagate; no missing-record fallback, fabricated metadata, initialization, Native session, Owner7 construction, commit or repair occurs during reads. Warm selected/current support remains in the shared reader without cold restrictions.

Exact complete source patch, including the new untracked test file: `.tmp/s54-block4ae-cold-read-01/source-diff.patch`. Its reverse application was checked without applying or staging it. The two original source files matched HEAD before this change.

## Fixture and meaningful regression

Before changing the API, rust-02 passed four actual authenticated V4 migration-to-Ready fixtures (one/two contexts × Native absent/present), checked uninitialized Ready and the original exact strict refusal; no Owner7 was constructed. Fixture uses OrderedVault/V4Source/preflight/fence/copy/verification/ready/open_ready and authenticated record mutation mechanisms already used by storage tests. Fresh temporary fixtures are restricted to `.tmp/s54-block4ae-cold-read-01/fixtures`; no consumed profile/key/database or historical raw logs are read.

This is a structural storage fixture with opaque certificate/authority/Native bytes and empty lanes, not cryptographic Engine/Native provenance or a valid complete selected-mode protocol transition. Actual Engine/Native F/P populated-lane lifecycle regression belongs to AF. No CLI dependency or fixture framework was added.

Final six exact selectors:

- `vault::indexed_dialog::migration7::live1::read::cold_tests::ae_actual_ready_strict_snapshot_requires_owner_initialization`
- `vault::indexed_dialog::migration7::live1::read::cold_tests::ae_cold_ready_reads_actual_bytes_without_persisted_change`
- `vault::indexed_dialog::migration7::live1::read::cold_tests::ae_cold_ready_rejects_partial_metadata_in_either_context`
- `vault::indexed_dialog::migration7::live1::read::cold_tests::ae_warm_snapshot_stays_strict_and_keeps_selected_native_domain`
- `vault::indexed_dialog::migration7::live1::read::cold_tests::ae_shared_head_native_checks_reject_pointer_and_mode_in_either_context`
- `vault::indexed_dialog::migration7::live1::read::cold_tests::ae_cold_ready_checks_slot_stage_and_propagates_corrupt_reads`

Read-only proof covers four cases: actual returned Head/Native equals migrated bytes; valid absent Native works; original strict snapshot still refuses. Full authenticated encoded root (including root summaries), generation, last_commit and source identity remain equal, including after close/reopen. During cold reads, backend writes, write_bytes, set_len_calls, growth_bytes, sync_calls and length remain unchanged, twice across reopen. This proves immutable authenticated semantic state and no backend write during the API interval. **It does not assert byte-identical physical redb containers across open/close**: an empty open/close control changes physical hashes in 4/4 cases. Hashing while redb is open also encountered Windows range-lock OS33 and was removed from the test.

Negatives include four partial-metadata refusals covering either metadata context and either requested context; initialized warm L1 and selected Active compatibility; global-present with missing/malformed metadata; non-L1 in the other context; eight pointer/mode/q/cuts corruptions tested through warm and cold/shared paths; three invalid slots; non-Ready stage; malformed existing/Head and poisoned-core propagation. Legitimate fixture initialization writes are outside read-only measurement intervals.

## Actual Windows commands and results

Pinned pwsh and Rust1.98.0, process-local PATH/offline, BelowNormal, Jobs2; no persistent environment/system settings changed. Compile/clippy limits1170/1200 seconds plus30 cleanup reserve; focused/format/seal570/600 plus30. Owner actual process handle/Job membership, child wait, bounded stdout/stderr EOF and verified empty cleanup were recorded for every row. Exit values are owner/child; these are source-check outcomes, not production whole-attempt verdicts.

| Step | Exit | Seconds | Cleanup |
| --- | --- | --- | --- |
| format-01 | 0/0 | 1.2749648 | empty; no force |
| compile-01 | 0/0 | 39.2221158 | forced residual 57740 |
| rust-01 | 1/1 | 7.0090263 | empty; no force |
| rust-02 | 0/0 | 20.8893965 | forced residual 39848 |
| format-02 | 0/0 | 1.3612404 | empty; no force |
| compile-02 | 0/0 | 23.8900672 | forced residual 66760 |
| rust-03 | 1/1 | 6.9971763 | empty; no force |
| rust-04 | 1/1 | 20.8680127 | forced residual 4708 |
| rust-05 | 1/1 | 21.7862824 | forced residual 43720 |
| rust-06 | 0/0 | 41.2693048 | forced residual 67832 |
| format-03 | 0/0 | 1.2007598 | empty; no force |
| clippy-01 | 0/0 | 16.1071309 | empty; no force |
| rust-07 | 0/0 | 29.2356883 | forced residual 61352 |
| seal-01 | 1/1 | 5.2700810 | empty; no force |
| seal-02 | 0/0 | 5.7745974 | empty; no force |

Every compile/run Cargo call was `test --manifest-path <root>/Cargo.toml --locked --jobs 2 --profile verification --no-run --message-format=json-render-diagnostics --package kilogram-state --offline --target-dir <root>/target/s54-diagnostics-source-checks`. Cargo did not execute a hash-named harness. Tests ran only through `scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Profile verification -CargoJobs 2 -SkipCacheStatus -Run -NoCapture -IsolatedTestsFile <fresh selectors> -IsolatedTestOrder BeforeAggregate`; aggregate harnesses were fully filtered and ran zero tests. Exact arrays/copies/commands are in each `*.cargo.json`, selectors and work scripts. No claim is made that the full package test suite ran.

Final rust-07: six tests passed, zero failed, isolated_test_count6; elapsed 29.2356883s. Stable library image `E:\AI-Projects\kilogram-p2p-messenger\target\s54-diagnostics-source-checks\verification\deps\kilogram-state-lib-kilogram_state-tests.exe`, SHA256 `74938e3b49734736da541ab56f92ae772f9b74598a32807adbb1de50f437873f`. Compile-only also produced two stable integration images; their aggregate runs executed zero tests. Required strict check: `cargo clippy --package kilogram-state --all-targets --offline --locked --jobs 2 --profile verification --target-dir <diagnostic target> -- -D warnings`, exit0. Scoped rustfmt --edition2024 --check and git diff --check passed in seal-02 after final format-03.

Original failures remain immutable: rust-01 fixture physical-quota mismatch; rust-03 Windows range-lock during open-container hash; rust-04 physical-container open/close hash expectation; rust-05 structural Head native-generation/offer fixture encoding. Corrections were confined to tests and proof controls. An initial after-selector creation shell call also failed due PowerShell quoting; file-based administration replaced it without product changes. Seal-01 failed an administrative count assertion because PowerShell emitted one nested array containing522 paths; fresh probe-r2/seal-r2 and seal-02 corrected JSON shape and passed. No failed evidence was overwritten or retried in place. No failing AE run established an additional product defect.

Forced cleanup rows terminated owned residual Job members after work exit; residual executable identities were not captured and remain unknown. All verified after-lists are empty. Final independent native process enumeration succeeded and found zero relevant processes across nine exact production paths and the diagnostic target. No claim is made that the unknown residuals were identified tests or product processes.

## Preservation and review handoff

Seal-02 verified1401 protected flat pins, including historical artifacts and all nine production images;519 old source pins unchanged, two source changes plus one new source produce522 current closure entries. Three current navigation files match the separate AE baseline; historical navigation observations were not reused. Full unrelated NUL-delimited Git status was reconstructed excluding only AE paths and matches initial SHA256 `d5e7f03e27030286e741201636c6b995fae0199c5187de4484e7a42658f91f8c`. Index remains empty, HEAD unchanged. No unrelated dirty changes are included.

AD report/manifest and AB/AC pairs were hashed **only as documents**, without expanding their JSON references. AD scoped16 raw input pins, F-origin bodies and historical command logs were excluded before hashing/reading. No new access to historical raw logs was granted by their presence in JSON. Original F01–F08 artifacts were preserved, not repaired or rerun; F09/WholeP directories remain absent.

Candidate contains **11 regular100644 paths**: three source files; this AE report and AE pins; six unchanged AB/AC/AD report+pin documents explicitly listed in `.tmp/s54-block4ae-cold-read-01/candidate.json`. That candidate contains exact raw bytes/SHA256 and normalized Git blobs, outside the self-pinned manifest. Root tasks, acceptances, navigation and unrelated changes are excluded. Administration, original failures, argv, receipts, source baseline/current pins and image pins remain under the fresh AE directory; manifest records their exact hashes. No commit/push until explicit root acceptance. Stop after one substantive callback; no polling or AF implementation. P1 passive30s Backoff risk is unchanged.
