# A3-S1: repair iteration 2

2026-10-24, Asia/Yekaterinburg. На повторный review, без commit/push.
R1/R2 исправлены и проверены; **R3 остаётся OPEN: сначала требуется решение root
по измеренному конфликту memory gate, затем один final initial100k run**.
Исходные [сдача](long-lived-dialog-a3-storage-s1-implementation.md), evidence и
[review1](long-lived-dialog-a3-storage-s1-review-1.md) не переписывались.
Review1 SHA-256 `1c4b9a6a3d443ba83832a3a1c7a17a1d5971ac74fa4a67146a9779020e9968cb`.

Baseline HEAD `f59d285d5704dc2c6969b6f8e416f204223e27cc`, index пуст.
Actual turn `01a16cf9-419f-7192-9ee9-4f805725fe08`: `gpt-6.1-sol`, effort `max`;
config `service_tier=default`, отдельный turn tier не записан. Fast не включался.
Новых чатов/субагентов, runtime/wire/native/migration работы нет.

## R1: charge от committed baseline всего плана

В `Work` добавлена bounded operation map `baseline_charge`. Первый изменяющий Put
или typed remove запоминает committed charge ключа; последующие версии считают
`max(0, final_charge − baseline_charge)`, а не delta от последнего overlay.
Remove устанавливает final positive charge0 и сохраняет baseline для возможного
последующего put. Map живёт только в одной операции; новых disk fields нет.

Существующий `spent` по-прежнему распределяет положительный charge один раз на
весь batch; immutable identical/conflict, full refs coverage и quota checks не
ослаблялись. Quota не увеличивается ради accepted frame. К existing plan buffers
добавлена одна bounded map тех же затронутых keys, не cache истории.

Root regression Native1024→2048 теперь имеет тот же успешный результат, что один
Native2048, при прежней tight quota. Дополнительная public-API матрица:
Native/Current/Request ×9 сценариев ×single-final/repeated control =54 исполнения.
Проверены absent/existing key, grow→grow, grow→shrink, shrink→grow, same-size content,
exact duplicate и возврат к committed bytes. Проверяется численный retained spend,
равенство конечных ledgers, точные bytes, C2 isolation и idempotent retry после reopen.

Multi-frame test оплачивает общий Native checkpoint один раз из двух funded frames.
Trial с незаявленной positive Current mutation отвергается без изменения root/ledger/
Pending; после reopen исправленный request с тем же commit_id проходит, duplicate
retry не списывает credit повторно. Начальная ошибка дополнительной fixture
(попытка нового grant без CancelGrant старого исчерпанного grant) сохранена в
`extra-test-fixture-initial.log`, исправлена в test setup; это не product regression.

## R2: receipt соответствует Pending ordinal

Readable до изменения ledger требует ReceiptA/B ref с тем же C и canonical ordinal.
Существование ref затем проверяется прежним authenticated `cost/get` path.
Pending1 + Receipt2 теперь даёт Bound; generation, last commit, counts, ledger и
Pending сохраняются, trial Event/Receipt не появляются. Это проверено также после
reopen; matching Receipt1 control проходит. Missing matching receipt и другой C
отвергаются. Signature/opaque receipt contents/current-policy остаются future adapter.

## R3: диагностические данные и решение для root

Добавлен opt-in `memory_tests.rs`. Один verification process10632, тот же production
Builder/cache64MiB, прежние persistent fixtures одной C, payload1024B. Warm baseline —
полный exact range собственных records, без сторонних allocations. Проверяется
строгий порядок/уникальность ключей, n∈[0,count), key=id(n), exact payload и count:
этого достаточно для полного совпадения домена без `Vec<Id>` размера N.

Для реальных cache bytes runner получил необязательный `-CargoFeatures` и передаёт
`--features redb/cache_metrics` при compile `--no-run`. Это уже существующий feature
redb4.2; Cargo.toml/Cargo.lock/dependency versions и production default features
не менялись. Он включает counters, не меняет cache size, geometry или allocator.
Без feature redb возвращает нулевые cache metrics, которые не выдаются за измерение.

Каждая фаза измеряет process PrivateBytes, PeakPagedMemorySize64, WorkingSet64,
redb reported cache data, backend stats, явно живой test vector и returned page bytes.
Запрос памяти — hidden PowerShell только собственного PID. Working set не урезался,
heap/cache не заполнялись искусственно. Нет попытки выдать native heap attribution
или PeakPagedMemorySize64 за точный allocator peak.

