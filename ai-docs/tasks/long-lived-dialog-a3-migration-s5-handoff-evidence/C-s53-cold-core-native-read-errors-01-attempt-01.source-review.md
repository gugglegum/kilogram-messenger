# S5.3 C2-N native read errors: source review

2026-10-31. **Source/static only**, Sol High/no Fast. Baseline published C2-M `d0e58c93e7287ca9b16c1eb246a6679ce86038da`. This closes only source preparation for the two outstanding [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md)/[r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) native-error cases, following the [current workflow](../../verification-workflow.md). Runtime remains unactivated.

Scope: [Core](../../../scripts/migration-s53-cold-preservation.ps1), new [worker](C-s53-cold-core-native-read-errors-01-attempt-01.preflight.ps1), [supervisor](C-s53-cold-core-native-read-errors-01-attempt-01.watchdog.ps1), this note. Family C-s53-cold-core-native-read-errors-01-attempt-01; cases read-locked/streams-unsupported; selectors native-read-locked/native-streams-unsupported. Core **+10/-3**: closed selector/path bindings and one fixture-only FindFirstStreamW argument predicate. Inverse substitutions restore HEAD exactly. Production/default caps, Read/Hash, Streams result/error/finally, mapper and receipt algorithms are unchanged.

| Source | Raw bytes | SHA256 |
| --- | ---: | --- |
| Core | 79451 | `9b4a7d6165c11d82ff122ca6c13f8876d616b711424e7355e5508e73c7a2cf06` |
| Worker | 96364 | `6e46b99a417f2193f225fbd618d4c02be4ec94107e10e17b52da044274f9785c` |
| Supervisor | 34793 | `e73002a46c6dad5c5e17a04bdff198612dc340fd246027229fd60ed334e634e7` |

Both fixtures retain producer-valid five rows/52B, mirrored payload, six empty locks and safe-empty-S. Strict healthy physical/ACL/stream/inventory checks finish before Core. This scope requires partial Core refusals, **not** a successful four-pass/208B result. Historical tuple H remains `54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5`; every observed complete-pass digest stays null.

## read-locked: real ReadFile refusal

RangeLockOwner has one fixed target: this family's read-locked/state/one/state/payload.bin, six bytes. No arbitrary path/command or read-result override. Before Core creation, after healthy premises, it opens that file GENERIC_READ/shareREAD/OPEN_EXISTING/synchronous/OPEN_REPARSE_POINT, checks regular disk/single-link/noninherit/final-path/private ACL/length6, records full non-access stamp/identity, then explicitly closes the baseline handle. Baseline/native-close success must be known before Core.

Core acquires all expected inputs, sets capture-complete time, then completes scan_before. At the **hash_s_before hook**, before any helper lock action, worker saves/asserts all38 Core fields, acquired10, current retained33/peak35, historical H, expected_set_captured=true, hash counters0, no observed complete tuple digest and no failure. The hook's current phase timer has not yet been committed to phase_seconds.

Only then the helper opens its second read-only/shareREAD target handle, repeats checks and requires the same baseline object/non-access stamp. Core's retained file disallows delete/replacement; its checked final path and the helper's same checked path establish that this is also Core's object. Helper observes matching IDs/stamps itself; **Core's per-file ID is not exported or independently instrumented**.

LockFileEx uses flags3 (EXCLUSIVE_LOCK|FAIL_IMMEDIATELY), reserved0, offset0/length1, zero-initialized pointer-width OVERLAPPED with event0. Exact success and unchanged zero offset/event are required; metadata/private ACL/stamp are checked again after locking. The helper is the sole owner of this valid, noninheritable native handle; no truncation, contents/attribute/ACL write or invalid/recycled handle.

