# Технические этапы

Актуально на: 2026-08-25.

## M0 — проверка сетевого и репликационного фундамента

### M0.1 — два процесса на одном хосте: выполнено

Реализовано:

- Git-репозиторий и Rust workspace;
- `kilogram-cli` с командами `listen` и `connect`;
- Iroh 1.0.3, QUIC и взаимно аутентифицированные Endpoint IDs;
- публичный base64url connection ticket без секретного ключа;
- передача ограниченного UTF-8 сообщения и acknowledgement;
- ограничение размера входного сообщения;
- unit-тесты round-trip и version rejection для ticket;
- строгие fmt, Clippy `-D warnings` и tests.

Проверено локально:

```text
Client endpoint 2512...5626
    -> hello from kilogram m0 smoke test
Listener endpoint 2397...23d9
    -> ack:hello from kilogram m0 smoke test
```

Это transport proof of concept, а не защищённый мессенджер. Endpoint identity
пока эфемерна; message-level E2EE, Account Identity, подпись события, история и
синхронизация отсутствуют.

### M0.2 — два хоста в одной LAN: не начато

- Передать ticket на второй физический хост.
- Проверить Windows Firewall и direct LAN path.
- Зафиксировать выбранный Iroh path и сетевую диагностику.

### M0.3 — два хоста в разных сетях: не начато

- Проверить hole punching при двух NAT.
- Отдельно принудительно проверить public relay fallback.
- Проверить reconnect и смену сетевого интерфейса.

### M0.1.1 — постоянная device identity и signed event: выполнено

Реализовано:

- отдельная прикладная Ed25519 device identity, не связанная с эфемерным Iroh
  Endpoint ID;
- сохранение development device secret и author sequence в явно заданном
  `--state-dir`;
- минимальная детерминированная модель `SignedEvent` с domain separation;
- BLAKE3 event ID, conversation ID, causal parents и лимиты входных данных;
- подписанные `Text` и `Acknowledgement`, строгая проверка подписи получателем;
- привязка публичного device ID listener к connection ticket и проверка автора
  acknowledgement клиентом;
- временный Postcard codec и новый ALPN `kilogram/m0/signed-event/1`;
- unit-тесты persistence, sequence, signature binding, tampering, versioning и
  causal acknowledgement;
- два успешных локальных прогона до и после перезапуска обоих процессов.

Проверка перезапуска показала: transport Endpoint IDs изменились, application
device IDs остались прежними, а sequence отправителя изменился с `0` на `1`.

Ограничения этого шага:

- secret key хранится без шифрования и пригоден только для разработки;
- allocator sequence не поддерживает два процесса на одном state directory;
- device key ещё не авторизован Account Root Identity;
- payload пока не имеет message-level E2EE и не сохраняется в историю;
- codec, схема события и derivation conversation ID не являются финальным wire
  protocol.

### M0.1.2 — локальный append-only event store: выполнено

Реализовано:

- отдельный crate `kilogram-store`;
- неизменяемый content-addressed файл на каждый проверенный signed event;
- запись через temporary file и atomic no-clobber persist;
- идемпотентная повторная вставка и ошибка при конфликте event ID;
- проверка подписи, event ID, имени файла и conversation directory при чтении;
- запрет equivocation: один device sequence не может обозначать два разных
  события в одном conversation;
- сохранение исходящего `Text` до сетевой отправки;
- сохранение входящего `Text` до создания acknowledgement;
- сохранение исходящего acknowledgement до отправки и входящего — до успешного
  завершения команды;
- вычисление causal frontier; новое сообщение ссылается на предыдущие локальные
  heads;
- команда `history` для проверки локальных events и frontier после перезапуска;
- 5 unit-тестов store: persistence, deduplication, corruption detection,
  writer-sequence conflict и frontier.

Storage smoke test после двух последовательных соединений и перезапуска:

- у Alice и Bob по 4 одинаковых события;
- у обоих один одинаковый frontier;
- второе сообщение причинно ссылается на acknowledgement первого;
- application device IDs и author sequences продолжаются между запусками.

Ограничения:

- event store хранит M0 plaintext без at-rest encryption;
- это проверка модели, а не выбранная production database;
- хранение рассчитано на один процесс на `state-dir`;
- история пока не синхронизируется, если один из участников пропустил событие;
- вывод `history` отсортирован по event ID и не является timeline ordering.

### M0.1.3 — bounded inventory/diff sync: выполнено

Реализовано:

- новый ALPN `kilogram/m0/sync/1` и typed Postcard envelopes для delivery и
  sync;
- connection ticket подписан application device key listener; подмена Iroh
  endpoint, listener ID или allowed requester ID обнаруживается до отправки
  inventory;
- команда `identity` позволяет заранее получить requester ID; `listen` требует
  явный `--allow-device`, а signed ticket фиксирует это разрешение;
- двухфазный bidirectional sync на одном Iroh connection: signed inventory,
  diff, ответный event batch и completion;
