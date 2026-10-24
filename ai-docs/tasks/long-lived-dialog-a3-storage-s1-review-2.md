# A3-S1 implementation review 2

2026-10-24. **R1/R2 закрыты. Memory amendment принято. S1 в целом ещё не принято:**
остался один final initial100k run и review итоговых результатов. Commit/push пока
не поручены. Тот же OD1 исполнитель, GPT-6.1 Sol Max/default без Fast.

## Независимая проверка

[Implementation2](long-lived-dialog-a3-storage-s1-implementation-2.md) SHA-256
`eb827bca2de7987877280fc37be74563d90ccc45cde01de789532eabd6867394`.
Root сверил все15 source SHA/Git blobs и12 evidence hashes из
[source-final](long-lived-dialog-a3-storage-s1-evidence-2/source-final.json)
`e17fc9ba14964d143be993a6d002f80b20dd5197d45f57a06edb964290faf55d` и
[validation-result](long-lived-dialog-a3-storage-s1-evidence-2/validation-result.json)
`2b9242707c72b7675f8351e97809b29985a35bae0ba4baf31b7b930ff10b8337`.
Из первой сдачи изменены только indexed_dialog.rs, credits.rs, tests/scale helpers;
добавлены memory helper и расширенные root tests, runner получил CargoFeatures.
Backend/index geometry/envelopes и owner fences прежние.

Root прочитал исправления и все5 public regressions и самостоятельно повторил их:
**5/5 PASS,14.93s**. [Новый лог](long-lived-dialog-a3-storage-s1-review-2-evidence/independent-regressions.log)
SHA-256 `e059aa468b7abaf740ceed195480c7fe199a11d5591d3f2175a7090573a603bb`.
TEMP/TMP `.tmp/a3-s1-root-review-2`, stable runner, filter `a3_s1_review_`,
CargoJobs2/BelowNormal/offline/locked. Старый0/2 baseline не изменён.
18 affected state/fault checks и Clippy/fmt подтверждены worker evidence;
это не объявляется вторым root повтором всего набора.

## R1/R2

`baseline_charge` захватывает charge committed leaf при первом изменении ключа;
новый charge считается по final overlay относительно этого baseline. Typed remove
сохраняет baseline и final positive charge0. Все generic Put предшествуют credit
settlements; `spent` сохраняет однократное распределение общего mutable checkpoint.
Не добавлены durable fields и обходы quota/immutable checks. Контрпример tight
Native1024→2048 и single2048 дают одинаковый успешный результат и15990B plan.
Расширенные grow/shrink/return-to-baseline/multi-frame/reopen/retry cases прошли.

Readable проверяет C+canonical ordinal receipt ref до ledger changes, а существование
и целостность подтверждаются обычным authenticated get/cost. Wrong ordinal возвращает
Bound; тесты подтверждают неизменные Pending/root/ledger и успешный matching control.
Opaque signature/EventId/role/current-policy validation остаётся future adapter.

## R3: решение и оставшаяся работа

Root независимо проверил memory diagnostic, числа и pinned redb source. Принято
[memory budget amendment1](long-lived-dialog-a3-memory-budget-amendment-1.md):
raw warm growth≤96MiB **и** cache-subtracted residual growth≤32MiB одновременно;
cache64MiB, app buffers192MiB, absolute384MiB, latency ratio≤4 и1200s не меняются.
Это явное уточнение контракта, а не переименование старого64MiB failure в PASS.
Final acceptance использует обе сопоставимые warm phases и измеренный cache каждой.

Поручение исполнителю: применить нормативные assertions/явный result для одного
final fresh initial100k run, не только напечатать numbers. Сначала завершить узкие
изменения helper, при необходимости проверить их малой fixture; затем ровно один
fresh heavy run `.tmp/a3-s1-repair-2/persistent-scale-initial`. Старый reusable store
не удалять. Не повторять уже принятые functional checks без затронутого кода.
Если default run не включает cache_metrics, результат этой метрики unavailable;
нормативный gate требует instrumentation и не может пропускаться с PASS.

Узкое замечание к helper: `memory_tests::memory` на non-Windows возвращает Err,
а обычные scale tests теперь вызывают `sample` без platform guard. Тем самым portable
storage checks стали бы падать из-за Windows-only diagnostics. Сделать availability
явной/ограничить только measurement, сохранив обычные functional tests; не добавлять
неподдерживаемые platform measurements с фиктивными0. Windows final gate при этом
обязателен. CI сейчас Windows; Linux validation здесь не заявляется.

Новая сдача `long-lived-dialog-a3-storage-s1-implementation-3.md` и
`long-lived-dialog-a3-storage-s1-evidence-3/`, final source manifest включая helpers,
run/source association, полный phase accounting и remaining A3 gates. Старые2 сдачи,
reviews и evidence не переписывать. После одного содержательного callback root
остановиться до итогового review; даже успешный benchmark не разрешает commit сам.

HEAD `f59d285d5704dc2c6969b6f8e416f204223e27cc`, index пуст. Foreign
`development-environment.md` независимо сохранён SHA
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Wire2/native/migration/runtime, extra secret getter и deferred A2 reproducer вне
этого поручения. A2 остаётся OPEN:7 warnings/strict gate FAIL.
