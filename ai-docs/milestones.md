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

### M0.7.7 — crash-consistent local state: выполнено

Реализовано:

- новый `kilogram-state` получает exclusive OS file lock canonical
  `STATE_DIR` на всё время любой device CLI-команды; второй процесс fail-fast;
- journal v1 сохраняет backup mutable `ratchet` и `next-sequence`, а также
  baseline файлов append-only `events`, `local-messages` и `history-rewraps`;
- durable markers `prepared`, `committed` и `rolled-back` делают recovery
  идемпотентным даже при повторном падении во время rollback/cleanup;
- prepared без terminal marker восстанавливает mutable backup и удаляет только
  новые immutable files; committed никогда не откатывается;
- symlink и небезопасные relative paths в managed state отклоняются;
- delivery связывает decrypt/prekey consumption, projection, received event,
  acknowledgement sequence/event; connect связывает sequence, весь ratchet
  fan-out, authored projection и event до network send;
- sync materialization/store, `seed-history`, `history-rewrap-import`, prekey
  publication/rotation и peer pool high-water используют ту же transaction;
- ticket v9, event v5 и sync/session v6 не изменились.

Проверки:

- exclusive/reusable lock, operation-error rollback, next-startup recovery и
  interrupted committed-cleanup покрыты отдельными unit tests;
- CLI fault-injection test восстанавливает ratchet/sequence и удаляет новые
  event/projection files;
- все 75 workspace tests проходят; форматирование, строгий Clippy и release
  build проверены;
- локальный release process smoke `.tmp/m077-smoke-20260901-035517` подтвердил
  lock refusal (`exit=1`), reuse после освобождения (`exit=0`) и отсутствие
  оставленного active journal.

Граница среза: filesystem snapshot/baseline имеет стоимость `O(local state)` и
не заменяет production encrypted DB/WAL. Lock координирует только процессы,
соблюдающие контракт, не включает Account Root каталоги и не защищает от
hardware loss/hostile local administrator. Полный контракт —
[`../docs/RFC-0010-crash-consistent-local-state.md`](../docs/RFC-0010-crash-consistent-local-state.md).

### M0.7.8 — network authenticated history rewrap: выполнено

Реализовано:

- `HistoryRewrapSas` связывает полный root-signed same-account device list и
  направленные роли source/recipient; UI-код — 12 цифр, полный digest входит в
  signed request;
- recipient подписывает session-bound conversation/source/recipient/range/SAS
  request, source сверяет его с exact local approval;
- source подписывает transfer поверх точного request и прежнего HPKE rewrap
  bundle; recipient повторно проверяет request equality, signatures, device
  authority, conversation, range и SAS;
- CLI добавил `history-rewrap-sas`, consent options у `listen`,
  `history-rewrap-fetch` и `history-rewrap-reconcile`;
- frame остаётся bounded 8 MiB, request ограничен 1–256 events, oversized
  transfer получает явный `TransferTooLarge`;
- `.rewrap`, `.transfer`, imported projections и events сохраняются одной
  crash-consistent transaction;
- reconciliation объединяет ranges каждого source inventory claim, выявляет
  same-source equivocation и печатает `incomplete`/`single-source`/`agreed`/
  `divergent` плюс неизменное `global_completeness_proven=false`;
- ALPN повышен до `kilogram/m0/sync/7`; ticket v9, sync/session v6 и event v5
  сохранены;
- async command dispatch box-pinned, чтобы выросший future не переполнял 1 MiB
  Windows main-thread stack ещё до Clap parsing.

Проверки:

- protocol tests покрывают SAS role binding, session/request/transfer binding,
  oversized request и tampering;
- CLI tests покрывают exact source consent, same-account restriction,
  gap/overlap coverage и все reconciliation outcomes;
- все 78 workspace tests, форматирование, строгий Clippy и release build
  проходят;
- direct process smoke `.tmp/m078-smoke-20260901-044605` передал 3/3 старых
  events в source-signed transfer размером 4,674 bytes, восстановил читаемую
  recipient history, сохранил bundle+transfer, получил complete
  `single-source` и не нашёл plaintext marker в recipient state.

Граница среза: один listener обслуживает один bounded range; pagination, resume,
source discovery и автоматический сбор нескольких claims ещё не реализованы.
`agreed` — согласие наблюдавшихся sources, не глобальный checkpoint. Полный
контракт —
[`../docs/RFC-0011-network-history-rewrap.md`](../docs/RFC-0011-network-history-rewrap.md).

