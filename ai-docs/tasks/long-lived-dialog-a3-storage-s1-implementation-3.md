# A3-S1: final R3 measurement, iteration 3

2026-10-24, Asia/Yekaterinburg. Сдача на review, без commit/push.
R1/R2 уже приняты [review2](long-lived-dialog-a3-storage-s1-review-2.md), включая
независимые root5/5 regressions. Здесь менялись только measurement helpers.

Прочитаны полностью и сверены:

- review2 SHA-256 `e01439325b07b3c060dbc52c64b58edfe92ef7a860818a878537ce1a65ff2f26`;
- [memory amendment1](long-lived-dialog-a3-memory-budget-amendment-1.md)
  SHA-256 `6f02c684d11ce5ae54cef58857719e60ab465032c0b2fb01308234b1a9abab5c`.

Это нормативное дополнение к исходному контракту. Старый raw64MiB FAIL/OPEN не
переименован в PASS; исходные contract, reports1/2, reviews и evidence сохранены.

Baseline HEAD `f59d285d5704dc2c6969b6f8e416f204223e27cc`, index пуст.
Actual turn `01a16d18-02d9-7b61-99fe-1fc28eebbd49`: `gpt-6.1-sol/max`;
config service_tier=default, turn tier отдельно не сериализован, Fast не включался.
Новых чатов/субагентов нет.

## Изменения helper

`memory_tests.rs`: unsupported platform возвращает unavailable process measurement,
а не ошибку storage functionality и не фиктивные0. Отсутствие redb instrumentation
даёт `cache_metrics_available=false`, cache bytes=null и причину. Реальный нулевой
cache может считаться измеренным только при наблюдавшихся counters. Windows query
остаётся hidden PowerShell собственного PID; WorkingSet/heap не сокращаются.

`scale_tests.rs`: normative flag требует Windows verification, новую full fixture и
доступные process/cache measurements **до fill**. Обычный run сохраняет functional
get/range/domain checks при unavailable metrics. Final result содержит explicit gate
status; normative UNAVAILABLE/FAIL завершает тест ошибкой. В каждой точке M и C берутся
из одного quiescent snapshot. Полный range проверяет strict unique keys, n∈[0,N),
key=id(n), exact1024B payload и count=N без N-sized expected vector.

Новый `scale_gates.rs` проверяет:

- полную последовательность1k/10k/100k, шесть уникальных фаз каждой, один PID,
  verification debug_assertions и одинаковые sizes: page32, Event1024B,16 appends;
- по каждой соответствующей warm-range и post-append фазе одновременно
  `M100k−M1k≤100663296B` и `(M100k−C100k)−(M1k−C1k)≤33554432B`;
- signed i128 arithmetic без clamp и без выбора выгодного minimum;
- quiescent cache≤64MiB, configured64MiB, каждая измеренная process-фаза≤384MiB,
  отсутствие expected vector; peak/latency-sample/live page bytes отражены отдельно;
- p95 page/append ratios≤4, actual plan≤16MiB, bounded ordinary node lookups,
  полный create/fill/read/append elapsed≤1200s.

Gate helper проверен отдельными arithmetic fixtures: exact residual boundary,
превышение на1B в каждой из двух фаз, raw failure, отрицательные signed deltas,
отсутствие cache/platform measurement, разные PID, cache excess и time exceed.
Эти vectors не являются storage/performance evidence.

App-owned192MiB, backend write128MiB и production cache/geometry не менялись.
Это прежние source bounds; samples не выдаются за точный global heap peak.
PeakPagedMemorySize64 выводится под собственным именем, не как allocator proof.

## Проверки перед единственным heavy run

Instrumented gate helper PASS; small1k/10k storage helper PASS45.50s. Последующее
добавление optional-measurement smoke не меняло benchmark/gate logic; gate helper
повторён на окончательном EXE и PASS. Default no-cache_metrics smoke PASS0.64s:
cache bytes=null/available=false, actual get/range проходят. Non-Windows ветка
исправлена в исходнике; Linux runtime/cross-target testing не заявляется.

