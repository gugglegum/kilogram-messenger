# Технические этапы

Актуально на: 2026-09-04.

## Правило нумерации и отчётов

- Канонический номер этапа задаётся этим файлом и соответствующим Git-коммитом;
  номера не резервируются и не пропускаются молча.
- В начале работы нужно назвать последний завершённый этап и ровно один
  следующий target. Если один turn фактически закрывает несколько этапов, в
  итоговом сообщении перечисляются все номера и коммиты, а не только последний.
- В финале каждого этапа указываются его номер, commit hash и номер следующего
  target. Package/binary SemVer не смешивается с product milestone `M`.

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

### M0.8.10 — repository receipts и authenticated manifest index: выполнено

Реализовано:

- `kilogram-store` возвращает `AppendOnlyWriteReceipt` из single/batch event,
  authorization и local-projection writes;
- history bundle/transfer/checkpoint writers возвращают тот же exact receipt;
- transaction канонизирует receipt path, требует exact state root и повторно
  применяет append-only kind/symlink allowlist;
- CLI больше не строит `.event`, `.authorization` и `.local-text` paths;
- vault schema v2 хранит AEAD-encrypted sorted path/length/content-hash index;
- direct journal commit обновляет index/manifest/delta без enumeration или
  decrypt неизменённых DB payload records;
- schema-v1 vault полностью проверяется и перестраивает index один раз без
  обязательного re-encrypt unchanged record envelopes;
- final exact DB/index/shadow confirmation и primary-shadow recovery marker
  сохранены без ослабления;
- diagnostics публикуют `vault_manifest_index_mode`,
  `vault_payload_records_loaded` и exact receipt path count.

Проверки:

- store/state tests проверяют exact receipts, чужой root, schema-v1 upgrade,
  index AEAD tamper и `payload_records_loaded=0` для schema-v2 direct commit;
- прежние atomic delta, injected failure, rollback, immutable modification и
  CLI ratchet/sequence tests проходят;
- все 93 workspace tests, rustfmt, strict Clippy и release build проходят;
- release process smoke `.tmp/m0810-smoke-20260901-201413` выполнил два
  Alice/Bob delivery+ack, получил одинаковые histories из 4 events и валидные
  vault schema v2 с 21 record на каждой стороне; repository receipts составили
  `3/2` paths у sender и `5` у listener, а каждый normal direct commit сообщил
  `vault_manifest_index_mode=incremental` и `vault_payload_records_loaded=0`.

Граница среза: normal direct commit больше не читает unchanged DB payload, но
цельный index metadata остаётся `O(record count)`. Pre-command exact gate,
initial crash baseline и post-commit confirmation всё ещё full-state. Trust
repository filesystem-backed. Полный контракт —
[`../docs/RFC-0022-repository-write-receipts-and-manifest-index.md`](../docs/RFC-0022-repository-write-receipts-and-manifest-index.md).

### M0.8.11 — DB-primary trust repository: выполнено

Реализовано:

- `TrustStateRepository` читает authenticated certificate, own/peer authority
  и conversation-membership records из vault;
- schema-v2 path использует encrypted manifest index и decrypt-ит только
  выбранные trust payloads, повторно сверяя path/length/hash;
- initialized vault не делает silent filesystem fallback при отсутствующей или
  повреждённой trust record;
- production trust reads в listen/connect/sync/history/seed/rewrap/recovery
  переведены на repository snapshot;
- certificate, own authority, peer pin и membership writes явно подготавливают
  DB-primary trust workspace внутри `StateTransaction`;
- durable primary backup и rollback/next-start recovery восстанавливают
  DB-authoritative trust baseline;
- direct commit больше не перечисляет retained trust directories и принимает
  только typed mutations подготовленного workspace;
- compatibility checkpoint и final mirror отклоняют любой
  незарегистрированный filesystem `Trust` delta;
- sync projection path получает проверенный Account ID и не перечитывает
  certificate из filesystem.

Проверки:

- state tests проверяют trust hydration поверх tampered shadow, совместный
  ratchet/sequence/trust rollback и interrupted recovery;
- отдельные regressions доказывают, что unregistered filesystem trust change
  не попадает ни в direct DB transaction, ни в final full-snapshot commit;
- CLI regression читает валидные authority/membership bytes из DB после
  повреждения shadow и транзакционно восстанавливает retained snapshot;
- все 95 workspace tests, rustfmt, strict Clippy и release build проходят;
- release process smoke `.tmp/m0811-smoke-20260901-204423` выполнил fresh
  Alice/Bob delivery+ack с DB-primary certificate/membership reads и двумя
  explicit peer-authority Trust upserts по одному record; обе histories
  совпали на 2 events, schema-v2 vault содержат по 16 records, generations
  sender/listener `5/4`, каждый direct commit сообщил
  `vault_payload_records_loaded=0`.

Граница среза: retained filesystem остаётся compatibility shadow, pre-command
и post-commit exact gates всё ещё full-state, encrypted index пока цельный
`O(record count)` blob, master key лежит рядом с DB. Полный контракт —
[`../docs/RFC-0023-db-primary-trust-repository.md`](../docs/RFC-0023-db-primary-trust-repository.md).

### M0.8.12 — protected vault key provider: выполнено

Реализовано:

- `state-vault.key` стал versioned envelope с magic, version, provider и
  bounded protected payload;
- Windows build использует DPAPI CurrentUser с запрещённым UI prompt и хранит
  на диске только protected blob;
- fresh 256-bit master key никогда не записывается открытым в persistent file;
- ровно 32-byte legacy key атомарно rewrap-ится без смены ключа и re-encryption
  существующего vault;
- invalid magic/version/provider, tampered DPAPI blob и missing key
  обрабатываются fail-closed до открытия redb;
- plaintext/DPAPI buffers и in-memory master key zeroize-ятся;
- non-Windows compatibility provider использует тот же envelope, но честно
  сообщает `plaintext-development`;
- CLI выводит format, protection provider и `created|legacy-migrated|already-current`.

Проверки:

- state regression покрывает fresh/reopen, migration существующего raw key,
  сохранение старого DB key, invalid magic, tampered DPAPI blob и wrong key;
- все 96 workspace tests, rustfmt, strict Clippy и release build проходят;
- Windows release process smoke `.tmp/m0812-key-smoke-20260901-220000`
  скопировал M0.8.11 Alice vault generation 5 с 16 records, мигрировал key file
  `32 -> 282` bytes с magic `KILOGRAM-VAULTK1`, сохранил тот же snapshot ID
  `a81942b5e5935c02514617aa605d79bd74dcb2b6ccf2b1a03570aae9d7ee2da8`
  и при повторном verify сообщил `already-current`.

Граница среза: DPAPI защищает offline key at rest, но не от процесса с
полномочиями того же Windows user. Envelope пока не переносим на другую машину,
secure erase legacy blocks не гарантируется, non-Windows provider остаётся
development, rollback witness и bounded key backup/restore отсутствуют. Полный
контракт —
[`../docs/RFC-0024-protected-vault-key-provider.md`](../docs/RFC-0024-protected-vault-key-provider.md).

### M0.8.13 — portable vault-key recovery and rollback witness: выполнено

Реализовано:

- `state-vault-key-export` создаёт только новый внешний recovery package и не
  разрешает path внутри canonical `state-dir`;
- package имеет bounded versioned format `KILOGRAM-VRECOV1`, Argon2id v0x13
  profile `m=65536 KiB,t=3,p=1` и XChaCha20-Poly1305 AEAD;
- passphrase читается из bounded не-symlink файла, один конечный LF/CRLF
  снимается, секретные buffers zeroize-ятся;
- encrypted plaintext содержит vault master key, schema, generation и snapshot
  ID, а полный header входит в AAD;
- export выполняет DB-only verify и публикует package через same-directory
  temporary + fsync + no-clobber persist;
- `state-vault-key-import` допускает missing/broken local envelope, но сначала
  открывает и полностью аутентифицирует DB candidate key;
- DB generation ниже witness отклоняется как rollback, другая ветка того же
  generation — как fork; более новая валидная DB принимается;
- local key file изменяется только после всех проверок и снова заворачивается
  текущим provider, на Windows — DPAPI CurrentUser.

Проверки:

- state regressions покрывают roundtrip, внешний output/no-clobber, short/wrong
  passphrase, ciphertext tamper, package другого vault и byte-exact отсутствие
  key mutation при ошибке;
- отдельный regression создаёт валидные ветки с общим master key и проверяет
  rollback `witness=2,DB=1` и divergent snapshots при generation 2;
- все 98 workspace tests, rustfmt, strict Clippy и release build проходят;
- Windows release process smoke `.tmp/m0813-recovery-smoke-20260901-230000`
  экспортировал 148-byte package из реального schema-v2 vault generation 5 с
  16 records, удалил локальный envelope, восстановил новый 282-byte DPAPI
  envelope с magic `KILOGRAM-VAULTK1` и сохранил snapshot ID
  `a81942b5e5935c02514617aa605d79bd74dcb2b6ccf2b1a03570aae9d7ee2da8`.

Граница среза: package является external snapshot witness, а не глобальным
monotonic service. Согласованный rollback DB вместе со старым package не
обнаруживается; passphrase file lifecycle и backup storage остаются
ответственностью пользователя. Production macOS/Linux local provider,
master-key rotation и защита остальных filesystem secrets не реализованы.
Полный контракт —
[`../docs/RFC-0025-portable-vault-key-recovery-and-rollback-witness.md`](../docs/RFC-0025-portable-vault-key-recovery-and-rollback-witness.md).

### M0.8.14 — DB-primary device identity: выполнено

Реализовано:

- `DeviceIdentityStateRepository` читает immutable signing/encryption identity
  из authenticated active vault generation;
- schema-v2 path проверяет encrypted manifest index и decrypt-ит только две
  записи kind `DeviceIdentity`; schema v1 сохраняет full-verify compatibility;
- initialized vault не делает silent filesystem fallback при missing,
  unexpected, wrong-length или повреждённой identity record;
- `DeviceState::from_secret_material` строит identity без чтения/создания raw
  shadow, а временные secret buffers zeroize-ятся;
- все 17 production `listen/connect/sync/identity/enroll/authorize/history/`
  `ratchet/rewrap/recovery/seed` call sites используют единый DB-primary loader;
- device identity не добавлена в direct mutation API: key rotation остаётся
  отдельной authority-операцией.

Проверки:

- identity regression доказывает отсутствие filesystem I/O конструктора из
  caller-authenticated bytes;
- state regression покрывает tampered shadow, missing second record и AEAD
  corruption выбранного identity ciphertext;
- CLI regression сохраняет исходные device/encryption IDs при подмене обоих
  shadow-файлов после открытия authenticated mirror intent;
- все 101 workspace test, rustfmt, strict Clippy и release build проходят;
- Windows release process smoke `.tmp/m0814-identity-smoke-20260901-234500`
  запустил `identity` на копии реального schema-v2 vault, получил source
  `db-primary`, generation 5, две identity records и прежние device ID /
  encryption public key; pre/post verify сохранили 16 records и snapshot
  `a81942b5e5935c02514617aa605d79bd74dcb2b6ccf2b1a03570aae9d7ee2da8`,
  а compatibility delta осталась нулевой.

Граница среза: raw `device-secret.key` и `device-encryption-secret.key` пока
физически остаются retained compatibility shadow и проверяются full-state
pre-command/final gate. Поэтому M0.8.14 устраняет чувствительное прикладное
чтение, но ещё не улучшает at-rest confidentiality этих двух копий. Полный
контракт —
[`../docs/RFC-0026-db-primary-device-identity.md`](../docs/RFC-0026-db-primary-device-identity.md).

### M0.8.15 — DB-only device identity layout: выполнено

