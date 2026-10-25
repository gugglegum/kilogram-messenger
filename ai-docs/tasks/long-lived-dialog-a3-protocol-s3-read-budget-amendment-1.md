# A3-S3: ограничение числа чтений для bound2 history

2026-10-25, Asia/Yekaterinburg. Уточнение к принятому [S3 plan](long-lived-dialog-a3-protocol-s3-plan.md)
и [implementation task](long-lived-dialog-a3-protocol-s3-implementation-task.md).
Frozen plan/reviews и S2 performance evidence не изменяются.

**Root разрешает отдельный bound2 predicate `point_gets <= 40*(limit+2)+3` при
сохранении старого unbound S2 predicate `8*(limit+2)+2`.** Это предел работы одного
bounded page request, не новая оценка UX latency и не приёмка реализации целиком.

## Основание и расчёт

Исполнитель остановился перед изменением лимита после реального failure
`a3_s3_component_authority_atomic_reopen`: `Bound("history read work")`,3.63s,
[atomic-2.log](long-lived-dialog-a3-protocol-s3-evidence/atomic-2.log).
Root адресно прочитал `history_read/history_projection/history_candidate`,
`history2_source_check/resolve_auth/check_receipt/proto_binding/history2_control_count`
и фактический diff. Ни heavy tests, ни frozen DB для решения не запускались.

На момент проверки SHA256 `history.rs` —
`53f9c26b958199ed01b3c91fdcdb3ef377e50523573f0edd30af7c18868bd619`,
`history_protocol2.rs` —
`1bab7bf9c97c75659c0a38cbdd1f6a5f2bc446ebff3e3a644f0cacb05dc8750a`.

Существующий causal reader загружает не более `limit+2` candidates, одновременно
держит максимум2. Каждый candidate проверяет собственную projection и максимум2
прямых parents; parents рекурсивно не обходятся.

| Состав | Верхняя граница point reads |
|---|---:|
| Старый candidate: lane/projection/source/previous и2 parent projection/lane pairs |8|
| Добавка для каждой bound2 projection: source и static binding |2|
| Original author, F link, final author, receipt marker и wire link |5|
| Только peer Text: Admission и admission author |2|
| Receipt author, если есть wire receipt |1|
| Всего дополнительно на projection |10|
| Candidate вместе с максимум3 projections |8+3*10=38|
| Startup: identity, heads, authenticated protocol binding |3|

Консервативный потолок40 оставляет фиксированный запас2 на candidate; он не разрешает
исторические scans, дополнительные parents или неучтённые getters. При limit32 это
не более1363 logical point reads; прежний unbound bound остаётся274. При limit1 —
123 и26 соответственно. Числа являются потолками, не желательным фактическим расходом.

## Обязательные условия реализации

1. Новый predicate выбирать только для authenticated bound2 view. Budget selection
   не зависит от caller/peer hint, parseable Event2 или нового произвольного флага.
   Установка лимита не должна добавлять reads после фиксации `stats.point_gets`.
2. **Текущий черновик ещё не сохраняет старый путь:** history_view всегда вызывает
   proto_binding, а history_projection всегда читает source и снова proto_binding,
   даже для unbound S2. Поэтому одно условное увеличение predicate недостаточно.
   Исправить dispatch/reuse так, чтобы legacy read path сохранял прежний8/2 bound;
   проверка Control root count1 вместе с существующей authenticated identity может
   отделять unbound от bound2 без лишнего lookup. Для предполагаемого bound2 missing/
   malformed proof — Integrity, не downgrade в trusted mode.
3. Допустим узкий reuse уже проверенного binding/source в пределах одного Work/read
   snapshot. Не читать повторно одни и те же bytes без необходимости и не вводить
   растущий cache всей истории. Если cache используется и на mutation Work, staged
   changes/initialization не должны получать устаревший cached mode или evidence.
4. Считать все фактические logical hget/hrequired calls, включая отсутствие листа,
   parent/evidence/authority/receipt reads. Сохранить максимум2 candidates,2 wire
   parents, bounded bytes/canonical response и общий O(k*d*h+bytes), без recursive
   ancestry/whole-context traversal. Снятие проверки, reset stats и скрытый lookup
   вне счётчика не являются исправлением.
5. H2 exact Control cardinality для bound2 `2 + source_count + receipt_marker_count`
   соответствует identity/proof/F-links/wire-receipts; unbound остаётся1. Root counts
   используются без перечисления namespace. Это необходимая coherence проверка,
   но одно равенство count не заменяет exact key/value correspondence и read checks.

## Достаточное evidence в текущем S3

- Сохранить atomic-2 FAIL и прежний H2 failure. Повторить исправленный bounded
  authority/atomic/reopen fixture, затем продолжить уже согласованную S3 matrix.
- Проверить реальные unbound и bound2 page paths с limit1 и32, двумя lanes и двумя
  parents на неначальных Text; для bound2 включить O/A/F/R renewal, peer receipts,
  own delivered/undelivered cases и reopen. Показать observed point_gets,
  candidate_loads/max_candidates и elapsed; bound только как отдельный assert.
- Проверить Integrity при missing/corrupt обязательном binding/F/Auth/receipt evidence,
  включая noninitial parent. Ошибка не выдаёт частичную страницу/advanced cursor.
  Использовать уже согласованные bounded negatives, где они покрывают эти случаи.
- Выполнить relevant существующие S2 bounded read/cursor tests: unbound budget нельзя
  увеличить или менять expected counts ради прохождения S3. Heavy100k повтор не нужен.

Изменение принимается только как архитектурное уточнение ресурса; итоговые код,
точность счётчиков, режимный fence и тесты проверяются на обычном implementation
review. Старые S2 measurements не описывают latency нового bound2 пути.
