# Cold-A: concrete inert decision / native plan for root review

2026-10-31. **Preparation only; no private grant.** Current committed baseline `064021611e95d30e4196f0ed4dfa7ad1c7726902`. The three production scripts match HEAD.

The numeric reread blocker is resolved by that commit and [root acceptance](P2-s53-exhausted-cold-a-01.numeric-validation-accepted.md): one root regression returned exit0, **134 accepts / 158 exact refusals / 28 Int64 observations**. This is numeric validation evidence, not complete production execution. Earlier integration/C2-T records retain their historical pins unchanged; numeric acceptance is supplemental provenance, not a new D1 protocol field.

## Drafts, pins and static consistency

Both [D1 draft](P2-s53-exhausted-cold-a-01-attempt-01.D1.draft.json) and [native plan draft](P2-s53-exhausted-cold-a-01-attempt-01.native-plan.draft.json) remain **activated=false**, outside the active D1 filename. D1: **4770 B / 9a2e26febc8e6f241de4ba9465df527a465a2dcd503f08d0604d50d792d4675b**. Plan: **1365 B / 34a8323e1cb9d5e9a8ebb78336e26bc7ef60f598a5b5fe63f4ca8587c20d5791**.

D1 contains the exact25 root fields,32 numeric caps and18 `{bytes,sha256}` pins required by committed launcher `Validate-Decision`:162–185. All18 actual public files were hashed at their literal `Pin-Spec`:134–152 paths;15 closed pins match constants, and launcher/source_review/source_acceptance use current raw bytes. The JSON contains every resolved pin; key current source pins are:

| Role | Bytes | SHA256 |
| --- | ---: | --- |
| launcher | 71513 | 8cb43e3e6dfba31b21f014ea4f82be1451417a15424ed046d87d2d455d6f3557 |
| core | 82999 | 59787bd263052b12cc77ab524ce48b27f18e2e7a83e0039d9b40f8c3066573af |
| output | 21721 | 331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98 |
| source_review | 16909 | 93178bb38eb5e031bb276261968feeb463b2be2dcb4fbf816f129ea762d719e4 |
| source_acceptance | 4655 | 85f7f1049712d8f2abeda152e4faa5c94e861813c85fdfca65f79807346329c0 |

The fixed review/acceptance paths remain `P2-s53-exhausted-cold-a-01.integration-source-review.md` and `P2-s53-exhausted-cold-a-01.integration-source-accepted.md`, even though their text describes earlier source revisions. Current numeric acceptance supplements that history without altering those files or the18-pin schema.

**66 static comparisons passed** against actual source text: closed field sets/caps,20 scalar bindings (activation intentionally false), pin schemas/values, write/output array order, fixed roles, and accepted corrected numeric predicates. No launcher/dependency import, function call, compilation or regression rerun. Native plan is a planning document; launcher consumes only fixed D1 plus DecisionSha256. Public schemas remain closed by Report-Fields/Validate-Report; no fields were added.

## Prospective activation and command — not executed

Draft serialization is UTF8/no BOM,2-space indent,LF,one final LF. After root review, fresh specific permission and allocation, the **only** intended active-byte change is the unique ASCII `"activated": false` → `"activated": true`. Calculated only in memory: **4769 B**, SHA256 **`f93374eb068eb65250b115722f74494386b0be43a34d34e53b257a5fb46efafb`**. No active D1 was read or written. Any source/document/decision change requires fresh hashes and review.

Future active path: `E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\P2-s53-exhausted-cold-a-01-attempt-01.D1.json`. Native-plan arguments already refer to that prospective hash, not the draft hash:

```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoLogo -NoProfile -NonInteractive -Mta -File 'E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s53-cold-preservation.ps1' -DecisionSha256 'f93374eb068eb65250b115722f74494386b0be43a34d34e53b257a5fb46efafb'
```

Cwd W below; one NativeToken attempt, **work45 / whole60 seconds, lane1/retry0, no network**. Keep internal Mode/clock default; guardians/job/common start remain production-controlled. Preserve actual UTC/native chunks/exit/cleanup; do not add overlapping durations. Root handles any future permission/allocation/activation.