Реализовано:

- vault schema v3 аутентифицированно фиксирует, что `DeviceIdentity` records
  являются DB-only и больше не принадлежат normal retained shadow;
- schema-v1/v2 upgrade сначала проверяет DB и exact non-identity shadow,
  атомарно публикует schema v3 со следующей generation и только затем удаляет
  совпадающие `device-secret.key`/`device-encryption-secret.key`;
- уже отсутствующий raw key не приводит к генерации новой identity или
  удалению DB record; interrupted cleanup можно безопасно повторить;
- matching raw copy, появившаяся во время mirror intent, удаляется final gate,
  mismatched copy не импортируется и завершает команду fail-closed;
- effective snapshot объединяет filesystem non-identity shadow с immutable DB
  identity, поэтому checkpoint/mirror не воспринимает отсутствие файлов как
  mutation;
- primary-shadow recovery не восстанавливает identity plaintext и удаляет
  случайно появившиеся reserved copies;
- typed shadow diagnostics сообщают `device-identity records=0`, хотя полный
  authenticated report продолжает учитывать обе DB records;
- explicit `state-vault-restore` сохраняет recovery/export semantics и поэтому
  создаёт чувствительный legacy plaintext output, но normal command path этого
  не делает.

Проверки:

- state regressions покрывают v2 → v3 upgrade при уже отсутствующем втором raw
  key, повторное завершение удаления, mismatched reappearance, cleanup внутри
  active mirror и DB-primary recovery без восстановления keys;
- CLI regression подтверждает прежние device/encryption IDs после физического
  удаления обоих files и отсутствие filesystem fallback;
- все 102 workspace tests, rustfmt, strict Clippy и release build проходят;
- Windows release process smoke
  `.tmp/m0815-identity-retirement-smoke-20260902-002043` обновил копию
  реального schema-v2 vault generation 5 до schema v3 generation 6, сохранил
  16 records, 5654 plaintext bytes, прежний snapshot ID
  `a81942b5e5935c02514617aa605d79bd74dcb2b6ccf2b1a03570aae9d7ee2da8`,
  device ID и encryption public key; оба raw identity files отсутствуют,
  повторный migrate сообщил `already-current`, typed shadow —
  `device-identity records=0`.

Граница среза: удаление filesystem name не обещает secure erase SSD blocks,
journal, backup или cloud-sync history. Остальные compatibility shadows,
account-root storage, ratchet pickle protection и production non-Windows key
provider остаются отдельными задачами. Полный контракт —
[`../docs/RFC-0027-db-only-device-identity-layout.md`](../docs/RFC-0027-db-only-device-identity-layout.md).

### M0.9.1 — bounded multi-page history recovery session: выполнено

Реализовано:

- `history-recovery-resume` по умолчанию переносит до 64 страниц через один
  ticket, одну device authorization и одно Iroh connection;
- `--max-pages=1..64` ограничивает работу coordinator, `--page-size=1..256`
  сохраняет прежний per-transfer wire bound;
- source listener принимает только смежные ranges внутри exact consent window,
  использует один immutable DB-primary snapshot и завершает plan при достижении
  меньшего из approved range end и source inventory count;
- source session имеет hard cap 64 и 60-second межстраничный timeout; штатное
  закрытие recipient connection превращается в resumable pause;
- recipient проверяет и атомарно коммитит каждую страницу вместе с новым signed
  checkpoint до запроса следующей, поэтому ошибка не теряет предыдущий прогресс;
- достижение client limit возвращает `history-recovery-session-paused`, а
  завершение — `history-recovery-complete`;
- wire objects, ticket v9 и ALPN `/7` не изменились; старый recipient безопасно
  использует новый source, а новый recipient со старым source сохраняет первую
  страницу и требует свежий ticket для продолжения;
- CLI coordinator запускается в отдельном 8 MiB stack thread, чтобы крупный
  debug async state machine не падал до command dispatch.

Проверки:

- unit regression проверяет contiguous next range, completion stop и hard cap;
- все 103 workspace tests, rustfmt, strict Clippy и release build проходят;
- direct process smoke `.tmp/m091-smoke-20260902-004647` одним authenticated
  connection передал ranges `0..2` и `2..3`, создал два checkpoint и получил
  одинаковую verified history на source и recipient.

Граница среза: source по-прежнему явно запускает listener и передаёт ticket;
автоматического discovery, постоянного scheduler и QR/device-link ceremony нет.
Полный контракт —
[`../docs/RFC-0028-bounded-multi-page-history-recovery-session.md`](../docs/RFC-0028-bounded-multi-page-history-recovery-session.md).

### M0.9.2 — signed history recovery device link: выполнено

Реализовано:

- source listener выпускает compact signed
  `kilogram://history-recovery/v1/...` descriptor для exact recipient;
- descriptor связывает endpoint, source certificate, root-signed device list,
  conversation, approved range, page size, route policy и короткий expiry;
- prekey pools из ticket v9 в link не входят, поэтому реальный payload занял
  1202 bytes при hard limit 2953 bytes и готов для QR byte mode;
- `history-recovery-link-inspect` полностью проверяет Root/source signatures,
  bounds и expiry без сетевого подключения;
- `history-recovery-link-accept` требует exact local recipient certificate,
  совпадающий conversation ID и ручное подтверждение SAS до открытия сети;
- listener после подключения всё равно независимо проверяет device proof и
  своё exact consent window; link не является bearer capability;
- legacy `history-recovery-resume` с ticket остаётся совместимым, wire objects,
  ticket v9 и ALPN `/7` не изменены.

Проверки:

- unit regression покрывает round-trip, expiry, wrong recipient и tampering;
- direct process smoke `.tmp/m092-smoke-20260902-010717` проверил offline
  inspect, wrong-device rejection до сети, один authenticated connection, две
  atomic pages, два checkpoint и identical source/recipient history;
- все 104 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: payload готов для QR, но renderer/scanner, OS deep link,
authenticated descriptor publication/discovery и background scheduler ещё не
реализованы. Полный контракт —
[`../docs/RFC-0029-signed-history-recovery-device-link.md`](../docs/RFC-0029-signed-history-recovery-device-link.md).

### M0.9.3 — bounded history recovery QR ceremony: выполнено

Реализовано:

- listener может сразу записать signed recovery descriptor в no-clobber PNG
  через `--history-recovery-qr-file`, без обязательного промежуточного text file;
- отдельная `history-recovery-link-qr-render` создаёт тот же PNG из проверенной
  URI или link file;
- стандартный QR использует EC level L, quiet zone 4 modules и 4 pixels/module;
  точный URI limit 2953 bytes помещается в Version 40;
- inspect/accept принимают mutually exclusive `--link`, `--link-file` или
  `--qr-file` и после decode используют один SignedHistoryRecoveryLink verifier;
- decoder принимает только PNG/JPEG content до 16 MiB и 4096×4096, задаёт
  64 MiB image allocation budget и требует ровно один QR;
- QR output публикуется atomic no-clobber; несколько найденных QR отвергаются
  как ambiguous вместо выбора первого;
- wire, ticket v9, URI v1, ALPN `/7`, device authentication, SAS consent и
  per-page recovery contract не изменились.

Проверки:

- unit regressions покрывают PNG/JPEG round-trip, no-clobber, wrong prefix,
  exact Version 40 maximum, ambiguous multi-QR и oversized payload/file/dimension;
- direct process smoke `.tmp/m093-smoke-20260902-012850` создал 1202-byte link,
  Version 25 PNG размером 13263 bytes, независимо перерисовал/декодировал его и
  отклонил overwrite;
- wrong device отклонён после QR decode до сети; exact recipient одним
  authenticated connection перенёс две atomic pages, создал два checkpoint и
  получил identical history;
- все 109 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: decoder работает с image file, но live camera/clipboard, GUI
confirmation screen и OS deep-link handler отсутствуют. Publication/discovery
и background scheduler также остаются следующими задачами. Полный контракт —
[`../docs/RFC-0030-bounded-history-recovery-qr-ceremony.md`](../docs/RFC-0030-bounded-history-recovery-qr-ceremony.md).

### M0.9.4 — authenticated LAN history recovery discovery: выполнено

Реализовано:

- source listener только по `--history-recovery-discovery-publish` рассылает
  точную signed recipient-specific URI на `239.255.75.71:45371` с TTL 1 и
  дополнительным same-host loopback;
- публикация повторяется каждые 750 ms только пока живёт consent-gated listener;
- `history-recovery-link-discover` слушает `1..=30` s, обрабатывает до 512
  датаграмм и собирает до 16 unique candidates;
- каждый candidate проходит Root/source signature, expiry, exact local
  recipient/conversation/membership, optional source и local authority
  anti-rollback/equivocation проверки;
- scan не открывает Iroh connection и явно сообщает
  `history_recovery_discovery_user_consent=not-granted`;
- один однозначный candidate можно сохранить no-clobber, а ноль, несколько или
  достигнутый cap не приводят к автоматическому выбору;
- recovery начинается только отдельным прежним `history-recovery-link-accept`
  с ручным SAS, после чего source повторно проверяет device и local consent;
- wire, ticket v9, URI v1, QR contract и checkpoint format не изменились.

Проверки:

- новый unit test покрывает opt-in publication, bounded receive и deduplication;
- direct process smoke `.tmp/m094-smoke-20260902-015219` подтвердил multicast
  join, три датаграммы и два дедуплицированных повтора;
- wrong device получил ноль candidates и не создал connection;
- exact recipient обнаружил один candidate без connection, затем отдельным
  accept перенёс две atomic pages по одному authenticated connection;
- получены два checkpoint и identical source/recipient history;
- все 110 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: LAN descriptor виден локальным наблюдателям и функция default-off.
Wide-area privacy-preserving discovery, multi-client socket semantics, GUI
picker, background scheduler и network/power policy не реализованы. Полный
контракт —
[`../docs/RFC-0031-authenticated-lan-recovery-discovery.md`](../docs/RFC-0031-authenticated-lan-recovery-discovery.md).

### M0.9.5 — consent-bound history recovery retry coordinator: выполнено

Реализовано:

- `history-recovery-plan-approve` после exact recipient/conversation/
  membership/authority/SAS preflight создаёт no-clobber recipient-signed plan;
- plan v1 до 64 KiB связывает exact device list, source/recipient, полный SAS,
  conversation, range, page size, route policy, execution policy и expiry
  `1..=168` hours;
- default разрешает Ethernet/Wi-Fi и battery, запрещает mobile/unknown;
  `--deny-ethernet`, `--deny-wifi`, `--allow-mobile`,
  `--allow-unknown-network`, `--require-external-power` подписываются в plan;
- `history-recovery-plan-run` получает caller-supplied network/power context и
  blocked policy завершает до discovery/connection;
- runner выполняет до 8 attempts, discovery `1..=30` s и fixed delay
  `0..=300` s, сохраняя M0.9.4 datagram/candidate bounds;
- source restart и новый Endpoint ID допустимы, только если свежая signed URI
  exact-match plan по device-list/source/recipient/SAS/conversation/range/page/
  route; новая authority revision требует нового approval;
- committed pages/checkpoints переживают failure и используются новой попыткой;
- runner не держит state lock/vault intent во время discovery/backoff; короткие
  exclusive sections защищают preflight, active transfer и checkpoint check;
- это bounded CLI coordinator, не OS daemon и не автоматический network sensor;
- wire, URI v1, ticket v9, ALPN `/7` и checkpoint format не изменились.

Проверки:

- unit tests покрывают recipient signature/round-trip/expiry, fresh endpoint
  matching и fail-closed network/power matrix;
- direct process smoke `.tmp/m095-smoke-20260902-022156` отклонил mobile до UDP,
  получил `no-candidate` на первой разрешённой попытке и запустил foreground
  identity на том же state во время backoff;
