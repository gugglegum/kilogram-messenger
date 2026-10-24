# A3-S2: performance repair 2 — partial result для root review

2026-10-24. **Измеренный выигрыш получен; полный S2 resource gate не закрыт.**
Новый small scenario завершён за188.486s вместо399.617s. На10k range ускорился
132.041→44.773s, fill8984 новых rows —161.564→98.844s. Этого недостаточно для
общего ceiling1200s: линейная оценка только следующих fill/range/resume равна1663.4s.
100k build не запускался; его memory/ratio gates **UNAVAILABLE**. Commit/push нет.

Основание: [implementation review1](long-lived-dialog-a3-history-s2-implementation-review-1.md),
SHA3370ebd932a07d6402cbd68423a60a0e579e53fba0e00b2cd7ceb36adf7bf447.
Baseline HEAD61524df3a77203d73673f47d38decf6b817615e3. Исполнитель прежний OD1,
turn01a16e2a-b7cd-7131-9b23-3760839c4a66, GPT-6.1 Sol/max; config default/no Fast.
Native E2E=0. Исходные [report](long-lived-dialog-a3-history-s2-implementation.md),
его evidence и root review evidence заморожены; этот документ описывает только repair2.

## Сохранённый baseline и диагностика

**До правок** сохранены точные14 build inputs, включая7 untracked S2 modules,
root process test и tracked patch. Это набор файлов поверх61524df, без копии repo/.git.
[Snapshot provenance](long-lived-dialog-a3-history-s2-evidence-2/source-before-provenance.json).
Исходный library EXE8320000B сохранён в постоянном
`.tmp/a3-s2/baseline-2/kilogram-state-lib-kilogram_state-tests.exe`, SHA
ff42d3932ac86816e6e29ff221fc3d8b0ed03c4a574a871a327c6fd4a3f34835.
Отдельно сохранены instrumented-before и value-only source/EXE snapshots.

Оригинальная `.tmp/a3-s2/history-small` с10016 Text не открывалась; DB осталась
df9d8293753daff382603baa2dcc4670705407e29155325007577c10644b0841.
Каждая диагностическая фаза использовала новую полную synthetic copy, включая
локальный DPAPI key file. SHA проверяется до открытия. Открытие redb само пишет320B,
поэтому исходную копию после open нельзя повторно выдавать за initial hash.

Новый private `history_profile.rs` проверяет provenance/hash, expected owner,
существующий authenticated history scope/count и неизменный duplicate Init. Число
rows32..100016 задаётся provenance; read replay делает максимум12 bounded page/point
samples с private cursor seeding известной synthetic chain. Для других размеров cuts
распределяются по сохранённой истории, без prefix traversal. Runtime importer/API нет.
Writes16 ordinary +fill128 выполнялись только на отдельных disposable copies.
Последний reuse smoke прочитал копию новой сохранённой fixture, без regeneration.

Профилирование `cfg(test)`, opt-in thread-local, выключено в обычных tests/scale.
Таймеры inclusive: вложенные sections нельзя суммировать. Page API включает собственные
реальные CBOR/JSON guards; внешнее encoding и streamed oracle измерены отдельно.
Подготовка writer oracle исключена из write API timer; sync взят из backend counters.
Tracking ограничен4096 keys; truncation в этих samples отсутствует.

На исходных12 pages +12 points было3410 gets,2011 повторных внутри Work,33919 node
opens/3706.9ms и3410 value opens/583.1ms. Typed decode около4.8ms; API encoding11.1ms,
oracle52.6ms. Prefix230.8ms не оказался главным местом; prefix/common code не менялся.
У fill128 value open519.5ms, node seal741.8ms/6757 calls, sync136.5ms. Это измеренные
storage sections, не полный CPU profiler и не нормативный acceptance timing.

## Ограниченные изменения

`verified_values.rs` переиспользует уже проверенный plaintext/absence в одном Work:
максимум128 entries и1MiB фактических Vec capacities, metadata считается дополнительно
в conservative app bound. Ключ — physical namespace slot +logical key; Author aliases
обоих C используют одну entry. На том же authenticated namespace root повторный get
берёт проверенный результат. При смене root сначала проверяется актуальная leaf proof,
затем полная версия hash/length/reserved/owner; совпадение разрешает reuse plaintext.
Put/remove обновляют positive/negative entry. Global owner prefix проверяется и на
cached return; read чужого global owner сохраняет прежнюю S1 семантику, mutation
связан с owner C. Account/context/incarnation проверяются; cache не переживает Work.
Plaintext хранится в Zeroizing, eviction освобождает его.