### M0.7.9 — resumable history recovery: выполнено

Реализовано:

- `SignedHistoryRecoveryCheckpoint` v1 подписывается recipient device и
  связывает account, conversation, направленные source/recipient, полный SAS,
  approved window, page size, source inventory claim и следующий индекс;
- immutable checkpoints образуют hash chain; при resume проверяются подпись,
  отсутствие gap/fork/regression и совпадение нового ticket с exact source;
- CLI `history-recovery-resume` сам вычисляет очередной диапазон, отправляет
  fresh session-bound request и после reconnect продолжает с durable index;
- source approval трактуется как полное окно, но каждая фактическая страница
  остаётся bounded 1–256 events и обязана лежать внутри consent window;
- первый source-signed transfer фиксирует inventory count/digest; смена claim
  на следующей странице fail-closed;
- bundle, transfer, projections, events и следующий checkpoint коммитятся одной
  crash-consistent transaction; `history-recovery` добавлен в append-only roots;
- завершённый plan повторно проверяет ticket/source/SAS, но не открывает network
  connection;
- для нескольких sources создаются отдельные явно выбранные plans, а
  reconciliation публикует `selected_inventory_*` только при совпадении минимум
  двух полных claims без equivocation;
- wire schema не менялась: ALPN остаётся `kilogram/m0/sync/7`, ticket v9.

Проверки:

- protocol test покрывает checkpoint signature, hash-link, advance/completion,
  wrong signer и повтор диапазона;
- state/CLI tests покрывают rollback checkpoint root и границы consent window;
- все 78 workspace tests проходят; форматирование, строгий Clippy и release
  build проверены;
- direct process smoke `.tmp/m079-smoke-20260901-051549` двумя fresh sessions
  импортировал `0..2`, затем `2..3`, создал два checkpoint, не подключался после
  completion и получил идентичные source/recipient histories.

Граница среза: новый listener/ticket пока запускается вручную для каждой
страницы; source discovery, background coordinator и QR/device-link ceremony не
реализованы. Signed local chain не обнаруживает удаление всей цепочки без
внешнего witness/backup, а agreed claims не доказывают глобальную полноту.
Полный контракт —
[`../docs/RFC-0012-resumable-history-recovery.md`](../docs/RFC-0012-resumable-history-recovery.md).

### M0.8.1 — encrypted transactional state vault: выполнено

Реализовано:

- `kilogram-state` использует embedded `redb` 4.2 и одной
  immediate-durability transaction сохраняет encrypted snapshot всего legacy
  device state;
- relative paths индексируются keyed BLAKE3, а path+content каждой записи
  шифруются XChaCha20-Poly1305 с отдельным nonce и domain-separated subkeys;
- keyed manifest фиксирует schema, record count, plaintext bytes и canonical
  snapshot ID; verify полностью расшифровывает records и пересчитывает его;
- `state-vault-migrate`, `state-vault-verify` и `state-vault-restore` работают
  под existing state lock; restore разрешён только в новый каталог и проходит
  staging verification до atomic rename;
- migration сохраняет legacy files, идемпотентна для неизменного state и
  fail-closed при drift; fault injection подтверждает невидимость transaction
  без commit;
- случайный 256-bit master key пока хранится в соседнем
  `state-vault.key`; это development key provider, не защита от компрометации
  всего локального аккаунта.

Проверки:

- state tests покрывают exact restore, отсутствие fixture plaintext/path в raw
  DB, abort до commit, legacy drift, wrong key и existing destination;
- все 81 workspace tests, rustfmt, строгий Clippy и release build проходят;
- release process smoke `.tmp/m081-smoke-20260901-070000` мигрировал 23 файла
  реального M0.7.9 recipient state (26,037 plaintext bytes), повторно получил
  `already-current`, проверил vault, восстановил byte-identical legacy tree и
  прочитал прежние 3 history events из restore; raw DB scan не нашёл fixtures.

Граница среза: vault пока является immutable shadow snapshot. Основные
repositories продолжают читать и писать legacy files; legacy plaintext/key
metadata не удаляются, master key не защищён OS keystore, согласованный rollback
DB+key не обнаруживается. Полный контракт —
[`../docs/RFC-0013-encrypted-transactional-state-vault.md`](../docs/RFC-0013-encrypted-transactional-state-vault.md).

