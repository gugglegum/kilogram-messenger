# A3-S2: бюджет полного history benchmark

2026-10-24. Root принимает узкое изменение: **один следующий final S2 persistent
1k/10k/100k history scenario получает ceiling 2400 s вместо 1200 s**. Это разрешение
на измерение после анализа bottleneck; S2 и A3 ещё не приняты.

Основание — [repair2](long-lived-dialog-a3-history-s2-implementation-2.md) и
[root review2](long-lived-dialog-a3-history-s2-implementation-review-2.md).
В [A3 contract §9](long-lived-dialog-a3-contract.md) временные ceilings заданы для
остановки/анализа bottleneck. Остановка на прежнем пороге соблюдена в обеих итерациях.
Root принимает решение в рамках разрешённых пользователем contract/implementation
review и циклов исправления A3; исполнитель не меняет бюджет самостоятельно.

## Обоснование

Измеренная ограниченная оптимизация сократила unprofiled small scenario
399.617 → 188.486 s, 10k range 132.041 → 44.773 s и fill 8984 rows
161.564 → 98.844 s. На одинаковых копиях выделены повторные node/value AEAD/proof
операции; integrity/credit проверки сохранены, кэши ограничены одной транзакцией.

Линейный прогноз полного сценария по новым small timings — 1851.9 s, около 31 min.
Это оценка, а не верхняя граница или результат 100k. Бюджет 2400 s даёт около 30%
запаса на увеличение глубины дерева и прочие фазы. Следующий полезный результат —
реальные 100k memory/latency/consistency measurements. Дополнительная оптимизация
до этого результата не поручается. Ordinary append не ускорился; это явно отражено
в review и остаётся предметом нормативного ratio gate.

## Точная граница изменения

- Только ceiling общего S2 scenario: create/setup, последовательные 1k/10k/100k,
  fill/read/point/ordinary append/cursor resume/reopen. Записать и scenario timer,
  и harness elapsed; отдельно обозначить границу последнего close/drop.
- Один общий test-only constant 2400 s для runtime budget checks и итогового
  `limits` assertion. Значение вывести в новый JSON/log. Никакого произвольного
  env override или автоматического продления. S1 и другие A3 ceilings не менять.
- Verification profile/debug assertions, payload/distribution, private batch128
  с настоящими validators/credits, public batch≤4 и реальные durability/sync прежние.
- Все остальные требования сохраняются: process≤384MiB, app≤192MiB, cache≤64MiB;
  warm 100k−1k raw growth≤96MiB **и** residual growth≤32MiB в обеих фазах;
  page/ordinary append p95 ratios≤4; actual plan≤16MiB, page32 gets≤274,
  candidates≤2, exact streamed domain, old cursors/restart/C2 isolation.
- Native E2E=0. Этот fixture не доказывает native/wire/runtime readiness.

Старые contract/task/reviews/reports/raw evidence не переписывать. Исторический
1200 s precheck остаётся STOP/UNAVAILABLE. Данный amendment имеет приоритет только
для указанного следующего S2 run; он не превращает прежние результаты в PASS.

## Один следующий запуск

После узкой test-harness правки собрать locked/offline через stable runner и
**до** запуска зафиксировать final source inputs и SHA stable EXE. Сохранить прежние
fixtures; новый full store создать в отдельном постоянном пути. Выполнить один
полный unprofiled run, без отдельного повторного 1k/10k разогрева: эти стадии входят
в full scenario. При ceiling, integrity/resource failure или missing metric —
сохранить фактическое состояние/логи/fixture, сообщить root и остановиться.
Повторный heavy run или новое расширение бюджета автоматически не разрешены.

S2 publication возможна только после отдельной приёмки полного результата root.
