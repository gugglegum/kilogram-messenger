# RFC-0038: Authenticated local runtime IPC (M0.9.11)

Статус: реализовано в M0.9.11

## 1. Цель

M0.9.10 дал runtime собственные contact, durable outbox, retry и automatic
sync, но локальный интерфейс всё ещё состоял из команд, которые сами открывали
`STATE_DIR`. GUI поверх такого интерфейса стал бы ещё одним state writer.

M0.9.11 вводит один локальный API к уже запущенному runtime actor. UI передаёт
намерение, а проверку trust/membership, шифрование outbox и transaction/vault
commit выполняет runtime. P2P wire protocol и E2EE payload этим этапом не
меняются.

## 2. Разделение crate

Версионированный transport contract находится в отдельном workspace crate
`kilogram-runtime-ipc`. Его могут одинаково использовать CLI-adapter и будущий
Windows GUI. Crate содержит descriptor, request/response types, bounded framing,
client и server adapter; он не открывает device state и не содержит messaging
business logic.

Runtime остаётся владельцем:

- Account/device identity и trust repositories;
- contact selection и membership verification;
- HPKE encryption локального outbox;
- sequence, ratchet, event, ACK и retry transactions.

## 3. Локальный transport и descriptor

При `runtime --ipc-file <PATH>` процесс:

1. bind-ит эфемерный TCP port только на `127.0.0.1`;
2. генерирует случайный 256-bit bearer token;
3. подписывает address, token, Account ID и Device ID текущим device key в
   отдельном signature domain;
4. JSON-кодирует descriptor и публикует его same-directory temporary file +
   fsync + atomic replace.

Клиент принимает только supported version, loopback address, token правильной
длины и действительную device signature. `Ping` дополнительно сверяется с
Account/Device ID подписанного descriptor. Descriptor обязан находиться вне
protected `STATE_DIR`. После штатной или аварийной остановки runtime удаляет
файл, только если его содержимое всё ещё совпадает с descriptor этого instance;
старый процесс не может удалить descriptor более нового runtime.

Descriptor содержит секрет. Его следует хранить в private machine-local
каталоге и нельзя класть в Yandex Disk, другой sync или общий каталог. Это
same-user M0 boundary: процесс, уже способный читать файлы пользователя или
его память, находится вне предоставляемой гарантии. Будущий production adapter
может заменить bearer-файл на Windows named pipe/Unix socket с OS peer
credentials, не меняя actor commands. Подпись обнаруживает изменение полей, но
полная замена файла другим self-signed descriptor со стороны вредоносного
same-user процесса требует заранее закреплённого Device ID или OS credentials и
не входит в гарантию M0.

## 4. Bounded protocol

Один loopback connection обслуживает ровно один request/response. Frame —
4-byte big-endian length и Postcard body, максимум 256 KiB. Connect ограничен
3 секундами, I/O и ожидание actor response — 30 секундами. До 64 проверенных
requests могут ожидать actor; медленный/неавторизованный client обрабатывается
в отдельной bounded connection task и не блокирует Iroh accept.

Request содержит version, bearer token и одну команду:

- `Ping` — получить точные runtime Account ID и Device ID;
- `QueueMessage` — request ID, conversation, peer Account ID и plaintext;
- `OutboxStatus` — структурированные counters и queue metadata без plaintext.

Неверный token отбрасывается до dispatch в actor. Ошибка business/state
операции возвращается только локальному клиенту и не завершает runtime.

## 5. Single writer и идемпотентность

После framing/authentication connection task передаёт команду через bounded
MPSC в главный runtime loop и ждёт ответ через one-shot channel. Iroh session,
poll tick и IPC command мутируют состояние последовательно.

`QueueMessage.request_id` — случайные 256 бит. Runtime использует эти же bytes
как durable Queue ID. Повтор с тем же request ID и тем же account,
conversation и plaintext возвращает `AlreadyPresent`; другая команда с тем же
ID отклоняется. Поэтому потеря локального response не создаёт второе видимое
сообщение, если UI повторяет сохранённый request ID.

Первичная постановка выполняется под state lock и vault dual-write: plaintext
сразу HPKE-шифруется локальному device и не попадает в outbox record.
`OutboxStatus` берёт согласованный read lock и сообщает `queued`, `materialized`
или `delivered`, peer/conversation IDs и optional ACK Event ID.

## 6. CLI adapter

Добавлены:

- `runtime-ipc-ping --ipc-file ...`;
- `runtime-ipc-queue-message --ipc-file ...`;
- `runtime-ipc-outbox-status --ipc-file ...`.

Они не принимают `--state-dir`. Прямые `runtime-contact-add`,
`runtime-queue-message` и `runtime-outbox-status` пока сохраняются как offline
M0/bootstrap adapters, но GUI должен использовать actor API. Queue adapter
печатает request ID до connection и принимает `--request-id` для безопасного
повтора после неопределённого результата.

## 7. Проверка

- shared-crate tests проверяют signed descriptor, loopback-only address,
  authenticated round-trip, wrong-token rejection до actor и instance-owned
  cleanup;
- process test поднимает Alice/Bob runtimes, выполняет `Ping`, дважды посылает
  один QueueMessage request, видит одну queue record, доставляет ровно один text
  event, получает ACK и подтверждает identical histories после automatic sync;
- проходят rustfmt, strict Clippy, все workspace tests и release build.

## 8. Оставшиеся ограничения

M0.9.11 ещё не даёт push subscription/change revision, IPC contact onboarding,
несколько локальных authorization roles, OS peer credentials и GUI. Status
можно безопасно опрашивать bounded requests. Descriptor не является remote
API, discovery, mailbox или средством HTTPS-маскировки.

Следующий этап M0.9.12 — minimal Windows GUI, который запускает/подключается к
runtime, отправляет сообщения через этот crate и отображает состояние без
прямой записи в device state.