- после source restart attempt 2 обнаружил новый Endpoint ID, открыл ровно один
  authenticated connection и перенёс две atomic pages;
- два checkpoint и source/recipient history совпали byte-for-byte;
- все 112 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: network/power context передаёт caller, retry state между
процессами не сохраняется, backoff fixed без jitter, lookup остаётся LAN-only.
OS background task/service, trusted platform adapters и wide-area discovery не
реализованы. Полный контракт —
[`../docs/RFC-0032-consent-bound-history-recovery-scheduler.md`](../docs/RFC-0032-consent-bound-history-recovery-scheduler.md).

### M0.9.6 — persistent history recovery scheduler state: выполнено

Реализовано:

- `recovery_scheduler` хранит отдельную recipient-signed append-only chain v1
  для exact Plan ID под `history-recovery/scheduler/<plan-id>`;
- record до 4 KiB связывает generation/previous State ID, recipient, transition,
  lifecycle, total attempts, consecutive failures, wall-clock high-water,
  persistent deadline и delay;
- loader ограничен 4096 records и fail-closed отклоняет modification, fork,
  duplicate/gap generation, чужой plan/device и filename/content mismatch;
- transitions `initialized`, `attempt-started`, `attempt-failed`,
  `attempt-progressed`, `cancelled`, `completed` образуют проверяемый state
  machine; terminal state не имеет successor;
- перед UDP записывается lease, вычисленный из discovery/connect/route/page
  bounds и ограниченный двумя часами; concurrent runner не открывает второй
  connection, stale lease сначала становится signed failure;
- failure deadline использует exponential equal-jitter: base default 5/max 300,
  CLI bounds `0..=300`/`0..=3600`; jitter зависит от plan/failure/attempt;
- `--max-attempts 1..=8` ограничивает один process invocation; следующий process
  продолжает persistent ordinal и до deadline завершает `deferred` без UDP;
- signed `last_observed` блокирует clock rollback; это fail-closed local
  high-water, а не trusted time или внешний whole-directory witness;
- `history-recovery-plan-cancel` даже после plan expiry добавляет terminal signed
  record; повторный run не выполняет discovery/connection;
- scheduler transitions используют короткий state lock и recoverable vault
  dual-write, но discovery/backoff остаются свободны для foreground client;
- wire, recovery plan/URI v1, ticket v9, ALPN `/7` и checkpoints не изменились.

Проверки:

- unit tests покрывают signed restart chain, fork rejection, clock rollback,
  bounded equal jitter и terminal cancellation;
- M0.9.5 regression smoke снова прошёл прежний no-candidate → source restart →
  complete сценарий через compatibility alias;
- direct process smoke `.tmp/m096-smoke-20260902-130246` выполнил attempt 1,
  завершил первый process, отклонил immediate restart до discovery и после
  deadline продолжил exact plan как persistent attempt 2;
- fresh source endpoint обслужил один authenticated connection и две atomic
  pages; пять scheduler records, два checkpoints и histories совпали;
- отдельный recipient state доказал `failed -> cancelled -> restart` с нулём
  discovery/connection после terminal record;
- все 114 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: network/power context всё ещё caller-supplied, wakeup выполняет
внешний caller, wall clock не является trusted, chain не compacted, lookup
остаётся LAN-only. Полный контракт —
[`../docs/RFC-0033-persistent-history-recovery-scheduler-state.md`](../docs/RFC-0033-persistent-history-recovery-scheduler-state.md).

### M0.9.7 — Windows recovery platform context: выполнено

Реализовано:

- новый `recovery_platform` отделяет platform-neutral context/provider от
  Windows implementation; protocol/core и signed plan не получают WinRT types;
- `platform-context` печатает один read-only native snapshot без state directory;
- `history-recovery-plan-run` при отсутствии context flags по умолчанию использует
  `windows-native`; прежние `--network-class` и `--power-source` сохранены как
  development override и принимаются только вместе;
- Windows Connectivity probe распознаёт WLAN/WWAN и exact IANA Ethernet/Wi-Fi/
  WWAN types, connection cost, roaming, connectivity и data-limit restrictions;
- tunnel/VPN не считается Ethernet: adapter ищет ровно один active exact physical
  profile, а при нуле/нескольких profiles оставляет `unknown`;
- fixed/variable cost или roaming переводят effective class в подписанный
  `mobile` policy bucket; unknown cost/roaming блокируют default plan как unknown;
- Windows PowerManager определяет adequate external supply, battery,
  Energy Saver и bounded charge percentage; contradictory/unavailable state
  становится unknown;
- diagnostics не выводят и не сохраняют profile name, SSID, adapter GUID или IP;
- network/power policy по-прежнему проверяется до UDP/Iroh, plan/scheduler/wire
  formats не изменились;
- non-Windows system provider явно возвращает unsupported/unknown и потому
  fail-closed с default plan до реализации native adapters.

Проверки:

- четыре новых unit tests покрывают exact/ambiguous interface mapping,
  metered/roaming → mobile, power classification и paired manual override;
- M0.9.6 regression `.tmp/m096-smoke-20260902-140653` снова выполнил persistent
  attempt 1 → deferred restart → attempt 2, две pages, identical history и cancel;
- Windows smoke `.tmp/m097-smoke-20260902-142158` через активный VPN нашёл
  единственный physical Ethernet, unrestricted/non-roaming cost и external power;
- completed plan без manual context flags использовал native provider, прошёл
  signed policy и не открыл discovery/connection; partial override отклонён;
- все 118 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: snapshot разовый. Windows service/Task Scheduler, network/power
change subscriptions и policy recheck во время попытки ещё не реализованы.
macOS/Linux/mobile providers также отсутствуют. Полный контракт —
[`../docs/RFC-0034-windows-recovery-platform-context.md`](../docs/RFC-0034-windows-recovery-platform-context.md).

### M0.9.8 — bounded Windows recovery worker: выполнено

Реализовано:

- `history-recovery-plan-watch` запускает один bounded process для exact signed
  plan: runtime default 3600/max 86400 секунд, meaningful wakeups default 64/
  max 1024, cancellation poll default 5/max 30 секунд;
- worker ждёт recipient-signed scheduler deadline либо WinRT network/power
  event, замечает внешний signed state ID и не считает polling timeout wakeup;
- Windows subscriptions покрывают NetworkStatusChanged и PowerManager supply/
  battery/EnergySaver changes; callbacks не сохраняют сетевой metadata, RAII
  снимает tokens при выходе;
- state lock удерживается только для кратких verify/transition; wait и platform
  probe проходят без lock. Typed `StateError::AlreadyLocked` повторяется до двух
  секунд, остальные ошибки остаются fail-closed;
- terminal cancellation/completion проверяется даже после plan expiry, чтобы
  worker мог остановиться; non-terminal retry после expiry запрещён;
- runtime timeout прерывает network future и переводит оставшийся Attempting
  lease в signed failure перед bounded cleanup/выходом;
- native context повторно читается перед attempt lease/UDP discovery и после
  unique descriptor непосредственно перед Iroh connect;
- policy block до discovery не создаёт lease или UDP; block перед connect не
  открывает connection, завершает lease signed failure и планирует retry;
- worker не регистрирует Windows service/Task Scheduler и не меняет OS state.

Проверки:

- три новых unit tests покрывают no-lost-wakeup event sequence, bounded wait и
  распознавание только typed state-lock contention;
- M0.9.6 regression `.tmp/m096-smoke-20260902-171150` снова прошёл attempt 1 →
  deferred restart → attempt 2, две pages, identical history и cancel;
- direct worker smoke `.tmp/m098-smoke-20260902-173045` подтвердил native event
  subscriptions, no-candidate retry, свободный lock во время wait и signed
  cancellation из второго процесса за 1.733 s;
- отдельный 30-second discovery был прерван при runtime=1 s, Attempting lease
  reconciled как signed failure; bounded cleanup завершила process за 4.022 s;
- worker завершился terminal и не выполнил connection после cancel;
- все 121 workspace tests, rustfmt, strict Clippy и release build проходят.

Граница среза: процесс нужно запустить явно. Установка/удаление Windows task или
service, sleep/reboot/logon lifecycle и non-Windows providers остаются дальше.
Полный контракт —
[`../docs/RFC-0035-bounded-windows-recovery-worker.md`](../docs/RFC-0035-bounded-windows-recovery-worker.md).

### M0.9.9 — long-lived messaging runtime: выполнено

Реализовано:

- новая команда `runtime` держит один Iroh endpoint/ticket и последовательно
  обслуживает любое число delivery/sync connections до Ctrl+C;
- каждый connection заново проходит route policy, Account Root/device
  authorization и membership checks; ошибка одной сессии не завершает runtime;
- runtime исключён из outer state lock/vault intent: network accept/route wait
  идут без lock, а одна application session получает отдельный lock и vault
  dual-write transaction;
- typed lock contention повторяется каждые 25 ms до 15 s, чтобы краткая
  foreground-команда не роняла уже принятый connection; остальные ошибки не
  маскируются;
- state-mutating sessions пока последовательны и тем самым сохраняют M0
  single-writer sequence/ratchet invariant;
- optional `--max-sessions 0..=65536` и `--idle-seconds 0..=86400` дают bounded
  test/embedding lifecycle; zero означает работу до Ctrl+C;
- ticket-файл публикуется same-directory temporary file + fsync + atomic replace,
  поэтому restart не показывает peer частично записанный новый Endpoint ID;
- специализированный consent/SAS history-recovery flow не расширен: runtime
  не становится неограниченным rewrap source.

Проверки:

- unit test проверяет создание parent directory и atomic replacement ticket;
- process smoke `.tmp/m099-smoke-20260902-182050` выполнил два `connect` и один
  `sync` через один Endpoint ID, получил 3/3 completed sessions, 4 события и
  identical Alice/Bob history;
- runtime штатно завершился по session bound, затем restart на том же Bob state
  опубликовал новый Endpoint ID, заменил ticket и доставил ещё одно сообщение;
  обе истории сошлись на 6 событиях;
- все 122 workspace tests, rustfmt, strict Clippy и release build проходят;
- final regression на том же fixture добавил delivery + sync через ещё один
  двухсессионный runtime, завершился idle-bound control без клиентов и сохранил
  identical histories на 8 событиях.

Граница среза: runtime пока только long-lived inbound/session core. Persistent
contacts, local outbound queue, automatic reconnect/sync trigger и local GUI API
не реализованы. OS autostart/background registration является optional UX
настройкой, а не условием работы запущенного мессенджера. Полный контракт —
[`../docs/RFC-0036-long-lived-messaging-runtime.md`](../docs/RFC-0036-long-lived-messaging-runtime.md).

### M0.9.10 — persistent runtime contact и durable outbox: выполнено

Реализовано:

- `runtime-contact-add` append-only сохраняет подписанную локальным device
  карточку: local/peer Account ID, exact peer Device ID, conversation label/ID,
  route policy и canonical absolute path обновляемого runtime ticket;
- descriptor проверяется по ticket signature, обеим Account Root authorities,
  messaging capability, membership, exact peer device, prekey freshness и
  high-water; публичный descriptor обязан находиться вне protected state;
- `runtime-queue-message` случайным Queue ID адресует contact, сразу HPKE-шифрует
  body локальному device с metadata AAD и append-only сохраняет signed record;
  plaintext не входит в runtime record/vault;
- runtime materialize-once transaction атомарно сохраняет local projection,
  единственный `AuthorizedEvent` и signed marker с полным event; все retries
  передают тот же event без нового sequence/ratchet message;
- listener на replay того же Event ID возвращает уже сохранённый ACK без нового
  acknowledgement sequence;
