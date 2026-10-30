# C2-H namespace observations: source review

2026-10-31. Source/static only under accepted [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md), [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and [verification workflow](../../verification-workflow.md). Runtime reservations 0. No compilation, fixture/native calls, private data or network access.

Baseline: accepted/published C2-G `df471f7a6660b03cc381ffaef14d4c727f369c5c`. Old Core remains available there; no duplicate source copy. Core diff is exactly three substitutions: append two selectors to each allowlist; add a closed namespaceLeaf binding. Inversion restores accepted Core 74989/`afe18a0f4ef306f0c59e941445cdc43da0c7eb28071230a42ffbb6001b05866e`. Production checks/caps and old cases are unchanged.

| Source | Bytes | SHA-256 |
|---|---:|---|
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) | 75659 | `9c2df2d85382a5f0b9fbca2981e0a8d88c2cfe9b3d589e03b68c5b01745a9164` |
| [Worker](C-s53-cold-core-namespace-observations-01-attempt-01.preflight.ps1) | 88967 | `6a8c58d1a97e30b5aa7e515e79e3a466afb90507a6494ad4a7d1294689fcea3f` |
| [Watchdog](C-s53-cold-core-namespace-observations-01-attempt-01.watchdog.ps1) | 35596 | `d5ce5fb717fb172856cd37d8703f217556b6c926ac1603e4464bf5d46b2b807a` |

Future closed family: `E:\AI-Projects\kilogram-p2p-messenger\.tmp\C-s53-cold-core-namespace-observations-01-attempt-01`. It has not been stat/read/created.

| Leaf / selector | Premise | Expected refusal and reached state |
|---|---|---|
| `extra-backup-directory` / `namespace-extra-backup-directory` | Healthy 28-object baseline, then protected empty `backup/extra-empty`; retained no-follow guard, exact location/ACL/stream set and complete 29-object BFS. | `directory_extra / scan_before / bkp`; captured=true, positive capture QPC, locks6/acquired10, hash passes0/bytes0, historical H + four null digests. |
| `persistent-extra-state` / `namespace-persistent-extra-state` | Complete healthy pair still present before Core. At start of `hash_s_before`, after both scan_before traversals, exactly one fixed CreateNew `state/extra-observed.bin` containing byte 0x5a. It stays present for scan_after and is independently read again after refusal. | `entry_set_changed / scan_after / s`; captured=true, same positive capture QPC, locks6/acquired10, hash passes4/bytes208, all five digests H. |

Both expect native error0, bounded_refusal, semantic three-exception chain, completed=false, observed_entry_sets_equal=false, remaining handles0 and known cleanup. H=`54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5`.

The mutation has one call site and no path argument. It requires the exact case/hook, one-use counter, acquired10, captured pre-hook observation/hash0 and zero controller writers/readers/guards. One tracked CreateNew writer writes/flushes/reads the byte and closes. A separate no-follow reader checks normal single-link shape/final path, exact ACL, byte/EOF and sole default stream, then closes before hook return. The same fixed file is re-read after the expected refusal. The 14-field mutation snapshot and QPC ordering prove these observations/owner handoffs within the cooperative scope; any hook/setup/cleanup exception prevents PASS. No deletion, transient replacement or timestamp restoration. Creation naturally changes state-root directory metadata: Scan checks its identity first, and Visit rejects the extra file before the final directory Same check. Expected files/inventory/metadata are not edited.

Source-derived resource expectations, not measurements:

| Case | Pre owned/peak; record/path/inventory bytes | Final remaining/peak; record/path/inventory bytes |
|---|---|---|
| extra directory, pre scan_before | 32/33; 12086928/7776/3658 | 0/35; 12087312/7832/3658 |
| persistent file, pre hash_s_before | 33/35; 12087312/7832/3658 | 0/35; 12087312/7832/3658 |

Model: record base12073840 + 32 acquisitions*384 + five rows*160. Scan(s,false) also acquires existing safe-empty-S: +384 record/+56 interned path bytes; current payload handles plus find/metadata slots give peak35. Base interned paths7776 use aligned `32+2*UTF16Length`; accepted C2-G path values7368/3128 calibrate the model. Both inventories3658 bytes: directory `ac5318e1207cdbb14edc9c88b0d2a1a05541c2c873319df107f1d6eb30ce3539`, persistent `90a92364b76d5702661b6bdb1fabc2007c43e51fa868000661506acdd02a0846`; five payload rows/52 bytes/H unchanged. Extra byte SHA-256 `bbeebd879e1dff6918546dc0c179fdde505f2a21591c9a9c96e36b054ec5af83`.

Controller expectations: directory 39 tracked file results, 31 enumerators, 16 guards; persistent 42 file results, 30 enumerators, 15 guards, all closed. Coverage baseline28 each/prepared29 or28, plus two extra-file read checks: family1+57+56+2=116. Final geometry32 directory names/27 file names/7525 logical default-stream bytes/no named streams observed; aggregate guard ledger36. These are observation counts, not continuous namespace absence.

Static: AST errors0 for Core/W/S; 12 shared oracle bodies identical; all38 Core observation fields remain checked (35 literal + capture QPC + two maps), with corrected outer-array cardinality. One mutation call site/two fixed reader call sites; strict stream C# algorithm only receives closed type/path/name rebindings, no added native helper. Pump/drain and supervisor non-function lifecycle match C2-G after closed bindings and removal of the unused alias type check. Existing work20/whole30 guardian/job/EOF/reap remains; four fresh types/three explicit initializers. No prior scenarios were rerun.

Compilation, actual filesystem behavior, measured counters and native owner cleanup await separate review/allocation. No new task/contract/readiness/native-plan/allocation, no runtime reservation, commit/push or S5.4. STOP for mutation-hook source review; whole S5.3/G1/S5.4 remain open.
