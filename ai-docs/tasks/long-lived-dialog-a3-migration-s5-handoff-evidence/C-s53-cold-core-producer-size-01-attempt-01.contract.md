# C2-R: one producer-size synthetic positive

2026-10-31. **Contract for review; no implementation or runtime allocation.** Baseline master/HEAD 0cbc322fe3ae06632af7ea4c1538f19b060d22ac; current [Core](../../../scripts/migration-s53-cold-preservation.ps1) is 80889B/SHA256 e4c426dafc68e1f727c51db9164aefd90efec8c607326fda9f8f2d72063f2e8e. Apply [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md) as replaced by [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and [r44](../long-lived-dialog-a3-migration-s5-handoff-r43-acceptance-and-cold-core-task-r44.md). Closed C2-C through Q negatives remain closed.

## Deterministic input and independent oracle

Future fresh family: W\.tmp\C-s53-cold-core-producer-size-01-attempt-01; case leaf producer-size; O=case\origin, S=case\state, BKP=case\backup, E=case\proof. W is E:\AI-Projects\kilogram-p2p-messenger. These are proposed synthetic literals, not existence observations. Add only a closed producer-size-r01 fixture selector and fixed output-owner binding after contract acceptance; no arbitrary root override or reuse of old baseline-real paths. No product/runtime7 changes.

[Actual producer](../../../scripts/preserve-migration-s5-l1-input.ps1), Inventory and lines55–56, includes both exact zero-byte locks in its file arrays. Thus **6956 rows = 6954 nonempty files + 2 locks**, not 6956 nonempty files plus locks. O has only its two existing fixture locks at one.state/.kilogram-state.lock and two.state/.kilogram-state.lock; each mirror has one/state/.kilogram-state.lock and two/state/.kilogram-state.lock. Create/close these six during fixture setup; Core subsequently opens existing objects only.

For global i=0..6953, path is one/state/pXXXXXX.bin for i<3477, otherwise two/state/pXXXXXX.bin; XXXXXX is invariant six-digit decimal i. Bytes are UInt32LE(i), then zeros: length6557 except i6953 length6578. Hence 6953*6557+6578=45597399B per mirror. Both lock rows are length0/SHA256(empty). No other payload/control-name exceptions.

Use the producer's exact eight root fields/order and four row fields/order, literal classification/os_lock_rule, source=S, preserved=BKP, ordered two S held_lock_paths. Arrays source_files/source_after_files/destination_files each contain the full ordinal path-sorted map. Source/source_after exact lock provenance is "held-lock length checked; empty-byte digest derived"; all other rows and every destination row use "actual file byte read". Stream compact JSON with ordinary escaping, strict UTF-8/no BOM, no spaces, one terminal LF. Whitespace differs from the producer's pretty output; schema, layout and provenance are identical.

Independent public in-memory calculations used (a) ordinal tuple/map construction and compact serialization, and (b) explicitly ordered side/index traversal, manual little-endian headers and Utf8JsonWriter streaming. They agree byte-for-byte:

| Static oracle | Exact value |
| --- | --- |
| Rows/bytes in each array |6956 / 45597399 |
| Synthetic inventory bytes |3465145 |
| Synthetic inventory SHA256 |c40ffdedd028be593c08ded6b05daba44abe0d1a9e1dac86cb6d78632cbf3843 |
| Synthetic historical tuple H |2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf |
| Physical files/bytes before E |13915 / 94659943 |

H is SHA256 over ASCII "ColdA-v1" + NUL, then each ordinal row's UInt32LE UTF8-path-bytecount, path bytes, UInt64LE length, raw32-byte digest. Runtime expected values come from this independent recipe, never from Core output. The synthetic H/inventory pins do not replace private historical pins.

## Setup, capture and resource preconditions

Case directories before E: case root + O(root,one.state,two.state) + S(root,one,one/state,two,two/state,safe-empty) + BKP(root,one,one/state,two,two/state) =15. Family adds1; E adds1:17 new directories total after output preparation. Only BKP root has preservation-inventory.json. Mirrors contain no other files. Parser parents are exactly empty-string,one,one/state,two,two/state. Scans observe6961 non-root entries per mirror: S6956 files+5 directories; BKP6956 files+4 directories+1 control.

Setup must stream one file at a time through restricted CreateNew, flush/readback/hash/EOF/stamp/ACL/strict-stream checks; payload buffer<=6578B and shared read/hash lane<=64KiB. Stream the inventory; independently verify its exact bytes/hash and all three maps without whole-text/DOM collection. Two complete bounded healthy-premise walks check all13915 files, all15 directories, distinct IDs, six empty closed locks, no ADS/reparse/extra entry, and exact map before Core. Close all15 setup directory guards before factory; verify ordinary healthy ownership handoff. Preserve created fixture/evidence on every outcome.

The small helper's 48-element registry, 4096B file reader and 65536B whole-receipt reader are **not scalable prerequisites already satisfied**. Implement only the finite producer-size loop/streamed validators later: one current file/enumerator owner, compact bounded row/seen/ID metadata (6956 rows,13915 file IDs plus15 directory guards,13929 descendant entries,15-directory queue), cumulative create/read/close counters, immediate clearing of successfully disposed Stream/Safe references. Across creation and two premise walks, 41745 file-owner results must close; do not retain41745 disposed streams or just inflate48. Stream one receipt record<=32768B; no full comparison receipt materialization.

Production limits remain unchanged: inventory16MiB; rows8000; entries20000; handles40126; payload64MiB/file and aggregate/mirror; path96MiB, records32MiB, shared scratch512KiB. Relative paths max30 UTF-16 units, depth3, component20; longest input absolute138, control135, below1024/32/255/32760. Strict Int64 integer/type validation and all existing path/identity/stream rules remain active.

Prepare restricted E and **transfer its sole directory handle into Core before CaptureAndCompare**, matching accepted small-positive order. Fixed input ownership is26 (19 ancestor/root directories +6 locks +inventory). Additional nonlock files13908:

| Owner-controlled quantity | Exact expected |
| --- | ---: |
| Captured input owners at scan_before boundary, before hashing |13934 |
| Acquisition notifications, including deliberate lock reuse |13912 |
| Core owners at that boundary, including E |13935 |
| Inputs after retaining safe-empty S directory |13935 |
| Maximum retained inputs+E |13936 |
| Core peak including scan/search/current metadata slots |13938 |
| Interned strings / path accounting peak |6988 / 563128B |
| Record accounting peak |18537840B |
| Inventory peak / scratch reservation |3465145B / 524288B |

Record arithmetic is12073840 fixed +13935*384 acquired-input charge +6956*160 row charge. Path charges sum align8(32+2*UTF16-length) for26 fixed paths+6956 relatives+5 parents+safe-empty. E/receipt slots do not acquire input-record charges. Output helper owns7 handles before transfer,6 ancestry handles afterward,0 after disposal; do not add these to Core's exact counter. Core input cleanup leaves E alone; receipt writing temporarily owns E+one receipt, then0. These are accounting expectations, not OS handle/RAM predictions.

## Required comparison and receipt acceptance

At scan_before hook require capture timestamp>start and<deadline,13912 acquisition notifications,13934 held inputs, E owned, zero hash passes/bytes. Retain all expected objects before four streamed passes; require S-before/BKP-before/S-after/BKP-after all H, **4 passes/182389596B**, complete equal observed scans and final inventory/lock/object checks. Scan equality establishes observed entry sets under cooperative exclusion; transient between-scan entries and continuous namespace absence remain unproved.

Final public observation must have property=bounded_payload_equality_cooperative_owner; completed,expected_set_captured_before_hash,observed_entry_sets_equal,input_handles_released,receipts_completed=true; expected_files6956,expected_bytes45597399,locks6,hash_passes4,hash_bytes182389596; all five tuple digests=H; remaining_owned_handles0, expected peaks above; cleanup_unknown/receipt_write_unknown=false; failure_code/phase/role/cause_kind=null and all native/HResult/cleanup errors0. Receipt charged=known-written=independently measured total; exactly four independent SHA256 values must match. Whole-attempt verdict and continuous_namespace_absence stay not_established.

Reuse accepted restricted output creation/ACL semantics and positive verifier field/type checks, adapting only fixed paths/counts and bounded streaming. E and its four receipts require protected exact owner/ACL, unchanged ancestor identity/ACL, sole ownership transfer, receipt mask15/transfers4, strict no-follow/final-path/ID/single-link/default-stream checks, terminal LF/no BOM and exact schemas. Independently enumerate exactly four names, hold/read/hash/recheck all four with bounded records, reject duplicate/missing/extra paths or properties. Compare all6956 comparison rows against oracle in ordinal order, both sets of IDs/stamps/four hashes/provenance, all11 namespace rows (S6/BKP5) and six lock records. Track every verifier owner/current/search slot and close explicitly.

| Receipt | Records | Source-derived maximum including LF |
| --- | ---: | ---: |
| owner.private.json |1 |32769B (file cap65536) |
| comparison.private.jsonl |6956 |11373060B (1635/row; cap33554432) |
| namespace.private.jsonl |11 |6919B (629/row; cap16777216) |
| result.private.json |1 |32769B (file cap65536) |
| Aggregate |6969 |11445517B (<67108864 cap) |

Comparison bound uses max30-character path,6578-byte length, longest source provenance, four stamps with maximum UInt32 decimal fields/16-hex ID and required single link; namespace bound uses max10-character path, signed-Int64 maximum length/UInt32 link fields. All other strings/booleans are fixed. Owner/result single records use Core's32768B shared writer limit; prelaunch source review must additionally prove their fixed-schema record construction fits that buffer. Exact emitted sizes/digests depend on native IDs/stamps/timing: record actual lengths later, never substitute these upper bounds as measured sizes.

Private result is intentionally provisional: receipts_completed=false, remaining_owned_handles2, three prior receipt digests/byte totals; it cannot contain its own final digest/duration. Final observation after WriteReceipts has true/0/four digests/full bytes. Preserve this existing verifier distinction.

## Measurement, lifecycle and finite future budget

Reuse accepted worker/supervisor/deadline/job/guardian ownership, fixed shell/stable paths and identity-bound child reap/EOF. Reuse current Q same-clock startup/timestamp handling, not its blocked-worker hooks or negative outcome. Fresh process: required types absent; pinned definitions loaded and each initializer called once. No Rust/Cargo/network tests. All JSON timestamp roundtrips use -DateKind String with strict original types and invariant exact strings; no DateTime coercion or weakened schema.

Report common-QPC boundary times and Core phase_seconds: allocation,output_binding,locks,inventory,capture_s,capture_bkp,scan_before,four hash phases,scan_after,final_inputs,receipts. Report bootstrap/job/source-definition/compiler,fixture creation/readback,two premise walks,guard handoff,factory,comparison,receipt verification,disposal/reporting separately. Hook counters/timestamps/snapshots are bounded; no per-file logging or process probes inside hash/acquisition hooks.

A sampler in the existing responsive supervisor loop targets100ms using the already retained worker Process identity/handle: Refresh then HandleCount/PeakWorkingSet64. No new sampler process, timer thread or event; retain aggregates only (sample count<=4801, first/last QPC, actual maximum gap, sample duration, maximum handle count and maximum OS high-water). Reuse/dispose the existing Process object after final observations/reap; no ownerless observer resources. [HandleCount](https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.handlecount?view=net-10.0) is refreshed current OS count; short peaks between samples can be missed and a delayed gap enlarges that interval. It is distinct from exact13938 Core accounting. [PeakWorkingSet64](https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.peakworkingset64?view=net-10.0) is process-lifetime OS high-water through observation, including PowerShell/compiler/setup/native overhead. Record last observation/exit gap; any unobserved exit tail is not certified. API/read/identity failure makes observer evidence failed/unknown and prevents whole-positive acceptance; it never overwrites Core facts. No claim that arena bounds equal RAM. Sampling must not postpone deadline checks/pipe draining; report its overhead/gaps.

Propose **one C attempt: work420s / whole480s / cleanup-report reserve60s**. Same absolute monotonic start includes supervisor bootstrap, compilation, all setup/verification, Core, four receipts and independent validation. Planning allowance:20s bootstrap/compiler,210s setup/readback/two premise walks,120s Core,70s output/verification =420; these are unmeasured allowances, not per-phase resettable grants or speed guarantees. Work includes13915 CreateNew/flush/readbacks,27830 independent premise reads,13934 retained input acquisitions,27844 observed-entry checks over both scans,182389596B Core payload hashing plus inventory and receipt traffic. This is materially more than the tiny positive; C70/85 is not activated or used as a deadline. No timing microbenchmark before source acceptance and no trimming rows/passes/checks to fit.

At420 stop ordinary work; supervisor terminates only its identity-bound/job-owned child if necessary. Reserve covers cooperative worker cleanup to450, bounded kill/reap/EOF/report through470; independent hard guardian child455/self475; actual native tool whole limit480. Wire all waits/helper deadlines to the same start; no sleep past an earlier cutoff. Success requires normal worker/supervisor returns, no force/unknown, receipts/observers/owned-child cleanup and actual tool-return evidence. Failure retains partial evidence; no automatic retry, extension or repair.

Ledger remains untouched: S5.3 C1634.6434909/F1562.2882567/P0; combined C2222.0921018/F6278.6084056/P0; ceilings1670/1642/0; active0, inactiveC35.3565091/F79.7117433/P0. A480s reservation would require root to authorize C ceiling>=2114.6434909 (increase444.6434909), then issue fresh bounded source/runtime instructions after reviews. This document allocates nothing.

## Review gate and stop

Decisive prerequisites: accept this exact oracle/layout/count/size model; implement/review only closed fixture/output binding plus scalable setup/receipt streaming and observer; prove normal healthy baseline/transfer, all resource/record bounds, pins/types/timestamps and same-clock cutoff/cleanup wiring; then obtain fresh allocation/activation. No new matrix or closed-negative rerun. Frozen launcher's failing binding-wrapper exception/ownership integration remains a separate follow-up before private use and is not certified by this positive.

Only this contract was written. No fixture/event/.tmp/private operation, project function invocation/dot-source/compile/native/Rust/network runtime, ledger edit, commit/push or S5.4. Private preservation, typed open, G1–G4 and final acceptance/package remain later. Callback to root, then **STOP for contract review**.

