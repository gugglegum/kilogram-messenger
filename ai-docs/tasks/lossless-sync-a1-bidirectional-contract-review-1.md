# OD1 contract — review iteration 1

2026-10-23. Baseline `1fce03ee2d2886f871e0ce44a641e3eeba9cdd0f`.
Проверен [контракт](lossless-sync-a1-bidirectional-contract.md), 739 строк,
SHA-256 `77c0c69babc7363353bcc422d144c057c374382a993560192415bdfca37686ab`.
Ссылки на строки ниже относятся к этой версии. Это review документа и кода
его зависимостей; новые runtime/crypto tests в этом ходе не выполнялись.

**Контракт iteration 1 пока не принят к реализации.** Направление OD1 соответствует
согласованному этапу, но C1–C3 нужно закрыть в самом контракте, не оставляя разные
трактовки реализации. Нужна адресная правка этого документа, без нового общего
spike, изменения продуктового кода или расширения до всего A1.

## Принятые решения и границы

- Fresh per-context Accounts, один selected epoch/SID, application order через DH,
  authenticated outer/inner binding и explicit tail/cause — подходящее направление.
  Pinned vodozemac 0.10.0 действительно создаёт sending key внутри encrypt и меняет
  sending state при успешном decrypt новой peer chain; pure trial + commit нужны.
  Это согласие с дизайном, не криптографическое доказательство его реализации.
- §15.1 принят: один committed/published materialized результат на request ID;
  unpublished volatile encrypt после rollback/crash можно вычислить повторно.
  Ничего не публикуется до durable commit, после него retransmit byte-exact.
  Это уточнение исходного требования, не ослабление защиты опубликованных сообщений.
- §15.2 принят **для данного opt-in этапа**: L1/fresh-only schema4/no epoch reset.
  512 encrypted frames/lane включает genesis; это максимум 511 Text/lane,
  после него UpgradeRequired без удаления истории. Это не пригодный для daily MVP
  lifetime limit и не закрытие A3; дальнейшая работа должна явно снять этот предел.
  Encode-bound tests T9 остаются gate перед activation. Арифметика reservation
  128 MiB/context, 4160 KiB/pending lane и 334.25 MiB/profile сходится; она не
  доказывает actual serialization/staging/disk fit.
- Versioned journal обоснован: существующий StateDirectoryLock вызывает recover
  до открытия vault (`kilogram-state/src/lib.rs:374–398`); recover удаляет terminal/
  incomplete journal до проверки версии (`939–962`). Проверять prepared и terminal
  crash seams старым binary необходимо. Возможность безопасного cleanup нельзя
  подменять обещанием отсутствия любых filesystem calls.

## C1 — P1: не задано durable private state до D/E/SID

Контракт §3 (`69–71`) выводит E/D только после полного Grant; SID возникает при
создании outbound Session. Но §6 (`247–250`) требует durable A Account вместе с
Offer **до Grant**, и durable B Account/reserved OTK вместе с Grant **до Init**.
Единственная private-state схема §12 (`605–608`) использует путь
`ratchet/domains-v1/<D>/state` и sealing AAD C/D/E/SID/generation.

Контрпример: A сохранил/отправил Offer, погиб и запустился до получения Grant.
Нужно прочитать именно прежний Account, но D/E/SID ещё не существуют. Аналогично
у B после Grant отсутствует SID. Это не разрешается повторным созданием Account
или условными zero IDs: они нарушают выбор уже опубликованных ключей либо вводят
неоговорённый формат/перенос секретов.

Нужно определить phase-tagged persisted records и location/AAD до selection:
какие bindings уже известны для Offered/Granted, где лежит Account/reservation,
как они атомарно переходят в selected-domain state и какая единственная запись
является authority после crash. Можно выбрать стабильный C-based namespace или
точные provisional-to-selected transitions, но вариант должен быть один.
Развести format/tag pre-session и selected session; budget, shadow encryption и
reader checks обязаны покрывать оба. Предписать проверки restart после Offer,
Grant и cutover, включая swapped/cross-context/stale phase blobs.

