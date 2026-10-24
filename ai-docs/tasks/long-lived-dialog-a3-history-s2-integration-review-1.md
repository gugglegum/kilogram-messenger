# A3-S2 integration review 1

2026-10-24. **Общая схема поддержана; integration contract пока не принят.**
До реализации уточнить H1/H2 ниже. Это адресное закрытие writer/init boundary,
не пересмотр A3 cursor algorithm и не новый общий RFC. Product edits/commit/push
пока не поручаются.

Проверена [первая сдача](long-lived-dialog-a3-history-s2-integration-plan.md), SHA-256
`0e955f3de2af71aa4411ac2d7d36b16076313dd805d741ab06ab13ab1e1f0bcf`,
Git blob`15d9cd7e5b9b1794b94c4f61576f389613d1d2e7`.
Baseline HEAD `4d1d8b191f6e84c812c10ed3a2fa17a8dfa03629`.
Root повторно сверил73 frozen S1 file hashes (кроме намеренно обновлённых S2
README/coordination), manifest self и foreign hash; index пуст, product drift нет.

## Поддержанные решения

- Immutable persisted scope/view + atomic fixed heads, source-linked typed binding,
  отдельный trusted adapter DTO без объявления его cryptographic proof.
- Source/Projection/Lane/heads в одном S1 engine, сохранение credit/poison/retry,
  snapshot F без generation/expiry/MVCC pin и scope из authenticated store.
- Два candidates, минимальный eligible EventId, implicit same-lane order и сохранённые
  signed Text parents. OD1 ingress уже проверяет kind3 для user parents; genesis
  отдельно от пользовательской истории соответствует существующему поведению.
- Internal cursor KDF/MAC без raw-key API. Pinned blake3 1.8.7 Hash equality
  действительно constant-time (`src/lib.rs:350–362`). Direct workspace base64
  допустим, если lock/version не меняются; private bounded CBOR encoder не требует
  dependency на native/полный protocol только ради serialization.

Независимая арифметика: identity177B, heads118B, binding≤255B, cursor275B и367
Base64URL chars. Max CBOR Row4170B/Page133775B; JSON Row24776B/Page793405B с4096
NUL bytes и u64 maxima. Числа совпали. Это арифметика, не выполненные codec tests;
actual encodings и candidate/get bounds остаются implementation gates.

## H1 [P1]: закрыть соответствие credit transitions и readable batch в обе стороны

План (§1/§2, строки25–27,64–81 первой сдачи) разрешает non-history operation с пустым
batch и требует matching Materialize/Readable **для каждого Text**. Обратное правило
пока не определено: какая проверка запрещает отдельный Materialize/Readable среди
credits при пустом batch, лишний consume второй queue или действие другого C.
`effects/credits` остаются caller-provided, поэтому одной проверки Text→action мало.

Контрпример на имеющейся семантике S1 (это анализ предлагаемого пути, не запуск
ещё не существующего S2 кода):

```text
В H2 уже есть readable A1, text="hello", request=R1, F_A=2.
В Queue с enqueue_seq=q2 помещён тот же text, но другой request=R2.
commit_history(batch=[], credits=[Materialize(queue=q2, request=R2||zero16,
    ordinal=2, event=A1, refs=[Event(A1),Projection(A1)])]).
```

S1 `credits.rs:561–600` проверяет queue ID/text и existence retained refs, но не
является H2 writer: существующий Event/Projection подходит. Без обратной H2 проверки
новая accepted Queue исчезает, появляется debt ordinal2 для старого A1, при этом
Lane2/новый request-bound Event не создаются и F не двигается. Будущий adapter не
должен быть единственной защитой от такой ошибки typed storage facade.

Нужно определить закрытую transition matrix/проверку correspondence:

1. Каждый consuming Materialize/Readable соответствует ровно одному подходящему
   row batch и наоборот; exact matches context/own-or-peer role/ordinal/EventId/
   logical request/queue-or-pending identity. Лишние/повторные consuming actions,
   один ticket для двух rows и один row для двух tickets запрещены. Правила exact
   retry/уже existing row описать отдельно, не replay Native.
2. Empty/non-history batch имеет конкретный allowlist credit actions и effects;
   consuming actions не становятся доступными просто через этот purpose. Указать
   когда Native/checkpoint mutation допустима без нового row, и где лежит проверка
   связи native state с frame. Непроверяемый opaque native content честно остаётся
   adapter responsibility, но разрешённые storage transitions должны быть закрыты.
3. Один commit_history scope либо explicitly validated multi-scope plan. Для
   простой схемы все effects/credits/refs привязать к выбранному C; global Author
   row сохраняет owner-C binding. Нельзя проводить изменение bound C2 через purpose
   C1, обходя его writer fence. Generic fence проверяет все реально затронутые C,
   включая credit-only operations, а не только наличие ordinary Put.

Добавить negative contract vectors: trace выше, лишний consume при валидном row,
cross-C effect/credit, двойное consumption и отказ без изменения root/queue/head/ledger.
Реализация и tests этих vectors будут после приёмки контракта.

## H2 [P2]: initial view не должна усыновлять накопленный session state

Первая сдача требует пустых Event/Projection/LaneA/LaneB. Но S1 разрешает durable
Queue/Pending/Undelivered/Native/Seen/Request до появления этих четырёх namespaces.
Например, unbound C с accepted Pending ciphertext и checkpoint, но без readable
Event, удовлетворяет указанному empty predicate. Затем initialize_history может
присвоить ему новые D/E/SID. Это переход существующего состояния, для которого
обычная fresh initializer не имеет migration/provenance проверки.

Определить точный authenticated fresh predicate и допустимый bootstrap namespace/key
набор. Отсутствие visible history недостаточно: ordinary initializer не переносит
accepted obligations, checkpoint, replay/author/request/domain state в новый view.
Metadata, действительно допустимые до init, перечислить и обосновать; не добавлять
O(N) scans, использовать fixed namespace summaries/точечные known keys. Если
нужна будущая explicit migration, оставить её отдельным purpose вне S2.

Для AlreadyInitialized определить проверку identity **и** coherent heads/tails;
identity-only/head-only/corrupt expected record не означает успешный repeat/fresh
init и не исправляется созданием нового view. Добавить state table/cases: полностью
fresh, допустимый bootstrap, pending-only, queued-only, native-only, прежняя typed
identity с нормальными/повреждёнными heads, другая identity. Отказ сохраняет всё.

## Следующая сдача

Обновить только unaccepted integration plan H1/H2, сохраняя выбранные codecs/cursor/
response bounds если эти уточнения их не меняют. Если меняют — явно пересчитать
затронутые числа, без несвязанного redesign. Старый SHA и контрпримеры сохранены здесь;
review и frozen S1 artifacts не переписывать. Read-only source checks достаточны,
новые100k/native/security runs на этой contract-only итерации не нужны.

Один substantive callback root с новым path/hash и точным disposition H1/H2, затем
остановка до review. GPT-6.1 Sol Max/default без Fast; новые чаты/агенты не нужны.
Product code/tests/fixtures, commit/push — после отдельной приёмки. Foreign
development-environment.md SHA330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798
сохранён. A2 OPEN7 warnings/strict gate FAIL; deferred reproducer и extra secret
getter вне scope. Это review proposed contract, не заявление о production exploit.
