# RFC-0040: Actor-owned chat read model (M0.9.13)

Статус: реализовано в M0.9.13

## 1. Цель

M0.9.12 доказал, что desktop GUI может ставить сообщения в очередь, не открывая
device state. Но пользователь всё ещё вручную вводил conversation label и peer
Account ID, а история была доступна только через CLI. M0.9.13 добавляет
read-only проекцию чатов, которой владеет тот же runtime actor.

GUI по-прежнему не знает `STATE_DIR`, не загружает ключи и не зависит от
state/store/session/transport crates. Расшифровка локальных projections
происходит внутри runtime под общим state lock; в GUI plaintext передаётся
только через уже аутентифицированный loopback IPC.

## 2. IPC-контракт

IPC version 2 получает две команды и несовместимый descriptor version, чтобы
новый GUI не подключался к старому runtime без read-model capability:

- `ConversationList` возвращает не более 256 подписанных runtime contacts и
  просматривает не более 16 384 events суммарно;
- `HistoryPage { conversation, cursor, limit }` возвращает от 1 до 100 text
  events выбранного разговора.

Conversation summary содержит Contact ID, label, Conversation ID, точные peer
Account/Device ID, route policy, число text messages и preview последнего
сообщения. Preview ограничен 96 UTF-8 bytes и помечает усечение. ACK остаются
проверяемыми replicated events, но не показываются как сообщения чата.

History item содержит Event ID, author Account/Device ID, author sequence и
полный локально расшифрованный body. Одна страница ограничена одновременно 100
сообщениями и 192 KiB plaintext body; framing по-прежнему не превышает 256 KiB.

## 3. Авторизация и read boundary

Перед выдачей списка runtime:

1. загружает DB-primary device identity и trust repository;
2. проверяет локальный device certificate и каждую signed contact card;
3. требует membership локального и peer accounts;
4. читает authenticated immutable event/local-projection snapshot;
5. повторно проверяет author authorization каждого event;
6. открывает только local projection текущего device/account.

Ошибки подписи, membership, projection или state I/O закрывают всю команду.
IPC handler исполняет чтение последовательно в runtime actor и держит тот же
typed state lock, что и остальные локальные snapshots. GUI не получает путь к
vault, ciphertext records, ratchet state или ключи.

## 4. Детерминированный порядок

Локальный store является DAG, а не блокчейном и не глобальным consensus log.
Для UI runtime строит topological order: каждый известный parent всегда раньше
child. Среди одновременно готовых событий используется стабильный tie-break
`(author_sequence, author_device_id, event_id)`.

Такой порядок детерминирован на одинаковом event set и пригоден для private
chat history. Он не обещает канонический внешний порядок concurrent group
messages; MLS epochs и group governance остаются отдельной задачей.

## 5. Pagination snapshot

Cursor содержит BLAKE3 digest упорядоченных text Event IDs и exclusive
`before_index`. Первая страница возвращает последние сообщения в прямом
порядке. Следующая страница заканчивается перед самым старым уже выданным
индексом.

Если text event set изменился, digest не совпадает и runtime требует начать
pagination заново. Добавление ACK не инвалидирует cursor. Индекс за пределами
snapshot и conversation свыше bounded event inventory отклоняются fail closed.

Cursor является process-local UI consistency token, а не переносимым
криптографическим доказательством полноты истории.

## 6. Desktop UI

После `Ping` клиент последовательно получает conversation list, последние 50
сообщений выбранного чата и outbox status. Раз в две секунды выполняется тот же
bounded polling cycle. Одновременно активна только одна IPC operation.

Левая колонка показывает контакты, message count и latest preview. Правая —
выбранную локальную историю, кнопку загрузки более старой страницы и composer.
Маршрут сообщения берётся только из выбранной signed contact summary; ручной
ввод peer Account ID удалён. Queue retry продолжает использовать стабильный
request ID из M0.9.12.

## 7. Проверка и ограничения

- cursor parser и snapshot binding проверяются unit tests;
- causal ordering test проверяет parent-before-child и стабильный ready tie-break;
- настоящий runtime process отвечает на `ConversationList`/`HistoryPage` до
  отправки, затем после P2P delivery read model возвращает один расшифрованный
  message;
- desktop adapter проходит полный signed loopback contract для новых команд.

Contact onboarding всё ещё выполняется CLI. Runtime запускается отдельно, push
subscription отсутствует, поэтому polling заново загружает bounded snapshot.
Следующий этап — IPC contact onboarding и runtime lifecycle из GUI без
ослабления signed-contact и single-writer границ.
