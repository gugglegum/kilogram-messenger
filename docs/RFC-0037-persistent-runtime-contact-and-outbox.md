# RFC-0037: Persistent runtime contact and durable outbox (M0.9.10)

Статус: реализовано в M0.9.10

## 1. Цель

M0.9.9 доказал, что один запущенный процесс может долго принимать delivery и
sync sessions, но отправитель всё ещё вручную запускал `connect`/`sync` и
передавал текущий peer ticket. M0.9.10 переносит это управление внутрь runtime:
локальная команда только регистрирует контакт или ставит сообщение в очередь,
а runtime сам следит за работой, повторяет доставку и запускает sync.

Wire protocol сообщений не меняется. Relay по-прежнему видит только Iroh/E2EE
frames и не получает plaintext или ключи истории.

## 2. Persistent contact

`runtime-contact-add` создаёт подписанную локальным Device ID карточку, которая
связывает:

- local Account ID и Device ID;
- точные peer Account ID и Device ID;
- conversation label и его `ConversationId`;
- route policy;
- абсолютный путь к обновляемому peer runtime ticket.

Перед сохранением проверяются подпись ticket, Account Root authority,
messaging capability обеих сторон, conversation membership, exact peer device,
route policy и prekey freshness/high-water. Descriptor должен находиться вне
защищённого `STATE_DIR`: это публичный addressing artifact, который peer
runtime или будущий discovery слой атомарно заменяет после restart.

Файловый путь — только M0 adapter. Он не является wide-area discovery protocol
и не решает privacy/social-graph, rollback witness или multi-source descriptor
gossip. Карточка M0.9.10 закрепляет один peer Device ID; безопасная смена
устройства/маршрута потребует нового versioned contact contract.

## 3. Durable encrypted outbox

`runtime-queue-message` не создаёт сетевой event. Она немедленно:

1. выбирает точную signed contact по peer Account ID и conversation;
2. генерирует случайный Queue ID;
3. HPKE-шифрует UTF-8 body публичным ключом локального устройства с metadata в
   AAD;
4. подписывает queue record локальным Device ID;
5. append-only сохраняет запись в `STATE_DIR/runtime/outbox`.

В plaintext filesystem/vault record тело не попадает. Contact, queue,
materialization, delivery и retry records имеют отдельные signature domains,
размерные лимиты и детерминированные имена. Новый typed `Runtime` state kind
включён в crash-consistent transaction и encrypted vault primary/shadow.

## 4. Materialize once и replay

При первом due-проходе runtime под state lock:

- открывает локально зашифрованное тело;
- фиксирует текущий frontier и author sequence;
- делает ratchet fan-out по свежему signed peer directory;
- создаёт `AuthorizedEvent` и локальную encrypted projection;
- одним transaction commit сохраняет projection, event и signed
  `.materialized` marker с полным event.

После этого любой reconnect отправляет байт-в-байт тот же signed event. Runtime
никогда не выделяет новый sequence и не шифрует второе сообщение для того же
Queue ID. Network wait проходит без state lock.

Listener ищет уже сохранённый ACK для повторно полученного Event ID. При replay
он возвращает тот же signed ACK и не выделяет новый acknowledgement sequence.
Поэтому обрыв после remote commit, но до local delivery marker, не создаёт ни
второго сообщения, ни второго ACK.

После проверки ACK sender одним transaction commit сохраняет ACK и signed
`.delivered` marker. Только marker делает queue item завершённым.

## 5. Persistent retry и automatic sync

Ошибка подготовки descriptor или network delivery создаёт новый подписанный
append-only retry state. Цепочка содержит Queue ID, monotonic generation,
previous State ID и `not_before`; restart проверяет всю цепочку. Задержка —
bounded exponential equal-jitter между `--retry-base-seconds` и
`--retry-max-seconds`.

Runtime проверяет outbox с `--poll-milliseconds`. Когда due delivery нет, он с
интервалом `--auto-sync-seconds` запускает sync каждого signed contact. Значение
zero отключает automatic sync. M0.9.10 выполняет outbound действия
последовательно и использует внутренний transient sync dialer, но владельцем
state остаётся тот же runtime process; внешний `sync` writer не требуется.

Для тестов `--max-outbound-actions` ограничивает число delivery/retry/sync
attempts. Ctrl+C, idle и inbound session bounds M0.9.9 сохраняются.

## 6. CLI boundary

Добавлены команды:

- `runtime-contact-add` — проверить и append-only закрепить peer descriptor;
- `runtime-queue-message` — локально зашифровать сообщение и поставить в
  очередь;
- `runtime-outbox-status` — проверить authenticated records и показать только
  metadata/status, не plaintext;
- расширенный `runtime` — polling, persistent retry и automatic sync.

Это временный локальный CLI adapter. Он не является IPC для GUI. M0.9.11 должен
дать UI один локальный API к runtime actor, чтобы GUI не становился вторым
state writer.

## 7. Проверка

- unit test выполняет encode/decode contact, encrypted queue round-trip,
  wrong-key/tamper rejection и restart-проверку hash-linked retry chain;
- state tests подтверждают append-only `Runtime` delta и DB-primary typed vault
  selection;
- process test запускает независимые Alice/Bob runtime, ставит сообщение в
  durable outbox, получает ACK, выполняет automatic sync и подтверждает
  идентичные истории: ровно один text event и один ACK;
- этот test также обнаружил и закрыл cancellation race: Iroh accept future
  теперь сохраняется между polling ticks и не отклоняет незавершённый
  handshake;
- проходят rustfmt, strict Clippy и все 124 workspace tests.

## 8. Оставшиеся ограничения

M0.9.10 ещё не даёт:

- локальный IPC/API, push stream для UI и multi-client authorization;
- privacy-preserving wide-area descriptor discovery/gossip/mailbox;
- multi-device contact routing и параллельную загрузку истории с нескольких
  устройств;
- portable contact update/rotation с rollback/equivocation witness;
- параллельные state-mutating outbound sessions;
- OS autostart/background registration.

Следующий этап M0.9.11 — локальный runtime IPC/API. M0.9.12 после него может
начать minimal Windows GUI без дублирования protocol/state логики.
