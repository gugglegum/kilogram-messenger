# A3-S2: final verification attempt 3 и matched profile probe

2026-10-24. **Full scenario FAIL по ceiling2400s. S2 не принят.** Один fresh run
записал100k Text, проверил полный range и16 ordinary appends, затем остановился
при entry/continuation старого midpoint resume. Обе warm memory-фазы измерены и
укладываются в bounds;100k page/append p95 ratios и полный suffix не подтверждены.
После остановки выполнена ровно одна разрешённая bounded verification/release pair.
Release page API в ней быстрее примерно13.6×, point25.6×; это диагностика, не замена gate.

Основание: [review2](long-lived-dialog-a3-history-s2-implementation-review-2.md),
SHA15f705e53f54714f6cade494f04897d9719ebbc8f87d110e9af8fb50e185f6b4,
и [time amendment1](long-lived-dialog-a3-history-s2-time-budget-amendment-1.md),
SHA5f9898a928b02c551be858483f5e1b043d3a2c4f95b14b711c6766f6a58415a5.
Baseline HEAD61524df3a77203d73673f47d38decf6b817615e3, index пуст. Исполнитель OD1,
turn01a16e8b-5b2f-74c3-9229-a8d3ad151ea3, GPT-6.1 Sol/max, config default/no Fast.
Native E2E=0. Product commit/push не выполнены.

## Изменение harness и provenance до запуска

Из19 repair2 inputs18 byte-identical. Изменён только test-only `history_scale.rs`:
единый `SCENARIO_BUDGET_SECONDS=2400.0` для runtime checks/final assertion,
явный budget в JSON, phase timers и граница final close. Product/helper algorithms,
verification/debug profile, cache64MiB, durability, payload/distribution и остальные
ceilings/limits не менялись. [Узкий patch](long-lived-dialog-a3-history-s2-evidence-3/harness-change-3.patch).
Старые1200s prechecks и timing EXE uncertainty из report2 не переписывались.

Locked/offline strict Clippy all state targets/cache_metrics PASS,3.20s, Jobs2:
он завершился до пользовательского steer по CPU. Оставшаяся сборка/stable runners
использовали Jobs12 на24 logical CPU, BelowNormal. Normative harness — test-threads1,
один процесс/PID29124, один store с последовательными1k/10k/100k. Во время него
других наших тяжёлых compile/tests не было. Повторных small/crash suites не запускали.

**До измерения** stable runner собрал verification `--locked --no-run`,6.21s.
Зафиксированы19 source inputs/bytes/SHA/blobs/snapshots и3 EXE:
[before-run provenance](long-lived-dialog-a3-history-s2-evidence-3/build-provenance-before-run-3.json),
SHA b7261dea51b900b4edea85b530d45dd9ae8223dde7149e2b4e1b0557a9f567c1.
Library EXE8828416B, SHA
7ce75659705024f399722f959394ab1f6cd27d350296f450315bd03dd103f0b6,
сохранён в `.tmp/a3-s2/final-3/kilogram-state-lib-kilogram_state-tests.exe`.
Перед run и после него hashes inputs/EXE совпали. Все исполнения — только через
`scripts/run-cargo-tests-stable.ps1`, постоянные paths, без прямого Cargo test run.

## Единственный нормативный запуск

Новая `.tmp/a3-s2/history-full-3`, без adoption/regeneration. Условия прежние:
source/text1024B, Native256B,2 lanes/до2 parents, own1/16; private batch128 использует
настоящие validators/credits, public batch≤4. Streamed oracle, без N-sized expected vector.
Raw [full-history-3.log](long-lived-dialog-a3-history-s2-evidence-3/full-history-3.log),
[parsed partial result](long-lived-dialog-a3-history-s2-evidence-3/full-history-partial-3.json).

| Время/статус | Факт |
|---|---:|
|Ceiling event, scenario clock|2400.1566769s /2400s: FAIL|
|Rust harness elapsed|2400.18s: FAILED, exit101|
|Stable runner wall|2402.542294s, включая compile-no-run/copy/error handling|
|Final100k stage summary /SCALE_FINAL|Не достигнуты|

Normal success timer включает явный последний `drop(store)` и исключает encoding
финального summary. При раннем `budget` error этот normal-close блок не выполняется:
event2400.1566769s — **до return/automatic drop**, harness elapsed включает teardown.
Отдельная duration последнего automatic close **UNAVAILABLE**; разность двух часов
не выдаётся за её точное измерение. [Runner wall](long-lived-dialog-a3-history-s2-evidence-3/runner-wall-3.json).

Полностью завершённые стадии этого full run:

