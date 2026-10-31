# C2-K late directory stamp drift: source review

2026-10-31. Source/static only under [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md), [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md), [workflow](../../verification-workflow.md), [R44-3 task](../long-lived-dialog-a3-migration-s5-handoff-cold-core-review-and-fixes-task-r45.md) and [accepted r45 review](S5.3-r45-cold-core.root-review.json). Baseline published `a6bf0b6fdbb65ca2c8ec82584d3ebc0220df4dbc`; no duplicate Core snapshot. No compilation/native/fixture/private/network access or runtime reservation.

| Source | Bytes | SHA-256 |
|---|---:|---|
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) | 77942 | `c66595540f022fe5310c23eb6486b919e6131d7167513839f3c23cfb27e7f612` |
| [Worker](C-s53-cold-core-final-directory-01-attempt-01.preflight.ps1) | 94545 | `89030490afd61ab349d5be67276c465f02bd24247bd910d1d450ddd05452fb2a` |
| [Watchdog](C-s53-cold-core-final-directory-01-attempt-01.watchdog.ps1) | 35354 | `80df5bc319f83338b0feb7330eb0d0ef032ad9ccfe3cbf8fcd448c28e421d6b5` |

Core changes only three closed binding sites: two appended selectors in C#/PS allowlists and directoryLeaf roots/attempts. Exact inverse restores77389/`f4a98cef1c57f7b239d0bcb23bd5aeb1bb91afffdd0291d1af33e44727f13a59`. Production/parser/caps/old selectors and FinalInputs algorithm remain unchanged.

Future family `E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-final-directory-01-attempt-01` has not been stat/read/created. Both cases have the full healthy five-row/52-byte pair, six empty locks, safe-empty-S and tuple H=`54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5`.

| Case / selector | Only mutation at final_inputs | Expected refusal |
|---|---|---|
| state-directory / late-directory-s | fixed same-case state root, attributes XOR0x20 | directory_changed / final_inputs / s |
| backup-directory / late-directory-bkp | fixed same-case backup root, attributes XOR0x20 | directory_changed / final_inputs / bkp |

At the hook, both scan_after traversals and four hashes must have returned: acquired10, captured=true, positive common captureQPC, passes4/bytes208 and all five digests H. Completed/ObservedEntrySetsEqual stay false. Phase order locks,inventory,capture_s,capture_bkp,scan_before,hash_s_before,hash_bkp_before,hash_s_after,hash_bkp_after,scan_after,final_inputs. Pre map is allocation plus first10 phases; final map adds final_inputs.

The script snapshots/asserts Core first. A closed S53C2KDirectoryStampR01 accepts only the two case names (no path/attribute/command arguments), chooses exactly its state/backup root, and permits one Mutate attempt. It opens a no-follow metadata handle, checks normal directory/disk/final path, records attributes/ID and Core stamp fields, then calls SetFileAttributesW exactly once with originalAttributes XOR FILE_ATTRIBUTE_ARCHIVE. Same-handle and separate fresh-handle readbacks must show the exact XOR, unchanged ID/final path/type and unchanged length/links/creation/write fields. Access/change time is not claimed as part of Core's compared stamp. No ACL/data/namespace operation or manual timestamp change occurs, and attributes are not restored.

Sharing prerequisite: Core directory Acquire uses FILE_READ_ATTRIBUTES0x80, share READ|WRITE3, no-follow/BACKUP_SEMANTICS. The helper uses the same read-only metadata access/sharing; SetFileAttributesW requires WRITE_ATTRIBUTES, permitted by the independently verified owner-full-control fixture ACL and not excluded by Core's directory sharing. It requires no file-data/delete write. This is intended successful metadata mutation; denied/unsupported setter or unconfirmed readback is a hook/setup failure, never PASS. Same-ID checks support the bounded cooperative premise, not adversarial continuous namespace exclusion.

Two pre-owned slot boxes bound returned/unreturned helper handles. Each CloseHandle result/error is captured immediately, the SafeHandle is invalidated to prevent retries, and ownership is released only on observed native success. Failed/unknown close retains truthful current slots/cleanup flags. Mutate finally closes current handles; explicit Dispose is one-use/idempotent and completes before hook return. Separate failure_operation/type/HRESULT/native_error and cleanup_native_error preserve causes; helper errors/Win32 chains cannot qualify as expected Core refusal.

Expected helper35 facts: attempts/set1/returned+success true; metadata opens2/returns2/closes2/remaining0/peak1, info/type/path calls3 each, before/same/fresh identity/Core-stamp/XOR checks true, dispose1, unknown/partial/native errors0. Four QPCs bracket attempt/set/readback cleanup; verifiedQPC <= hookReturnQPC <= caught-Core-failureQPC < deadline, starting after pre-hook observation. Shared hook variables use explicit script scope.

FinalInputs re-reads each existing input, sets InputRole for its actual path and compares initial stamp for payload directories before Streams. Thus the changed S/BKP root must cause the specified directory_changed/native0/bounded_refusal semantic three-exception chain. No earlier object/stream/entry/share cause qualifies. Final captureQPC/H/passes4/208 remain unchanged, completed/observed sets false, input handles released/remaining0/cleanup known; Core explicitly disposed. No output binding/proof/receipts.

Source-derived resources (not measurements): both pre owned33/peak35/record12087312 and final remaining0/peak35/record12087312. Base12073840 +33 acquisitions*384 +five rows*160; safe-empty-S adds its directory owner/384 record/56 interned path bytes at scan_before.

| Case | Pre/final path bytes | Healthy raw inventory bytes / SHA-256 including LF |
|---|---:|---|
| state | 7248 | 3602 / `ddc1ae85b291adf540956542e1f746ebd217e71a10f760ffe853bd2a9d3a37c7` |
| backup | 7272 | 3606 / `a44df2daebe29fe3bb5e1ddfd92b0bcdd2cfd1f5d725766c8209da8643986e17` |

Default caps unchanged. Two healthy BFS/default-stream/content/ACL checks per case remain: geometry31 directory names/26 files/113 stream observations/7416 logical default-stream bytes/no observed named streams. The helper performs metadata only, no stream enumeration, so its six total metadata reads do not inflate113. Each case retains39 tracked file results/30 enumerators/15 guards closed before factory; owner-record cap/ledger35=ancestry/family5+two*15. Peak simultaneous controller guards20; helper peak1 is separate from Core accounting.

Static: AST0,12 identical shared oracles/all38 Core fields with dynamic capture timestamp and corrected outer-array expression; helper35 fields match literal/dynamic oracle; C# lexical delimiters balanced, not compiled. All spec refs defined; exact raw inventories verified. Strict C# stream algorithm, healthy physical setup, tracked owner/cleanup, Pump/drain/guardian/job/EOF/reap unchanged. Parent top-level changes only closed labels/two-case cardinality and fresh-helper type. New fresh types5/initializers3; no old file-add/extra-BKP/path-mutant code.

Future work20/whole30/lane1/retry0 remains proposal. Reused C2-J six-case worker5.0385749s/compilation0.5067401s provides context only; this new setter/helper behavior is unmeasured. Existing per-report65536/total196608 retained; public DTO-shape example plus4096 margin26115, not runtime guarantee. Actual timeout/output overflow/unknown cleanup cannot pass. No reruns of accepted cases/extra faults/readiness/native-plan/allocation/ledger edits/commit/push/S5.4. STOP for focused source review; whole S5.3/G1/S5.4 open.
