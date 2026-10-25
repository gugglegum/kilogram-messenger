# S4: учёт времени final verification set

2026-10-25, Asia/Yekaterinburg. Baseline9547ed171c64bf5ea529b4ba487acf6aa1a61b5b.
Это разъяснение уже принятого правила, **не повышение ceiling**, не приёмка кода
и не принятие ещё не завершённой P/F matrix.

Основание: [S4 contract](long-lived-dialog-a3-runtime-s4-contract.md), §11.3
lines837–857 и §12 lines888–891, SHA256
`5e21324167573281e1668cd6091c5a6e63ff72b0894009c57c0a122b6b0a688c`;
[implementation task](long-lived-dialog-a3-runtime-s4-implementation-task.md),
SHA256 `0e2bdef16c84291a20bfa5d2128a5304263bfb6f0092caea3585bd3e5d98b865`.
Оба hash root проверил; normative files не изменены.

## Решение для текущей реализации

1. **P≤2400s — сумма команд выбранного финального acceptance набора P.**
   Failed/intermediate attempts на уже заменённых candidate diffs не вычитаются
   из этого ceiling. Их имя `*-final-*` не превращает неудачную предыдущую попытку
   в принятый final result. Они остаются FAIL с прежними bytes/names/source hashes.
2. Для исправленного candidate допустим полный final P в пределах2400s **вместе
   с его собственными compile/copy/setup overhead, если они входят в runner elapsed**.
   Это не2400s controller плюс overhead сверху. Настройка внутреннего deadline
   оставляет обоснованный запас на накладные расходы именно данного прогона;
   прошлые development attempts этот запас не уменьшают.
3. Идущий P не прерывать только ради ошибочно заниженного controller limit1410s.
   Сначала сохранить его реальный outcome. Если он завершится полноценным PASS —
   не повторять без причины. Если именно локальный1410s deadline прекратит его,
   сохранить этот FAIL и причину; разрешён новый полный run с корректным watchdog
   и теми же нормативными counts/profile/caps. Не объединять незавершённый prefix
   и отдельный suffix в вымышленный whole-suite PASS, не переписывать старые logs.
4. Smoke/controller timeouts не становятся новыми требованиями контракта сами
   по себе. Их можно настроить перед новым полным run под принятый aggregate
   ceiling с явным source/command evidence. Это **не** разрешение увеличить
   нормативные TLS/admission/stream/lease deadlines, ослабить fairness/assertions,
   сократить1640 Text/3288 successful packet calls или заменить actual processes.
5. **F≤1200s и components≤600s неизменны.** В каждом final set учитывать команды,
   результаты которых реально используются для покрытия его matrix, включая
   выбранные допустимые reuse и необходимые supplements. Не выбрасывать из суммы
   длительный обязательный gate только потому, что удобнее показать меньшую сумму.
   Coverage/source correspondence reuse проверяется при implementation review.
   Compile-only/static остаются отдельно по принятому контракту.
6. Это не разрешение без объяснения повторять успешные тяжёлые suites или бесконечно
   сбрасывать часы одного незавершённого run. При превышении нормативного final
   ceiling сохраняется stop/evidence/bottleneck review. Изменение ceiling/profile/
   обязательных gates требует отдельного решения root.

## Что проверено сейчас

Root прочитал реальные result JSON двух завершённых P attempts:

| Attempt | Exit | Runner seconds |
|---|---:|---:|
| p-full-final-1 |1|585.5830845|
| p-full-final-2-batch4 |1|317.1270524|
| Сумма development P attempts | |902.7101369|

У них разные executable fingerprints. Полная корректность изменений между этими
source snapshots этим уточнением не проверялась. На момент чтения root result JSON
для `p-full-final-3-idle-read` отсутствовал: текущий P ещё не является заявленным PASS.
У full F batch4 и idle-read supplement result exit0 (438.7881169s и138.1962705s);
сообщённый исполнителем полный aggregate1175.196s/coverage пока не принят root:
полная матрица и все contributing runs будут проверены вместе с реализацией.

В implementation report нужны две видимые таблицы: final accepted-candidate suites
с counts/source hashes/суммами и отдельный development/failed attempts ledger с
реальными длительностями и причинами. Не скрывать фактическое время разработки
за более коротким final acceptance числом. Старые evidence не удалять и не править.

Root не менял product/tests/helpers и не запускал новый benchmark для этого решения.
Продолжать разрешённую S4 реализацию/проверки; commit/push только после implementation
review и отдельной явной приёмки, как прежде.