Strict Clippy verification all-targets default и instrumented PASS, fmt и PowerShell
parser PASS. Initial Clippy helper complaint об `expect` исправлен обычным checked
Option handling, без suppression; исходный log сохранён.

Принятые R1/R2 production sources и root integration tests не менялись и повторно
не запускались. По сравнению с принятой iteration2 изменены только memory_tests.rs
и scale_tests.rs, добавлен scale_gates.rs. Cargo.toml/lock/default features/backend
и общий stable runner без новых изменений.

## Нормативный run

Только один fresh initial100k run, path
`.tmp/a3-s1-repair-2/persistent-scale-initial`. Перед началом путь отсутствовал;
REQUIRE_NEW guard запрещает reuse/reset. Старый `.tmp/a3-s1/persistent-scale`
сохранён и в этом этапе не открывался benchmark.

Все этапы1k→10k→100k выполняет один verification process/test с production cache64MiB
и `redb/cache_metrics`. Bounded bulk≤512 создаёт реальные persistent encrypted Event
records; после каждого этапа drop/open, exact range и16 ordinary single appends.
До append counts1000/10000/100000; после1016/10016/100016. Синтетические1024B records
не являются native E2E messages и не закрывают A3 history cursor gate.

Source/harness frozen до запуска:16 source/test/helper files,
[source-final.json](long-lived-dialog-a3-storage-s1-evidence-3/source-final.json)
SHA-256 `af4662621709ed73c4280eebc6c142cbafbcd32c736fc614dd1864a7d9daaa78`;
fixed EXE `target/verification/deps/kilogram-state-lib-kilogram_state-tests.exe`,
SHA-256 `1f8a71407afcefc20bcb46f341fd41e6b76197b85b910ecbaf8c33c37e1ac08b`.

**Нормативный результат: PASS.** Один fresh create/fill/read/append run занял
558.309309s (9min18.31s) при1200s ceiling; harness558.33s. Один PID34508,
244 application commits. До финальных16 append:100000 logical Event records,
199999 physical node rows и100000 value rows; в конце100016/200031/100016.
Fixture269,422,592B сохранена после закрытия. Повторного heavy run не было.

| Один verification setup, до16 append |1k|10k|100k|
|---|---:|---:|---:|
| Fill interval records |0→1000|1016→10000|10016→100000|
| Fill, s |2.796|28.814|431.391|
| Reopen, ms |31.632|244.679|2279.934|
| Full exact range, s |1.033|6.930|78.908|
| Page32 p50/p95, ms |19.567/21.344|20.961/22.143|25.077/26.442|
| Ordinary append p50/p95, ms |10.104/11.881|12.734/13.377|16.828/19.066|
| Max single append node reads / plan B |13/18101|15/19087|18/20566|
| Max bulk actual old/new plan B |1030424|1909711|2795168|
| Physical nodes / values |1999/1000|19999/10000|199999/100000|

100k/1k p95 ratios: page**1.238875**, append**1.604722**, обе≤4.
Все ordinary appends имеют growth0 и по одному sync; новые records оплачены обычным
путём, не обособленным importer. Числа не являются native/crypto engine throughput.

| Сопоставимая memory-фаза |M1k B / C1k B|M100k B / C100k B|Raw growth|Residual growth|
|---|---:|---:|---:|---:|
| Full warm range, counts1000→100000 |6295552 /2527552|99401728 /67108864|93106176B /88.793MiB|28524864B /27.203MiB|
|16 appends, counts1016→100016 |6406144 /2588992|99401728 /66883904|92995584B /88.688MiB|28700672B /27.371MiB|

В обеих фазах одновременно raw≤96MiB и residual≤32MiB: **PASS** по принятому
amendment, без clamp/вычитания test buffers. Quiescent cache max67,108,864B=64MiB,
configured64MiB. Max M среди18 измеренных фаз99,401,728B=94.797MiB≤384MiB,
включая fill/reopen. Max отдельно сообщённый PeakPagedMemorySize64=106,631,168B;
это OS property sample, не утверждение о global allocator peak. Live returned page
payload≤32,768B. Capacity latency samples512/4096/32768B не вычитается из M;
N-sized expected vector отсутствует. Raw64MiB исходного критерия по-прежнему был бы
FAIL: изменение результата относится к явно принятому amendment, не к старому gate.

