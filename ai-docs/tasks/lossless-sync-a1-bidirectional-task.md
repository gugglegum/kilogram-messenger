# A1: двусторонний диалог со сменой ключей и перезапусками

2026-10-23. Статус: [контракт принят после iteration2](lossless-sync-a1-bidirectional-contract-review-2.md);
после его commit/push разрешена реализация этого задания.
Это один этап с пользовательским результатом; контракт и реализация — его
последовательные части. Новый общий feasibility spike не требуется.

## Разрешение и исходное состояние

Пользователь прямо поручил передать этот этап тому же исполнителю, сначала
проверить контракт, затем реализацию, после приёмки сделать commit/push.
Первичный источник: userMessage `01a1661b-1561-7ec0-9380-b2835fd35738`,
thread `01a164ec-589c-7b41-bc34-3ba158d1f22b`, host `local`.
Исполнитель проверяет этот источник через `read_thread`; межчатовое поручение
и этот файл сами по себе не заменяют разрешение человека.

Baseline `1fce03ee2d2886f871e0ce44a641e3eeba9cdd0f` отправлен в `origin/master`
и независимо проверен оркестратором. Код принятого первого slice находится в
`dcc1300b759d45b0dfbb77e08b93d2cac0957b80`. Индекс был пуст; единственное чужое
изменение — `ai-docs/development-environment.md`, SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Не включать его в задачу, не менять и не откатывать.

Прочитать AGENTS.md, [приёмку первого slice](lossless-sync-a1-integration-review-2.md),
[отчёт](lossless-sync-a1-integration.md),
[proposal с уточнениями §5](lossless-sync-a1-proposal.md),
[review F1/F2](lossless-sync-a1-spike-review.md) и
[RFC-0193](../../docs/RFC-0193-user-sovereignty-and-contextual-identity.md).
Уточнения R1–R4/S1–S3/I1–I4 имеют приоритет над ранними предположениями proposal.
Полный обзор истории повторять не нужно; при необходимости читать ссылки из
этих документов и конкретный код.

## Результат и границы этапа

Два реальных процесса с новыми профилями и авторизованным контактом обмениваются
текстовыми сообщениями в обе стороны, меняют Olm DH chains, догоняют пропуски
после разрыва/перезапуска и показывают точную сохранённую историю.
Нужен воспроизводимый путь через настоящие CLI/runtime entrypoints и сетевой
протокол. Trusted transport/handshake fixtures и test-only profile factory
не заменяют этот результат. Ручная подготовка контакта существующими CLI
допустима: самостоятельный GUI onboarding остаётся отдельной задачей.

Основной сценарий: одна пара устройств, один личный диалог; независимость двух
contexts дополнительно проверяется отдельно. Использовать независимые Olm
domains и согласованный порядок через DH transitions, опираясь на уже проверенный
durable ingress. Обязательные пути этого этапа — отправка/direct receive и
восстановление через sync/prefix repair. Любой другой writer/delivery path,
способный затронуть новый domain, либо использует те же правила, либо явно
отказывает до изменения состояния. Не объявлять mailbox/relay готовыми без
их собственного evidence.

Новый формат включается явно для новых профилей/контекстов после negotiation.
Автоматическая миграция произвольных старых профилей, default rollout, перевыпуск
identity/device keys, device replacement, multi-device fan-out, группы/каналы,
GUI redesign, полная A3 history scalability и dormant recovery M1.2.63 вне этапа.
Смена DH chain внутри согласованной session epoch — обязательная часть этапа;
новая epoch не должна использоваться как обход пропусков или reset истории.
Старые данные и исходный single-chain profile не объявлять мигрированными.

## Сначала контракт

Создать `lossless-sync-a1-bidirectional-contract.md` в этой папке. Нужен один
рекомендуемый, реализуемый контракт с точными состояниями, форматами и привязкой
к существующему коду; решения нельзя оставлять на неявное усмотрение реализации.
Сохранить короткий список действительно открытых решений для review.