| Метрика |1k|10k|
|---|---:|---:|
|Stage elapsed s|17.141|180.908|
|Fill s|7.771|102.031|
|Full range s, API+oracle/loop|3.884|48.872|
|Point phase ms|72.384|93.119|
|Ordinary phase s,16 appends|0.745|0.941|
|Page32 p95 ms|123.802|173.365|
|Ordinary append p95 ms|51.396|61.715|
|Point p95 ms|12.266|15.442|
|Midpoint resume s|1.913|24.733|
|Cold /resume reopen ms|89.926/103.413|831.437/933.128|
|Fill transactions /ordinary transactions|71/32|632/32|

Их C2/checkpoints/exact domain/restart checks PASS. Counters separate: cumulative
fill703, ordinary64, measured767 после10k, setup excluded. Это не counters final100k.
Для10k actual fill backend reads16965632B/writes855270912B, range reads28340224B,
append reads32768B/writes9500672B. Для1k — fill writes31815872B, range reads0,
append writes6313984B. Нулевой backend read не означает cold-I/O proof: debug redb open
прогревает cache. Полные I/O snapshots этих стадий сохранены в parsed JSON.

100k fill завершился на общем clock1718.887468s. Последующие markers
`after_warm_range` records100000 и `after_ordinary_append` records100016 доказывают
завершение exact streamed full range и всех16 appends. Между verified8192 и98304
пройдено90112 rows за585.028634s: это измеренный range slice **с oracle/loop/logging**,
не отдельный pure API timer и не полная duration100k range.

После `after_ordinary_append` единственный call site `budget(overall)` находится в
midpoint suffix loop. Поэтому по control flow до clock error выполнены новая first,
last point, empty old finished cursor, same-process drop/open и3 old-snapshot
checkpoint pages. Это inference из pinned кода+порядка markers, не независимый
OS-process test. Сколько suffix pages успело завершиться, не логируется. Полный
suffix и завершающий100k C2 assertion не достигнуты/не подтверждены.

Точные local100k fill/range/point/ordinary/resume timers, p95 и I/O summary остались
до end-of-stage JSON и **не были выведены** при early return. Они UNAVAILABLE;
приближённые интервалы не подставляются вместо них. Причина остановки — time bound,
новый integrity/credit defect не воспроизведён. Второго heavy run, расширения бюджета
или нового performance repair после этого не выполнялось.

## Resource gates и сохранённая fixture

Все memory samples — PID29124, real redb cache metrics доступны. Значения роста:

| Фаза100k−1k |Raw growth B, limit100663296|Residual growth B, limit33554432|Запас residual B|
|---|---:|---:|---:|
|after_warm_range|92053504 PASS|32088384 PASS|1466048|
|after_ordinary_append|92389376 PASS|33308672 PASS|245760|

Residual — разность `(private−cache)` между размерами. После append запас всего
240KiB; один такой результат не доказывает устойчивый большой запас. Observed process
peak paged107732992B/private max104108032B, app bound31604366B, cache≤67108864B:
observed absolute bounds384/192/64MiB соблюдены. Max cumulative bulk plan1943944B
при16MiB limit. Completed page APIs проверяют guards gets≤274/candidates≤2; actual
maximum100k и ordinary plan statistics не напечатаны в отсутствующем final summary.

| Gate | Статус |
|---|---|
|Whole2400s|FAIL|
|Обе warm raw/residual comparisons|PASS по записанным samples|
|Observed absolute process/app/cache/bulk plan|PASS по записанным samples|
|100k/1k page/ordinary append p95 ratios≤4|UNAVAILABLE|
|Полный old suffix +final100k C2|Не подтверждены|
|Итоговый S2 resource gate|**Не пройден**|

Закрытая fixture сохранена,100016 user Text, DB1182797824B, SHA
bc3410a155a55b115f458f541b2fbd3ee6f9900ee33b34ccffd24358d39dbafb.
После run она только хэшировалась, заново как DB не открывалась. Ключ/прочие store
files остались рядом для future reuse. [Post-full preservation](long-lived-dialog-a3-history-s2-evidence-3/post-full-preservation-3.json).

## Почему медленно: один разрешённый matched probe

31min был прогнозом **всего scenario**, включая fill/reopen/ordinary/resume, а не
временем одной дешифровки100k texts. Этот run не завершился даже за40min. Storage reader
на page32 делает до274 logical point gets; каждый get включает authenticated tree
work. Это больше одной AEAD операции на сообщение. Cargo явно сообщает verification
`unoptimized + debuginfo`; optimized release измерен отдельно после остановки.