### M0.8.2 — recoverable shadow dual-write: выполнено

Реализовано:

- публичный `StateMirrorRepository` отделяет begin/finish/recovery lifecycle от
  конкретных `redb` tables;
- vault metadata получает monotonic `mirror_generation`, exact snapshot ID и
  keyed-authenticated generation record с backward-compatible upgrade M0.8.1;
- перед каждой live device-state CLI-командой immediate transaction сохраняет
  authenticated intent к active generation/snapshot;
- после команды фактически committed legacy state атомарно зеркалируется в
  vault, intent удаляется в той же transaction, generation растёт только при
  реальном изменении;
- после crash M0.7.7 сначала откатывает/завершает filesystem journal, затем
  authenticated intent разрешает закончить mirror; drift без intent остаётся
  fail-closed;
- completion выполняется и после ошибки команды, потому что локальный commit
  мог предшествовать network failure; двойная ошибка сохраняет оба контекста;
- `state-vault-recover` явно восстанавливает только pending authenticated
  operation, а `verify`/`restore` отказываются считать active snapshot текущим
  до recovery;
- restore внутрь source `STATE_DIR` теперь запрещён.

Проверки:

- unit tests покрывают unchanged/changed generations, crash после intent,
  injected DB abort, retry, forged intent, external drift и CLI guard restart;
- все 83 workspace tests, rustfmt, strict Clippy и release build проходят;
- release smoke `.tmp/m082-smoke-20260901-080000` мигрировал 23 файла как
  generation 1, оставил её после read-only `identity`, после live prekey-pool
  rotation создал generation 2 (26,037 → 31,750 bytes), подтвердил verify/no
  pending recovery и восстановил byte-identical tree с теми же 3 history
  events; plaintext/path markers в raw DB не найдены.

Граница среза: legacy остаётся primary store, changed command пересобирает весь
encrypted snapshot за `O(state)`, read-only command пишет intent metadata.
Generation не является внешним rollback witness, а key остаётся соседним
development-файлом. Полный контракт —
[`../docs/RFC-0014-recoverable-shadow-dual-write.md`](../docs/RFC-0014-recoverable-shadow-dual-write.md).

### M0.8.3 — typed incremental shadow repositories: выполнено

Реализовано:

- active vault и legacy tree сравниваются по canonical relative path; mirror
  формирует точные upsert/remove/unchanged множества;
- changed/new records шифруются с новым nonce, удалённые keyed records
  удаляются, а неизменённые ciphertext values не переписываются;
- delta, manifest, generation и intent removal публикуются одной immediate
  `redb` transaction; fault сохраняет прежний active snapshot и pending intent;
- `VaultMirrorCommit` возвращает outcome/report и delta counters, которые
  печатаются live CLI как при normal completion, так и recovery;
- девять `StateRecordKind` покрывают identity, ratchet, events, local
  projections, rewrap, recovery, trust, sequence и явный `other` fallback;
- `TypedStateRepository` выполняет exact DB/legacy shadow comparison и
  возвращает per-kind record/byte inventory;
- `state-vault-shadow-read` проверяет и печатает все typed views без
  переключения primary read path.

Проверки:

- unit test подтверждает delta `2 upsert / 1 remove / 7 unchanged`, сохранение
  ciphertext неизменённой identity-записи, замену ratchet ciphertext, удаление
  event и typed mismatch для local projection;
- все 84 workspace tests, rustfmt, strict Clippy и release build проходят;
- release smoke `.tmp/m083-smoke-20260901-100000` на 23 реальных records дал
  `0/0/23` для `identity` и `2/0/21` для prekey rotation, generation 1→2;
  девять typed views совпали, restore дал byte-exact 23 files и прежние 3
  history events, raw DB scan не нашёл plaintext/path markers.

Граница среза: шифрование и DB mutations теперь `O(changed + deleted)`, но
сбор legacy, decrypt и exact compare остаются `O(state)`. Filesystem всё ещё
primary; master key лежит рядом с DB. Полный контракт —
[`../docs/RFC-0015-typed-incremental-shadow-repositories.md`](../docs/RFC-0015-typed-incremental-shadow-repositories.md).

### M0.8.4 — immutable vault primary-read canary: выполнено

Реализовано:

- `TypedStateRepository::read_primary_canary` возвращает owned DB records
  только для явно разрешённых `event` и `local-projection` kinds;
