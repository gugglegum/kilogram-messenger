# Block4AO: deferred memory provenance and genuine F source growth — review candidate

2026-11-03. On published AN HEAD `2697673313079744b3cdbffd2f9f6b72ffafdbc8`,memory provenance is first bound per owner after verified migration and retained record installation. **Three new focused tests,three directly impacted AF regressions and strict CLI all-target Clippy pass.** Genuine F source growth and a separate actual Child/query adapter are proved within the bounds below. This is a source/component candidate,not a full F runtime result. F01–F10 remain consumed; no new attempt,Prepare,activation,Run,WholeP or commit/push occurred. Extended P/AP work remains pending.

## Narrow implementation and ordering

The two premature `memory_scope::bind_legacy` calls are removed from F genesis setup (`migration_s54_feasibility.rs`) and P p0 (`migration_s54_early.rs`). Original genesis witnesses,identity/accounting,registrations,samples and stop/wait/workload logic remain. Shared `Controller::migrate` now returns the result of `self.migration_sources().bind_memory(owner)` only after stopped-source capture,same_identity,actual upgrade/after_migration,accounting migration,interval registration,remember,Ready insertion and retained migration witness installation. Binding failure propagates and cannot continue as scenario success; no rollback/retry is added.

The small `MigrationSources::bind_memory` seam obtains `self.source(owner)?` from AN's strict accessor before calling the unchanged `bind_legacy`. Missing/contradictory records fail before touching the observer. It is per owner: A can bind while B remains unbound. No unchecked source/genesis shortcut,manual source overwrite,lifetime rebinding or two-owner barrier exists. Existing late backfill and later legacy registration behavior are reused. This source denotes the eventually migrated inventory's provenance,not identical bytes for every earlier mutable snapshot.

`bind_legacy`,`bind_current` and the memory ledger algorithms are unchanged. `migration_s54_memory_scope.rs` changes only test-module visibility so focused tests can use narrow opaque support APIs. AN normal-source/closure logic remains intact; no product storage/runtime/wire/crypto code or F/P deadlines/purpose constants change. `Controller.original` is not overwritten.

## Genuine F fixture and unchanged AF entrypoint

The shared real Engine/Native fixture keeps `Fresh::legacy(purpose)` with its existing signature,frontiers and C1slot1 default. Its optional `f_with_genesis(slot)` path allocates a fresh owned TempDir under AO administration and supports physical C1slot0/1. It performs real enrollment,context creation and Native handshake; drops both Engines and their real vault leases; captures genuine zero-Text/genesis-only witnesses under separate real storage leases; then reopens Engines and restores only transient binding leases. It does not redo the handshake or read through a live Engine lease. It sends the original real two Text per role and queues actual A3/A4 before dropping Engines again.

For both roles and both mappings,tests prove genesis source_id differs from the pre-migration inventory source while same_identity succeeds. Actual migration Ready,`after_migration`,exact queue bytes/cardinality and AN lookup agree: C1=[3,3],C2=[1,1],Aqueue2,Bqueue0;the sealed queue does not advance committed frontiers;strict metadata is still cold. Source/request IDs and private witness/key material are not exported. These are fresh real cryptographic/storage fixtures,not historical account/profile replay. Quiescent leases are not OS wait proof.

Changing the shared constructor/module placement creates a specific impact on existing AF entrypoints. Exactly the existing F/P cold positives and20substitution/cardinality negatives were rerun. They pass,covering real after_migration/pre-spawn,read-only cold behavior,legitimate initialization and strict warm restart. Existing early P frontiers are A17/16 and B17/17;this does not establish AP's later B25/25 source growth or mixed migration timing. No other AF/AE/V/Z/crypto/full matrices were run.

## Cases,shared seams and real versus injected evidence

`cases-before-tests.md` was written before execution. `audit-before-tests.py` checked actual migration ordering,absence of both setup binds,unchanged binding/ledger algorithms,Engine-drop→genesis-capture→reopen order,the old fixture default and all6exact selector names. All new selectors are under `live_l1::handoff::final_p5::source::deferred_memory_tests`:

- `ao_real_f_growth_deferred_backfill_roles_and_slots`:two fresh real F fixtures,C1slot0/1,both roles and actual unequal genesis/migration IDs. Through the exact shared `MigrationSources::bind_memory`,unbound legacy records backfill to canonical A,while B stays unbound;current Stats and a new-nonce restart use A's migrated source. B backfills all its prior records when its checked records bind;later component registration uses the bound source. Ledger measurements/PIDs/handles/terminal records in this wiring portion are injected and are not actual OS processes.
- `ao_real_migration_binding_refuses_genesis_missing_foreign_and_nonce_reuse`:seven independent observer states reject missing witness,genesis endpoint mismatch,old premature genesis binding followed by migrated binding,foreign owner/source,genesis Stats,reused nonce and invalid owner. Pre-lookup refusals leave the observer unmodified; intentionally poisoned observer cases are discarded,never continued as a positive. The old F10 source mismatch is exercised with genuinely unequal real IDs.
- `ao_actual_child_query_deferred_binding_and_new_nonce_restart`:separate Windows adapter with genuine migrated IDs and roots,new synthetic profile aliases,and the existing pinned PowerShell resident script. Four actual OwnedChild lifetimes (legacy A/B and two current A incarnations) are registered and sampled with original Child handles;all four are explicitly killed and waited. A legacy close/wait precedes checked A binding;A current/new-nonce restart runs while B remains unbound;B then closes/waits and binds,backfilling its legacy lifetime. Final memory coverage succeeds. The original query uses actual Windows PID/handle/live brackets and PrivateBytes. Application decoder/Stats/nonces are injected. This is not an Owner7/network process or a full runtime proof.