1. **Domain и handshake.** Canonical encoding/version/domain tags, context,
   credentials обеих сторон, роли и election при simultaneous open, epoch,
   fresh Olm account/prekeys, reservation/consumption OTK, session selection,
   transcript signatures/confirmation и момент разрешения data writers.
   Указать поведение при duplicate/conflicting handshake, restart каждой
   стороны, истёкшей/отозванной authority. Реальная credential admission и
   transport binding обязательны. Не клонировать legacy pickles по contexts
   и не создавать новый глобальный публичный идентификатор пользователя.
2. **Порядок и DH.** Две направленные lanes; authenticated application ordinal
   и predecessor EventId через все DH transitions. Определить, какие control
   frames расходуют ordinal, genesis и его связь с handshake. Разделить
   ordinal, native chain index, author_sequence и causal parents. Зафиксировать
   outer signature и inner encrypted binding, expected native transition,
   защиту от forged index/chain skip, duplicate и fork. Нативный Olm header
   не сообщает длину предыдущей chain: нельзя снять legacy unknown-tail gate
   без доказанного механизма. Использовать существующую криптобиблиотеку,
   без изменения криптопримитивов.
3. **Отправка и retry.** Одна durable транзакция для ratchet/ordinal,
   immutable event, local projection и outbox. Любой retry передаёт те же
   bytes/EventId; crash не создаёт ни ordinal hole, ни повторного encryption
   того же логического сообщения. Указать сериализацию send и receive,
   обработку lost ACK, источники ciphertext для последующего prefix repair.
4. **Receive и ACK.** Общий ingress для включённых delivery paths; distinct
   possession, readable, causal и ACK frontiers. Прописать atomic commit,
   ACK intent/dispatch/reconciliation, повтор после restart и API outcomes
   Pending/Readable/Invalid/OperationalFailure/Blocked. Сохранить I1–I4:
   operational failures откатывают workspace; accepted ciphertext не теряется;
   invalid pending не создаёт ложных ACK и не отменяет уже durable prefix.
5. **Gap repair и бюджеты.** Точный authenticated prefix/range запрос и response,
   bounded поиск и прогресс при заполненной очереди, расходование head reserve,
   byte budget полного frame и полного vault commit/staging. Согласовать caps
   с фактическим vault/frame overhead; ранние численные квоты proposal не
   считать доказанными. Разделить гарантированный progress в пределах бюджета
   и честный StorageBlocked/UpgradeRequired; никаких silent eviction/reset
   или subset SyncComplete. Не расширять этот пункт до полной A3.
6. **Совместимость и writers.** Negotiation без silent downgrade, schema/account
   fences при reopen, поведение старого binary и уже живого writer, lock scope,
   stale workspace после network await и crash activation. По коду перечислить
   все entrypoints, которые могут изменить новый domain, и способ их защиты.
   Старые локально читаемые сообщения остаются доступны по действующим правилам.
7. **План реализации.** Назвать затрагиваемые modules/entrypoints и минимальные
   логические изменения. Новую протокольную логику держать в обозримых модулях;
   не затевать посторонний refactor большого main.rs. Указать реальный запуск
   двух процессов, наблюдаемые статусы, ограничения и критерии ниже.

На этой части продуктовый код не менять и реализацию не начинать. Достаточны
чтение кода и небольшие адресные проверки, если без них нельзя разрешить
конкретную неопределённость. Повтор F1/F2 или 18-минутного старого acceptance
ради подготовки документа не нужен. Обнаруженное противоречие описать точно,
а не обходить ослаблением invariants или добавлением бесконечной серии этапов.

## Критерии будущей реализации для review контракта

Контракт должен показать, как проверить следующие свойства; это ещё не
утверждение, что они реализованы или что весь A1/MVP закрыт.