- перед выдачей bytes полностью проверяются vault/manifest/generation, matching
  intent и exact shadow всего retained legacy tree;
- stale legacy после intent блокирует DB read; empty и mutable kind selections
  отклоняются;
- `EventReadRepository` и `LocalMessageReadRepository` отделяют прикладное
  чтение от filesystem write implementation;
- strict immutable snapshots принимают DB bytes, проверяют canonical
  Conversation/Event path layout, duplicate paths, signatures, IDs,
  authorization/membership, writer sequence и projection binding;
- read-only `history` при инициализированном vault использует только snapshot,
  сообщает `encrypted-vault`/`legacy-verified` и не делает silent fallback;
- полностью немигрированный state сохраняет прежний legacy history path.

Проверки:

- state tests покрывают allowed/forbidden selection, matching intent, stale
  shadow refusal и recovery;
- store test удаляет исходные event/authorization/projection files после
  построения snapshot и всё равно читает verified history только из snapshot;
- CLI seeded-history test получает те же 3 events/3 projections через vault
  adapters под live intent;
- все 85 workspace tests, rustfmt, strict Clippy и release build проходят;
- release smoke `.tmp/m084-smoke-20260901-120000` прочитал generation 2,
  6 event/authorization records, 3 projections и прежние 3 сообщения из vault,
  завершил mirror `0/0/23`; изменённый projection shadow дал exit 1 typed
  mismatch без legacy fallback.

Граница среза: только `history` и только immutable reads переведены на DB.
Writes, sync, rewrap, ratchet/trust/sequence остаются legacy-primary; полный
shadow scan всё ещё `O(state)`. Полный контракт —
[`../docs/RFC-0016-immutable-vault-primary-read-canary.md`](../docs/RFC-0016-immutable-vault-primary-read-canary.md).

### M0.8.5 — vault-primary history rewrap sources: выполнено

Реализовано:

- один `ImmutableReadRepositories` выбирает legacy compatibility либо
  authenticated vault snapshot и публикует общие diagnostics;
- manual `history-rewrap-export` захватывает snapshot до authority update и
  строит inventory/bundle только через read traits;
- rewrap-enabled listener захватывает owned snapshot до authority/prekey,
  transport awaits, requester pinning и application request;
- listener без approval сохраняет прежний explicit `NotApproved`, не открывая
  canary;
- `build_history_rewrap_bundle` больше не зависит от concrete filesystem
  stores;
- `EventReadRepository: Send + Sync` получил authorized inventory и
  events-by-ID primitives, нужные будущему sync overlay;
- initialized vault при auth/shadow/layout/decode ошибке блокирует manual и
  network source без silent fallback.

Проверки:

- CLI fixture получает authorized inventory/events-by-ID из vault, временно
  убирает legacy event/projection directories и всё равно строит полный bundle
  из 3 entries;
- все 85 workspace tests, rustfmt, strict Clippy и release build проходят;
- release smoke `.tmp/m085-smoke-20260901-153149` создал manual bundle и
  передал network bundle из 3 events через authenticated direct Iroh session;
  оба source сообщили generation 1, 6 event/authorization records и 3
  projections, а post-transfer verify сохранил generation 1;
- изменённая legacy projection дала exit 1 и `silent_fallback=false`.

Граница среза: `history` и оба rewrap source-history paths теперь DB-primary,
но delivery/sync/import и все writes остаются legacy-primary. Snapshot начала
команды нельзя напрямую использовать в mixed sync. Полный контракт —
[`../docs/RFC-0017-vault-primary-history-rewrap.md`](../docs/RFC-0017-vault-primary-history-rewrap.md).

### M0.8.6 — command-local sync read overlay: выполнено

Реализовано:

- sync client и listener захватывают authenticated immutable read-set до своих
  command-local authority/prekey/trust mutations;
- `CommandEventReadOverlay` объединяет vault/filesystem base с только успешно
  committed events, повторно проверяя membership, authorization, Event ID и
  writer sequence;
- `CommandLocalMessageReadOverlay` даёт overlay-first lookup и отклоняет
  conflicting projection для одного Event ID;
- staged records невидимы до завершения M0.7.7 filesystem transaction;
- `DecryptingSessionStore` разделяет legacy writes и overlay reads, поэтому
  каждый следующий bounded round видит предыдущий committed batch;