API basis: [LockFileEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex) permits GENERIC_READ, specifies immediate failure mode and denies access through another handle even within the same process. [ReadFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfile) fails for overlap with a locked range. [ERROR_LOCK_VIOLATION](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-) is33. These justify the narrow expected locked-read profile; another error or an earlier open/seek/metadata/setup refusal is not accepted. No helper ReadFile probe or second failure injection is added.

Source order after hook is HashTree(S-before) → Hash(row) → Info → Seek(0) → actual ReadFile. Sorted rows first complete one/Preservation-Inventory.json (23 bytes) and the empty S lock (0 bytes/EOF), then attempt payload.bin. Its first six-byte Read overlaps locked byte0 and must fail with33. HashBytes is charged only after a whole row hash succeeds: **23, HashPasses0**, all four observed tuple digests null. Error is **input_busy / hash_s_before / s**, Win32 native33/HRESULT -2147467259.

Lock remains held through actual Core refusal, all input cleanup and returned Core.Dispose. Helper then revalidates its still-held object/stamp/private ACL, calls explicit UnlockFileEx for exactly offset0/length1 with event0 and explicitly closes its native handle once. [UnlockFileEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-unlockfileex) requires the same region and recommends explicit unlock. Failed unlock/close or unknown current owner fails; there is no retry of a failed close. Finally cleanup still attempts to release its own lock/handle on failed setup/hook paths; such paths never qualify.

Three complete **36-field helper snapshots**: baseline, locked, final. Opens1→2; explicit successful closes1→1→2; owned0→1→0; current_owned=false on return. Info/private-ACL checks1→3→4; lock1/success/error0 held through Core.Dispose; final unlock1/success/error0, post_verified/disposed true and cleanup known. QPCs bracket baseline, hook/open, lock, Core failure/Dispose, post-check, explicit unlock and helper disposal. Driver uses reachable helper ownership before baseline/hook work and records script-scoped delegate results.

## streams-unsupported: controlled invalid-parameter reaction

**Root approved the flags alternative before implementation.** InfoLevel1 was not used: its output format is undocumented, while the current typed buffer is WIN32_FIND_STREAM_DATA. [FindFirstStreamW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirststreamw) documents only level0's buffer format and requires reserved dwFlags=0. To keep the known format, the real native call remains **level0/out StreamData**, but uses flags1 only when all three exact conditions hold: spec.IsFixture, the new fixed streams-unsupported attempt literal, and the full fixed S payload path.

The predicate is closed source logic, with no mutable public toggle or external flags/path. Default/production/all old selectors use level0/flags0. The real native return and immediate LastError flow through **unchanged Streams/NativeFailure/finally**. No87 is substituted, no fallback/retry/copy of the mapper. Error87 from invalid reserved flags is an **expected runtime prerequisite, not a documented guarantee**. Native success, another error or an earlier refusal cannot qualify. This is native invalid-parameter injection to test streams_unsupported handling, **not evidence that this filesystem lacks stream support**.

At capture_s hook, save/assert full38 Core fields: historical H and parsed inventory, retained26/peak27, acquired0, no capture timestamp/hash progress. S required directories reuse retained lock ancestors. Row0 ordinary nested inventory is acquired/transferred (hook1); row1 reuses the S lock (hook2). Row2 payload.bin opens into Acquire's local current handle and reserves its stream-search slot. Expected native87 is **streams_unsupported / capture_s / s**, before current-handle registry transfer/hook3.

Budget sequence: retained27 plus current1 =28; search reservation raises peak29. Native invalid search releases that slot before NativeFailure, slot=false prevents duplicate release/FindClose. Acquire finally explicitly closes the still-current file; outer CloseInputs closes retained27. Thus final owned0/input_handles_released=true/cleanup_unknown=false, acquired2, capture timestamp0, expected_set_captured=false, hash bytes/passes0. A different stage/acquired/capture/current-cleanup result fails the case.

## Assertions, accounting and source gates