| Sampled phase I/O, B |1k|10k|100k|
|---|---:|---:|---:|
| Fill read / write |0 /3383936|0 /114591360|847093760 /3403533312|
| Fill grow / syncs |3158016 /4|29474816 /21|235741184 /179|
| Open read / write / syncs |2527561 /320 /1|24445257 /320 /1|243839305 /320 /1|
| Full range read |0, debug-warm|0, debug-warm|714633216|
|16 appends read / write / grow |0 /1795072 /0|0 /2745344 /0|532480 /3724288 /0|
|16 appends syncs |16|16|16|

Range writes/grow/syncs0 на всех трёх этапах.100k fill имеет3 actual set_len calls,
growth time549,091,300ns и sync time28,244,904,600ns; ordinary appends sync time
75,377,400ns. Полные per-phase backend snapshots/разности находятся в result JSON.
Counters привязаны к открытому backend instance: native close/shutdown I/O после
последнего snapshot отдельно не суммируется; overall elapsed включает close.
Это не полная host I/O trace. Redb debug open сам прогревает малые DB, поэтому
нулевые1k/10k page backend reads не называются cold disk I/O. Их open reads измерены;
separate production release cold probe был проверен в первой сдаче и здесь не повторён.

```powershell
$env:CARGO_NET_OFFLINE='true'
$env:RUST_TEST_THREADS='1'
$env:TEMP=Join-Path (Get-Location) '.tmp/a3-s1-repair-3'
$env:TMP=$env:TEMP
$env:KILOGRAM_S1_SCALE_DIR=Join-Path (Get-Location) '.tmp/a3-s1-repair-2/persistent-scale-initial'
$env:KILOGRAM_S1_REQUIRE_NEW_SCALE='1'
$env:KILOGRAM_S1_NORMATIVE_SCALE='1'
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -CargoFeatures @('redb/cache_metrics') -Filter indexed_scale_persistent_100k -CargoJobs 2 -Run -NoCapture
```

Runner компилирует `--no-run` и исполняет fixed path; никаких прямых network cargo test.
CargoJobs2/BelowNormal/locked/offline. Hash в имени EXE не используется. Cache-report
footer с default jobs12 не является настройкой компиляции этого прогона.

## Сохранность и remaining gates

Сводный [validation-result.json](long-lived-dialog-a3-storage-s1-evidence-3/validation-result.json)
SHA-256 `cd741ca95770a344a9d3368fce0a3a25ac7f46741b18b20b82c5dd8a75412757`.
Полный [initial100k-result.json](long-lived-dialog-a3-storage-s1-evidence-3/initial100k-result.json)
SHA-256 `8712d424ead30dc49a8796936d97b1120969d1aa778c0a624857179e20804dc9`.
Всего15 новых evidence files; manifest связывает все остальные14 hashes.
Обе signed формулы отдельно пересчитаны после run по captured M/C и совпали с
Rust gate result. Все18 memory points available; failures/unavailable пусты.

Новые public evidence только в `long-lived-dialog-a3-storage-s1-evidence-3/`;
vault DB/DPAPI files/private keys не публикуются. После measurement fixture сохраняется.
Source/EXE hashes повторно сверяются после run; старые accepted/evidence hashes и
foreign `development-environment.md` сохраняются SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Baseline compatibility EXE остаётся d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1.

Wire2/native Olm, Root2/offline dialogue, causal history cursor, live L1 migration,
runtime activation и direct/relay acceptance остаются следующими A3 частями.
Extra secret getter не добавлялся и отказ не обходился. Deferred A2 security
reproducer не запускался/не разрабатывался; A2 OPEN7 warnings/strict advisory FAIL.
S1 и публикация не принимаются автоматически успешным benchmark: после одного
содержательного callback root — остановка до review, без commit/push.
