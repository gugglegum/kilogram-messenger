# Block4AN: checked migration source and four normal consumers — review candidate

2026-11-03. On HEAD `f06dcd7e0d372dca2b7f5cb29d9c945961dd41cf`, normal lifecycle source checks now use the retained verified migration witness/endpoint instead of the genesis snapshot digest. Three exact synthetic tests, three directly affected real-data regressions and strict CLI all-target Clippy passed. **AN alone does not fix F10 startup**: both genesis memory bindings and migration-time memory behavior are deliberately unchanged. No Prepare,Run,new activation/attempt,purpose transition,WholeP,commit or push occurred.

## Implementation

Five source paths only: three existing test-support files (`migration_s54_control.rs`, `migration_s54_normal.rs`, `migration_s54_source.rs`) and two new unit-support files (`migration_s54_migration_source_fixture.rs`, `migration_s54_migration_source_tests.rs`). The actual current source closure is526, up from524; the other521 existing sources are unchanged. No product storage/runtime/wire/crypto files changed.

`MigrationSources::source(owner)` checks range before indexing, requires the retained migration witness, Ready observation and registered interval migration endpoint, validates owner/role/context/physical slot/committed frontier/source equality, and returns no default/genesis fallback. A lookup for A succeeds when B has not migrated. Pair lookup requires both distinct migrated sources only at pair closure. It receives the migration endpoint array, not later live starts/closed intervals. The helper is read-only and performs no storage I/O or initialization.

`Legacy::migration_endpoint` factors the existing contiguous committed witness projection from `migrated_endpoint`. Both the offline migration verifier and later source lookup now use it. It checks Ready/source/zero-migration-call status, exact two-context mapping, retained role/readable/queue counts, nonzero bounded inclusive-genesis lanes and contiguous original events. The repository verifier continues checking actual source counts/cardinality, owner material, exact Spec/Head/Native, every original event, queue bytes/cardinality and repository source. No cold/warm fallback or source mutation was added.

The four consumers are `request_normal`, `normal_pair`, `feasibility_normal_pair`, and `finish_memory_coverage`. Expected request source is resolved before the mutable child borrow. The existing PID/source request predicate is shared with the focused test; three pair consumers share `MigrationSources::normal_pair`, which calls the unchanged strict `normal::closed_pair`. Current-owner,one request,nonce,ack/wait/EOF,latest outcome,final-tail and F/P phase/relay gates remain in place. The original `closed_pair` function text is equal to baseline after excluding its separating newline. Genesis `Controller.original` remains intact for stable identity/accounting.

## Focused cases and actual checks

The prerequisite table was saved before test execution in `cases-before-tests.md`; `audit-before-tests.py` verified actual selector names/count, case ranges and unchanged AO files before the first test invocation. Inputs are **synthetic unequal-ID unit records**, not real source growth, valid cryptographic storage, actual residents or OS wait proof. Genesis IDs11/12 differ from migration31/32; owners and both C1slot mappings are exercised. The stable runner executed exactly these three selectors under `live_l1::handoff::final_p5::control::migration_source_tests`:

- `s54_migration_source_roles_slots_frontiers_and_per_owner_absence`:8owner cases over roles0/1,slots0/1,F3/3 and P A17/16/B25/25;C2=1/1;Aqueue2/Bqueue0;valid per-owner A with absent B,pair refusal,invalid owner2/usizeMAX. A separate advanced-frontier record differs from the retained baseline; the lookup structurally receives only migration endpoints.
- `s54_migration_source_refuses_missing_and_contradictory_records`:18executed negatives for missing witness/endpoint/Ready,genesis source,role/context/slot/frontier mismatch,Ready source/role/readable/queue,logical-context order,gapped/extra-role events,duplicate context,wrong witness role and non-Ready stage.
- `s54_migration_source_normal_seams_reject_genesis_swapped_and_stale_closures`:the exact shared source-selection/request/pair seams accept migrated IDs;refuse genesis/swapped owners/missing migration,wrong PID/nonce,latest-outcome violations and missing closure/outcome/EOF fields. It does not construct a running Controller or claim real process acknowledgement/waits. All four call sites route through these shared seams.

