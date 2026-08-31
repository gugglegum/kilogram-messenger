# Технические этапы

Актуально на: 2026-08-31.

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

### M0.2 — два хоста в одной LAN: выполнено

Проверено на двух физических Windows-PC:

- Alice `192.168.0.134` и Bob `192.168.0.135` обменялись signed Text и signed
  Acknowledgement;
- обе стороны выбрали `transport_path=direct`, RTT около 1–2 ms, relay не
  использовался;
- Bob перезапустил listener с новым transport Endpoint ID и новым signed ticket;
- recovery store Alice содержал прежний device key, но нулевой inventory;
- за один sync round recovery store получил ровно 2 отсутствующих events и не
  отправил лишних;
- повторное чтение проверило подписи, event IDs, causal parent и восстановило
  `event_count=2`, `frontier_count=1`;
- первый recovery прогон обнаружил Windows `MAX_PATH` bug; M0.1.6 исправил его,
  а повтор на тех же физических хостах прошёл успешно.

### M0.3 — два хоста в разных сетях: выполнено

Инструменты проверки готовы:

- signed ticket v2 фиксирует `auto`, `direct-only` или `relay-only`;
- `direct-only` не передаёт Kilogram protocol frames до выбора direct IP path,
  сохраняя relay-assisted NAT traversal;
- `relay-only` физически отключает IP transports у обоих endpoints;
- connection, route selection, stream open и wire I/O имеют ограниченные
  таймауты с явным названием зависшей операции;
- локальный process smoke проверил delivery, перезапуск listener с новым
  Endpoint ID/ticket и sync в `direct-only` и `relay-only`;
- public relay smoke выбрал `euc1-1.relay.n0.iroh.link`, причём relay-only ticket
  содержал только Relay address, без IP candidates.

На двух физических хостах в разных сетях подтверждено:

- `direct-only` корректно отказался передавать frames, когда NAT не позволил
  создать direct path;
- `auto` доставил event через public relay fallback;
- strict `relay-only` через явно выбранный `aps1` доставил event без IP paths;
- relay-only sync после restart listener восстановил 6 недостающих events за
  один bounded round и свёл обе стороны к 8 events.

Промежуточный внешний прогон без VPN выбрал direct path и успешно доставил
event, но transport addresses оказались `192.168.0.134` ↔ `192.168.0.111`.
Оба адреса находятся в одной private LAN, поэтому этот результат пока не
засчитывается как cross-network hole punching. VPN действительно мешал первой
попытке, однако сетевую изоляцию Bob через 4G надо подтвердить отдельно.

Корректный cellular-only прогон выполнялся при чистой routing table Alice:
единственный default route шёл через `192.168.0.1`, а `singbox_tun` отсутствовал.
Relay handshake прошёл, но direct path не возник за 15 секунд. Это валидный
результат: выбранная home-to-cellular topology требует relay, вероятнее всего
из-за NAT телефона и CGNAT/фильтрации мобильного оператора.

Первый внешний `relay-only` control оставил Bob в состоянии
`relay_status=online` / `status=listening`, тогда как Alice не дошла до
`peer_id` и получила 30-секундный connection timeout. Локальное public-relay
воспроизведение тем же кодом успешно. Диагностическая сборка теперь на Alice
отдельно ждёт online relay и печатает target Endpoint ID для проверки, что
скопирован ticket именно текущего listener Bob.

Повтор подтвердил совпадение Endpoint ID и online relay на обеих сторонах, но
strict relay-only снова не дошёл до `peer_id`. Следом `auto` на тех же двух
сетях успешно доставил и подтвердил signed event через единственный relay path:
Bob ticket рекламировал `euc1`, а установленное соединение выбрало `aps1`, RTT
составил 457–686 ms. Это доказывает работоспособность public relay fallback и
локализует проблему в строгом отключении IP transports при разных relay
selections. Следующая сборка pin-ит relay-only dialer к relay URL подписанного
listener ticket; остался один внешний retest delivery и sync.

