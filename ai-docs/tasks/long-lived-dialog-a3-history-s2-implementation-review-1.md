# A3-S2 implementation review 1

2026-10-24. **Partial result проверен; S2 пока не принят.** Следующая итерация —
адресная диагностика/оптимизация и завершение resource gate, без смены архитектуры,
verification profile или лимитов. Product commit/push пока не поручается.

Вход: [implementation report](long-lived-dialog-a3-history-s2-implementation.md),
SHA-256 `e083c14d696da435e95b541ad5f11b5229f2f622a74f822da964c23bf097d9a0`,
Git blob `4cf008ad9237b9575e92f54a7e4a9ac7ef00a642`.
Baseline HEAD `61524df3a77203d73673f47d38decf6b817615e3`, executor turn
`01a16dd0-f077-7c82-9609-132c920aaf6c` completed. Root сверил 14 build input hashes/
bytes/blobs, 19 evidence hashes/bytes, report и stable library EXE
`ff42d3932ac86816e6e29ff221fc3d8b0ed03c4a574a871a327c6fd4a3f34835`.

## Проверка кода и независимые результаты

Прочитаны common engine changes, typed input/binding/request codecs, writer lowering/
private correspondence, fresh/repeat, cursor/MAC, candidate reader и scale harness.
Проверены ключевые H1/H2, FIFO, mixed batch, cross-C/global owner, fault и integrity
tests. Закрытые purposes действительно не выставляют raw credits/refs/digest наружу;
проверка нового row и consuming transition выполняется в обе стороны. Существующие
S1 R1/R2 механизмы сохранены. Trusted DTO/native/receipt marker остаются заявленными
storage claims, не доказательством будущего protocol/native verification.

Новых воспроизведённых функциональных дефектов эта итерация не выявила. Это review
текущего S2 scope, не приёмка ещё отсутствующего wire/native/runtime слоя.

- Root повторил S2 small через stable runner: **14/14 PASS, 7.28 s**. Scale tests
  явно исключены; unchanged S1 16+5 приняты по совпавшему evidence исполнителя.
- Добавлен независимый public-API regression
  `crates/kilogram-state/tests/a3_s2_root_review.rs`: four-row mixed batch, signed
  cross-lane chain, cursor и finished cursor сериализуются, затем добавляется новый
  Text, store закрывается и отдельный owned OS process открывает тот же профиль.
  Child проверяет exact old suffix, пустой старый конец, новую first с новым Text,
  UTF8/NUL, cross-C cursor rejection и сохранность credits/Native/C2 Queue.
- Новый процесс **PASS**: parent 0.89 s, child 0.04 s, PID 43224. Child запускается
  из stable executable; ignored child entry не считается отдельным standalone PASS.
  Это закрывает отмеченный исполнителем пробел same-process drop/open проверки.
- Targeted strict Clippy нового root test, rustfmt и tracked whitespace PASS.
  Первичный root-test compile failure E0618 (имя локальной переменной перекрыло helper)
  сохранён; после переименования helper тест пересобран и выполнен. Product code root
  не изменял; original executor evidence не переписывались.

Raw root evidence: [small-independent.log](long-lived-dialog-a3-history-s2-review-evidence-1/small-independent.log),
[process-cursor-1.log](long-lived-dialog-a3-history-s2-review-evidence-1/process-cursor-1.log),
[process-cursor-2.log](long-lived-dialog-a3-history-s2-review-evidence-1/process-cursor-2.log),
[root-test-clippy.log](long-lived-dialog-a3-history-s2-review-evidence-1/root-test-clippy.log).
Сводная фиксация: [review-verification.json](long-lived-dialog-a3-history-s2-review-evidence-1/review-verification.json).

## R1: performance precheck не разрешает закрыть A3-4/A3-5

На 10k реальная range traversal заняла 132.041 s, fill следующих 8984 rows —
161.564 s, resume от midpoint — 66.329 s. Линейные оценки 100k range 1320.4 s и fill
1798.4 s по отдельности превышают общий ceiling 1200 s. Это прогнозы, не результаты
100k. Остановка перед дорогим build соответствует принятому контракту. Memory/ratio
gate 100k остаётся UNAVAILABLE, small status не заменяет его.

Нужна одна ограниченная итерация диагностики и ремонта:

1. Сначала отделить затраты page/point от fixture oracle/encoding, а fill — от
   commit/sync и validation/crypto. На сохранённой 10k fixture измерить bounded samples
   и counters: повторные point/value loads, node decrypt/proof work, namespace/value
   bytes, время выбранных hot sections. Профилирование не должно регенерировать 100k
   или выдавать instrumented numbers за нормативные timings без пометки.
2. Код действительно повторно расшифровывает values в `Work::get`, а candidate loader
   очищает node proofs перед каждым candidate. Это объясняет направление поиска, но
   **не является измеренным CPU profile**. Проверить также стоимость tree prefix/common
   и typed decode только если counters/profile указывают на них. Не внедрять несколько
   больших оптимизаций вслепую и не добавлять новую query/cache platform.
3. Разрешена bounded verified-value/typed reuse внутри одного Work/read transaction,
   если она даёт измеренный выигрыш. Указать byte/entry caps и lifetime; positive и
   negative entries корректно invalidated/updated при put/remove, mutable Native/
   Current/Request и global Author aliases. Scope/incarnation/owner и authenticated
   proof/value version нельзя терять. Cache не переживает transaction, не удерживает
   MVCC snapshot и не растёт с полной историей. Обновить conservative buffer bound.
4. Сохранить H1/H2, R1/R2 charge/spent и actual credit transitions. Проверки source/
   Core/parents/AEAD нельзя выключать ради скорости. Нужны адресные stale-cache,
   eviction/bounds, cross-C/alias и corruption-on-access regressions; byte-level tree
   primitive changes, если понадобятся, проверяются по полному диапазону границ.
5. Сравнить до/после на том же verification/cache64 setup. Release/profile switch,
   искусственный warming/trimming, повышение limits и изменение распределения/размера
   fixture ради PASS не поручаются. Если измеренного выигрыша недостаточно — честный
   узкий report, а не автоматическое расширение бюджета.

## R2: reuse fixtures и завершение нормативных измерений

`history_scale::measure` сейчас отказывается открывать существующую fixture; отдельного
bounded replay/reuse режима нет. Для следующей итерации добавить private test-only
reuse/profiling path с явной проверкой scope/counts/fixture provenance. Это нужно для
измерений на сохранённых данных без многократного fill. Root API/importer не расширять.

Оригинальную `.tmp/a3-s2/history-small` с 10016 Text и DB SHA
`df9d8293753daff382603baa2dcc4670705407e29155325007577c10644b0841` сохранить. Read samples
можно брать из неё; если нужны writes для сравнения, использовать явно обозначенную
изолированную synthetic copy или маленькую отдельную fixture, без изменения исходного
evidence. Исправить название нового fill counter: separate fill, ordinary и cumulative
transactions; старые raw logs/JSON не переписывать.

После малых post-repair timings, подтверждающих запас на полный сценарий, выполнить
один новый persistent 1k/10k/100k history build через тот же validator/ledger. Измерить
fill, pages, point, ordinary appends, old-snapshot resume/reopen и реальные I/O/cache/
memory phases. Требования прежние: runtime batch≤4, actual plan≤16MiB, candidates≤2,
gets≤274/page32, process≤384MiB, app≤192MiB, cache64MiB, warm raw≤96MiB **и** residual
≤32MiB в обеих фазах, page/append ratios≤4, общий ceiling1200s. Ничего не объявлять PASS
при missing metrics. Сохранить полный итоговый fixture для дальнейшего reuse.

## Следующая сдача

Тот же OD1 executor, GPT-6.1 Sol Max/default без Fast, без новых чатов/агентов.
Сохранить новый root process test и повторить его после затрагивающих read/cache правок.
Новый report/evidence с суффиксом `-2`, исходные report/quality/EXE provenance и root
review evidence не изменять. Подготовить точные source/build hashes и before/after
measurements, затем один содержательный callback и остановка до root review.

Foreign override действует: development-environment.md clean SHA53074ff…; старый
dirty блок330ad3… не восстанавливать. L1 EXE, S1 fixtures и frozen contract evidence
сохраняются. Native E2E=0. Wire2/native continuation/Root2/migration/runtime/UI/A2
deferred work вне этой итерации. Commit/push — только после отдельной приёмки S2.