## C2 — P1: single dialer требует нормативного автоматического reconnect

§5 (`191–198`) оставляет dial только A; B OpenIntent ждёт A и может отправить
его лишь по уже установленному connection. §10 даёт backoff, но не задаёт
условие, по которому A инициирует/восстанавливает connection при пустом своём
outbox и отсутствии собственного IPC open.

Допустимый сейчас trace: оба runtime работают, старое соединение разорвано;
пишет только B, у A нет pending send и пользователь A ничего не нажимает.
B не может dial/отправить OpenIntent, а для A нет обязательного события reconnect.
Просто fair scheduling уже существующих задач не создаёт отсутствующую dial task.
Одновременный запуск ordered-open на обеих сторонах в happy-path test скроет пробел.

Зафиксировать обязанность A поддерживать/reconnect разрешённые C независимо от
локальной очереди, bounded timers/backoff и ситуации явной приостановки. Указать
источник startup/restart intent и восстановление handshake/Ready для peer с иной
durable phase, не менять роли и не вводить вторую session. Один creator Hello/
nonce/proof state на connection должен исключить две конкурирующие auth attempts.

Согласовать lifecycle с authorization lease ≤300 s и locator ≤1 h (`144,214–216`):
кто начинает reauth, когда обновляется descriptor и как peer получает его bytes
(Hello сейчас содержит только ref), либо где явно требуется operator refresh.
Ручное обновление locator для этого ограниченного CLI этапа допустимо, но граница
должна быть честной: не обещать автоматический progress при истёкшем locator.
Это не требование нового discovery/relay/consent-renewal проекта.

Добавить обязательный B-only-send trace с пустым outbox A, разрывом и relaunch A,
без IPC open/send у A, при заранее валидных pins/descriptors; он должен сам дойти
до Delivered. Отдельно проверить lease/locator expiry и выбранный recovery outcome.

## C3 — P2: обещанная possession reconciliation отсутствует в wire schema

§10 (`487–490`) говорит, что durable subset прерванного batch виден в новом
inventory/Resume. Но среди 17 kinds inventory нет, а Resume/Head/Cursor (`430–447`)
передают committed cursors, status и credit без pending IDs/slots. Эти данные
не позволяют отличить, например, pending {5,7} от {6,7} одинакового размера
при одном receive_next. Readable cursor нельзя объявлять possession frontier.

Выбрать и описать один вариант: bounded exact possession summary/query с понятной
ролью в retransmit, либо прямо отказаться от его передачи и задать достаточный
receiver-driven prefix/ACK reconciliation без remote possession knowledge.
Второй вариант допустим и может быть проще; тогда убрать несоответствующее обещание
inventory/Resume и точно указать поведение после lost Complete/partial admission,
автоматический drain после reopen, выбор следующего range и приоритет head repair
над повтором заведомо переполненного future batch. Не добавлять inventory ради
формального заполнения пункта, если progress обеспечивается выбранным алгоритмом.

Trace приёмки: будущая часть batch durable, connection погиб до Complete/Error;
после reopen receive_next не изменился, ordinary pending заполнен, source не знает
accepted subset. Source bytes остаются прежними; exact head через reserve позволяет
продвижение, повтор pending не расходует capacity, partial не становится full Complete
или message ACK. Гарантия должна следовать из реально указанных wire fields.

## Следующая итерация

Исправить C1–C3 в контракте, сохранить исходную версию iteration1 побайтно отдельным
историческим документом и дать короткую таблицу изменений/trace expectations.
Не повторять builds/F1/F2/старый 256 acceptance: пока необходимы согласованный
контракт и проверка его ссылок/форматов/арифметики. Продуктовый diff должен остаться
пустым. После содержательного callback оркестратор повторно проверяет контракт;
до приёмки commit/push и реализацию этого этапа не начинать.