Retest с одинаковым pinned `euc1` на Bob и Alice снова завершился timeout до
`peer_id`, поэтому разные home relay не являлись достаточной причиной. В CLI
добавлен явный `--relay-url`; следующий минимальный тест принудит оба strict
relay-only endpoints использовать `aps1`, который уже успешно перенёс
cross-network `auto` exchange. Успех локализует проблему в `euc1` route, а
повторный timeout — в cross-network поведении Iroh без IP transports.

Локальный release smoke нового override успешно выполнил strict relay-only
delivery через явно выбранный `aps1`; ticket, home/target URL и selected path
совпали, открытым был ровно один relay path. Осталась внешняя проверка теми же
двумя хостами.

Внешний test5 также успешно выполнил strict relay-only delivery через `aps1`:
Bob home, Alice target/home и selected path совпали, был открыт ровно один
relay path, а signed event и acknowledgement совпали на обеих сторонах. RTT
составил 461.9–466.6 ms. Это подтверждает cross-network работу
`clear_ip_transports`; прежние timeout были специфичны для доступности `euc1`
route в момент тестов.

Финальный test6 перезапустил Bob с новым Endpoint ID/ticket. Alice имел 8
events, Bob уже имел 2; за один round Alice отправил, а Bob получил ровно 6
недостающих events. Обе стороны сообщили `status=synchronized`,
`sync_more_available=false`, `transport_path=relay`, один open path и RTT около
472 ms. Таким образом, M0.3 выполнен полностью.

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

### M0.1.6 — Windows long-path event store: выполнено

Реальный M0.2 recovery test дважды дошёл до sync diff, но пустой client store
получил `os error 3` при записи. Derived event path имел 268 символов. Системный
`LongPathsEnabled=1` был включён, однако atomic-file операция получила обычный
путь без verbatim prefix.

Исправлено:

- `EventStore::open` canonicalizes созданный root; Windows возвращает verbatim
  absolute path, используемый всеми последующими event operations;
- Windows regression test намеренно строит event path длиннее 260 символов;
- до исправления тест воспроизводил тот же `os error 3`, после исправления
  проходит;
- полный recovery smoke release-бинарником сохранил 2 events при максимальной
  длине пути 298 символов, получил `event_count=2`, `frontier_count=1` и
  `transport_path=direct`;
- строгий Clippy и все 25 workspace tests проходят.

Внешний M0.2 delivery и recovery sync между двумя физическими Windows-хостами
подтверждены: direct LAN path, RTT около 1–2 ms, 2 восстановленных events и
правильный causal frontier.

### Реализовано для M0.3: route policies и bounded failures

Реализовано:

- transport-level `RoutePolicy` и endpoint factory;
- `auto`, `direct-only` и строгий `relay-only` в CLI;
- route policy входит в подписанный ticket v2 и автоматически применяется
  connector, поэтому стороны не могут незаметно выбрать разные режимы;
- `direct-only` ждёт до 15 секунд direct path перед первым прикладным stream;
- `relay-only` использует Iroh endpoint без IP transports и требует online relay;
- connect/handshake ограничены 30 секундами, path/stream/wire операции — 15;
- ошибки называют операцию, timeout и фактически выбранный path, если он есть;
- после exchange выбранный path повторно сверяется с signed policy.

Process smoke на одном Windows-хосте:

- direct delivery и sync после restart listener: `transport_ready_path=direct`,
  новый Endpoint ID, одна sync round без лишних events;
- relay delivery и sync после restart listener: `transport_ready_path=relay`,
  `transport_path=relay`, public n0 relay, одна sync round;
- на момент первого route-policy smoke проходили все 28 workspace tests.

Первый внешний direct-only прогон обнаружил, что единичная ошибка packet
authentication в `Incoming` завершала listener. Iroh предупреждает, что ранний
`Incoming::accept` может штатно отклонять посторонние или retransmitted UDP
datagrams. Listener исправлен: такие initial/handshake attempts диагностируются
и игнорируются, после чего accept-loop продолжает ждать валидное соединение.
Regression test подтверждает продолжение работы после несовместимого ALPN
handshake и успешный приём следующего клиента; текущий workspace содержит 29
проходящих tests.

### M0.4 — pause/reconnect sync: выполнено

Реализовано:

- `sync --max-rounds N` может штатно остановить reconciliation после N
  полностью подтверждённых rounds, если `more_available=true`;
- `SyncPause` / `SyncPaused` завершают текущий connection без ложной ошибки у
  listener; обе стороны печатают `status=paused` и
  `sync_resume_checkpoint=event-store`;
- события по-прежнему сохраняются до acknowledgement/completion и
  дедуплицируются, поэтому новый запуск с новым Endpoint ID и session binding
  начинает со свежего signed full inventory, но передаёт лишь отсутствующий
  остаток;
- отдельный переносимый cursor не вводится: с текущим full-ID inventory он не
  сокращает трафик, а snapshot/staleness semantics пока не определены.
- development-only `seed-history` создаёт локальную цепочку подписанных events,
  чтобы воспроизводимо получить расхождение больше одного batch без 140 ручных
  сетевых отправок.

Проверки:

- deterministic in-memory divergence 70/70 выполняет первый round 64/64,
  меняет transport session binding и после reconnect передаёт ровно оставшиеся
  6/6 за один round;
- old inventory/diff не переиспользуются между sessions; новая сторона создаёт
  новый session-bound inventory из durable store;
- wire round-trip покрывает новые pause request/response;
- локальный Iroh smoke создал по 70 расходящихся events поверх 2 общих,
  остановился после 64/64 с `status=paused`, перезапустил listener с новым
  Endpoint ID и передал только остаток 6/6; обе histories содержали 142 events,
  одинаковый frontier и полностью совпадающий вывод;
- внешний test7 на двух физических Windows-хостах остановил direct LAN sync
  после 64/64, затем после переключения Bob на cellular продолжил через pinned
  `aps1` только остатком 6/6. Автоматическое сравнение подтвердило 142 events,
  `frontier_count=2` и полностью одинаковые histories;
- `cargo fmt`, строгий Clippy и все 32 workspace tests проходят.

M0.4 закрывает correctness-level возобновление между подтверждёнными rounds при
смене process, Endpoint ID, session binding и сетевого пути. Он не обещает
переносимый compact cursor или эффективный inventory для больших histories.

### M0.5.1 — Account Root → Device authority: выполнено

Реализовано:

- отдельный случайный Ed25519 Account Root и публичный `AccountId`, не
  совпадающие с device/transport identities;
- root-signed `DeviceCertificate` связывает Account ID, Device ID, монотонный
  authority sequence и canonical capabilities `sign-events,sync-history`;
- root-signed `DeviceRevocation` навсегда отзывает конкретный device key;
  более поздний сертификат не возвращает его в доверенное состояние;
- `DeviceState` устанавливает сертификат только для собственного Device ID и
  не заменяет его другим сертификатом;
- публичная проверка требует ожидаемый Account ID, необходимые capabilities и
  валидный account-scoped revocation view;
- CLI-команды `account-create`, `account-show`, `device-enroll`,
  `device-authorize`, `device-revoke` покрывают полный локальный lifecycle.

Проверки:

- unit tests покрывают persistence, idempotent install, wrong account/device,
  signature tampering, noncanonical/duplicate capabilities, missing capability,
  tampered revocation и permanent revoke после более поздней reissue;
- CLI lifecycle test и отдельный локальный smoke подтверждают create → enroll →
  authorize → revoke → отказ authorization с ненулевым exit code;
- форматирование, строгий Clippy, release build и все 38 workspace tests
  проходят.

Граница среза: root secret пока хранится plaintext в отдельной development
директории; seed/recovery, rotation/quorum, distributed revocation view,
device encryption/session keys и сетевое применение сертификатов не входят в
M0.5.1. Формат описан в
[`../docs/RFC-0002-account-device-authority.md`](../docs/RFC-0002-account-device-authority.md).

### M0.5.2 — account-authorized network sessions: выполнено

Реализовано:

- несовместимый ticket v3 содержит Endpoint, root-signed listener certificate,
  allowed requester Account ID и route policy; весь ticket подписан listener
  device key;
- `connect` / `sync` требуют явный `--expect-account`, а listener использует
  `--allow-account` вместо привязки к одному Device ID;