- полный inventory ограничен 4096 event IDs;
- один diff ограничен 64 events в каждом направлении и сообщает
  `more_available` для продолжения;
- каждый embedded event повторно проверяется protocol и store слоями;
- listener запрашивает только IDs из подписанного inventory, клиент отправляет
  только явно запрошенные events, обе стороны сверяют точные множества IDs;
- inventory подписывается application device key и session-bound к текущему
  Iroh Endpoint ID listener;
- diff подписывается application device key listener, также session-bound, а
  клиент проверяет владение публичным ключом из connection ticket до отправки
  запрошенных локальных events;
- listener синхронизирует историю только device ID, уже встречавшемуся как
  автор локального conversation и совпадающему с `--allow-device`;
- неизвестный requester получает явный `SyncRejected`, не историю и не
  зависшее соединение;
- store умеет строить двунаправленный bounded sync plan и выбирать events по
  запрошенным IDs.

Recovery smoke tests:

1. Новый пустой store с прежним ключом Alice восстановил 2 события с Bob.
2. Частичный store Bob запросил и получил недостающее событие у Alice во второй
   фазе протокола.
3. Обе восстановленные истории получили одинаковый frontier.
4. Device, не совпадающий с allowed requester в signed ticket, отклонён клиентом
   до Iroh connection.
5. Явно allowed, но отсутствующий среди authors device получил
   `RequesterNotKnown`; listener завершил запрос штатно без раскрытия events.
6. Финальный recovery прогон повторён с одновременно signed ticket и signed
   session-bound diff.

Ограничения:

- правило «известный author device» — временная M0 authorization, а не замена
  Account Root certificates, membership epochs и revocation;
- ticket раскрывает listener и allowed requester device IDs;
- full-ID inventory раскрывает peer известный набор event IDs и не масштабируется
  как Merkle/range summary;
- continuation пока требует нового запуска listener и команды `sync`;
- batch sync не имеет resumable cursor;
- payload и локальный store всё ещё не зашифрованы.

### M0.1.4 — transport boundary и automatic continuation: выполнено

Реализовано:

- `kilogram-session` содержит transport-independent client/server sync state
  machine и все проверки responder identity, session binding, conversation,
  requested IDs, exact batches и completion;
- `SessionStore` отделяет reconciliation rules от конкретного файлового store
  и позволяет детерминированно тестировать их в памяти;
- `kilogram-transport-iroh` содержит Iroh ALPN, лимит wire message и typed
  Postcard framing; Iroh stream types больше не используются session-слоем;
- CLI автоматически повторяет inventory/diff/batch/completion rounds в одном
  Iroh connection до convergence;
- один connection ограничен 64 rounds, один round — прежними 64 events в каждом
  направлении;
- новый `EventStore::put_batch` проверяет существующую writer history один раз
  для всего принятого batch и сохраняет dedup/equivocation guarantees;
- CLI печатает per-round counters, общее число rounds и итоговые totals.

Проверки:

- in-memory расхождение 70 client-only и 70 server-only events сошлось ровно за
  два rounds (64 + 6) в обе стороны;
- неизвестный requester по-прежнему получает `RequesterNotKnown` до раскрытия
  events;
- локальный Iroh smoke после разделения crates восстановил отставшему client 2
  events, завершился с `sync_rounds_completed=1` и одинаковой валидной историей;
- `cargo fmt`, строгий Clippy и все 24 workspace tests прошли.

Ограничения:

- full-ID inventory по-прежнему ограничен 4096 IDs, поэтому это только M0
  reconciliation profile;
- после разрыва connection нет signed resumable cursor: следующая команда
  начинает reconciliation с нового полного inventory;
- 64-round cap защищает connection от бесконечной работы, но не является
  production policy;
- transport adapter пока покрывает только Iroh, а его граница ещё должна быть
  проверена альтернативным transport или test adapter;
- payload и локальный store всё ещё не зашифрованы.

### M0.1.5 — диагностика Iroh path: выполнено

Реализовано:

- Iroh adapter снимает snapshot фактически выбранного connection path;
- после прикладного exchange adapter ждёт до трёх секунд возможной миграции с
  relay на direct path;
- `listen`, `connect` и `sync` печатают `transport_path`, remote transport
  address, RTT и число открытых paths;
- результат различает `direct`, `relay`, `custom` и `unknown`;
- локальный smoke на обоих концах показал `direct` IP path.

Назначение — сделать M0.2 доказательным: успешная доставка через relay не должна
ошибочно считаться успешной LAN direct проверкой. Диагностика пока выводится в
stdout и не является production telemetry API.

### Следующее расширение M0

1. Выполнить M0.2 на двух физических Windows-хостах в LAN и зафиксировать
   `direct` path с обеих сторон.
2. Проверить reconnect/error paths и определить минимальный resumable sync
   cursor до замены full-ID inventory на Merkle/range summary.
3. Начать Account Root → Device authorization model либо pairwise E2EE spike по
   приоритету следующего RFC/ADR.