| После full warm, до test vector |1016 records|10016 records|100048 records|
|---|---:|---:|---:|
| Process PrivateBytes B |4,513,792|29,569,024|95,612,928|
| redb reported cache data B |2,580,800|24,412,480|67,108,864|
| PrivateBytes − cache data B |1,932,992|5,156,544|28,504,064|
| Live test expected vector B |0|0|0|
| Max returned page payload capacity B |32,768|32,768|32,768|

Observed raw growth =91,099,136B =**86.88MiB**, больше исходных64MiB.
Из него cache data growth64,528,064B =61.54MiB; оставшийся growth26,571,072B
=**25.34MiB** включает реальную cache metadata/Arc/allocator/process overhead.
Это наблюдение и inference о DB-associated памяти, не точное разложение native heap.

На100k test vector имеет capacity3,201,536B и увеличивает PrivateBytes на3,207,168B.
После его drop PrivateBytes возвращаются ровно к95,612,928B. После16 normal appends —
95,629,312B; после store drop —3,579,904B. Значит исключение test vector не устраняет
превышение. Избыточная память присутствует уже после redb open до полного range;
долго живущих N-sized структур в `IndexedDialogVault` нет. Нормативный raw64MiB
gate **не PASS**, прежнее превышение и данные сохранены.

**Узкое предложение для архитектурного решения, ещё не применено:** сохранить
cache64MiB, app buffers192MiB, absolute process384MiB, ratio≤4 и ceiling1200s.
Для warm1k→100k одновременно требовать raw PrivateBytes growth≤96MiB и growth
`PrivateBytes − measured redb cache data`≤32MiB; test-only buffers явно показывать
отдельно. Это учитывает filling настоящего64MiB data cache и ограничивает дополнительную
память, а не прячет её. В diagnostic residual25.34MiB, однако окончательный initial
import ещё должен проверить обе величины. Контракт/лимиты/configuration не изменены.

Это предусмотренная R3.2 остановка: review требует сначала вернуть root конкретный
анализ и узкое предложение, если cache64MiB с overhead конфликтует с целью.
Поэтому единственный новый heavy initial100k run **ещё не запускался**, under1200s
не заявляется. Нельзя выдавать старый reuse за initial import.

## Подготовленный final benchmark и уже выполненные проверки

`scale_tests.rs` теперь проверяет тот же exact domain без expected vector, измеряет
before/after fill, reopen, first-page live, warm range и ordinary append memory/stats.
Timer начинается до create/open;1200s проверяется также во время range и в конце.
`KILOGRAM_S1_REQUIRE_NEW_SCALE=1` отвергает существующий DB, сохраняя его.
Prepared final path `.tmp/a3-s1-repair-2/persistent-scale-initial` ещё не создан.

Новый phased benchmark проверен на **fresh1k/10k**, одном verification setup с metrics:
fill2.756/30.750s, full range1.040/6.956s, page p95=20.866/22.106ms,
append p95=11.204/13.013ms; PrivateBytes5,873,664/32,997,376B.
Обе fixtures проходят exact uniqueness/count/domain checks без expected vector.
Это small helper check, не100k timing и не full A3 history/native E2E.

| Проверка | Результат |
|---|---|
| Root public regressions + расширения |5/5 PASS,14.66s; исходные2 regressions теперь PASS |
| Affected indexed state/fault/credit/legacy |18/18 PASS,20.72s |
| Targeted memory diagnostic |1/1 PASS,97.39s; старый64MiB gate OPEN |
| Phased fresh1k/10k helper |1/1 PASS,46.46s |
| Strict Clippy verification all-targets |PASS, default и redb/cache_metrics |
| fmt и PowerShell parser |PASS |

18 checks включают maximal old/new plan, integrity/owner fences, full pending/head,
accepted bytes, пять injected error points, два честных non-trigger controls,
exact configured-capacity refusal/reopen/retry и три real owned-child kill points.
Backend/geometry/owner dispatch не менялись; production growth и полный OD1 corpus
повторно не запускались. Initial extra fixture failure и исправленный Clippy test
style warning сохранены отдельно; окончательные checks не используют suppressions.

Команды выполняются из workspace, CargoJobs2/BelowNormal/locked/offline. Все harnesses
исполняются только stable runner; compile `--no-run` встроен в него. Hash-based
artifact paths используются лишь для проверки содержимого, не для запуска EXE.