- sync diagnostics публикуют physical primary/shadow, vault generation/record
  counts и размеры event/projection overlay;
- initialized invalid/stale/drifted vault блокирует sync без silent fallback;
  never-migrated state сохраняет legacy compatibility.

Проверки:

- store test доказывает stage invisibility, commit visibility, frontier merge и
  writer-sequence conflict;
- CLI tests проверяют два последовательных batches поверх пустого immutable
  vault base, idempotent retry и нулевой overlay после transaction failure;
- все 85 workspace tests, rustfmt, strict Clippy и release build проходят;
- release smoke `.tmp/m086-smoke-20260901-160635` передал 73 события rounds
  `64 + 9`, listener сообщил encrypted-vault и overlay `73/73`, итоговые
  vault-primary histories совпали, peer vault достиг generation 2;
- projection shadow drift дал exit 1 и `silent_fallback=false`.

Граница среза: overlay command-local и не заменяет persistent storage. Sync
reads теперь DB-primary, но events/projections и весь mutable state сначала
пишутся в retained legacy tree. Полный контракт —
[`../docs/RFC-0018-command-local-sync-read-overlay.md`](../docs/RFC-0018-command-local-sync-read-overlay.md).

### M0.8.7 — vault-primary transaction checkpoint: выполнено

Реализовано:

- каждая M0.7.7 `StateTransaction` при initialized vault сначала коммитит exact
  staged delta одной immediate-durability redb transaction;
- один atomic DB commit публикует encrypted records, manifest, новую generation,
  rotated outer mirror intent и authenticated primary-shadow intent;
- filesystem journal после этого публикует retained legacy shadow, а exact
  comparison предшествует очистке primary marker;
- delivery/acknowledgement, sync batches, seed-history, history import/recovery
  и prekey/ratchet operations используют общий `PendingVaultPrimaryWrite`;
- event/projection фиксируются атомарно со связанными ratchet/sequence changes;
- crash до DB commit оставляет старый vault и откатывает staging, crash после DB
  commit восстанавливает shadow из vault;
- active filesystem journal блокирует преждевременную recovery, unsafe paths,
  symlinks и forged marker отклоняются fail-closed;
- network frames и успешный CLI result появляются только после vault commit и
  shadow confirmation;
- одна команда с несколькими transactions может увеличить generation несколько
  раз; final compatibility mirror обычно `already-current`.

Проверки:

- state fault test проверяет невидимость aborted redb delta, crash между DB и
  shadow commit, восстановление event+ratchet, идемпотентность, последующий
  legacy mirror и forged marker;
- CLI multi-batch test получил generation 3 после двух sync batches вместо
  единственного command-end mirror;
- все 86 workspace tests, rustfmt, strict Clippy и release build проходят;
- release smoke `.tmp/m087-smoke-20260901-170711` выполнил direct delivery,
  получил source generation 4 и listener generation 3; commit/confirmation
  предшествовали `sent_event_id`/`received_event_id`, final mirrors остались
  `already-current`, обе DB-primary histories подтвердили event и ack.

Граница среза: filesystem остаётся staging input и shadow, full scan `O(state)`.
Mutable read adapters и нетранзакционные trust updates ещё не DB-primary.
Полный контракт —
[`../docs/RFC-0019-vault-primary-transaction-checkpoint.md`](../docs/RFC-0019-vault-primary-transaction-checkpoint.md).

### M0.8.8 — typed journal delta и mutable sequence canary: выполнено

Реализовано:

- active `StateTransaction` формирует typed mutations только для journal-managed
  ratchet, sequence и append-only roots;
- changed/added/removed ratchet records сравниваются с backup, sequence — с
  единственным backup record, а payload читается только у новых append-only
  paths;
- baseline append-only removal, duplicate/unsafe path, kind mismatch,
  неразрешённый repository kind и transaction от другого state root
  отклоняются;
- `commit_primary_transaction` применяет journal mutations к authenticated
  DB-owned record set и атомарно публикует delta/manifest/generation и оба
  intents без полного legacy payload scan;
- bounded compatibility ingress включает текущие authority/certificate,
  membership и peer-authority records в ту же transaction, пока эти writers
  ещё не переведены на journal; удаление committed trust record запрещено;
- live CLI `run_state_transaction` и `run_store_transaction` используют direct
  path; старый full checkpoint сохранён только как compatibility primitive;