- ACK и signed delivered marker коммитятся вместе; только marker завершает
  queue item;
- failed prepare/network attempt создаёт signed hash-linked retry state с
  monotonic generation, previous State ID, `not_before` и bounded exponential
  equal-jitter; restart проверяет цепочку;
- polling runtime выполняет due delivery, затем periodic automatic sync contacts;
  `--auto-sync-seconds 0` отключает sync, `--max-outbound-actions` ограничивает
  process tests;
- contact/queue/materialization/delivery/retry добавлены как новый append-only
  `StateRecordKind::Runtime`, разрешённый в typed DB-primary canary и primary
  transaction delta;
- Iroh accept future сохраняется между polling ticks: отмена tick больше не
  отклоняет handshake, находящийся в процессе установления.

Проверки:

- authenticated format test: contact/queue encode/decode, absence plaintext в
  record, wrong local key, tamper и restart retry-chain verification;
- state/vault tests проверяют registered append-only Runtime delta и zero/nonzero
  typed shadow report;
- process test поднимает Alice/Bob runtime, доставляет durable queued message,
  получает ACK, выполняет automatic sync и подтверждает identical histories:
  ровно один text event + один ACK, queue/materialized/delivered = 1/1/1;
- rustfmt, strict Clippy и все 124 workspace tests проходят.

Граница среза: M0 descriptor — exact-device карточка с локальным file-path
adapter. Privacy-preserving wide-area discovery, versioned device/route rotation,
multi-source gossip/witness, локальный IPC и parallel actor sessions ещё не
реализованы. Internal automatic sync пока использует runtime-owned transient
dialer, но не внешний state writer. Полный контракт —
[`../docs/RFC-0037-persistent-runtime-contact-and-outbox.md`](../docs/RFC-0037-persistent-runtime-contact-and-outbox.md).

### M0.9.11 — authenticated local runtime IPC: выполнено

Реализовано:

- новый reusable crate `kilogram-runtime-ipc` отделяет локальный transport
  contract от CLI и доступен будущему GUI;
- `runtime --ipc-file` bind-ит только `127.0.0.1:0`, генерирует 256-bit bearer,
  device-подписывает descriptor и публикует его atomic replace вне protected
  state; token не печатается;
- client проверяет version, loopback, token length и device signature; framing
  ограничен 256 KiB, connect 3 s, I/O/actor response 30 s, actor queue 64;
- per-connection tasks делают framing/auth и только затем передают command через
  MPSC в основной runtime loop; state mutations остаются последовательными с
  Iroh sessions и polling;
- контракт `Ping`, `QueueMessage`, `OutboxStatus`; CLI adapters не принимают
  `STATE_DIR` и не становятся вторым writer;
- 256-bit IPC request ID становится durable Queue ID; exact replay возвращает
  AlreadyPresent, reuse с другим content fail closed;
- stop/drop удаляет descriptor только если он всё ещё принадлежит этому runtime
  instance, поэтому старый process не стирает replacement;
- `OutboxStatus` возвращает typed counters/items без plaintext; push revision
  пока не реализован, GUI может делать bounded polling.

Проверки:

- shared crate tests: signed/loopback descriptor, authenticated round-trip,
  wrong-token rejection до actor dispatch, replacement-owned cleanup;
- process test теперь ставит Alice message только через runtime IPC, повторяет
  тот же request ID, видит одну queue record, затем получает один text + ACK и
  identical Alice/Bob history после automatic sync;
- rustfmt, strict Clippy, все 129 workspace tests и release build проходят.

Граница среза: bearer descriptor — same-user M0 boundary и должен храниться
локально, не в sync/shared folder. OS peer credentials, push stream, IPC contact
onboarding и GUI остаются дальше. Полный контракт —
[`../docs/RFC-0038-authenticated-local-runtime-ipc.md`](../docs/RFC-0038-authenticated-local-runtime-ipc.md).

### M0.9.12 — minimal desktop runtime client: выполнено

Реализовано:

- новый `apps/kilogram-windows` собирает оконный Windows-клиент на safe Rust и
  не зависит от state/store/session/transport crates;
- путь к private runtime descriptor принимается через `--ipc-file`, редактируется
  в окне или меняется drag-and-drop; `Ping` показывает точные Account/Device ID;
- composer валидирует conversation, peer Account ID и message, затем ставит
  сообщение только через `QueueMessage`, не открывая `STATE_DIR`;
- request ID сохраняется при неопределённой ошибке и повторно используется для
  неизменённого draft; успешная постановка очищает только body;
- отдельный Tokio worker не блокирует оконный event loop, сериализует локальные
  requests и опрашивает structured outbox status не чаще раза в две секунды;
- outbox показывает counters, Queue/peer/conversation IDs, materialization,
  delivery и ACK metadata без plaintext;
- `eframe` закреплён на 0.33.3 с MSRV 1.88, поэтому workspace сохраняет Rust
  1.91 и получает переиспользуемую desktop UI основу.

Проверки:

- 5 pure UI/view-model cases проверяют validation, draft semantics, feedback и
  idempotent retry request ID;
- IPC integration case поднимает настоящий signed loopback server и проверяет
  desktop `Ping` → `QueueMessage` → `OutboxStatus` contract;
- rustfmt, strict package Clippy и 6 package tests проходят.

Граница среза: runtime должен быть запущен отдельно; contact создаётся CLI
bootstrap-командой. Contact list, conversation summaries, readable history,
IPC onboarding, runtime launch/autostart и push subscription ещё не
реализованы. Полный контракт —
[`../docs/RFC-0039-minimal-desktop-runtime-client.md`](../docs/RFC-0039-minimal-desktop-runtime-client.md).

### M0.9.13 — actor-owned chat read model: выполнено

Реализовано:

- `ConversationList` выдаёт GUI только проверенные signed runtime contacts,
  bounded metadata, message count и 96-byte latest preview;
- `HistoryPage` возвращает до 100 local text projections и не более 192 KiB
  plaintext body; ACK не превращаются в видимые chat messages;
- cursor привязан BLAKE3 digest к точному ordered text-event snapshot и
  fail-closed устаревает при изменении истории;
- runtime строит causal topological order с deterministic ready tie-break, не
  обещая group-consensus ordering;
- все read/decrypt операции выполняет runtime actor под state lock; GUI не
  получил `STATE_DIR`, vault/ratchet keys или зависимость от storage/transport;
- desktop UI заменил ручной conversation/peer ввод на двухколоночный список
  чатов, readable history, `Load older messages` и composer выбранного contact;
- двухсекундный polling остаётся последовательным и bounded: chats → selected
  history → outbox.

Проверки:

- shared IPC cursor tests, causal ordering/snapshot tests и desktop view-model
  tests;
- настоящий runtime process отвечает на пустые list/history reads, затем P2P
  delivery materializes readable local history;
- desktop adapter проходит signed loopback `Ping`, queue, outbox, conversation
  list и history page contract.

Полный контракт —
[`../docs/RFC-0040-actor-owned-chat-read-model.md`](../docs/RFC-0040-actor-owned-chat-read-model.md).

### M0.9.14 — desktop contact onboarding и runtime lifecycle: выполнено

Реализовано:

- IPC v3 добавил typed `AddContact` и `Shutdown`; mutation выполняется только
  runtime actor под state lock/vault dual-write, а GUI не читает ticket/trust;
- runtime проверяет membership, expected peer Account ID, certified Device ID,
  local authorization, route и canonical descriptor path до append-only contact;
- `runtime-profile-create` пишет no-clobber JSON v1 с абсолютными public paths и
  settings вне protected state, без seed/device/vault/bearer secrets;
- `runtime-from-profile` заново валидирует profile и запускает ту же runtime
  реализацию с foreground-unbounded lifecycle;
- desktop GUI умеет стартовать соседний CLI process, bounded retry ждать signed
  IPC descriptor, подключаться и штатно останавливать runtime;
- при закрытии окна desktop-owned runtime сначала получает authenticated
  shutdown; hard kill используется только после bounded timeout;
- форма `+ Contact` принимает label, expected Account ID и public peer ticket,
  поддерживает drag-and-drop и после успеха обновляет signed chat list;
- autostart, Windows service и Task Scheduler не устанавливаются.

Проверки:

- profile round-trip/no-clobber/protected-state tests и настоящий
  profile-started runtime → authenticated shutdown → descriptor cleanup;
- Alice/Bob process test импортирует peer contact через IPC, затем сохраняет
  прежнюю exactly-once queue/delivery/sync сходимость;
- desktop signed-loopback adapter проходит ping → add contact → queue → outbox →
  conversations → history → shutdown;
- rustfmt, strict workspace Clippy, все 144 tests и release build проходят.

Полный контракт —
[`../docs/RFC-0041-desktop-contact-onboarding-and-runtime-lifecycle.md`](../docs/RFC-0041-desktop-contact-onboarding-and-runtime-lifecycle.md).

### M0.9.15 — desktop runtime setup и change notifications: выполнено

Реализовано:

- desktop launch panel загружает, редактирует и атомарно сохраняет
  `RuntimeLaunchProfile` для уже enrolled device без обязательного
  `runtime-profile-create` CLI шага;
- GUI canonicalizes существующие state/device-list/prekey inputs, resolves
  output paths и использует shared bounded profile validation;
- редактирование/сохранение запрещено при connected или desktop-owned runtime;
  profile по-прежнему не содержит seed/device/vault/bearer secrets;
- IPC v4 добавил bounded `WaitForChange`/`ChangeState` с in-memory revision;
  long poll обслуживается connection task и никогда не занимает actor MPSC;
- runtime публикует wake-up после contact/queue, delivery/retry, automatic sync
  и успешной inbound session, но не после idempotent replay;
- отдельный desktop watcher отслеживает revision и coalesces refresh pipeline
  conversations → selected history → outbox; unconditional polling раз в две
  секунды удалён, manual refresh сохранён;
- runtime start/stop остаётся foreground-only без Task Scheduler/service.

Проверки:

- shared profile replacement/bounds и desktop public-draft round-trip tests;
- IPC long-poll test проверяет wake, quiet timeout и отсутствие actor dispatch;
- desktop watcher integration получает revision через настоящий signed
  loopback server, также не создавая actor work;
- rustfmt, strict workspace all-target/all-feature Clippy, все 147 tests и
  release build проходят;
- полный контракт —
  [`../docs/RFC-0042-desktop-runtime-setup-and-change-notifications.md`](../docs/RFC-0042-desktop-runtime-setup-and-change-notifications.md).

### M0.9.16 — desktop first-account bootstrap: выполнено

Реализовано:

- 256-bit entropy кодируется 24-word English BIP39 phrase; отдельный BLAKE3
  derivation domain детерминированно даёт Ed25519 Account Root и тот же
  `AccountId`, а sensitive intermediate buffers zeroize;
- `account-root-secret.key` стал bounded versioned envelope: Windows DPAPI
  CurrentUser, explicit plaintext-development fallback на остальных ОС и
  автоматическая миграция прежнего raw 32-byte key без смены Account ID;
- отдельный `kilogram-bootstrap create` в same-parent staging создаёт root,
  первый device/certificate/authority/device-list, signed prekey pool и verified
  encrypted vault, затем no-clobber публикует всю workspace одним rename;
- persistent receipt содержит только IDs/public paths/protection/recovery scope;
  phrase возвращается ровно в bounded redacted/zeroizing process response;
- first-run GUI запускает соседний helper без secret command-line arguments,
  показывает phrase до explicit offline-save acknowledgement и заполняет public
  launch-profile paths; peer Account ID/prekeys остаются пустыми до contact
  ceremony;
- seed-alone restore намеренно отсутствует: без authenticated current authority
  history он мог бы создать signed rollback/fork.

Проверки:

- deterministic phrase/Account ID, invalid phrase, root envelope round-trip и
  legacy raw migration;
- atomic no-clobber bootstrap, receipt без phrase, public artifacts и verified
  encrypted vault;
- bounded/redacted output contract, GUI helper option и launch profile без peer
  prekeys;
- rustfmt, strict workspace Clippy, all tests и release build;
- полный контракт —
  [`../docs/RFC-0043-desktop-first-account-bootstrap.md`](../docs/RFC-0043-desktop-first-account-bootstrap.md).

### M0.9.17 — existing-account device link: выполнено

Реализовано:

- новый device атомарно создаёт provisional workspace, DB-primary vault, signed
  prekey pool и short-lived device-signed request без seed/private-key transfer;
- request exact-account/device/key/nonce bound, ограничен 30 минутами и 256 KiB,
  а 12-digit SAS вычисляется из digest всего signed объекта;
- Root authorization требует свежесть и ручной exact SAS, сериализует enrollment
  OS lock и идемпотентно публикует certificate только внутри нового полного
  Root-signed device list;
- повтор Device ID с другим key/capabilities и permanently revoked key
  отклоняются; сбой до atomic list replace может оставить только sequence gap,
  но не авторизованный partial device;
- Root-signed exact authorization HPKE-шифруется на новый device; чужой device,
  другой request и tampering отклоняются;
- accept читает identity из authenticated DB-primary vault, коммитит certificate
  и authority через trust transaction, публикует public artifacts и terminal
  receipt идемпотентно;
- после accept device готов быть exact recipient нескольких независимых
  resumable history recovery plans; существующий signed reconciliation сохраняет
  `incomplete`/`agreed`/`divergent` и не обещает global completeness.

Проверки:

- полный request → inspect → authorize → accept, exact retry и repeated accept;
- wrong SAS, wrong recipient, response tampering и Device ID/key conflict;
- форматирование, strict workspace Clippy, все 154 tests и release build;
- real release process smoke `.tmp/m0917-smoke-20260904-014515` прошёл четыре
  helper-команды, authority revision 2 и encrypted response 911 bytes;
- полный контракт —
  [`../docs/RFC-0044-existing-account-device-link.md`](../docs/RFC-0044-existing-account-device-link.md).

### M0.9.18 — desktop device-link и multi-source recovery wizard: выполнено

Реализовано:

- desktop проводит четыре явных device-link шага через sibling bootstrap helper:
  create request, inspect, SAS-gated authorize и exact-workspace accept;
- request/response поддерживают явно вооружаемый drag-and-drop target, SAS
  показан крупно, а authorize требует тот же canonical request path, который был
  inspected;
- bounded strict JSON adapter проверяет IDs/status/SAS/absolute paths, exact
  requested outputs, authority revision и recipient-encrypted response;
- после accept secret-free launch-profile draft получает новые state/device-list/
  ticket/IPC paths, а прежние peer Account ID/prekey paths очищаются;
- recovery UI утверждает новый recipient-signed plan с network/power consent,
  добавляет несколько существующих plans, запускает одну bounded attempt и
  подписывает irreversible cancellation только после отдельного confirmation;
- structured terminal result сохраняется и при policy-blocked nonzero exit;
  строки показывают status/lifecycle/attempts/complete;
- reconciliation показывает source/complete/covered/equivocation counts и exact
  `incomplete`/`single-source`/`agreed`/`divergent`, всегда отдельно отображая
  `global_completeness_proven=false`;
- runtime должен быть stopped/disconnected; Task Scheduler/background service не
  регистрируется;
- полный контракт —
  [`../docs/RFC-0045-desktop-device-link-and-recovery-wizard.md`](../docs/RFC-0045-desktop-device-link-and-recovery-wizard.md).

Проверки:

- targeted GUI adapter/parser tests, rustfmt, strict workspace Clippy, все 157
  workspace tests в serial regression и release workspace build;
- release process smoke выполнил create → request → inspect → authorize → accept
  и empty reconciliation (`.tmp/m0918-smoke-20260904-102552`): authority
  revision 2, `incomplete`,
  `global_completeness_proven=false`.

### M0.9.19 — portable Account Root authority recovery: выполнено

Реализовано:

- Root-signed bounded package содержит current complete authority snapshot,
  latest complete device list и canonical current conversation-membership heads;
- отдельный Root-signed witness связывает Account ID, authority revision и
  domain-separated digest exact package;
- export сериализован тем же OS authority lock, что enrollment/revocation и
  теперь membership mutations; artifacts публикуются no-clobber вне Root;
- offline inspect проверяет nested/outer signatures и exact witness binding без
  seed; bounded direct symlinks отклоняются;
- phrase принимается helper только через stdin, восстанавливает Root только в
  новый same-parent-staged path и на Windows создаёт новый DPAPI CurrentUser
  envelope;
- перед final rename восстановленные sequence/revocations/device-list/
  memberships повторно строят byte-exact authority views;
- wrong phrase, tampering, stale package + latest witness, existing target и
  output внутри Root fail closed;
- matching rollback package+witness остаётся честно обозначенной границей без
  global monotonic witness/current-device quorum;
- полный контракт —
  [`../docs/RFC-0046-account-root-authority-recovery.md`](../docs/RFC-0046-account-root-authority-recovery.md).

Проверки:

- unit round-trip сохраняет Account ID, authority/list/membership state и после
  restore выдаёт следующему device более новую revision;
- formatting, strict workspace Clippy, все 159 serial workspace tests и release
  workspace build проходят;
- release process smoke `.tmp/m0919-release-smoke-20260904-112123` прошёл create
  → export → inspect → stdin phrase restore с DPAPI CurrentUser provider.

### M0.9.20 — desktop Account Root recovery ceremony: выполнено

Реализовано:

- Windows desktop добавил offline Root recovery panel: export current Root в
  новые package/witness paths, dedicated drop targets, strict inspect и restore
  только при stopped runtime;
- UI показывает Account ID, authority revision, package ID и captured counts,
  отдельно предупреждает, что signature validity не доказывает global freshness,
  и требует explicit confirmation newest independently retained witness;
- редактирование/drop artifacts и новый inspect заранее сбрасывают прежний gate,
  confirmation и введённую phrase;
- phrase masked, хранится в zeroizing type, redacted из WorkerRequest Debug,
  удаляется из UI сразу после enqueue и передаётся helper только bounded stdin;
- restore связан с inspected package ID/revision: helper повторно сравнивает их
  после чтения exact package/witness и до staging, поэтому valid same-path
  replacement fail closed;
- результат заполняет Account ID и restored Root path существующей device-link
  ceremony; enrollment и multi-source history recovery не запускаются неявно;
- полный контракт —
  [`../docs/RFC-0047-desktop-account-root-recovery.md`](../docs/RFC-0047-desktop-account-root-recovery.md).

Проверки:

- targeted bootstrap/desktop tests покрывают strict output, secret redaction,
  wrong/stale inputs и same-path replacement;
- configured debug и release process smoke прошёл реальный create → export →
  inspect → stdin-only restore adapter с DPAPI CurrentUser;
- formatting, strict workspace Clippy, все 162 serial workspace tests и release
  workspace build проходят; hashes зафиксированы в `development-environment.md`.

### M0.9.21 — recovery freshness design и exact-export lifecycle: выполнено

Реализовано:

- Account Root хранит bounded Root-signed witness последнего действительно
  опубликованного exact recovery package без phrase/private keys;
- export после внешней no-clobber публикации повторно берёт authority lock,
  пересобирает current package и при race удаляет внешнюю пару вместо записи
  ложного receipt;
- `account-recovery-status` под тем же lock сравнивает пересобранный package с
  receipt и сообщает `current`/`update-required`, current/recorded package IDs,
  revisions и captured counts;
- membership-only mutation при неизменной authority revision тоже делает status
  stale; restored Root начинает с `current`, последующий enrollment — с
  `update-required`;
- Windows recovery panel показывает lifecycle status и явно отделяет его от
  global freshness;
- RFC-0048 выбрал fresh challenge-bound device approvals, DB-primary
  anti-equivocation head и strict majority exact recovery roster; для безопасной
  смены roster требуется joint-majority old/new epoch transition;
- полный контракт —
  [`../docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md`](../docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md).

Проверки:

- targeted identity/bootstrap/desktop regression и real debug helper-process
  create → status due → export → status current → restore → status current;
- formatting, strict workspace Clippy и release workspace build проходят;
  full serial run прошёл 161/162, а единственный Iroh runtime outbox timeout
  прошёл exact rerun. Три unit-only authorization endpoints закреплены на IPv4
  loopback без production relay map, чтобы убрать прежнюю order-dependent flake;
  детали и hashes зафиксированы в `development-environment.md`.

### M0.9.22 — current-device recovery quorum core: выполнено

Реализовано:

- bounded `.karq` request содержит exact Root-signed package+witness, fresh
  256-bit challenge и expiry; `.kara` device approval связывает exact request,
  package/state-vector/roster, approver и previous local head;
- package требует device-list той же authority revision; каждый approver
  проверяется против current candidate authority, включая revocation;
- approval читает certificate/authority/membership/head только из DB-primary
  vault, отклоняет rollback, missing local head, same-revision equivocation и
  non-add-only membership;
- authority/membership high-water и signed approval head коммитятся одной trust
  transaction до публикации approval; retry того же Request ID идемпотентен;
- первый head замораживает exact roster до реализации joint transition;
- verifier считает только distinct exact-request approvals, применяет
  `floor(n/2)+1`, честно различает `artifact-integrity-only`,
  `single-current-device-observed` и `current-device-majority-observed`;
- `kilogram-bootstrap` получил команды `account-recovery-quorum-request`,
  `account-recovery-quorum-approve` и `account-recovery-quorum-verify` с
  optional hard gate `--require-majority`;
- полный контракт —
  [`../docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md`](../docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md).

Проверки:

- targeted identity/state/bootstrap tests покрывают expiry/replay, duplicate и
  insufficient approvals, offline fallback, DB-primary commit-before-publish,
  idempotent retry, stale/missing membership, same-revision fork и cross-roster
  отказ;
- debug process smoke прошёл create → export → request → 1-of-1 approve → strict
  verify с `current-device-majority-observed` и явным
  `cross_roster_fork_safety=false`.
- formatting, strict workspace Clippy, все 169 serial workspace tests и release
  workspace build проходят; release smoke и hashes зафиксированы в
  `development-environment.md`.

### M0.9.23 — networked recovery quorum ceremony: выполнено

Реализовано:

- device-signed bounded `.kart` связывает exact Account/Request ID, expiry,
  candidate-roster certificate, Iroh endpoint, 256-bit bearer capability и
  route policy;
- one-shot listener сначала выполняет неизменную DB-primary rollback/
  equivocation проверку и коммитит approval head, только затем no-clobber
  публикует ticket и отдаёт `.kara` предъявителю exact request+bearer;
- Iroh переносит approval по `auto`, `direct-only` или strict `relay-only`, а
  collector проверяет endpoint ticket, exact current roster и сам device-signed
  response;
- collector принимает до bounded account roster tickets, запрещает duplicate
  Device IDs, идемпотентно сохраняет `{device_id}.kara` и передаёт distinct set
  прежнему strict-majority verifier без отдельной сетевой трактовки claim;
- `--require-majority` остаётся hard gate; partial collection сохраняет честный
  weaker claim и не превращается в majority;
- Windows desktop получил полный request → current-device listener →
  multi-ticket collect/verify flow и literal claim/threshold display;
- restore gate принимает только majority для exact inspected package либо
  отдельный explicit reduced-assurance offline fallback; cross-roster safety
  остаётся false.

Проверки:

- direct loopback regression проходит полный signed ticket → bearer fetch →
  committed approval → exact verifier flow и отдельно проверяет tampered
  request/route signature отказ;
- real debug process smoke
  `.tmp/m0923-process-smoke-20260904-190155` прошёл one-shot `direct-only`
  listener/collector с `current-device-majority-observed`, 1/1 approvals и
  `cross_roster_fork_safety=false`;
- targeted bootstrap/desktop regression проходит 29 tests; strict Clippy,
  все 172 serial workspace tests, release workspace build и release process
  smoke проходят, hashes записаны в `development-environment.md`.

### M0.9.24 — recovery-policy epoch и joint transition: выполнено

Реализовано:

- DB-primary `recovery-policy/current.policy` фиксирует Account ID, monotonically
  increasing epoch, exact roster digest и latest transition ID; epoch 0
  мигрирует из уже committed recovery approval head;
- bounded `.karpt` связывает старые/новые Root package+witness, exact
  `N -> N+1`, previous transition ID, fresh challenge и expiry; new package
  обязан быть monotonic successor, а roster и authority revision — измениться;
- `.karpa` подписывается union old/new voter set, но distinct new-only подпись
  считается только в new quorum; overlap signer считается в обоих;
- old voter сверяет `.karpt` с локальным policy anchor и до публикации подписи
  атомарно коммитит anti-equivocation head в DB-primary trust;
- canonical permanent `.karpc` создаётся только при одновременном strict
  majority old и new roster, после чего active new-roster device устанавливает
  epoch одной crash-consistent trust transaction;
- существующий recovery approval roster freeze снимается только exact
  установленным bridge certificate; без него прежний different-roster отказ
  сохранился;
- recovery verifier с `--policy-certificate-file` выдаёт
  `cross_roster_fork_safety=true` только вместе с majority exact candidate
  roster и monotonic successor certified new package.

Проверки:

- identity regression проверяет overlap counting и невозможность заменить old
  majority подписью new-only device;
- end-to-end bootstrap regression проходит `1 -> 2`: partial certificate
  отвергается, две подписи создают `.karpc`, оба device устанавливают epoch 1,
  новый roster перестаёт блокироваться, а policy-bound recovery majority
  повышает claim до cross-roster safety;
- прежние rollback/equivocation, roster freeze, DB-primary commit-before-publish
  и direct `.kart` regressions остаются зелёными.

### M0.9.25 — networked recovery-policy activation: выполнено

Реализовано:

- signed `.karpticket` связывает exact transition Request ID/expiry, voter
  certificate из union old/new roster, Iroh endpoint, one-shot bearer и
  `auto`/`direct-only`/`relay-only` route policy;
- listener коммитит DB-primary transition approval anti-equivocation head до
  no-clobber публикации ticket и отдаёт ровно один independently verifiable
  `.karpa` только по exact request+bearer fetch;
- collector отклоняет duplicate signer Device IDs, повторно проверяет ticket,
  endpoint, route и response, идемпотентно сохраняет `{device_id}.karpa` и
  показывает old/new quorum progress отдельно;
- joint collection сама по себе честно сохраняет
  `cross_roster_fork_safety=false`; true появляется только в canonical `.karpc`
  и установленном policy epoch;
- bootstrap CLI получил `account-recovery-policy-transition-listen` и
  `account-recovery-policy-transition-collect`, включая hard
  `--require-joint-majority` gate;
- Windows wizard рядом с device-link ведёт request → network approvals →
  certificate → per-device install и отдельно показывает Root roster operation,
  recovery-policy activation и multi-source history recovery;
- строгий desktop JSON boundary проверяет absolute paths, exact statuses,
  independent thresholds, transport route consistency и невозможность claim
  fork safety без joint majority.

Проверки:

- direct loopback regression собирает old 1/1 и new 2/2 через два независимых
  one-shot listener, затем создаёт canonical `.karpc`;
- M0.9.24 end-to-end policy/install/recovery regression и Windows wizard suite
  остаются зелёными; финальные workspace/release проверки записываются в
  `development-environment.md`.

### M0.9.26 — first-class device removal: выполнено

Реализовано:

- `AccountRootState::revoke_and_publish_device_list` сериализует permanent
  revocation и публикацию полного оставшегося roster; промежуточная revision
  mismatch остаётся fail-closed для recovery export, а retry повторно использует
  existing revocation и завершает публикацию;
- запрещены unknown target и удаление последнего active device;
- `kilogram-bootstrap device-remove` требует exact independently witnessed
  before checkpoint и проверяет, что after state отличается ровно одним новым
  revocation, одним удалённым certificate и revision `N -> N+1`, без изменения
  conversation memberships;
- helper idempotently публикует public revocation, refreshed device list и
  after `.karp`/`.karw`, затем отмечает новый checkpoint current;
- Windows GUI требует повторный ввод полного Device ID, автоматически заполняет
  exact policy-transition before/after paths и draft runtime device-list path;
- UI отдельно показывает complete removal, required policy activation,
  required runtime directory refresh, required ratchet/session retirement и
  честный факт `existing-copies-remain-readable` для старой истории.

Проверки:

- identity regression моделирует crash между revocation и list publication,
  успешный repair/retry, last-device guard и unknown-device guard;
- bootstrap regression проверяет exact `2 -> 1`, current checkpoint и
  byte-idempotent повторный запуск;
- strict Windows JSON regression не принимает ложное удаление старой истории.

### M0.9.27 — live runtime device-directory refresh: выполнено

Реализовано:

- runtime IPC поднят до v5 и получил authenticated
  `ApplyOwnDeviceDirectory`; CLI и Windows GUI используют тот же typed actor
  contract без прямого доступа к `STATE_DIR`;
- runtime принимает только canonical Root-signed same-account transition с
  monotonic revision, exact retained certificates, active current device и без
  additions/replacements; каждое исчезновение требует permanent revocation;
- одна vault-primary transaction устанавливает новый own authority high-water
  и удаляет ratchet session + peer-prekey observation для revoked Device IDs;
- failure rollback восстанавливает удалённые records, commit сохраняет typed
  authenticated delta, exact повтор операции идемпотентен;
- runtime сохраняет Endpoint ID/route/requester authority и атомарно заменяет
  public ticket; при другом canonical device-list path отдельно требует
  convergence launch profile;
- sender при чтении refreshed peer ticket retire revoked-device state до
  наблюдения active pools и нового fanout; unmaterialized queue использует
  только active roster;
- старый materialized recipient table не переписывается, потому что входит в
  signed append-only event; IPC/UI отдельно показывают future exclusion,
  immutable old slots и `existing-copies-remain-readable`.

Проверки:

- live runtime regression без restart проверяет `2 -> 1`, atomic ticket
  replacement, session/prekey retirement и idempotent retry;
- injected transaction failure восстанавливает оба ratchet records, а commit
  записывает два removals в vault delta;
- Windows adapter regression проверяет exact IPC command/path и typed result;
- подробный contract зафиксирован в
  [`../docs/RFC-0049-live-runtime-device-directory-refresh.md`](../docs/RFC-0049-live-runtime-device-directory-refresh.md).

### M0.9.28 — restart-safe runtime device directory: выполнено

Реализовано:

- live apply создаёт device-signed append-only receipt, связывающий Account,
  local Device, authority revision, canonical device-list digest/path, active
  count, generation и previous receipt ID;
- receipt append, own-authority high-water и revoked-device ratchet/prekey
  retirement входят в одну vault-primary transaction; exact retry сохраняет
  ту же generation;
- startup до ticket/IPC проверяет bounded contiguous receipt chain и выбирает
  exact applied roster вместо stale profile; missing/symlink/tamper fail closed,
  revoked peer-prekey paths больше не мешают восстановлению;
- IPC v6 `OwnDeviceDirectoryStatus` сообщает receipt, revision/digest,
  launch/applied paths и `current`/`convergence-required`/restart source;
- Windows desktop читает status сразу после Ping и предлагает bounded
  `Reconcile launch profile`: exact state/IPC/old/new path и unchanged-bytes
  gates, atomic replace только `device_list_file`, reload equality;
- runtime по-прежнему не получает general launch-profile write authority.

Проверки:

- receipt unit regression отклоняет tamper, rollback, fork/equivocation и
  разорванную chain;
- DB-primary live regression проходит `2 -> 1`, receipt generation 1,
  idempotent retry, stop и restart с намеренно stale profile, затем проверяет
  republished ticket без revoked device;
- desktop regression проверяет status-after-ping, exact one-field convergence и
  отказ при внешнем path drift;
- contract зафиксирован в
  [`../docs/RFC-0050-restart-safe-runtime-device-directory.md`](../docs/RFC-0050-restart-safe-runtime-device-directory.md).

### M0.9.29 — signed wide-area ticket publication: выполнено

Реализовано:

- directional lookup channel связывает conversation, publisher Account/Device
  и recipient Account без публикации этих идентификаторов в открытом виде;
  отдельный publisher Device исключает коллизию endpoint-ов одного аккаунта;
- current ticket входит в device-signed expiring hash chain с generation и
  previous ID, а локальный publisher head сохраняется append-only в
  vault-primary `Runtime` repository;
- одна HPKE envelope содержит отдельный recipient slot для каждого active
  устройства из последнего проверенного peer directory; store получает только
  pseudonymous outer metadata и ciphertext;
- receiver проверяет publication, ticket, exact contact Device/route,
  membership, local authorization, authority и prekey high-water, затем одной
  transaction продвигает trust/ratchet и device-signed observation high-water;
- rollback/same-generation equivocation после локального observation fail
  closed, exact retry идемпотентен; descriptor заменяется атомарно только после
  state commit;
- HTTPS client запрещает redirects, ограничивает time/body, разрешает plain
  HTTP только numeric loopback; сетевой wait не удерживает state/vault lock;
- IPC v7, CLI и Windows desktop дают explicit publish/refresh для уже
  enrolled contact и честно показывают expiry, generation, privacy и
  first-contact boundary без background service.

Проверки:

- crypto regression проверяет recipient binding, wrong key/device, expiry,
  publisher-device channel separation, observation monotonicity и rollback;
- реальный loopback HTTP regression проверяет exact PUT/GET path и opaque
  envelope round-trip;
- два live runtime actor проходят mutual contact enrollment, publish, fetch,
  atomic install и idempotent replay с durable publication/observation records;
- Windows adapter regression проверяет exact typed IPC commands и ответы;
- rustfmt, strict workspace Clippy, все 189 workspace tests и release build
  проходят; новый live runtime lifecycle отдельно проходит в release mode;
- contract зафиксирован в
  [`../docs/RFC-0051-signed-wide-area-ticket-publication.md`](../docs/RFC-0051-signed-wide-area-ticket-publication.md).

### M0.9.30 — self-hostable opaque ticket store: выполнено

Реализовано:

- отдельный dependency-light `kilogram-ticket-store` не зависит от identity,
  protocol, ratchet, runtime IPC и transport crates и никогда не декодирует
  HPKE envelope;
- loopback-only bounded HTTP/1.1 предназначен для HTTPS reverse proxy и
  отказывается слушать public cleartext address;
- Redb transaction атомарно создаёт/replaces opaque value: greater generation
  принимается, exact same-generation replay идемпотентен, rollback и
  same-generation different body получают HTTP 409;
- fixed service retention 30–3600 секунд не продлевается GET/exact retry;
  startup, periodic cleanup и expired GET удаляют старые records;
- body/channel/total-bytes/connections/header/target/time limits, per-IP/global
  fixed-window rate limits, HTTP 429/507 и optional trusted-proxy `X-Real-IP`
  ограничивают ресурсы без заявления о Sybil/DDoS защите;
- runtime publication regression вместо временного mock теперь использует этот
  реальный store между двумя live actors;