```powershell
$env:CARGO_NET_OFFLINE='true'
$env:RUST_TEST_THREADS='1'
$env:TEMP=Join-Path (Get-Location) '.tmp/a3-s1-repair-2'
$env:TMP=$env:TEMP
# Diagnostic completed against preserved fixtures:
$env:KILOGRAM_S1_MEMORY_DIAGNOSTIC='1'
$env:KILOGRAM_S1_PREVIOUS_FIXTURE_BASE=Join-Path (Get-Location) '.tmp/a3-s1'
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -CargoFeatures @('redb/cache_metrics') -Filter indexed_memory_diagnostic_preserved_fixtures -CargoJobs 2 -Run -NoCapture
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Filter a3_s1_review_ -CargoJobs 2 -Run -NoCapture
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Filter indexed_ -SkipTest @('indexed_scale_persistent_100k','indexed_scale_small_verification','indexed_memory_diagnostic_preserved_fixtures','indexed_component_production_growth_single_append','ordered_maximal_index_commit_and_prepaid_reservations_are_bounded') -CargoJobs 2 -Run -NoCapture
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -CargoFeatures @('redb/cache_metrics') -Filter indexed_scale_small_verification -CargoJobs 2 -Run -NoCapture
./scripts/invoke-cargo-friendly.ps1 -CargoCommand clippy -CargoJobs 2 -CargoArguments @('--locked','--offline','--profile','verification','-p','kilogram-state','--all-targets','--','-D','warnings')
./scripts/invoke-cargo-friendly.ps1 -CargoCommand clippy -CargoJobs 2 -CargoArguments @('--locked','--offline','--profile','verification','-p','kilogram-state','--all-targets','--features','redb/cache_metrics','--','-D','warnings')
cargo fmt --package kilogram-state -- --check
```

**Подготовленная команда следующего heavy run — не выполнялась:**

```powershell
$env:KILOGRAM_S1_SCALE_DIR=Join-Path (Get-Location) '.tmp/a3-s1-repair-2/persistent-scale-initial'
$env:KILOGRAM_S1_REQUIRE_NEW_SCALE='1'
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -CargoFeatures @('redb/cache_metrics') -Filter indexed_scale_persistent_100k -CargoJobs 2 -Run -NoCapture
```

## Evidence и сохранность

Новые public files только в [evidence-2](long-lived-dialog-a3-storage-s1-evidence-2/).
`source-diagnostic.json` фиксирует pre-R1/R2 diagnostic source/EXE; `source-final.json`
включает все15 source/test/helper files, в том числе root integration regression.
`memory-analysis.json` содержит фазовые исходные метрики и proposal; validation result
связывает executions и hashes. Production sources R1/R2 не менялись после fast checks;
затем изменился только исключённый из них benchmark helper, проверенный малым run.

Итоговый [source-final.json](long-lived-dialog-a3-storage-s1-evidence-2/source-final.json)
SHA-256 `e17fc9ba14964d143be993a6d002f80b20dd5197d45f57a06edb964290faf55d`;
[validation-result.json](long-lived-dialog-a3-storage-s1-evidence-2/validation-result.json)
SHA-256 `2b9242707c72b7675f8351e97809b29985a35bae0ba4baf31b7b930ff10b8337`.
Всего13 новых public evidence files. Source manifest содержит raw SHA/Git blobs,
version/profile data и phase-specific executed EXE hashes; paths EXE постоянные,
содержимое между builds различается и связано с соответствующим run.

Старый private100k store сохранён, диагностикой добавлены16 ordinary Event records:
logical count100064. Для одного полного radix это соответствует200127 nodes и100064
values; отдельный повтор table-count measurement не выполнялся. Прежние records
проверены полным range, ни один не удалён.
Новые малые fixtures находятся под `.tmp/a3-s1-repair-2`; DB/keys не публикуются.
Foreign `development-environment.md` сохраняет SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Baseline compatibility EXE сохраняет `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`.
Historical submitted report/evidence, review1 и accepted contracts/manifests unchanged.

Extra secret getter не добавлялся; прежний отказ не обходился. A2 OPEN:7 warnings,
strict advisory FAIL; deferred security reproducer не запускался/не разрабатывался.
S1 не принят; после одного содержательного callback root — остановка до review.