The transcript has3one-test success summaries and one aggregate0-test result;322tests were filtered per exact selector,323filtered in the empty aggregate. No unchanged AE/AF/V/Z/crypto/whole matrix was rerun. Diagnostic compile/run copies stay under `target/s54-diagnostics-source-checks`; stable executable `verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`. Cargo compilation used `--no-run --locked --offline`,Jobs2,BelowNormal,pinned Rust1.98/pwsh; tests ran only via `scripts/run-cargo-tests-stable.ps1`. Strict Clippy argv ends with literal `-- -D warnings` and includes `--all-targets`.

|Step|Actual app chunks|Child/owner exit|Seconds|Work/whole/cleanup seconds|
|---|---|---|---|---|
|format-01|`89a3f8 → 89a3f8`|0/0|1.4140880|570/600/30|
|compile-01|`ae1efd → 9f4dcc`|0/0|39.8006367|3600/3660/60|
|rust-01|`c1fc71 → c1fc71`|0/0|4.3489683|570/600/30|
|clippy-01|`06d4a0 → 7ba670`|0/0|21.1851714|3600/3660/60|
|seal-01|`305956 → 305956`|1/1|4.6238140|570/600/30|
|seal-02|`33edf3 → 33edf3`|0/0|4.5430410|570/600/30|

Compile forced cleanup of owned residual PID62880; executable identity was not captured and remains unknown. All other supervisor cleanup was unforced; every after-list is empty and verified. All receipts establish actual handle/Job membership,child wait and bounded EOF; no reader failure or work expiry. Compile/Clippy limits3600/3660/60 and focused570/600/30 are diagnostic administration only; runtime F450/480/510 and P2350/2375/2400 stay unchanged.

The first seal failed because the administrative function extractor included an extra separating newline after the otherwise identical `closed_pair`. Original verifier/work script,failed receipt/logs and `seal-01.failure.json` are preserved. A fresh verifier/`seal-02` excludes only that final separator and passed. No source correction,compilation retry or test retry followed this administrative correction. Scoped rustfmt,whitespace,reverse patch applicability,source/image/origin/document preservation and successful native process enumeration with0relevant processes passed.

## Candidate and preservation

Exactly13regular100644candidate paths:5sources,new AN report/pins,unchanged accepted AK preparation pair,AL failure pair and AM diagnosis pair. Each path has raw bytes/SHA256 and its normalized Git blob in `candidate.json`;source diff is `source-diff.patch`. Six supporting documents are byte-identical to the accepted baseline. No root/navigation or unrelated document belongs to this candidate. The index remains empty.

All45explicit current production image/origin/dep-info/preparation-public pins remain unchanged. The original seal preserved3navigation baselines; later navigation drift observed during supplemental finalization is disclosed below and preserved. Outside-candidate Git status is compared with the initial NUL-delimited baseline. Historical report/pin documents were not recursively expanded; AM16/AD16raw and prior-attempt per-command/profile/key/DB/f-origin contents were excluded before any administrative read/hash. Supplementary real-data tests use only newly allocated owned temporary fixture storage. The mutable diagnostic cache is observation evidence only,not a production prepared/runnable image set. The old successful AK inventory has not been restamped for these dirty sources.

## Deferred AO contract

After AN acceptance/publication, AO must remove the premature F/P genesis memory bindings and bind once per owner only after successful exact migration plus interval registration,using the checked source. Preserve late backfill,all legacy samples/waits,no rebinding and all owner/PID/nonce/currentStats guards. AO needs genuine genesis→traffic→migration inequality coverage,F3/3queue3/4,PfirstA17/16queue17/18,laterPB25/25debt1,mixedAcurrent/Blegacy,cold-first/strict-warm/newnonce/currentStats and bounded actualChild/query adapter coverage with honest wait assertions. Existing AF fixture returns after traffic and requires a narrow earlier capture hook; equal injected IDs are insufficient. No AO implementation or new runtime attempt is included here. P1passive30s Backoff and later B25 coverage remain separate pending concerns. S5.3closed,S5.4open,F01–F10consumed. Stop after one callback for root review.