- русский Internet procedure описывает HTTPS proxy, publish/fetch, restart и
  retention test без ложного утверждения об анонимности.

Проверки:

- store regression проходит durable reopen, generation jump, exact replay,
  conflict/rollback, capacity и TTL expiry;
- реальный HTTP regression проверяет content type/path/generation, body limit,
  trusted `X-Real-IP` и точный 429;
- live runtime regression проходит publish/fetch/install/idempotent replay через
  production store implementation;
- rustfmt, strict workspace Clippy, все 193 workspace tests и release workspace
  build проходят;
- release process smoke `.tmp/m0930-release-smoke-final` подтвердил HTTP
  `201/200/409/204`, exact GET generation 2/body `06070809`, принудительное
  завершение отдельного EXE и durable GET после нового запуска на том же Redb;
- release-mode runtime lifecycle повторно прошёл publish/fetch/install и
  idempotent replay двух live actors через production store;
- contracts зафиксированы в
  [`../docs/RFC-0052-self-hostable-opaque-ticket-store.md`](../docs/RFC-0052-self-hostable-opaque-ticket-store.md)
  и
  [`../docs/M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md`](../docs/M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md).

### M0.9.31 — opt-in runtime ticket automation: выполнено

Реализовано:

- IPC v8 устанавливает и читает device-signed per-contact policy для
  автоматической публикации собственного ticket и refresh peer ticket;
- policy связывает exact contact/conversation/peer/store, TTL/refresh lead,
  bounded retry и разрешения Ethernet/Wi-Fi/mobile/unknown; повтор той же
  конфигурации идемпотентен, изменение или disable создаёт следующую generation;
- отдельные signed append-only attempt chains сохраняют publish/refresh result,
  failure count и `not_before`, поэтому restart не сбрасывает exponential
  backoff; success планируется от signed expiry, near-expiry recheck не чаще 30s;
- automatic publish создаёт новую signed publication generation и не выдаёт
  randomized reseal той же generation за idempotent replay;
- постоянный monotonic runtime ticker не может быть вытеснен частыми IPC/UI
  reads; сетевой wait не удерживает state lock, а commit повторно проверяет exact
  current policy head;
- Windows UI даёт явные enable/disable/status и отдельные network permissions,
  показывает обе action states и честно сообщает: работа только пока открыт
  runtime, без Task Scheduler/service/autostart и без участия в чужом relay;
- content-addressed короткие `.tap`/`.taa` имена исключают Windows long-path
  failure для новых signed records.

Проверки:

- unit regression проверяет policy succession, bounded backoff, success reset и
  minimum near-expiry recheck;
- два live runtime actor через production opaque store проходят automatic mutual
  publish/refresh, durable status и signed disable generation;
- strict Windows adapter проверяет exact safe defaults и typed IPC v8 status;
- полный workspace/release snapshot записывается в `development-environment.md`;
- contract зафиксирован в
  [`../docs/RFC-0053-opt-in-ticket-automation.md`](../docs/RFC-0053-opt-in-ticket-automation.md).

### M0.9.32 — authenticated bounded runtime ticket compaction: выполнено

Реализовано:

- device-signed `.rtc` checkpoint сохраняет exact Account/Device,
  monotonic generation/previous ID, canonical removed-delta digest, cumulative
  history digest и total compacted count;
- checkpoint anchors сохраняют exact chain key, generation и signed record ID
  для publication, observation, policy и отдельных publish/refresh attempts;
- retained signed head остаётся обычным record, поэтому следующий элемент
  продолжает прежнюю generation/hash-link без дублирования большого ticket в
  checkpoint;
- любая covered chain после девятого retained record compact-ится до одного
  head; новый checkpoint заменяет старый и запрещает исчезновение, rollback или
  same-generation смену authenticated anchor;
- loader допускает ненулевую initial generation только при exact checkpoint
  anchor, проверяет signature retained head и обычную непрерывность всех новых
  successors; missing checkpoint/head и extra pre-anchor data fail closed;
- state transaction получил узкую crash-safe регистрацию removal только для
  existing Runtime records: synced backup, manifest recovery, typed DB-primary
  deletion; другие append-only namespaces остались immutable;
- runtime выполняет compaction локально на persistent maintenance tick после
  delivery/sync/ticket automation, не делает network request и публикует только
  безопасные count diagnostics/change notification.

Проверки:

- state regression подтверждает rollback и simulated crash recovery удалённого
  runtime record вместе с удалением uncommitted checkpoint;
- два последовательных publication checkpoint дают generation `1 -> 2`,
  retained publication продолжает `9 -> 17`, а restart видит ровно один current
  checkpoint;
- отдельный multi-chain regression compact-ит observation, policy, publish и
  refresh attempt chains до signed generation 9 heads;
- tampered checkpoint signature отклоняется, а production automatic ticket
  lifecycle проверяет logical high-water независимо от физического количества
  compacted files;
- contract зафиксирован в
  [`../docs/RFC-0054-authenticated-runtime-ticket-compaction.md`](../docs/RFC-0054-authenticated-runtime-ticket-compaction.md).

### M0.9.33 — unlinkable self-authenticating ticket write capability: выполнено

Реализовано:

- новый narrow crate `kilogram-ticket-publication` является общей реализацией
  per-peer capability KDF, Ed25519 write key, self-authenticating channel и
  exact PUT authorization для клиента и opaque store;
- capability seed выводится BLAKE3 `derive_key` из protected Device seed и
  recipient Account ID, временные buffers zeroize-ятся, private key не
  сохраняется и не передаётся;
- connection ticket/signature domain подняты до v10 и подписывают public write
  key; recipient выводит lookup только из своего уже проверенного contact ticket;
- channel равен domain-separated BLAKE3 hash write key, поэтому другой key не
  может захватить известный channel даже при первом PUT и после expiry;
- PUT headers несут canonical key/signature, связанную с channel, generation,
  body length и digest exact opaque HPKE envelope; invalid proof даёт 403 до
  Redb generation transaction;
- GET остаётся public-to-channel, а store не получает Account/Device ID,
  conversation label или plaintext envelope; IP/timing/size correlation и
  Sybil/global capacity остаются честно открытыми границами.

Проверки:

- новый crate проверяет restart stability, peer unlinkability, canonical wire
  encoding и binding подписи к channel/generation/body;
- real HTTP store отклоняет valid attacker key с `u64::MAX` на чужом channel и
  сохраняет legitimate generation;
- connection ticket v10 проверяет exact derived public key;
- полный 199-test workspace regression, strict Clippy и release verification
  зафиксированы в `development-environment.md`;
- contract зафиксирован в
  [`../docs/RFC-0055-unlinkable-ticket-write-capability.md`](../docs/RFC-0055-unlinkable-ticket-write-capability.md).

### M0.9.34 — authenticated multi-device endpoint failover: выполнено

Реализовано:

- stable v1 runtime contact и его ID не изменены; дополнительные peer devices
  сохраняются как local-device-signed append-only endpoint-candidate records в
  vault-primary Runtime repository;
- повторный `AddContact` для той же пары peer account/conversation добавляет
  другой Device endpoint, exact повтор идемпотентен, а смена path/policy для уже
  enrolled Device ID fail closed;
- contact ограничен четырьмя endpoint-кандидатами; IPC v9 и Windows GUI
  показывают enrolled candidate count;
- resolver независимо отбрасывает unreadable/expired/invalid descriptor,
  сортирует remaining candidates по authority revision, primary flag и Device
  ID, отклоняет same-revision authority equivocation;
- новый event materialize-ится ровно один раз по newest usable complete prekey
  directory; runtime последовательно отправляет тот же immutable event и
  принимает ack только от exact attempted Device ID;
- older endpoint допустим для delivery только если его exact certificate всё
  ещё active в newest observed roster; automatic sync использует только
  current-authority candidates и не обходит existing durable authority
  high-water;
- single-candidate contacts работают как раньше, queued/materialized/delivered
  records и automation policy не мигрируются;
- contract зафиксирован в
  [`../docs/RFC-0056-authenticated-endpoint-candidate-failover.md`](../docs/RFC-0056-authenticated-endpoint-candidate-failover.md).

### M0.9.35 — multi-candidate publication refresh and endpoint health: выполнено

Реализовано:

- одна refresh action snapshot-ит stable contact и максимум четыре signed
  endpoint candidates, независимо готовит opaque publication channel каждого
  peer Device;
- подготовленные HTTP GET выполняются параллельно, но authority pin, ratchet
  retirement/prekey observation, per-channel signed observation high-water и
  atomic descriptor replace коммитятся последовательно без state-lock race;
- partial success не откатывает успешные endpoints: IPC возвращает typed result
  каждого Device и exact `refreshed/total`, а automation считает action failed и
  применяет существующий bounded exponential backoff;
- complete automation success планируется по самому раннему expiry всех
  кандидатов; cross-channel generation не объявляется общей последовательностью;
- IPC v10 conversation read model содержит каждый candidate, `usable`/`stale`
  counts, authority revision, opaque channel и local observed publication
  high-water;
- Windows GUI показывает aggregate health, per-Device role/state/reason/high-
  water и честный partial refresh result;
- missing/expired/invalid descriptor, older/pinned-high-water authority,
  inactive certificate и same-revision authority equivocation становятся явным
  `stale`, не скрывая остальные usable endpoints;
- contract зафиксирован в
  [`../docs/RFC-0057-multi-candidate-ticket-refresh.md`](../docs/RFC-0057-multi-candidate-ticket-refresh.md).

### M0.9.36 — expiry-independent signed publication binding: выполнено

Реализовано:

- каждый primary/alternate endpoint получает отдельный local-device-signed
  vault-primary `.epb` record с exact contact, peer Account/Device,
  conversation, route/path contract и self-authenticating publication write
  key;
- binding не содержит network address/prekey и не истекает: он разрешает только
  opaque publication GET, тогда как expired descriptor остаётся `stale` и не
  участвует в delivery/sync;
- refresh channel берётся из binding, а fetched ticket до установки проходит
  полный normal expiry, Root/device authority, membership, requester,
  anti-rollback и exact pinned-write-key gate;
- existing M0.9.35 enrollment без binding мигрируется до network fetch из exact
  authenticated legacy ticket; пропускается только current-time validity его
  signed prekey pools, все подписи/identity/contract поля обязательны;
- resolver отклоняет valid signed descriptor, если он меняет уже pinned
  publication capability; stale endpoint status сохраняет известный channel и
  observation high-water;
- live two-endpoint regression проверяет одновременно expired pinned-binding
  refresh, expired legacy backfill, partial `1/2`, запрет использования второго
  stale endpoint и итоговый fresh `2/2`;
- contract зафиксирован в
  [`../docs/RFC-0058-expiry-independent-publication-binding.md`](../docs/RFC-0058-expiry-independent-publication-binding.md).

### M0.9.37 — authenticated own-device endpoint announcements: выполнено

Реализовано:

- IPC v11 экспортирует bounded canonical bundle: максимум 256 contacts и четыре
  endpoints, exact current Root-signed own roster, source/recipient, короткий
  expiry, authenticated ticket/binding и latest local observation;
- source Device подписывает plaintext, затем весь bundle HPKE-seal-ится exact
  active recipient Device; output создаётся no-clobber вне protected state;
- import требует exact byte-equal current roster, active source/recipient,
  existing membership и полный immutable ticket/endpoint contract; bundle не
  обновляет Root authority или membership;
- recipient создаёт собственные signed contact/candidate/`.epb` records и
  canonical external descriptors одной vault-primary transaction; fresh ticket
  может pin-ить только собственную peer Root authority/prekeys, expired ticket
  остаётся non-dialable и даёт лишь refresh binding;