После value-only измерения оставалось1942 node opens и6757 seals на fill128. Поэтому
добавлены два ограниченных изменения существующего node overlay:

- Только короткий `history_read` включает FIFO проверенных decoded nodes:
  максимум512 entries **и** conservative1MiB, lifetime одного read transaction.
  Полный per-candidate proof map по-прежнему очищается; reuse требует совпадения
  scope и всей expected Summary. На mutation FIFO отключается/очищается. Нового
  Store cache/MVCC snapshot между страницами нет; app bound включает обе структуры.
- Новые trusted nodes уже находились в `decoded` у прежнего engine. Теперь промежуточные
  версии в Work резервируют точную длину envelope, но AEAD получают только финальные
  публикуемые версии. `seal_changes` вызывается common engine до чтения bytes для plan
  и до backend write; missing node/change или несовпадение длины — Integrity.
  Удалённые промежуточные nodes не публикуются. Hash/summary каждого изменения,
  actual16MiB plan bound и node envelope cap256B сохраняются. Persisted/old nodes
  по-прежнему проходят AEAD open +decode +proof; source/value validation не отключена.

Последний пункт затрагивает common S1 engine и требует отдельного внимания root review.
Node/value format/schema5, AAD/KDF и durable transaction protocol не менялись. H1/H2,
R1 committed baseline/spent, R2 matching receipt, actual credits и Runtime batch≤4
сохранены. Повышения limits/cache/profile и изменения fixture distribution нет.

## Before/after на одинаковых synthetic copies

Полные raw/parsed результаты и namespace counters:
[comparison](long-lived-dialog-a3-history-s2-evidence-2/profile-comparison-2.json).
Каждая read колонка —12 page API calls; node counters относятся к12 pages +12 points.

| Измерение | Before | Value only | Value +node changes |
|---|---:|---:|---:|
|12 pages total ms|5443.572|2996.735|1721.724|
|12 points total ms|183.489|158.715|150.941|
|Read node opens|33919|19134|10746|
|Read node open ms|3706.923|2093.212|1106.171|
|Fill128 ms,9 real commits|2728.958|2005.985|1453.823|
|Fill value open ms|519.460|11.656|11.604|
|Fill node seals|6757|6757|2935|
|Fill node seal ms|741.842|724.629|287.558|
|Fill sync ms|136.463|128.437|131.163|
|Ordinary16 ms,32 real commits|853.068|802.748|875.438|

Actual bulk plan в этих samples одинаков:1457498B. App bound вырос27259967→28985286B.
Logical page gets≤274, candidates≤2 во всех фазах; backend read bytes4792320B во всех
read samples. CPU reuse не выдаётся за снижение этого измеренного backend I/O.
Ordinary append не ускорился; суммарный sample875.4ms против853.1ms.

## Новая persistent small fixture

Один новый unprofiled1k/10k scenario выполнен с прежними verification/cache64,
source/text1024B, Native256B, обеими lanes/до2 parents и local1/16. Fill использует
прежний private batch128 с реальными QueueAccept/AcceptPending/TextBatch transitions,
обычный API batch≤4 не расширен. Oracle потоковый; N-sized expected vector отсутствует.
Сохранена `.tmp/a3-s2/history-small-2`,10016 user Text, DB147755008B, SHA
7c613e31c74cb78733e5d459161bf83d67fbbc8096d94251c9b53c9df2e01814.

| Метрика |1k before→after|10k before→after|
|---|---:|---:|
|Fill seconds|14.726→7.938|161.564→98.844|
|Full range seconds|10.668→3.790|132.041→44.773|
|Page32 p95 ms|340.885→121.258|451.749→148.518|
|Point p95 ms|12.950→12.094|14.574→13.415|
|Ordinary append p95 ms|43.179→40.589|49.981→59.383|
|Old midpoint resume seconds|5.801→2.039|66.329→22.567|

Полный elapsed188.486s, только **SMALL_TIMINGS_ONLY**. Сохранились exact streamed
domain,3 checkpoints, старый finished cursor, новые append/first/point, reopen и C2.
10k ordinary append p95 вырос примерно на19%; отдельного подтверждения причины нет.
Raw [history-small-2.log](long-lived-dialog-a3-history-s2-evidence-2/history-small-2.log),
[parsed](long-lived-dialog-a3-history-s2-evidence-2/history-small-2.json),
[comparison](long-lived-dialog-a3-history-s2-evidence-2/small-comparison-2.json).