Each case retains **three full38-field Core observations**: before designated fault hook, after native refusal and after Core.Dispose. Both completed=false/observed_entry_sets_equal=false, receipt counters0, no output owner/proof/receipts/digests, final input release/known zero Core ownership. The read case preserves expected_set_captured=true and a positive unchanged capture timestamp; streams keeps both false/0. Neither case accepts cleanup_unknown.

Both exact error chains are **MethodInvocationException → normalized InvalidOperationException → marked native InvalidOperationException → Win32Exception**, with native33 or87. Outer HRESULT -2146233087, middle -2146233079, native -2147467259; normalized/original cold code/native marks, phase/role and deepest cause are checked separately. Helper/hook/setup errors, wrong native type/error or unrelated managed exceptions never pass. Parent repeats the full branch checks/QPCs/phase events, known helper/controller cleanup, normal guardian/child exit0/EOF/reap. Whole-attempt verdict remains not_established.

| Quantity | read-locked | streams-unsupported |
| --- | ---: | ---: |
| Inventory raw bytes |3598|3630|
| Inventory SHA256 |548a151083eed305ed3ea885fcf3ed772645d597ea010e6f7098c4687f62588f|480b2d04e399f6805a32aa96de50fb4b1a1593560202fddd6980c11775b5b8cb|
| Interned strings / path bytes |37 / 7208|36 / 7488|
| Core record before hook / final |12087312 / 12087312|12084624 / 12085392|
| Core owned before hook / peak / final |33 / 35 / 0|26 / 29 / 0|
| Expected-file hooks before / final |10 / 10|0 / 2|
| Hash bytes / complete passes |23 / 0|0 / 0|
| Capture timestamp / expected set |positive / true|0 / false|

Default record precharge12073840; rows5×160. Read case has33×384 acquisitions, including safe-empty; stream case pre26×384/final28×384 includes the failed current acquisition. Path sum align8(32+2×UTF16 length):26 fixed paths+five rows+five parents, with safe-empty interned only after scan_before. Inventory peaks as above/scratch524288 unchanged. These are Core accounting values, not measured process memory. The helper's StringBuilder32768, single target handle and fixed OVERLAPPED/metadata are separate fixture overhead.

Controlled aggregate geometry: **31 directories,26 files,113 strict input stream checks,7436 default-stream bytes**, no output creation. Family5+two case15 guards =35 returned/closed; case guards relinquished before Core. Helper metadata/ACL/open/lock checks are not stream enumerations. Healthy strict stream checking remains level0/flags0; unsupported injection changes only Core's one designated target call.

Static gates: AST0/Core3 definition-only functions/worker50/supervisor28;25 shared functions exact. All38 Core/36 helper keys covered, no undefined Spec members. C# lexical delimiters balance (Core1781/helper460);11 distinct usings/two fresh helper types/five fresh-type gates/three initializer calls. No output-owner/collision/receipt helper in the new harness. Public-data recount verified both raw inventories/maps/provenance context/five payload bytes and hashes/H, fixed26/path/record/prefix23 models, Core exact inverse and frozen output/RootCopies/deadline pins. Default diff/raw whitespace/link checks accompany callback.

No .tmp query/create/read, project helper invocation, Add-Type/compile/native/Rust/product network runtime/private/keys/DB/original access, allocation/reservation/ledger edit, commit/push or immutable evidence change. Actual C# acceptance, LockFileEx/ReadFile33 and reserved-flags87 behavior/latency remain unverified. Proposal **work20/whole30, lane1/retry0**, unchanged source131072/report65536each/196608total caps and guardian/job/Pump/EOF/reap; no evidence yet warrants widening.

Closed accounting unchanged: S5.3 C1587.6924909/F1562.2882567/P0; combined C2175.1411018/F6278.6084056/P0; ceilings1620/1642/0; active0/headroomC32.3075091/F79.7117433/P0. Private grants none; whole S5.3/G1/S5.4 open. Callback exact4 pins/diff/profiles/models, then **STOP for focused source review**.