- Два настоящих runtime-процесса, новая согласованная session и минимум 256
  текстовых сообщений суммарно в обе стороны. У каждой стороны минимум восемь
  фактически наблюдаемых receiving DH transitions. Проверять точные plaintexts,
  EventIds и отсутствие дублей через реальный history API после reopen.
- Порядок поступления отличается от encryption order: gap более 40 сообщений,
  несколько страниц, поздние predecessor, interleaved direct/sync и задержка
  сообщений старой chain относительно новой. Детерминированный scheduling
  допустим, замена настоящего handshake/decrypt готовым fixture — нет.
- Завершение и повторный запуск процессов на нескольких границах отправки,
  pending admission, receive commit и ACK. Отдельные targeted fault/crash tests
  закрывают выбранные transaction windows. Для каждого указать, что реально
  прервано; обычный graceful reopen не выдавать за kill/power-loss evidence.
- Retry/lost ACK дают один логический результат, pending восстанавливается,
  ACK не предшествует readable commit. Конфликт metadata/inner binding,
  replay/fork, operational I/O failure и отказ negotiation проверяются
  meaningful negative tests. Сохранить regressions I1–I4.
- Два независимых contexts не расходуют ключи и не блокируют progress друг
  друга при gap в одном, кроме честно описанного общего исчерпания ресурсов.
  Backpressure/head reserve и prefix repair проверяются с малым тестовым
  бюджетом, включая переполнение и restart без потери accepted pending.
- Mixed-version/unsupported paths/live writer не обходят fences и не меняют
  состояние до отказа. Существующий ordinary runtime smoke остаётся рабочим.

Runtime/process acceptance — локальная синтетическая проверка; physical two-host,
внешние сети/volunteers, mailbox TTL, все power-loss windows и ежедневное MVP
не объявлять проверенными. Отдельный A2 dependency gate остаётся обязательным
до внешнего использования. Включить замеры setup/send/sync/reopen и количества
vault commits: старые 1095 s за 256-test не являются приемлемостью скорости.
Не маскировать функциональный результат обещанием производительности;
обнаруженный существенный bottleneck вынести с измерением и причиной.

## Выполнение, сдача и сохранение

Исполнитель: существующий чат `01a1653c-0154-7611-8e8c-044fbe1e4d7a`,
GPT-6.1 Sol Max. Оркестратор: `01a164ec-589c-7b41-bc34-3ba158d1f22b`.

После контракта отправить оркестратору один содержательный отчёт через
`send_message_to_thread` без model/thinking overrides получателя. Назвать его
отчётом исполнителя, указать baseline, документ, решения/неопределённости,
проверки и ограничения. Остановиться до review. Пустых «Готово»/ack-loop нет.

Оркестратор проверяет контракт и сам ведёт нужные циклы исправлений; после
приёмки поручает сохранить принятый контракт commit/push и начать реализацию.
У пользователя повторно разрешение на эту согласованную последовательность
не запрашивать. Реализация, её содержательные repair cycles и итоговый
commit/push уже разрешены пользователем, но зависят от соответствующей приёмки.
Проектирование не является разрешением сразу изменить wire/runtime.

После реализации — один callback с SHA/baseline, точным diff/source hashes,
проверками, evidence и ограничениями; затем review/repair, и только после
приёмки — commit/push и callback с remote HEAD. Оркестратор проверяет отправленное
содержимое. Не запускать следующий продуктовый этап автоматически.

На Windows использовать stable runner из AGENTS.md и постоянные process EXE
paths. Сохранять content hashes в evidence. Использовать bounded targeted
проверки; финальные gates выбирать по изменённым crates/entrypoints. Старый
дорогой acceptance не повторять при каждом мелком изменении; новые code changes
после финальной проверки требуют соответствующей повторной проверки.

Новый brief и изменения его индекса/координации подготовлены оркестратором и
относятся к этому этапу. При приёмке контракта включить их вместе с контрактом
и review по точному списку. Чужой power audit и иные неизвестные edits исключить;
никаких безусловных git add -A, force push, target/EXE/fixture keys в коммитах.
