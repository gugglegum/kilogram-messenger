# S5 decision 1: честная provenance уже readable V4 истории

2026-10-25, root architecture decision for the S5 contract. Baseline
`0c1baa887008f784a86fd7ebabdcd5145fb4d1c0`. Это принятие направления проектирования;
полный S5 contract и implementation ещё не приняты, product edits не разрешены.

## Подтверждённый исходный факт

Root независимо прочитал текущий OD1 код:

- [ingress.rs](../../apps/kilogram-cli/src/ordered_dialog/ingress.rs):205–220
  сохраняет original/admission refs в pending-state.
- Там же317–339: после actual Native decrypt/inner validation фиксируются event,
  projection, credential bundle текущего peer и signed receipt/ACK intent, затем
  pending-state удаляется. Результат add_auth(final peer) не связывается с event.
- [outbox.rs](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs):86–104
  projection содержит C/D/E/ID/Text/request, без A/F; make_receipt:267–284 сохраняет
  R в подписанном Receipt1. Event1 содержит O.
- [engine.rs](../../apps/kilogram-cli/src/ordered_dialog/engine.rs):356–380,
  412–443 и [Head](../../apps/kilogram-cli/src/ordered_dialog/state.rs):13–26
  не создают отдельный immutable per-event A/F ledger. Auth catalog доказывает
  наличие credential bytes, а не их роль в конкретном историческом переходе.
- Новый [H2 reader](../../crates/kilogram-state/src/vault/indexed_dialog/history_protocol2.rs):763–790
  требует отдельные final/admission links. Нельзя выдать V4 rows за уже имеющийся
  полный H2 proof. Таблица O/A/F/R в S3 plan описывает более сильный новый контракт.

Это не разрешает считать все legacy refs отсутствующими. Сохранённые exact links
должны сохраняться; pending A отличается от уже удалённого readable admission.
Нет достоверного способа выбирать потерянный A/F по current pins, похожей дате,
единственному оставшемуся bundle или совпадению сертификата после renewal.

## Выбранное направление

Поддержать отдельный **migration-only LegacySnapshot provenance** для допустимых
уже committed/readable V4 записей, входящих в проверенный source snapshot до fence.
Сохранить пользовательскую историю и имеющиеся факты без фабрикации недостающих
исторических ссылок. Отказ от всех таких V4 origins только из-за не записанного
старым кодом A/F не является рекомендуемым продуктовым решением.

Это явное уточнение предпосылки A3/S3 о полноте старых captures, а не ослабление
новых live O/A/F/R checks. S5 contract должен содержать формальное различие и
соответствующую version/purpose/read/write матрицу; нельзя скрыть его в реализации.

## Обязательные границы

1. Для каждой роли различать Recorded(ref), NotApplicable и UnrecordedLegacy.
   Own source не имеет peer Admission по определению; отсутствующий opposite
   receipt у ещё undelivered own Text не равен потерянному обязательному receipt.
   Нужна таблица для own/peer Text, genesis, pending и already-delivered source.
   Не утверждать наличие R там, где его нет. Полный сохранившийся O/R/A/F валидировать
   и сохранять, а не заменять Unknown ради удобства.
2. LegacySnapshot привязывается к immutable migration incarnation/source manifest
   hash+generation и проверенным старым index/Head/Native/immutable source/projection
   facts, C/role/ordinal/EventId/request. Только membership в этом frozen inventory
   даёт право на импорт; одного наличия plaintext или валидной подписи недостаточно.
   Это local authenticated migration evidence, не восстановленный historical A/F
   certificate и не доказательство новых policy checks.
3. Конкретный typed migration writer создаёт такие записи только в своём copy/
   verification lifecycle. После Ready этот путь закрыт. Нельзя создать/продлить
   LegacySnapshot обычным history/runtime/generic writer, IPC, peer input или JSON.
   Повреждённая full-proof запись не должна автоматически попадать в legacy fallback.
   Новые profiles/events не могут выбрать legacy provenance флагом или null refs.
4. History/point/cursor reader явно различает provenance variants и честно проверяет
   каждую по её собственному доказательству. Одинаковые Text/IDs/requests/cursor
   semantics сохраняются. Не добавлять fabricated Admission/Final records ради
   удовлетворения прежнего H2 reader. Нужен bounded путь без per-read full V4 scan.
   Различие должно оставаться доступным для внутреннего audit/status API, без
   заявления о полном историческом H2 proof.
5. Legacy provenance не даёт current permission, admission/lease, Native restore,
   handoff readiness/cut, нового ACK или Delivered. Эти решения сохраняют отдельные
   проверки. Миграция не делает повторный decrypt/re-encrypt старых readable Text,
   не создаёт receipt из projection и не меняет старые receipt signatures/IDs.
   Signed receipt при его наличии проверяется со своим R/opposite role/event scope.
6. Missing A/F — не разрешение игнорировать отсутствующий обязательный O bundle,
   source/ID/signature, Head/Native/Seen/ordinal mismatch, corrupted AEAD/index,
   чужую projection, fork или потерянный обязательный peer-readable receipt.
   Такие несогласованности дают explicit blocked/unsupported результат с сохранением
   исходных данных. Правила полного против частичного origin указать по категориям.
7. Pending V4 с сохранёнными O/A переносится с точными refs. Не превращать pending
   в readable LegacySnapshot и не терять admission при resume. Новая readable/native
   transition после upgrade фиксирует действительные F/R по current policy.
   Любой новый indexed L1/mixed/L2 переход с момента Ready сохраняет полный набор
   применимых captures; Auth catalog bytes без per-event links недостаточны.
8. Exact old retry и late old ACK могут ссылаться на уже импортированный source,
   но не повышают его исторический уровень доказательств и не переопределяют refs.
   Known old credentials/catalog bytes сохраняются в source inventory; отсутствие
   ссылки на A/F не даёт оснований произвольно удалить authenticated source records.
   Native checkpoint provenance и Guard1→Guard2 equivalence доказываются отдельно.

## Требуемые проверки в S5 contract

- Реальный baseline V4 readable Text после credential renewal: O и R проверяемы,
  A/F остаются Unrecorded, original Text/IDs/history сохранены; никакого угадывания
  по нескольким подходящим Auth catalog entries.
- Ещё pending V4 сохраняет точный A; новый drain после upgrade создаёт точные F/R.
  Own unacked Text остаётся undelivered до настоящего opposite receipt.
- Wrong source digest/generation/C/lane/EventId, подмена projection/ref, missing
  mandatory bundle/receipt, fake membership и corrupt full-proof record отклоняются.
- Обычный writer/live IPC/peer input не может создать provenance variant, а старый
  binary не получает writable обход нового purpose/layout. Граница закрыта после Ready.
- Legacy unknown refs не удовлетворяют current revocation/expiry/lease или cut gate;
  continuation через mixed formats и обычный strict writer не получает fallback.
- Restart/copy resume сохраняет exact provenance marker и source binding, исключает
  двойной import, ложный Delivered и повторный Native переход.

Если эти boundaries требуют нового root/history encoding, вынести точное решение
в контракт со схемой совместимости. Настоящий документ не принимает автоматически
предложенное имя Ready7 или любой конкретный layout. Full contract review впереди.