- первый application stream несёт `SignedDeviceSessionAuthorization`: requester
  подписывает certificate и binding текущего listener Endpoint ID;
- `kilogram-session` проверяет session proof, ожидаемый Account ID,
  capabilities и caller-supplied `DeviceRevocation` до event/inventory;
- обе стороны принимают повторяемые `--peer-revocation-file`; общий ответ peer
  не раскрывает детали причины отказа;
- known-author удалён из `SessionStore` и sync policy. Новый сертифицированный
  device разрешён даже при пустой локальной истории, но inventory signer обязан
  совпадать с уже авторизованным device.

Проверки:

- protocol tests подтверждают root/device/session binding и wire round-trip;
- transport-independent session tests принимают новое сертифицированное
  устройство, отклоняют другой device, другой Endpoint binding и root-signed
  revocation;
- CLI/Iroh integration test выполняет отдельный authorization stream до
  application exchange; ticket test отклоняет revoked listener;
- локальный process smoke между отдельными Alice/Bob Account IDs успешно
  доставил Text/Acknowledgement, затем второе неизвестное устройство Alice с
  пустым inventory получило 2/2 events Bob без synthetic event;
- после root revocation этого второго device новый listener session напечатал
  `authorization=rejected`, клиент получил ненулевой exit code до inventory;
- форматирование, строгий Clippy, release build и все 42 workspace tests
  проходят.

Ограничение: валидность переданных revocation проверяется, но отсутствие
скрытого или ещё не доставленного отзыва не доказывается. Conversation
membership и полномочия авторов history batch также ещё не реализованы.

### M0.6.1 — signed authority snapshots и anti-rollback: выполнено

Реализовано:

- Account Root хранит каждый permanent revocation в durable canonical set и
  подписывает полный `AccountAuthoritySnapshot` с монотонной revision;
- snapshot валидирует вложенные root signatures, Account ID, уникальный
  порядок Device IDs и покрытие всех authority sequences;
- device state атомарно хранит snapshot своего аккаунта и max-seen snapshot
  каждого peer account; более старая revision и два разных состояния одной
  revision отклоняются;
- legacy root, у которого уже были authority operations без durable log, не
  может объявить неполный набор полным и требует явной будущей миграции;
- ticket v4 несёт listener certificate + snapshot, а несовместимый
  Endpoint-bound session authorization v2 несёт requester certificate + snapshot;
- snapshot pinning происходит до certificate authorization: валидный новый
  snapshot сохраняется даже тогда, когда он отзывает предъявившее его device;
- `--peer-revocation-file` удалён из network CLI; добавлены
  `account-snapshot` и `device-authority-update`, а `device-enroll`
  автоматически устанавливает текущий own snapshot.

Проверки:

- identity tests покрывают persistence полного набора, permanent revoke,
  atomic update, idempotency, rollback, equivocation и legacy-root refusal;
- protocol/session tests покрывают snapshot внутри device-signed session proof
  и отказ revoked certificate;
- ticket test подтверждает, что старый криптографически валидный ticket
  отклоняется устройством, уже закрепившим более новую peer revision;
- CLI lifecycle выполняет enroll → authorize → revoke → snapshot export/update
  → ожидаемый отказ authorization;
- `cargo test --workspace` проходит 46 tests, включая сетевой regression:
  listener сохраняет новый snapshot до ожидаемого отказа revoked device.
- локальный process smoke свежими Alice/Bob Account Roots выполнил ticket v4,
  установил peer revision 1 на обеих сторонах и доставил подписанные
  Text/Acknowledgement по direct path;
- `cargo fmt --all -- --check`, строгий Clippy и release workspace build
  проходят.

Граница среза: completeness доказана на конкретной подписанной revision, а
rollback — после наблюдения более новой. Узел при первом контакте не может
узнать, существует ли ещё более свежая revision, пока нет authenticated
discovery/gossip/witness.

### M0.6.2 — signed conversation membership и author authorization: выполнено

Реализовано:

- owner Account Root подписывает полный canonical
  `ConversationMembershipSnapshot`: conversation ID, revision, owner и список
  Account IDs;