## Review supplement: directly affected real migration verifier

Root review `root-review-01.json` identified one missing validation: the factored `Legacy::migration_endpoint` also affects the real `after_migration`/repository verifier and adds Ready stage/readable/queue predicates. The source remained byte-identical to revision01. The previous report,pins,candidate and delivery result are archived under `.tmp/s54-block4an-migration-source-01/revision-01`; no prior receipt was rewritten.

Before execution, `setup-real-supplement.py` checked the exact existing selector names in source, all526source pins,the same diagnostic images,45protected production pins and6supporting documents. It inspected the fixture allocation contract: `Fresh::legacy` creates a new `tempfile` directory with prefix `fresh-` below `.tmp/s54-block4af-cold-witness-01/fixtures`, with newly created accounts/Engine/Native storage. Only ancestor directory metadata was checked for containment/reparse points; existing fixture children were not enumerated or opened. Owned TempDir instances handle their own cleanup. No old profiles,keys,DBs,command logs,AM16 or AD16 were read.

Exactly these three existing selectors ran through the same diagnostic stable harness:

- `source::cold_witness_tests::af_real_f_cold_witness_first_start_and_strict_warm_restart`: passed; both owners C1=3/3,C2=1/1,points2/2.
- `source::cold_witness_tests::af_real_p_cold_witness_first_start_and_strict_warm_restart`: passed; A C1=17/16,point16/15; B C1=17/17,point16/16;C2=1/1.
- `source::cold_witness_tests::af_real_witness_substitutions_and_cardinality_are_refused`: passed; all20existing substitutions/cardinality/slot/account/device negatives refused.

Both positives exercise real retained event/queue bytes and Native state, the factored repository verifier, the actual `after_migration` wrapper and first cold endpoint, then legitimate initialization and a strict warm restart. C1 is deliberately slot1; A has two queued requests,B none. Read-only intervals check authenticated fingerprints/backend write counters and zero extra Native work, including reopen. The Ready stage/readable/queue predicates are therefore compatible with these real migrations. This does not establish physical DB byte identity. The printed two legacy lifetimes are component ledger records,not actual OS process waits; these tests do not start Owner7/network peers and do not prove AO's genesis-source growth or memory binding integration. P B17/17 is the early fixture,not later B25/25 coverage.

Actual app return `15b736 → 7aa1bb`: `rust-02` child/owner/tool0/0/0 in144.0182350seconds,limits570/600/30. Handle/Job ownership,wait and bounded EOF verified,no reader failure/expiry,cleanup unforced and empty. Transcript has3one-test success summaries,4positive owner markers,one20-negative marker and an aggregate0-test result. The stable wrapper's required Cargo step remained compile-only (`--no-run --locked --offline`,Jobs2/BelowNormal); all diagnostic executable hashes are unchanged, including the selected harness SHA256 `a4ede5b521c6a5a8bcdbe49851526d7bfdc965b289a40ce4135235657e2d037d`. The prior AN synthetic selectors,Clippy,format and other AF/AE/V/Z/crypto matrices were not repeated. No source changes or predicate weakening were needed.

Revision02 keeps the same13candidate paths. Its5source and6AK/AL/AMsupporting files have exactly the revision01 raw hashes and normalized Git blobs; only AN report/pins change. Additional administration,original receipt/stdout/stderr/cargo/image pins and verified facts are included. No commit/push,AO or runtime/new-attempt work follows before root review.

Supplemental finalization first returned1 (`6d46d8`) before changing report/pins/candidate because `chat-coordination.md` differed from the original navigation pin. Other source,production/diagnostic image and supporting-document pins matched. The observed navigation change is165886→165821bytes,SHA256 `0055187e79e210f1d9307a7ed641829b0e86d725de309e60ffc19ba70a7612d1`→`af5dfc5fa520f21d7e346f543d78e0fe0b25d2cb3f5acfbc4516928693eae313`. This task did not write that file; its author/timing within the interval was not inferred. The fresh finalizer records current navigation bytes without restoring them or changing the archived revision01 observations. Navigation stays outside the candidate. Original failed finalizer and failure receipt are retained; no source predicates/tests were changed or rerun.