## Exact target roles and operations

W=`E:\AI-Projects\kilogram-p2p-messenger`; none of these private paths was queried in preparation.

| Role | Fixed absolute path | Future scope |
| --- | --- | --- |
| O | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01` | Only the two existing O lock files below plus no-follow ancestor/identity checks; no content traversal. |
| S | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb` | Complete bounded raw-byte traversal/hash of existing test state copy. |
| BKP | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01` | Complete bounded raw-byte traversal/hash of preserved copy; direct mirror, no payload child. |
| Inventory | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01\preservation-inventory.json` | Existing retained inventory:6765525 B/SHA256 `c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3`; validate three maps/provenance. |
| Output parent | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence` | Optional exact create-if-missing with restricted ACL; existing ACL unchanged. |
| E | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-a-01-attempt-01` | Fresh restricted directory and four new receipts only; collision refuses. |

Six existing read-only/share0 locks, never created/truncated:

```text
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01\one.state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01\two.state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb\one\state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-storage\l1-exhausted-observed-NCjBHb\two\state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01\one\state\.kilogram-state.lock
E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preserved\F-exhausted-preservation-01\two\state\.kilogram-state.lock
```

Hold all expected objects before hashing; verify empty locks and raw S/BKP against the pinned historical inventory:6956 files/45597399 payload bytes per tree, four passes/182389596 hash bytes. Raw reads include opaque key/credential envelopes **without decryption or typed credential/DB open**. Bounded local process-name/ID observations (<=4096; conflicting kilogram-* refuses) are auxiliary; only own attempt processes may be terminated.

Locks deny conflicting ordinary opens during ownership, supporting cooperative exclusion. Scans prove observed entry sets, not continuous namespace absence or state after release. **CaptureAndCompare releases inputs before receipts**: the comparison interval ends at release. A later typed open needs separate authorization and treatment of the non-atomic ownership gap. LastAccess updates by the OS remain excluded.

Only E receipts: owner.private.json<=65536B; comparison.private.jsonl<=33554432B; namespace.private.jsonl<=16777216B; result.private.json<=65536B, aggregate<=67108864B. Public CreateNew prefix `W\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\P2-s53-exhausted-cold-a-01-attempt-01` has exactly `.started.json,.claim.json,.p2.json,.parent.json,.supervisor.json`, each<=65536B/total<=327680B. Public reports contain aggregates/bindings/sanitized causes, never per-file private paths/hashes or input bytes. Whole-attempt verdict and continuous namespace absence remain not_established.

Current presence/ACL/lock availability, private equality and I/O duration are **unobserved**. No repair/recovery/rewrap, original payload/ACL change, copy/restore/delete/reuse or retry; retain partial evidence on failure. No new static blocker found in this preparation; root review/private permission/allocation remain required. Whole S5.3/G1–G4/typed-open/S5.4 stay open.

## Проект краткого разрешения — root проверит до запроса пользователю

Разрешаю одну локальную Cold-A проверку тестовых копий Kilogram по точным ролям и шести lock-путям таблицы выше: прочитать и сравнить полные S/BKP с существующим inventory, включая непрозрачные файлы ключей/учётных данных как байты без расшифровки. В O разрешены только два перечисленных существующих lock-файла и проверка их предков; остальные исходные данные O не читаются.

Разрешаю удержание этих read-only блокировок, ограниченную локальную проверку конфликтующих процессов, создание указанного output parent только при отсутствии (ACL существующего не менять), свежего E и только четырёх private receipts и пяти агрегированных публичных отчётов выше. Работа45с/общий предел60с, lane1/retry0, один NativeToken-запуск без сети; завершать только свои процессы проверки. При ошибке сохранить частичные доказательства и остановиться.

Разрешение не включает typed DB/credential open, расшифровку, repair/recovery/rewrap, изменение исходного содержимого O/S/BKP или системных настроек и повторный запуск. Это пока **проект**, не полученное разрешение.

Only these three inert draft/review documents updated. No private stat/list/read/ACL/process probe, helper runtime, E/parent creation, active decision/claim/started, allocation/nav/ledger/commit/push. Callback and **STOP for root review**.