Counters теперь отдельные: fill71+632=703, ordinary32+32=64, cumulative measured767;
setup явно исключён. Старый ошибочно названный `fill_commits` в frozen logs не менялся.
Actual max plan1496996B, app bound29212662B, process peak paged97914880B,
max sampled private93683712B; cache≤67108864B. Это малые absolute checks;100k/1k
raw96MiB/residual32MiB и page/append ratio≤4 остаются непроверенными.

[Precheck2](long-lived-dialog-a3-history-s2-evidence-2/performance-precheck-2.json):
для следующего fill89984 rows линейно990.0s, range100k447.7s, midpoint resume225.7s;
только эти три фазы1663.4s. С уже измеренными1k/10k получается1851.9s до дополнительных
100k reopen/ordinary/memory расходов. Это оценки, не100k measurement/математическая
граница. Запаса на1200s нет; соблюдена остановка до дорогого build. Оставшееся время
fill распределено между финальным AEAD/proof/validation/plan и durable I/O; timings
inclusive, новой массовой оптимизации или увеличения бюджета в этой итерации нет.

## Проверки и точность evidence

Все harnesses компилировались `--locked --no-run`, затем запускались только через
`scripts/run-cargo-tests-stable.ps1`, постоянные EXE paths/test-threads1. Offline,
verification, Jobs2/BelowNormal; [commands2](long-lived-dialog-a3-history-s2-evidence-2/commands-2.json).

| Проверка | Результат |
|---|---|
|S2 small, финальный source|19/19 PASS8.60s; прежние14 +5 cache/node regressions|
|S1 affected components|14/14 PASS18.45s, включая maximal plan15231748B, split/merge, IO/fault/integrity/reopen|
|S1 independent R1/R2|5/5 PASS17.93s; Native/Current/Request growth/repeat conservation и receipt ordinal|
|Root public cursor, owned OS process|PASS parent0.61s/child0.03s, PID31668; root test bytes не менялись|
|Final fixture reuse smoke|PASS3.60s, без fill; scope/count/hash/duplicate и12 bounded samples|
|Strict Clippy state all targets/cache_metrics|PASS clippy-2-2.log|
|Workspace rustfmt/whitespace|PASS verification-final-2.log|

Production-growth и3 owned-child kill points S1 не повторялись: unchanged backend
commit/recovery покрыты прежним frozen16/16 evidence; текущие14 затронутых tests и
новый root OS-process test указаны отдельно. Это не новый PASS пропущенных проверок.
Новые regressions проверяют stale positive/negative, Native/Current/Request updates,
reserved version, global aliases/owner/cross-C, entry/byte eviction, scope/incarnation,
corruption на новом доступе/transaction, reused Summary mismatch и AEAD всех финальных
nodes. Старые H1/H2/fault/cursor/encoding tests сохранены.

Первичные profiler failures сохранены: `Get-FileHash` недоступен в child Windows
PowerShell; ошибки произошли до open. Успешный helper использует streaming.NET SHA256,
hidden/BelowNormal process, без изменения global PSModulePath. Первый strict Clippy
нашёл только derivable Default в test-only Stats; исправлено derive, log сохранён.

[Final build provenance](long-lived-dialog-a3-history-s2-evidence-2/build-provenance-final-2.json)
фиксирует19 inputs/bytes/SHA/blobs и3 stable EXE; final library8798208B, SHA
bccec3ff1b4cce66aea2e54a1d496496222125f7bf46c746b85e56d543a8c946.
Ограничение provenance: точный SHA EXE при node-profile/small timings отдельно не был
записан. После них менялись только `cfg(test)` fixture precheck и эквивалентный derive
Default; product/scale sources не менялись. Final correctness/reuse выполнены на
финальной сборке. Её hash не выдаётся за hash прежнего timing EXE. Дооптимизационный
EXE и instrumented-before/value-only EXE pinned отдельно, без этой неопределённости.

[Preservation](long-lived-dialog-a3-history-s2-evidence-2/preservation-final-2.json):
98 frozen/preserved artifacts совпали, включая57 S1 docs/evidence,4 S2 contract docs,
старые19 S2 evidence, root artifacts, L1 EXE и обе S1 DB. Original history df9… и
new history7c61… сохранены; development-environment.md clean SHA53074ff… по прямому
human override. Index пуст, HEAD61524df…; commit/push отсутствуют.

**Следующая точка — root review этой ограниченной итерации и оставшегося performance
blocker.** S2/A3 не объявляются принятыми. Wire/native continuation/Root2/migration/
runtime/UI/A2 deferred scope не реализовывался.