- `VaultMutableRead` и `read_mutable_primary_canary` открывают пока только
  `sequence` при exact active outer intent;
- `CommandTransactionContext` лениво читает `next-sequence` из vault и ведёт
  локальный cursor, а `DeviceState::allocate_sequence_from` пишет retained
  transactional shadow;
- sequence по-прежнему коммитится атомарно с event/projection/ratchet, а
  never-migrated state сохраняет filesystem behavior.

Проверки:

- state tests проверяют точный typed write-set, append-only removal rejection,
  DB-primary sequence при изменённом shadow, injected direct-delta abort,
  точные counters `4/1/1` с новым trust record, pending-marker block и
  root/kind rejection;
- CLI test после authenticated intent меняет shadow counter `2 -> 99`, но
  allocator возвращает DB value `2`, публикует shadow `3`, generation `1 -> 2`
  и final mirror `already-current`;
- все 90 workspace tests, rustfmt, strict Clippy и release build проходят;
- release process smoke `.tmp/m088-smoke-20260901-185655` выполнил Alice/Bob
  delivery и acknowledgement с source/listener generations `4/3`, DB-primary
  sequence, typed journal delta и совпадающими final vault/legacy histories.

Граница среза: append-only directories ещё перечисляются, ratchet сравнивается
с bounded backup, active DB records полностью decrypt/re-hash для manifest, а
post-commit exact confirmation читает retained tree. Ratchet/trust mutable
reads пока filesystem-backed. Полный контракт —
[`../docs/RFC-0020-typed-journal-delta-and-mutable-sequence.md`](../docs/RFC-0020-typed-journal-delta-and-mutable-sequence.md).

### M0.8.9 — DB-primary ratchet workspace и registered appends: выполнено

Реализовано:

- mutable vault canary разрешает `ratchet` вместе с `sequence` только при exact
  active outer intent и без pending primary-shadow marker;
- transaction context гидратирует retained ratchet staging из authenticated DB
  до открытия `RatchetState`; mutation сравнивается с DB baseline;
- отдельный durable primary backup ratchet/sequence публикуется в crash manifest
  до изменения live shadow, поэтому rollback возвращает DB-authoritative state;
- listener, delivery, connect, sync, prekey export и seed-history больше не
  загружают production `RatchetState` вне transaction context;
- sync получает свежее vault generation для каждого committed batch, не держит
  filesystem-backed ratchet между rounds;
- append-only event, authorization, projection, rewrap, transfer и checkpoint
  paths явно регистрируются до записи;
- direct delta проверяет baseline existence и читает только registered paths;
  второй append-only directory walk удалён;
- committed append-only modification-in-place и removal отклоняются fail-closed;
- diagnostics сообщают `typed-registered-delta`, ratchet workspace status и
  точный размер append write-set.

Проверки:

- state/vault tests проверяют DB hydration поверх изменённого shadow, durable
  primary rollback ratchet+sequence, registered-only delta, wrong kind/root,
  append removal и modification rejection;
- CLI test после открытия authenticated intent повреждает ratchet secret,
  восстанавливает его из DB и коммитит registered canary event;
- все 92 workspace tests, rustfmt, strict Clippy и release build проходят;
- release process smoke `.tmp/m089-smoke-20260901-193740` выполнил Alice/Bob
  delivery+ack с DB-primary ratchet/sequence, append write-set `3/5`, source/
  listener generations `4/3`, совпавшей history и final `already-current`.

Граница среза: vodozemac всё ещё пишет в filesystem staging, initial append
baseline и post-commit exact confirmation сканируют retained tree, active DB
manifest полностью decrypt/re-hash, trust repository filesystem-backed. Полный
контракт —
[`../docs/RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md`](../docs/RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md).

### Следующий этап

1. M0.8.10 перенести typed write receipts из CLI в domain repositories и
   добавить authenticated incremental manifest index, чтобы normal commit не
   зависел от full-state scan.
2. Защитить vault master key через OS keystore/passphrase/seed wrapping и
   спроектировать rollback witness, backup и versioned migrations.
3. Source discovery/background coordinator и QR/device-link UX строить поверх
   M0.7.9 без ослабления explicit consent.
4. Membership removal и group governance проектировать вместе с ordered
   security events и MLS epoch.
5. First-contact authority/membership gossip-witness, seed/root recovery и
   compact Merkle/range summary остаются отдельными направлениями.