- owner всегда является участником; M0 membership только расширяется, потому
  что removal должен быть связан с ordered security event и новой key epoch;
- device state атомарно устанавливает membership только для собственного
  member account и отклоняет rollback, equivocation, смену owner и удаление
  прежнего участника;
- `AuthorizedEvent` связывает `SignedEvent`, root-signed `DeviceCertificate` и
  полный `AccountAuthoritySnapshot` автора;
- delivery, acknowledgement и каждый event history batch проверяются по цепочке
  membership → account snapshot → device certificate → event signature;
- direct exchange дополнительно требует совпадение account/device автора с
  уже авторизованной session;
- event store сохраняет исходный content-addressed `.event` и обязательный
  immutable `.authorization` sidecar; history и sync fail-closed при его
  отсутствии или несовпадении;
- sync envelopes/signature domains переведены на v2, Iroh ALPN — на
  `kilogram/m0/sync/2`; ticket остаётся v4, так как его структура не менялась;
- CLI получил `conversation-create`, `conversation-member-add` и
  `conversation-membership-install`; `connect`, `listen`, `sync`, `history` и
  `seed-history` требуют установленный membership.

Проверки:

- identity tests покрывают canonicalization, persistence, add-only update,
  rollback и equivocation;
- protocol/session tests отклоняют event аккаунта вне membership;
- store test требует совпадающий authorization sidecar;
- прежние multi-round 64+6 и reconnect sync tests работают с полными
  authorization envelopes;
- все 50 workspace tests проходят;
- финальный release direct-Iroh smoke между отдельными Alice/Bob Account IDs
  создал и установил общий membership, доставил авторизованные
  Text/Acknowledgement, добавил Alice ещё 3 events, передал Bob только эти 3 за
  один sync round и подтвердил две одинаковые истории из 5 events.

Граница среза: автоматической миграции старых `.event` без доказательства
Account ID нет. Один owner/add-only не реализует group governance или removal.
Embedded authority snapshot доказывает состояние на своей revision, но без
trusted time/epoch и gossip не исключает forged «historical» event украденным
отозванным ключом. Полный контракт описан в
[`../docs/RFC-0003-conversation-membership.md`](../docs/RFC-0003-conversation-membership.md).

### M0.7.1 — pairwise HPKE payload baseline: выполнено

Реализовано:

- новый `kilogram-crypto` изолирует RFC 9180 HPKE Base mode ciphersuite
  X25519/HKDF-SHA256/ChaCha20-Poly1305;
- device state хранит отдельный encryption key seed, а root-signed
  DeviceCertificate v2 связывает Account ID, signing Device ID и X25519 public
  key;
- plaintext `EventPayload::Text` удалён; encrypted text содержит canonical
  recipient boxes для sender device и одного peer device;
- HPKE AAD связывает conversation, author, sequence, parents и recipient;
  внешняя Ed25519 event signature аутентифицирует полный ciphertext envelope;
- listener обязан расшифровать полученный text до persist/ack; `history`
  расшифровывает локальный recipient box; sync и store работают с ciphertext;
- `seed-history` теперь требует `--peer-certificate-file` и не создаёт plaintext
  fixtures;
- event/signature domains повышены до v2, sync/session authorization — до v3,
  ticket — до v5, Iroh ALPN — до `kilogram/m0/sync/3`.

Проверки:

- HPKE round trip, wrong key, AAD tampering, missing recipient и ciphertext
  tampering покрыты unit tests;
- serialized event и сохранённый `.event` проверены на отсутствие fixture
  plaintext;
- certificate persistence проверяет совпадение локального encryption key;
- все 54 workspace tests проходят после перевода store/session fixtures на
  encrypted events.
- release process smoke между отдельными Alice/Bob accounts выполнил direct
  encrypted delivery и reconnect sync трёх events; обе расшифрованные histories
  содержат одинаковые 5 events, а fixture plaintext отсутствует в `.event`.

Граница среза: static recipient key не даёт forward secrecy или PCS. Нет
asynchronous prekeys, session ratchet, multi-device fan-out, protected local
keystore и metadata hiding. Полный контракт —
[`../docs/RFC-0004-pairwise-hpke-payload.md`](../docs/RFC-0004-pairwise-hpke-payload.md).

