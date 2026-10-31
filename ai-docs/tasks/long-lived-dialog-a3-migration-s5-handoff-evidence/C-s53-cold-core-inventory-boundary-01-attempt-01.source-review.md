# C2-P dormant source review

2026-10-31. **Source/static only; STOP before allocation/runtime.** Baseline753b5683c6e97120a210afaa33f5b5a90a5611c6. Implements the single-case [contract](C-s53-cold-core-inventory-boundary-01-attempt-01.contract.md) under [root acceptance/arithmetic decision](C-s53-cold-core-inventory-boundary-01-attempt-01.contract-accepted.md). No accepted test is repeated.

## Exact delta and pins

Own source scope: new [worker](C-s53-cold-core-inventory-boundary-01-attempt-01.preflight.ps1), [supervisor](C-s53-cold-core-inventory-boundary-01-attempt-01.watchdog.ps1), this note, and [Core](../../../scripts/migration-s53-cold-preservation.ps1) closed selector/factory binding. Existing contracts/evidence/navigation/helpers remain unchanged; unrelated worktree changes are preserved.

| Source | Raw bytes | SHA256 |
| --- | ---: | --- |
|Core|80325|fc4b9a78b1215b132644b8dd0ef544a6fcc304f85b42641614407fbbe92823cb|
|Worker|78503|9e6375bd2babc7fd742d3f6a37df78827c9b781dfe9f344f59fa60d11b8db2f5|
|Supervisor|34040|cc90b350404c34d584500c033a5dea26494d2c1759de6a241b301fdde9d3f32b|

Core +10/-3: append boundary-inventory-size to C#/PowerShell allowlists; before the existing factory guard require this selector's exact bytes16777217/hash1003b1b5dc078189799a1216ce0f9fbcebb94e8b6b83c58c4b03345f07f94ced/files5/payload52 tuple; exempt only that exact selector from the factory's upper-length precondition; bind its fixed family/case/attempt. Wrong tuple still throws before construction and cannot pass this test. All old selectors retain their old preconditions. Exact inverse equals actual HEAD's79709-character source byte-for-byte.

Limits, production binding, ReadInventory/Accounting, all five checked expressions and their supporting Number/aggregate/one-use/copied-observation logic are unchanged. The newly admitted Spec metadata does not admit a larger parser buffer: ReadInventory still refuses it before allocation. Root's arithmetic-safety decision is preserved for review of this source pin; no observed OverflowException/mapper coverage or new arithmetic test is claimed.

## Raw control setup and ownership

Fixed family C-s53-cold-core-inventory-boundary-01-attempt-01/case inventory-size. Payload mirrors retain five ordinary expected files/52B each, six empty locks and safe-empty-S. Only backup/preservation-inventory.json is the raw16777217-byte zero control. No JSON inventory literal/DOM is produced or parsed by fixture setup. Healthy-inventory verification is explicitly false; raw_inventory_size_boundary_verified has a distinct true premise.

Write-RawControl gates the exact Spec and fixed control path. The accepted NewFile/current-file owner pattern performs CREATE_NEW; its returned stream is registered before later validation. A fresh zero64KiB buffer writes256 full chunks+1 byte, then Flush(true). Managed write-call/byte/unknown/flush facts remain truthful on exceptions. Identity is recorded before writing. The writer then performs a bounded readback before its one normal tracked close.

Read-RawControl uses one64KiB read buffer and incremental SHA256, at most1024 managed chunk reads (at least257 for full content), exact total bytes, EOF, before/after length and full stable stamp. No17MiB array, SetLength-only assumption or sparse setting. Hash objects are disposed in finally; failed disposal records cleanup unknown/first cause and retains the uncertain object without a retry. This function borrows its stream; tracked creator/read owners perform file closure separately.

Two later premise traversals each open a new readonly no-follow, deny-write/delete stream, verify ACL/type/link/path/ID, strict ADS and bounded content digest, and close it. All three raw checks require the same writer ID, each stable stamp/length16777217/hash, ordered clocks and known hash disposal. These are **setup managed chunk counts**, not ReadFile syscall evidence. The raw ledger checks10 fields plus three13-field read records.

The fixture-only ADS checker is the accepted strict C2-I primitive with fresh type/family/case names and one narrow length exception: only this fixed regular control path may have length16777217; all other files keep4096. Directory/default/named-stream/error87/cleanup behavior is unchanged. Normal setup files still use the accepted small writer, now explicitly guarded<=4096 before its bounded readback array.

## Core refusal and oracle

Factory-returned and capture-invoked facts are mandatory. A passive inventory phase hook observes locks6 and25 fixed owners before the inventory is retained. Exact setup content/size plus the closed Spec tuple and reviewed held-handle Info flow establish equal actual/declared oversize length. The mismatch operand is false and the unchanged16MiB guard refuses **inventory_binding/inventory/inventory/native0/bounded_refusal/HRESULT-2146233079**.

The original/normalized markers and semantic three-node shell exception chain are checked; normalized/original Data counts are additionally fixed3/1. Factory ArgumentException, read/digest/schema/setup/unexpected failure is not PASS. Pre-hook, refusal-before-Dispose and after-Dispose observations are separately retained and validated across all38 fields. Core facts include capturefalse/QPC0, completedfalse/observedsetsfalse, acquisitions0, hashes0/all five digestsnull, peak_inventory_bytes0 and no receipts/output. Refusal/Dispose clocks are distinct and ordered.

**Core inventory non-read is a control-flow inference:** the accepted equal-length premise, exact stage/refusal and zero accounting agree with the unchanged guard before InventoryTake/new array/Seek/Read/hash/parse. No new Core read counter, syscall hook or instrumentation is invented. Six ordinary lock EOF checks and three full fixture-control readbacks remain separate.

Current/search/retained file/directory ownership, setup failures and cleanup_unknown handling remain active. Cases require input_handles_released/remaining0/cleanupfalse/native cleanup0 after refusal and Dispose; no E/output owner, receipts or positive preservation proof. Retained fixture files are not removed or reused.

## Independent static/model checks

PowerShell Parser.ParseFile: Core/W/S zero errors, no duplicate functions,3/51/28 functions. All25 shared W/S functions match exactly. Relative to accepted C2-I,35 worker function bodies match after name/family normalization; parent New-Drain/Pump bodies also match. The guardian/job/whole-cutoff, captured pipes/EOF/reap, fresh4types/3initializers, source pins and actual-exit requirements are retained. Lexical C# delimiters balance; **not compilation**. Default Core git diff --check passed.

Independent public in-memory recount: zero-control SHA matches the fixed hash; writer257 calls, three raw readbacks50331651 data bytes. Setup physical bytes16777217+104=16777321; family geometry16 directories/13 files/57 strict stream observations. Setup39 file owners and30 namespace enumerators close; controller ledger20 (family5+case15).

Core model: pre25/final26 fixed interned strings, path6256→6576; record12073840+25*384=12083440 before inventory,12073840+26*384=12083824 at refusal; peak26→27 from the inventory stream-search slot, remaining0. No row charge or inventory charge. Fixed arrays/scratch already exist before refusal; zero inventory buffering is not a claim of zero process allocation.

Prospective work20/whole30/lane1/retry0 remains inactive. Source131072/public65536each/196608total bounds remain; actual native transition/timing/cleanup requires separate source acceptance/allocation. No scripts were dot-sourced/compiled/executed; no fixture/.tmp/private operation, allocation/budget/ledger change, commit/push or S5.4. **Callback and STOP for focused source review.**
