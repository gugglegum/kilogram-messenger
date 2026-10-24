# A3-S1 final implementation review

2026-10-24. **A3-S1 ПРИНЯТО.** R1/R2 закрыты review2 и независимыми root5/5
regressions; final measurements закрывают R3 по явно принятому memory amendment1.
Разрешена публикация точного accepted allowlist отдельным commit/push. Весь A3
не завершён; live runtime/wire2/migration/full history reader не принимаются здесь.

## Итоговая ревизия и сверка

Baseline `f59d285d5704dc2c6969b6f8e416f204223e27cc`, index пуст.
[Отчёт iteration3](long-lived-dialog-a3-storage-s1-implementation-3.md) SHA-256
`3431c11a2f4bae947d57b00608514ae5f171bbe49bb18aa65a4c21c4523c2f58`.
[Source manifest](long-lived-dialog-a3-storage-s1-evidence-3/source-final.json)
`af4662621709ed73c4280eebc6c142cbafbcd32c736fc614dd1864a7d9daaa78`,
[validation result](long-lived-dialog-a3-storage-s1-evidence-3/validation-result.json)
`cd741ca95770a344a9d3368fce0a3a25ac7f46741b18b20b82c5dd8a75412757`.

Root независимо сверил16 source SHA/Git blobs и14 evidence hashes. По сравнению с
review2 изменены только memory/scale helpers и добавлен scale_gates; принятые
production R1/R2 и integration tests побайтно прежние. Fixed-path executed EXE
`1f8a71407afcefc20bcb46f341fd41e6b76197b85b910ecbaf8c33c37e1ac08b` совпал с manifest.
Результат initial100k JSON точно совпал с единственным final object реального run log.
Повторный heavy run root не запускал: новых сомнений в unchanged storage code нет.

Прочитаны новые helpers: обе memory-фазы сравниваются signed arithmetic без clamp;
недоступные метрики не превращаются в0/PASS; нормативный run требует нового store,
Windows verification, cache instrumentation и полного1k/10k/100k набора. Ordinary
unsupported-platform checks сохраняют functionality с явной availability. Helper
negative/boundary tests, fmt/strict Clippy и default/instrumented smoke подтверждены
представленными журналами. Linux runtime validation не заявляется.

## Независимый пересчёт финального результата

[Root verification](long-lived-dialog-a3-storage-s1-review-3-evidence/independent-verification.json)
SHA-256 `3814e782b9b6aea3185843da0f9c5d448d045bd47b2566f28b1f0aa824956cae`.
Один PID34508,18 доступных measurement points, один новый persistent store.

| Gate | Независимо подтверждённое значение |
|---|---|
| Fresh create/fill/read/append |558.309309s; harness558.33s,≤1200s |
| Warm range raw / residual growth |93,106,176B /28,524,864B,≤96MiB /32MiB |
| После16 append raw / residual growth |92,995,584B /28,700,672B,≤96MiB /32MiB |
| Max sampled PrivateBytes / cache |99,401,728B /67,108,864B,≤384MiB /64MiB |
|100k/1k p95 page / append ratio |1.238875 /1.604722,≤4 |
| Final logical Event / physical node / value rows |100016 /200031 /100016 |

100k stage до16 обычных append содержит ровно100000 Event,199999 nodes и100000
values. Полный range проверяет unique/order/exact domain/payload без N-sized
expected vector; bulk использует тот же persistent commit/index/AEAD, не отдельный
обход quota/integrity. Normal append максимальные node reads18, plan20566B;
max bulk old/new encoded plan2795168B. Storage fixture сохраняется.

Точная граница timing: recorded558.309309s снято до последнего drop открытого store;
оно включает intermediate closes/reopens. Harness558.33s включает возврат функции
и последний drop. Оба ниже1200s с большим запасом. Sampled I/O snapshots не являются
полной host trace; final shutdown I/O не подменяется нулём. Max sampled PrivateBytes
не является доказанным global allocator peak. Малые debug-warm pages не названы
cold I/O; прежний release-only cold proof остаётся отдельным evidence.

Raw64MiB исходного критерия остаётся не выполненным. PASS относится к
[принятому amendment1](long-lived-dialog-a3-memory-budget-amendment-1.md), сохраняющему
absolute384MiB/app192MiB/cache64MiB, bounded operations, ratios и time ceiling.
Ни исходные превышения, ни первый0/2 regression baseline не переписаны.

## Что принимается и что остаётся

Принимается отдельная библиотека IndexedDialogVault: protected encrypted root,
authenticated radix records/ranges, atomic records+ledger, bounded mutation plans,
typed queue/pending/debt transitions, owner/schema fences и backend quotas/stats.
Реальные fault/kill/capacity/growth evidence первой сдачи и адресные repair tests
относятся к неизменённым соответствующим paths. Не объявляется UI/публичный MVP.

Остаются A3 части: native/protocol2 continuation за511, Root2/offline dialogue,
causal history cursor, existing L1 migration/handoff, live runtime и routes gates.
Synthetic100k storage records =0 native E2E messages. A2 остаётся OPEN:7 warnings,
strict advisory FAIL; deferred security reproducer и extra secret getter не входят
в принятый diff и не добавлялись как обход прежних отказов.

## Публикация

Прежний OD1 исполнитель GPT-6.1 Sol Max/default без Fast получает отдельное
поручение commit/push **только** files из
`long-lived-dialog-a3-storage-s1-accepted-files.json` плюс сам manifest, чей hash/blob
root зафиксирует в dispatch. SHA/Git blobs должны совпасть до stage, в index и в
commit. Не менять принятые evidence/docs ради publication status, не добавлять
посторонние files; не выполнять force/reset/clean. Ожидается parent baseline выше.
После callback root самостоятельно сверяет committed path set/blobs и remote ref.

Foreign `ai-docs/development-environment.md` сохраняется и исключён, SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Исторические accepted manifests/contracts остаются неизменными. Publication не
даёт исполнителю разрешения самому начинать следующую A3 часть до root dispatch.