### M0.7.2 — local encrypted history projection: выполнено

Реализовано:

- replicated `EncryptedText` v3 содержит один peer recipient HPKE box; отдельный
  static-key sender box удалён;
- `LocalTextProjection` связывает Event ID и локальный Device ID и шифрует
  читаемую копию на key текущего устройства;
- новый `LocalMessageStore` атомарно и immutable хранит projection отдельно от
  `events`; projection никогда не входит в `AuthorizedEvent`, inventory или wire;
- connect/listener сохраняют projection до event, а history читает только её;
- sync создаёт projection после успешного recipient AEAD open и до event batch;
  повтор exact event идемпотентен, outsider event без local box отклоняется;
- event/sync/session/ticket/ALPN границы повышены до v3/v4/v4/v6/sync-4.

Проверки:

- sender больше не расшифровывает replicated event, но читает собственную
  encrypted projection;
- serialized event и projection не содержат fixture plaintext;
- seed-history создаёт projections для всех fixture events;
- sync создаёт recipient projection и повторно принимает тот же event без
  randomized-HPKE immutable conflict;
- все 56 workspace tests проходят.
- release process smoke `.tmp/m072-smoke-20260831-194220` выполнил direct
  delivery, reconnect sync трёх events, подтвердил одинаковые histories из 5
  events и по 4 projections; plaintext отсутствует в `.event` и `.local-text`.

Граница среза: static peer HPKE box всё ещё не даёт FS/PCS. Потерянную sender
projection нельзя восстановить из возвращённого peer ciphertext; нужен будущий
authenticated history rewrap. Development key file рядом с projection не
является production at-rest protection. Полный контракт —
[`../docs/RFC-0005-local-encrypted-history-projection.md`](../docs/RFC-0005-local-encrypted-history-projection.md).

### M0.7.3 — authenticated persistent pairwise Double Ratchet: выполнено

Реализовано:

- новый `kilogram-ratchet` изолирует `vodozemac::olm` 0.10.0 и не раскрывает
  его private account/session types другим слоям;
- стабильные Olm Curve25519/Ed25519 identity keys и single-use OTK связаны с
  application Device ID отдельными domain-separated device signatures;
- connection ticket v7 переносит и проверяет listener `SignedPrekeyBundle`;
- encrypted account/session pickles атомарно сохраняются под
  `STATE_DIR/ratchet`, одна session адресуется peer Device ID;
- `RatchetText` event v4 заменил static recipient HPKE box на opaque Olm
  PreKey/Normal ciphertext; HPKE остался только у local-only projection;
- первое исходящее сообщение создаёт outbound session, первое входящее
  PreKey — inbound session и ротацию OTK, ответ и дальнейшие сообщения идут
  как Normal;
- delivery не повторяет ratchet decrypt для уже принятого event, а использует
  существующую authenticated projection;
- sync сортирует входящие ratchet texts по author sequence, сохраняет session и
  projection до event batch; `seed-history` принимает peer certificate и
  signed prekey bundle;
- добавлена команда `ratchet-bundle` для offline экспорта публичного bundle;
- event/sync/session/ticket/ALPN границы повышены до v4/v5/v5/v7/sync-5.

Проверки:

- real persistent cycle PreKey → Normal reply → Normal subsequent проходит с
  reload account/session между шагами;
- tampered bundle, ciphertext/event metadata и смена peer ratchet identity
  отклоняются;
- account/session/event/projection files не содержат fixture plaintext;
- sync трёх prekey-events в пустой recipient event store создаёт три читаемые
  local projections и остаётся идемпотентным;
- все 60 workspace tests, форматирование, строгий Clippy и release build
  проходят;
- release process smoke `.tmp/m073-smoke-20260831-232212` выполнил три direct
  delivery с перезапуском listener: PreKey → Normal reply → Normal subsequent,
  один session ID на обеих сторонах, одинаковые histories из 6 events и
  отсутствие трёх plaintext markers в 22 event/projection/account/session
  ciphertext files.

