# C2-I reduced resource caps: source review

2026-10-31. Source/static only under accepted [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md), [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and [workflow](../../verification-workflow.md). Runtime reservations0. No compilation, fixture/native calls, private inputs or network. Baseline is published C2-H `766289555b68680537ef30ba8abeeb90643b4c49`; no duplicate Core snapshot.

| Source | Bytes | SHA-256 |
|---|---:|---|
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) | 76524 | `bd28ceeb50be5d2ec139b04c83c4e2bbf30a8838ec3dbf06bdd3962e29a2238e` |
| [Worker](C-s53-cold-core-resource-caps-01-attempt-01.preflight.ps1) | 84681 | `823fcb3328c60e30b77c36d8611a357a095dc965753a66c3b76d9bd4e979e13d` |
| [Watchdog](C-s53-cold-core-resource-caps-01-attempt-01.watchdog.ps1) | 33402 | `70779db684be331d8b2dd893f28e0ba60c7d8d47520aa6d4b408c7146cbdb973` |

Core has four closed substitutions: fixture-mode alias switch, two selector allowlists, fixed resourceLeaf roots/attempts. Inversion restores accepted Core75659/`9c2df2d85382a5f0b9fbca2981e0a8d88c2cfe9b3d589e03b68c5b01745a9164`. Old selectors, Production/default limits and security/cleanup semantics are unchanged; no arbitrary cap/path arguments.

Future family `E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-resource-caps-01-attempt-01` has not been stat/read/created. All three cases have the full healthy five-row/52-byte pair, six empty locks, unchanged payload/provenance/H and safe-empty-S. Two independent complete BFS/default-stream/content/ACL/no-follow checks per case precede the factory. No mutation hook or output-owner binding/receipts.

| Leaf / selector → existing fixture mode | Expected semantic refusal | Reached state |
|---|---|---|
| row-cap / resource-row-cap → cap-rows (Rows2) | row_count_cap / inventory / inventory | first array third row rejected before its row allocation; two rows stored, six locks, acquired0, historical null |
| entry-cap / resource-entry-cap → cap-directories (Entries7) | entry_count_cap / inventory / inventory | five rows + five parents =10 exceeds7; acquired0, historical null; not separate directory_count_cap evidence |
| handle-cap / resource-handle-cap → cap-handles (Handles32) | handle_cap / capture_bkp / bkp | pre-hook acquired5; final acquired8; current 32nd file handle opens, temporary stream-search slot33 refused before search API; current handle closes, then registered inputs close |

All expect native error0/bounded_refusal/three-exception chain, captured=false, capture QPC0, hash passes/bytes0, completed=false, observed sets=false, final remaining0/known cleanup. Handle-cap alone retains historical H=`54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5`; all four content tuple digests stay null. Safe-empty-S is physically present but never reached by Core scan_before.

Source-derived expectations, not runtime measurements:

| Case | Base record | Pre owned/peak; record/path/inventory | Final remaining/peak; record/path/inventory |
|---|---:|---|---|
| row | 11242048 | 25/26; 11251648/5768/0 | 0/27; 11252352/6256/3562 |
| entry | 7595408 | 25/26; 7605008/5872/0 | 0/27; 7606192/6856/3570 |
| handle | 9187072 | 29/30; 9199008/6880/3574 | 0/32; 9200160/6880/3574 |

Reservation formula: `60000*64 + Handles*72 + Rows*104 + Entries*224 + 32768`; +384 per acquisition, +160 per stored row. Row/entry pre inventory has25 fixed owners; inventory adds one. Handle pre capture_bkp has29; its refused current acquisition is still record-charged. Paths use unique aligned `32+2*UTF16Length`: fixed25 pre inventory; row refusal fixed26+two rows; other refusals fixed26+five rows+five parents. These prove reduced-cap logic, not production exhaustion or OS memory pressure.

Healthy inventory bytes/hash, including LF:

- row 3562/`c860c097bc8e01008cb0b0d1ac52a6560ed715a410788bbbe9c51214fd69521b`
- entry 3570/`2e8e11395e5e612111d95d8d8badd7c849a25df8e8468ffec91affe9a72f15f2`
- handle 3574/`cf32a926cfd196504627c819689d731a7ae3688035c37eced6360fdb7a695e27`

Harness changes: three ordered selectors/loops/cardinalities; geometry46 directory names/39 file names/169 stream observations/11018 logical default-stream bytes/no observed named streams. Each case has39 tracked file results,30 enumerators and15 guards, all closed. Global directory-owner records persist after close: family/ancestry5 + three*15 =50; only that harness cap changes48→50, with ledger50/50/remaining0. Peak concurrent controller guards20; product caps are not changed by this bookkeeping.

Static checks: AST0,12 common oracle bodies identical, all38 Core fields and corrected outer-array assertion retained; all case-spec references defined. No fixed2 case assumptions or mutation code remain. Strict C# algorithm changes only closed names/paths; Pump/drain and supervisor non-function lifecycle match C2-H after labels/cardinality binding. Raw literals match modeled inventories; four Core substitutions invert exactly.

Work20/whole30 remains a proposal: one cold compilation, four fresh types/three initializers, three bounded healthy setups and early refusals. Reused C2-H worker elapsed3.9483326s (compilation0.5730114; two cases0.8029916/0.4412804); three times its slower case plus shared phases projects about4.0s, not a timing guarantee. Report caps remain65536 each/196608 total; prior public DTO-shape projection with three cases and2560-byte margin is29166 bytes, a sizing example rather than a runtime bound proof. Actual timeout/output overflow/unknown cleanup cannot PASS.

Compile/sharing/native cleanup/measured counters await separate source acceptance and bounded allocation. No old C2-A–H/positive/regression reruns, new contract/readiness/native-plan/allocation, commit/push or S5.4. STOP for source review; whole S5.3/G1/S5.4 remain open.