The adapter reuses the accepted independent ancestor Job rendezvous,not a JSON membership assertion alone. Actual owner PID18552,launcher52708,harness57448;the ancestor verified the harness handle's Job membership and fixed executable pin before writing its permit. One join/permit pair matches;request membership=false,permit=true. The diagnostic harness was35464192B,SHA256 `990a1ab2c260bba571e766804c8eb1b40ccfb77914d4861692cfa98ba15b0814`. Admission files are fresh public component records under `adapter-01.admission`,not a new F/P activation.

|Evidence|Established|Limit|
|---|---|---|
|F storage/Native|Real genesis→traffic→queue→migration,4owner/slot combinations,Ready/AN/cold agreement|Fresh synthetic accounts;no actual old process waits|
|Memory wiring|Actual checked binding seam,real migrated IDs,late backfill,7independent negatives|Ledger/process readings and outcomes injected|
|Windows adapter|4actual registered resident Child handles,queries,4kill/waits,restart,coverage,verified Job membership|Decoder/Stats/nonces injected;resident is PowerShell,not Owner7|
|Impacted AF|Existing real F/P cold/warm positives and20witness negatives|Early P fixture,not extended AP traffic/timing|

Actual markers confirm Fslot0/1,7deferred negatives,20AF negatives and `resident_spawns=4;actual_waits=4`. The first transcript has5one-test successes;the adapter transcript has1;both have an empty aggregate result. Exactly3new+3impacted selectors ran,with no test/compiler retries or weakened predicates.

## Actual commands,limits and preservation

All compilation/execution uses `scripts/run-cargo-tests-stable.ps1`,fixed diagnostic executable names and the mutable `target/s54-diagnostics-source-checks`. Cargo is compile-only `--no-run --locked --offline`;test execution occurs only through stable exact selectors. Process-local pinned Rust1.98/pwsh,Jobs2,BelowNormal;strict CLI `--all-targets -- -D warnings`. Scope is nine Rust support files:eight modified and one new test file. The actual source closure is527 versus526;other518sources remain byte-identical.

|Step|Actual app chunks|Child/owner exit|Seconds|Work/whole/cleanup seconds|
|---|---|---|---|---|
|format-01|`b60d20 → b60d20`|0/0|1.4736049|570/600/30|
|compile-01|`cb851c → 26f9fc`|0/0|32.6525406|3600/3660/60|
|rust-01|`41b56d → c23a72`|0/0|228.4583030|570/600/30|
|adapter-01|`0983cd → 48fb38`|0/0|35.9665262|570/600/30|
|clippy-01|`487377 → 9d88c0`|0/0|14.8434889|3600/3660/60|
|seal-01|`ccaedb → ccaedb`|0/0|4.1830816|570/600/30|

All6receipts pass actual handle/Job membership,child wait,bounded stdout/stderr EOF,no expiry/reader failure,and verified empty cleanup after-list. Compile cleanup forced owned residual PID65620;its executable identity was not captured and remains unknown. Other steps,including actual adapter,were unforced. No foreign process was terminated. Compile/Clippy3600/3660/60 and focused570/600/30 do not alter runtime F450/480/510 or P2350/2375/2400.

Final scoped rustfmt,whitespace,reverse patch check and successful native process enumeration with0production/diagnostic conflicts passed. All45explicit current production image/origin/dep-info/preparation-public pins and the accepted AN report pair remain unchanged. All3 current navigation pins equal the fresh AO baseline. Outside-candidate Git NUL status is compared with the fresh baseline;index remains empty. AM16/AD16raw and prior command/profile/key/DB/f-origin contents were not read/hashed for this task. Only fresh fixture data is used by tests;no private test data or command-log copies are collected into the report. Historical JSON is treated as a document,without following raw references. The old AK inventory is stale for these sources and was neither restamped nor run.

Exactly11regular100644candidate paths:9AOsource/test files plus this AO report/pins only. Raw bytes/SHA256 and normalized Git blobs are in `candidate.json`;the exact source diff is `source-diff.patch`. Older supporting records are already published and are excluded from the candidate. No root/navigation/task/review or unrelated file is staged.

## AP and next review boundary

AP must separately establish genuine P first A17/16queue17/18 and later B25/25debt1,with mixed migration timing before a full attempt. Its extended fixture is not implemented here. The existing P setup bind is removed now because it shares the corrected migrate path;AO's per-owner/negative wiring applies without changing P workload. Full F feasibility/S5.4 acceptance is not claimed. P1passive30s Backoff remains separate. S5.3closed,S5.4open,F01–F10consumed. Stop after one callback for root review;no commit/push or AP/Prepare/Run/activation/new purpose/attempt before a separate authorization.