Граница среза: pickle key хранится рядом с encrypted state; файловые
ratchet/projection/event updates ещё не объединены одной транзакцией. Одна
session и один опубликованный OTK на peer Device ID не разрешают concurrent
session initiation и multi-device fan-out. Копии device signing key больше
недостаточно для расшифровки старой истории — нужны ratchet state backup либо
authenticated history rewrap. Полный контракт —
[`../docs/RFC-0006-pairwise-double-ratchet.md`](../docs/RFC-0006-pairwise-double-ratchet.md).

### M0.7.4 — signed multi-device ratchet fan-out: выполнено

Реализовано:

- `AccountDeviceListSnapshot` связывает полный authority snapshot с
  каноническим root-signed списком от 1 до 32 messaging devices; Root не
  подписывает два разных списка на одной authority revision;
- `AccountPrekeyDirectory` требует ровно один device-signed bundle для каждого
  certificate из списка без пропусков, дубликатов и outsiders;
- connection ticket v8 переносит весь directory, а `listen` принимает
  `--device-list-file` и повторяемый `--peer-prekey-bundle-file`, автоматически
  добавляя текущий bundle online listener;
- `RatchetText` event v5 содержит root-signed recipient device list и
  канонический ciphertext slot каждого устройства peer account; проверка event
  требует точного совпадения списка и slots;
- sender атомарно на уровне каждой файловой операции продвигает отдельную
  persistent Olm session на каждый Device ID и создаёт один общий signed event;
- online listener расшифровывает только свой slot, а другое устройство того же
  account получает immutable event через sync и создаёт свою local projection;
- sync/session/ticket/ALPN границы повышены до v6/v6/v8/`kilogram/m0/sync/6`;
- `seed-history` требует peer certificate, root-signed device list и prekey
  bundle, а `account-device-list` публикует список явно от Account Root.

Проверки:

- canonical/duplicate/revoked device lists и неполные prekey directories/events
  отклоняются;
- два устройства одного Bob account расшифровывают разные ciphertext одного
  event, а устройство без slot не может принять event в local history;
- все 63 workspace tests, форматирование, строгий Clippy и release build
  проходят;
- release process smoke `.tmp/m074-smoke-20260901-004422` доставил Alice →
  Bob-1 один event с двумя slots, затем Bob-2 получил event и acknowledgement
  через sync; histories всех трёх devices совпали, session counts равны 2/1/1,
  plaintext marker отсутствует в 16 event/projection/account/session files.

Граница среза: Root пока получает перечень certificate files вручную, public
prekey distribution не имеет discovery/freshness и использует один OTK.
Одновременная инициализация sessions и общая транзакция нескольких ratchets,
projection и event ещё не решены. Новый device не получает старые ciphertext
slots. Полный контракт —
[`../docs/RFC-0007-multi-device-ratchet-fanout.md`](../docs/RFC-0007-multi-device-ratchet-fanout.md).

### M0.7.5 — authenticated history rewrap: выполнено

Реализовано:

- same-account `HistoryRewrapBundle` связывает root-signed device list, source
  и recipient Device ID, conversation, digest полного source text inventory и
  явный canonical диапазон `[start, end)`;
- каждая entry содержит исходный `AuthorizedEvent`, inventory index и HPKE
  ciphertext на encryption key нового device; source Device signature покрывает
  manifest, event и ciphertext;
- bundle ограничен 256 text events и выводит
  `source_inventory_complete`, не выдавая source claim за глобальную полноту;
- `history-rewrap-export` открывает только проверенные local projections живого
  source, а `history-rewrap-import` повторно проверяет target account/device,
  membership и каждого event author;
- local projection v2 сохраняет rewrap manifest/entry/signature и открывается
  только с совпадающими local Account ID, Device ID и immutable event; direct
  projection v1 остаётся совместимой;
- import сохраняет исходный bundle в `STATE_DIR/history-rewraps`, projection,
  неизменённый event и authorization sidecar; перекрывающиеся bundles с тем же
  plaintext идемпотентны;
- существующая rewrapped projection позволяет обычному sync повторно принять
  старый event без отсутствующей у нового устройства ratchet session.

Проверки:

