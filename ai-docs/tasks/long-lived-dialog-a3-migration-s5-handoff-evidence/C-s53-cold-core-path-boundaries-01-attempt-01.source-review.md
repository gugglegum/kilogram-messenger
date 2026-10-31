# C2-J path boundaries: source review

2026-10-31. Source/static only under [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md):87/91, [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and [workflow](../../verification-workflow.md). Published baseline `aa822050af1751e37a2f3bbb6116d13eca6e411d`; no source snapshot. No compilation/native/fixture/private/network access or runtime reservation.

| Source | Bytes | SHA-256 |
|---|---:|---|
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) | 77389 | `f4a98cef1c57f7b239d0bcb23bd5aeb1bb91afffdd0291d1af33e44727f13a59` |
| [Worker](C-s53-cold-core-path-boundaries-01-attempt-01.preflight.ps1) | 100484 | `fb2bf7396664a3888c5ea61567a64e9dd327a4ab76bb72ecfe549b2741be1db8` |
| [Watchdog](C-s53-cold-core-path-boundaries-01-attempt-01.watchdog.ps1) | 36263 | `19780a01e8b45a708c2365e7ee76f6748039549a9f4137a7d8c826f07a1190dd` |

Core has exactly three substitutions: six appended selectors in each allowlist; closed pathLeaf roots/attempts. Actual inverse restores Core76524/`bd28ceeb50be5d2ec139b04c83c4e2bbf30a8838ec3dbf06bdd3962e29a2238e`. Production parser/default caps and all old selectors are unchanged. New cases use default limits.

Future family `E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-path-boundaries-01-attempt-01` has not been stat/read/created. Each case physically retains five real payload rows/52 bytes on both mirrors, actual P=`one/state/payload.bin`, six empty locks and safe-empty-S. Only control inventory path for logical row3 changes in all three arrays; lengths/hashes/provenance/order and headers stay healthy. Malformed paths never supply filesystem arguments.

| Leaf / selector | Exact replacement of P | Branch before third-row Record/Intern/open |
|---|---|---|
| absolute-path / path-absolute | `E:/s53-outside.bin` | colon in component E: → inventory_path; target never inspected |
| ads-path / path-ads | `one/state/payload.bin:stream` | colon → inventory_path |
| device-path / path-device | `one/state/NUL.bin` | reserved stem NUL → inventory_path |
| trailing-dot / path-trailing-dot | `one/state/payload.bin.` | trailing dot → inventory_path |
| case-alias / path-case-alias | `ONE/preservation-inventory.JSON` | valid components/schema/non-lock provenance, case-fold collision with stored row1 `one/Preservation-Inventory.json` in expected.ContainsKey → inventory_alias |
| excessive-depth / path-depth | `a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/a/payload.bin` | exact75-character/depth33 literal, 32 components a plus payload.bin; Depth>32 guard → inventory_path |

All six expect inventory/inventory/native0/bounded_refusal and exact semantic three-exception chain. First two source rows pass; third is rejected before its 160-byte record charge/Intern. Acquired events0, six locks, historical/four content digests null, captured=false/QPC0/hash passes0/bytes0/completed=false/observed sets=false; cleanup known/remaining0. No duplicate-property, triplet or missing-file substitution.

Control evidence is separate from physical payload health. Both complete BFS passes hash/read the actual malformed control and verify its length/hash/default stream. Verify-ControlInventory checks row3 location in all arrays, exactly three case-sensitive bad path tokens, and a reversible replacement of those tokens with P. Only that reconstructed in-memory healthy copy must match its own frozen bytes/hash and pass the existing five-row/provenance/header validator. The malformed file itself is never required to pass a healthy-inventory validator. Fixed Actual-Files/Payloads, creation/read/owner algorithms are unchanged; MutatedPath is referenced only by this string/data validator. Premise fields distinguish healthy_payload_layout_verified, control_mutation_verified and healthy_inventory_reconstructed_verified.

| Case | Actual control bytes / SHA-256 (including LF) | Pre path / final path bytes |
|---|---|---|
| absolute-path | 3585 / `8a2347ffc447191656a67c34cc19832b54ef5758ceabaad6729b7bcf2822a2d8` | 6088 / 6592 |
| ads-path | 3595 / `04d0458582992bb9d272fee6fbde6e3aa669b19564ccc14ebe8246c1459223b9` | 5896 / 6384 |
| device-path | 3574 / `623a074450a8b871075302c795a50a32c26c6ca47d2607b8c7851395062202be` | 6032 / 6528 |
| trailing-dot | 3593 / `6d57a70bda2e9649025f7ea7852c4111633f80cf6547b869f43dc9585e7e26ae` | 6056 / 6552 |
| case-alias | 3612 / `354cf9514935bd1f9b1a0246f323b9f45163178f4d5df9c3961ab49f7e7bacf0` | 5984 / 6480 |
| excessive-depth | 3764 / `7ff9f5712056f3e37c3b4adaca84245c17ace52bda03174b6f3b75688ac5f80d` | 6192 / 6696 |

The six healthy inverse bytes/hashes are independently frozen in Case-Specs and checked after reconstruction. Source/data comparisons confirm that only row3.path differs in each array, with exact inverse to each healthy literal. Same five payload rows/52 bytes remain physically checked.

Common source-derived budget (not measurements): base12073840, PRE inventory owned25/peak26/record12083440/inventory0; FINAL remaining0/peak27/record12084144/inventory=actual control bytes above. Formula base +25*384 pre; +26*384+2*160 final. Path model charges unique fixed25 pre; fixed26 plus first two relative rows final, aligned32+2*UTF16Length. No invalid path is interned and no parent table or row-directed capture is reached.

Harness six-case changes: allocation selectors/loops, execution/cardinalities/report6, geometry91 directory names/78 files/337 stream observations/22347 logical default-stream bytes/no observed named streams. Per case39 tracked file results,26 readbacks,30 enumerators,15 guards, all closed. Global owner ledger holds closed records:5+6*15=95; only its cap50→95, ledger95/95/0, peak simultaneous controller guards20. No assertion or owner cleanup dropped. Three native output files stay fixed, each65536/total196608; sources100484/36263 fit131072 source caps. Public prior-report DTO projection including new control fields plus4096 margin=52628 bytes, a sizing example rather than runtime proof.

Static: AST0,12 identical shared oracles/all38 Core fields, corrected outer array, all spec properties defined, six mutation/inverse/branch-data checks. Strict C# algorithm changes only closed type/path/name bindings. Physical setup/file ownership, guardian/job/Pump/drain/EOF/reap remain unchanged; supervisor non-function changes only closed labels/cardinality. No incompatible three-case assumptions remain (three output files/initializers/exception-chain entries intentionally remain).

Future work20/whole30/lane1/retry0 is a proposal only. C2-I worker measured3.8888775s with one compilation0.5452817s and three cases0.7639067/0.3347966/0.3728519s. Shared phases plus six slower-case durations project about6.06s; the extra bounded control parsing is not measured and host latency is not guaranteed. Actual timeout/output overflow/unknown cleanup cannot pass. Root sets allocation/measured boundary after review; ledger/ceiling unchanged.

No C2-A–I/positive/broad regressions rerun; no extra contract/readiness/native-plan/allocation, product changes, commit/push or S5.4. STOP for source review. Whole S5.3/G1/S5.4 remain open.