- source observation сохраняется внутри local-device-signed `.aeo` evidence;
  lower generation, same-generation conflict и sibling equivocation fail
  closed, повторный import byte-exact idempotent;
- contract зафиксирован в
  [`../docs/RFC-0059-authenticated-own-device-endpoint-announcements.md`](../docs/RFC-0059-authenticated-own-device-endpoint-announcements.md).

### M0.9.38 — network own-device endpoint announcements: выполнено

Реализовано:

- IPC v12 команда `PushEndpointAnnouncements` принимает fresh recipient runtime
  ticket, требует same listener/requester Account, другого active Device и
  byte-exact current own roster, затем строит M0.9.37 envelope в памяти;
- wire ALPN v8 переносит один recipient-HPKE envelope до 7 MiB после обычной
  Root/session Device authorization; source bundle Device обязан совпасть с
  authenticated requester;
- recipient вызывает тот же `import_runtime_endpoint_announcement_envelope`
  gate, не принимает authority/membership из сети и выбирает собственный
  external `kilogram-received-endpoints` directory;
- после commit recipient подписывает ACK по bundle ID, source/recipient,
  authority revision, result counts и current session binding; exact replay
  idempotent, captured ACK в новой сессии недействителен;
- один bundle/ACK на connection, actor-serialized IPC и transport deadlines
  дают bounded backpressure; это foreground online transfer, а не mailbox,
  gossip, autostart или durable background retry;
- network regression с двумя runtime одного Root Account проверяет direct
  transfer, local materialization и source-side signed ACK verification;
- contract зафиксирован в
  [`../docs/RFC-0060-network-own-device-endpoint-announcements.md`](../docs/RFC-0060-network-own-device-endpoint-announcements.md).

### M0.9.39 — multi-audience own-device automation: выполнено

Реализовано:

- один long-lived Iroh endpoint теперь публикует обычный per-peer ticket и
  отдельный stable own-account ticket; runtime принимает exact primary Account
  либо exact-current собственный Root Account, не расширяя аудиторию дальше;
- own-device ticket атомарно заменяется рядом с IPC/primary ticket под полным
  Device ID и переиздаётся после live Root-signed roster update;
- IPC v13 конфигурирует opt-in Device-signed policy chain на каждый active
  sibling Device: interval, envelope validity, bounded retry и отдельные
  Ethernet/Wi-Fi/mobile/unknown permissions;
- foreground actor выполняет максимум один due push за automation check через
  тот же M0.9.38 session/import/recipient-ACK gate, сохраняет Device-signed
  success/backoff attempt и учитывает общий outbound-action limit;
- schedule переживает restart из vault-primary state, честно показывает
  `due`/`fresh`/`backoff`/`network-blocked`/`recipient-revoked`/`disabled` и не
  включает Windows Task Scheduler или иной OS background service;
- policy/attempt chains имеют global record bound и входят в существующую
  transactional Device-signed checkpoint compaction;
- сквозной regression использует внешний peer Account как primary audience,
  успешно принимает собственный Device по второму ticket, выполняет explicit и
  automatic direct push, затем проверяет сохранённые policy/attempt heads;
- contract зафиксирован в
  [`../docs/RFC-0061-multi-audience-own-device-automation.md`](../docs/RFC-0061-multi-audience-own-device-automation.md).

### M0.9.40 — pairwise own-device ticket discovery: выполнено

Реализовано:

- certified long-lived X25519 Device keys дают симметричный pairwise secret;
  domain-separated KDF дополнительно связывает Account ID, digest byte-exact
  current Root roster и направленные source/recipient Device IDs;
- каждое направление получает отдельную существующую self-authenticating
  store capability; store видит только pseudorandom channel, generation,
  expiry, public write proof и opaque ciphertext, но сохраняет visibility
  IP/timing/size/access correlation;
- source публикует fresh own-device connection ticket как Device-signed
  monotonic publication, HPKE-sealed только exact active recipient Device;
- recipient проверяет HPKE slot, source signature, exact account/roster,
  listener/requester binding и freshness, затем применяет существующий signed
  observation anti-rollback/equivocation gate и атомарно заменяет ticket в
  runtime-managed public directory;
- IPC v14 одним вызовом устанавливает Device-signed discovery policy и
  M0.9.39 announcement schedule; foreground workflow последовательно делает
  publish → fetch → authenticated push, сохраняя общий action limit и signed
  success/backoff chain;
- revoked recipient больше не планируется и остаётся доступен в status как
  `recipient-revoked`; discovery policy chains входят в 4096-record bound и
  существующую transactional signed checkpoint compaction;
- live regression с двумя runtime одного Root Account и отдельными public
  directories доказал отсутствие shared ticket file, directional channel
  symmetry, store lookup, managed ticket installation и последующий direct
  authenticated announcement push;
- contract зафиксирован в
  [`../docs/RFC-0062-pairwise-own-device-ticket-discovery.md`](../docs/RFC-0062-pairwise-own-device-ticket-discovery.md).

### M0.9.41 — roster-wide own-device availability: выполнено

Реализовано:

- одна local-Device-signed append-only policy pin-ит exact Root roster
  revision/digest, store URL, locator TTL/refresh, announcement
  interval/validity/retry и отдельные Ethernet/Wi-Fi/mobile/unknown permissions;
- deterministic reconciliation проецирует parent в существующие discovery и
  announcement child chains для каждого другого active Device, а historical
  revoked recipient получает disabled generation;
- одна vault-primary transaction сохраняет parent и все изменённые children;
  одинаковая конфигурация и повторный restart reconcile не создают records;
- startup подхватывает Device из полного обновлённого launch roster, а live
  removal-only Root update немедленно отключает отозванный recipient; hot
  enrollment намеренно не обходит существующий fresh-prekey/profile gate;
- IPC v15 даёт единые configure/status операции, active/configured/retired
  counts и полный per-recipient diagnostic; старые ручные child-команды после
  появления global policy fail closed;
- foreground actor обрабатывает ровно один due child за automation check и
  завершает publish/fetch/push до выбора следующего, сохраняя общий outbound
  limit и signed per-recipient backoff; это local process bound, не
  cross-machine distributed mutex;
- новая parent chain имеет отдельный 1024-record defensive limit, входит в
  общий 4096-record bound и compact-ится до authenticated head существующим
  transactional Device-signed checkpoint;
- pure regression проверяет initial projection, idempotence, expanded roster,
  revocation и restart no-op; live IPC regression проверяет немедленный disable
  после Root removal и сохранение после authenticated receipt restart;
- contract зафиксирован в
  [`../docs/RFC-0063-roster-wide-own-device-availability.md`](../docs/RFC-0063-roster-wide-own-device-availability.md).

### M0.9.42 — convergent sibling publication evidence: выполнено

Реализовано:

- endpoint-announcement bundle/signature domain v2 несёт на endpoint максимум
  один higher direct либо accepted observation, причём accepted variant
  сохраняет original observer signature и source Device acceptance signature;
- exact-current source выбирает максимум local direct и accepted high-water;
  equal generation с разными publication ID/ticket digest блокирует export;
- recipient unwrap-ит forwarded evidence до original observation, объединяет
  его со всеми local direct/accepted claims до любых descriptor/vault writes и
  fail-closed отклоняет same-generation equivocation;
- совместимый claim получает новую local-Device-signed `.aeo` acceptance и
  может идти дальше A -> B -> C; уже сохранённый publication tuple не создаёт
  новый record при другом bundle/witness;
- accepted evidence вошла в existing compaction trigger: после восьми records
  остаётся один highest signed `.aeo`, а checkpoint anchor связывает exact
  channel/generation/publication ID/ticket digest/evidence ID;
- старые checkpoint discriminants и `.aeo` v1 сохраняются, IPC остаётся v15;
  bundle v1 как короткоживущий artifact намеренно несовместим и пересоздаётся;
- regression с тремя Devices проверяет forwarded convergence, atomic rejection
  signed conflict, отсутствие mutation и reload одного compacted high-water;
- contract зафиксирован в
  [`../docs/RFC-0064-convergent-sibling-publication-evidence.md`](../docs/RFC-0064-convergent-sibling-publication-evidence.md).

### M0.9.43 — durable publication-conflict quarantine: выполнено

Реализовано:

- первый same-channel/same-generation mismatch сохраняет один append-only
  `.pcf` с canonical pair исходных signed observations, local Account/detector,
  временем и detector Device signature; proof ID content-addressed;
- restart проверяет обе observation signatures, реальный mismatch, canonical
  order, detector signature, local identity, exact filename и matching durable
  endpoint binding; tampered proof не принимается;
- повторный конфликт возвращает существующий proof ID и не создаёт record;
  conflicting bundle не пишет descriptor или accepted evidence;
- quarantined endpoint исключён из delivery, auto-sync и HTTP ticket refresh
  до network I/O, а install повторно проверяет stop после in-flight fetch;
- healthy endpoints остаются доступны только при exact pinned peer-authority
  high-water, поэтому fallback не может откатиться к revoked authority;
- IPC v16 добавляет explicit `quarantined`, proof/generation/time и отдельный
  count; Windows client показывает красное manual audit/re-enrollment состояние;
- successful network proof persistence отвечает rejection, но сохраняет
  long-lived runtime и публикует IPC change; неожиданные local-state failures
  всё ещё останавливают runtime fail closed;
- proof не compact-ится и не имеет GUI/store delete/override API; это local
  conflict evidence, а не peer equivocation proof или global consensus;
- contract зафиксирован в
  [`../docs/RFC-0065-durable-publication-conflict-quarantine.md`](../docs/RFC-0065-durable-publication-conflict-quarantine.md).

### M0.9.44 — sibling conflict proof и Root channel rotation: выполнено

Реализовано:

- endpoint-announcement bundle v3 переносит либо monotonic observation, либо
  полный detector-Device-signed conflict proof; detector обязан входить в
  byte-exact current Root roster, а recipient сохраняет собственный `.pcf`;
- detector-specific proof ID остаётся локальным, но canonical signed
  observation pair имеет stable evidence ID, одинаковый на всех siblings;
- ticket v11 подписывает publication channel epoch; epoch 0 сохраняет старую
  derivation, non-zero epoch domain-separates новый write key/channel;
- `runtime-publication-channel-rotate` ведёт append-only Device-signed `.pcrn`
  chain и требует restart runtime для публикации нового ticket;
- Root-signed `.pcr` связывает exact authority revision, evidence ID, peer
  Account/Device, old/new keys и digest свежего replacement ticket;
- apply заменяет descriptor fail-closed, сохраняет старый binding и `.pcf`, а
  effective channel меняется только после transactional resolution commit;
- один portable resolution применим на A/B/C с разными local proof IDs;
  tampering, wrong ticket, stale authority, same channel и conflicting second
  resolution отклоняются;
- IPC v17 и network ACK отдельно считают propagated conflict evidence;
- contract зафиксирован в
  [`../docs/RFC-0066-sibling-conflict-proof-and-root-channel-rotation.md`](../docs/RFC-0066-sibling-conflict-proof-and-root-channel-rotation.md).

### Следующий этап

1. M0.9.45: отделить Root signer от online runtime через bounded
   request/response artifact, переносить Root resolution к exact-current
   siblings и добавить guided desktop incident-recovery UI без передачи Root
   secret в GUI/runtime.
2. Optional autostart/background mode оставить отдельной явной настройкой, не
   обязательным Windows Task Scheduler step.
3. Добавить macOS/Linux/mobile providers той же platform boundary.
4. Спроектировать privacy-preserving gossip/mailbox и first-contact freshness;
   M0.9.29 скрывает payload/явные IDs, но не access correlation.
5. Membership removal и group governance проектировать вместе с ordered
   security events и MLS epoch; compact Merkle/range summary и независимый
   криптографический аудит остаются до публичного выпуска.