- invalid range, signature/ciphertext tampering и wrong recipient отклоняются;
- protocol round trip сохраняет `AuthorizedEvent`, plaintext и provenance;
- partial `1..2` помечен incomplete, полный `0..3` восстанавливает три events;
- source и recovered histories совпадают; удалённый после import event успешно
  восстановлен обычным authenticated sync через rewrapped projection;
- все 64 workspace tests, форматирование, строгий Clippy и release build
  проходят;
- release process smoke `.tmp/m075-smoke-20260901-020138` подтвердил partial/full
  import, wrong-account refusal, histories equality, sync reuse и отсутствие
  plaintext markers в 16 event/projection/rewrap files.

Граница среза: bundle передаётся файлом, source должен быть online и иметь все
нужные projections. `source_inventory_complete` — подписанное утверждение
конкретного source, не consensus checkpoint. Cross-account recovery,
multi-source reconciliation, user consent/SAS и общая storage transaction не
реализованы. Полный контракт —
[`../docs/RFC-0008-authenticated-history-rewrap.md`](../docs/RFC-0008-authenticated-history-rewrap.md).

### M0.7.6 — authenticated prekey pools и concurrent initiation: выполнено

Реализовано:

- `SignedPrekeyPool` публикует 1–64 (по умолчанию 16) independently consumable
  OTK, стабильную signed ratchet identity, generation, непрерывный sequence
  range и signed publication/expiry interval;
- `AccountPrekeyDirectory` v2 требует fresh pool для каждого устройства
  root-signed списка; ticket/signature domain повышены до v9, event v5 и
  sync/session v6 не менялись;
- `connect` и `sync` сохраняют max-seen pool каждого peer device; более старая
  generation, different pool той же generation, sequence/timestamp rollback и
  ratchet identity substitution отклоняются;
- после успешного inbound PreKey private OTK удаляется `vodozemac`, а устройство
  публикует следующую generation; истёкший current pool также ротируется
  автоматически;
- deterministic hash selector распределяет разные initiator Device IDs по
  entries, не превращая collision в повторное использование private key;
- persistent session record v2 хранит active и максимум одну retained session;
  crossed outbound X/Y выбирают одинаковый lexicographic-min active ID на обеих
  сторонах, losing session обслуживает только уже отправленные сообщения;
- session record v1 читается как confirmed legacy session и мигрирует при
  следующей записи;
- CLI добавил `ratchet-prekey-pool`; `listen` и `seed-history` принимают
  `--peer-prekey-pool-file`.

Проверки:

- signature/tamper, pool size, sequence continuity, expiry и automatic rotation;
- max-seen rollback/equivocation и смена ratchet identity fail-closed;
- simultaneous outbound unit cycle расшифровывает оба crossed first messages,
  сохраняет две sessions и сходится на одном active ID;
- все 69 workspace tests, форматирование, строгий Clippy и release build
  проходят;
- release process smoke `.tmp/m076-smoke-20260901-032421` создал Alice/Bob
  first messages до соединения, передал их 1/1 через sync, ротировал pools
  `generation 0 -> 1` и `sequence 0..15 -> 16..31`, отклонил stale pool,
  выполнил post-convergence delivery с одной retained session, получил
  одинаковые histories и не нашёл plaintext в 24 ciphertext state files.

Граница среза: ticket/file является authenticated M0 discovery boundary, но не
глобальным DHT/gossip freshness proof. Нет remote atomic OTK reservation,
session-reset protocol, TTL retained session или общей DB transaction. Полный
контракт —
[`../docs/RFC-0009-authenticated-prekey-pools.md`](../docs/RFC-0009-authenticated-prekey-pools.md).

### Следующий этап

1. M0.7.7: объединить ratchet advancement, local projection, immutable event и
   prekey rotation одной crash-consistent транзакцией и добавить state lock.
2. Затем связать history rewrap с сетевой device-to-device сессией, user consent
   и multi-source completeness reconciliation.
3. Membership removal и group governance проектировать вместе с ordered
   security events и MLS epoch.
4. First-contact authority/membership gossip-witness, seed/root recovery и
   compact Merkle/range summary остаются отдельными направлениями.