Две новые isolated synthetic copies `.tmp/a3-s2/probe-verification-3` и
`.tmp/a3-s2/probe-release-3` взяты из сохранённой `.tmp/a3-s2/history-small-2`:
initial DB SHA7c613e31c74cb78733e5d459161bf83d67fbbc8096d94251c9b53c9df2e01814,
10016 rows. Original DB не открывалась/не менялась. Existing private helper проверил
provenance/hash/existing owner/scope/count до использования. Ровно12 одинаковых cuts
с12 page32 +12 point calls на профиль; никакого нового fill/full traversal/benchmark loop.

Source/EXE hashes зафиксированы **до каждого** probe; после совпали. Verification EXE
тот же7ce75659…; release4757504B, SHA
022a782ab50f10e12fbc1dfdf7a18394f54f617bb8f335817d05db2cbd2d7d75,
снимок в `.tmp/a3-s2/probe-bin-release-3/`. Full-run EXE snapshot не переписан.
Release compile21.82s, Cargo `[optimized]`, locked/offline/Jobs12/BelowNormal.
Rust1.98.0(88d9e12ae), Cargo1.98.0(797e8a9bc), x86_64-pc-windows-msvc.

| Instrumented probe |Verification|Release|
|---|---:|---:|
|12 page APIs total ms|1698.254|124.942|
|Mean page API ms|141.521|10.412|
|12 point APIs total ms|146.281|5.723|
|Mean point API ms|12.190|0.477|
|Node open calls /inclusive ms|10746/1088.276|10746/22.724|
|Value open calls /inclusive ms|1399/248.622|1399/3.553|
|Source check calls|420|420|
|Oracle ms, отдельно/исключён из API|48.553|3.729|
|Backend reads B|5038080|21540864|

Mean page speedup13.592×, point25.560×. Все phase call counts, namespace
get/open/cipher-byte vectors,2011 cache hits и24 Work совпали; max gets274,
candidates2 в обоих. Node/value open sections быстрее примерно47.9×/70.0×.
Эти measured sections подтверждают значимую цену unoptimized storage primitives;
выключение наших integrity/source checks ради выигрыша не выполнялось.

Compile/DB hash/open/coherence/cursor seeding исключены из page timers. API собственные
CBOR/JSON guards включены; внешнее encoding и oracle измерены отдельно. Sections
inclusive и не суммируются. Probe instrumented, одна pair, не unprofiled UX benchmark.
Backend reads различаются: redb4.2.0 в debug при clean open обходит страницы для
allocator bookkeeping; release этот debug traversal не делает. Это меняет начальное
состояние cache. Проверено по локальному primary source db.rs972–1014/1172–1174 и
page_manager.rs506–513; [source hashes/context](long-lived-dialog-a3-history-s2-evidence-3/profile-cache-context-3.json).
Pair поэтому не выдаётся за CPU-only microbenchmark или полное устранение всех
bottlenecks. Release library имеет дополнительный cfg(not(debug_assertions)) S1 test;
он **не выбран** и не исполнялся, обе диагностики выполнили только один read helper.

Raw [verification](long-lived-dialog-a3-history-s2-evidence-3/probe-verification-3.log),
[release](long-lived-dialog-a3-history-s2-evidence-3/probe-release-3.log),
[comparison](long-lived-dialog-a3-history-s2-evidence-3/matched-probe-comparison-3.json),
[verification pin](long-lived-dialog-a3-history-s2-evidence-3/probe-verification-before-3.json),
[release pin](long-lived-dialog-a3-history-s2-evidence-3/probe-release-before-3.json).
Нельзя умножать результаты12 samples на всю историю и объявлять готовую production
latency. Relative ratio≤4 сам по себе не гарантирует приемлемую пользовательскую
задержку. Реальные opening-chat/page latency, cold open, native/wire/runtime/UI и
пользовательские absolute latency criteria остаются будущими проверками.

## Сохранность и точка остановки

[Preservation3](long-lived-dialog-a3-history-s2-evidence-3/preservation-final-3.json):
111 frozen evidence2 files,98 ранее pinned artifacts и3 root2 raw logs совпали;
review2/amendment1/root verification2 SHA сохранены. L1 EXE, S1 DB, обе прежние S2
fixtures, source/EXE snapshots, reports и прежняя timing uncertainty не переписаны.
development-environment.md clean SHA53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f.
Index пуст, HEAD61524df…; product algorithms после repair2 не менялись.

Commands/provenance/format checks и финальный inventory находятся в evidence3.
Strict Clippy/locked compile/fmt/whitespace PASS; прежние functional/crash suites
приняты по root review2 и здесь заново не выполнялись. **Один содержательный callback
root и остановка до review.** Нового ремонта/бюджета, commit/push, wire/native/Root2/
migration/runtime/UI/A2 deferred работ эта итерация не начинает.
