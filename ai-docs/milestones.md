# Технические этапы

Актуально на: 2026-09-08.

## Правило нумерации и отчётов

- Канонический номер этапа задаётся этим файлом и соответствующим Git-коммитом;
  номера не резервируются и не пропускаются молча.
- В начале работы нужно назвать последний завершённый этап и ровно один
  следующий target. Если один turn фактически закрывает несколько этапов, в
  итоговом сообщении перечисляются все номера и коммиты, а не только последний.
- В финале каждого этапа указываются его номер, commit hash и номер следующего
  target. Package/binary SemVer не смешивается с product milestone `M`.

### M1.2.46 — guided recipient-import-consumption runtime attachment: выполнено

- Runtime process создаёт один actor, владеющий ровно одним M1.2.45 service и
  одним optional guided-context slot; IPC v34 handler делегирует только ему.
- Context process-local/non-durable, живёт не более 300 секунд и exact-привязан
  к state directory, Account, Device, coordinator plan/sequence и canonical
  membership digests.
- Context разрешено создавать только из already-authenticated recovery
  ceremony. IPC payload, environment и filesystem scan не создают authority;
  обычный runtime стартует unattached с прежним явным Error.
- Clear, expiry и restart уничтожают context и все opaque Plan stages;
  different live replacement отклоняется, exact install replay идемпотентен.
- Stable-path real recovery fixture прошёл 1/1; boundary — 11/11 mutations;
  readiness inventory расширен до 172 gates. CLI/desktop/timer/network,
  release/ZIP/tag и automatic GitHub build не использовались.

### M1.2.45 — process-owned recipient-import-consumption service core: выполнено

- Один service object владеет ровно одним M1.2.41 dispatcher и одним bounded
  M1.2.42 cache; nested request канонически декодируется до dispatch.
- Exact повтор Plan request возвращает исходный signed response byte-for-byte,
  не выбирает новый timestamp и не вызывает membership loader повторно.
- Commit требует exact same-process Plan. Stage сохраняется до TTL для
  безопасного `AlreadyConsumed` replay; новый service после restart Commit
  отвергает.
- Service принимает только явный live authenticated recovery context и не
  реконструирует consent/source authority из неполного IPC payload.
- Ordinary runtime handler пока остаётся service-unavailable. Stable-path real
  recovery fixture прошёл 1/1; boundary — 8/8 mutations; network/second host,
  release/ZIP/tag и automatic GitHub build не использовались.

### M1.2.44 — active runtime IPC recipient-import-consumption envelope: выполнено

- Postcard runtime IPC и signed descriptor атомарно мигрировали с 33/v14 на
  34/v15; command/response variants добавлены append-only и отражены в CDDL,
  schema, completion inventory, compatibility registry и live-binding gates.
- Runtime actor распознаёт новую команду, но до M1.2.45 возвращает отдельный
  service-unavailable Error; success response зарегистрирован, но недостижим.
- Process-local cache/dispatcher, CLI/desktop ceremony и product network не
  подключены; регистрация envelope не выдаёт mutation authority.
- v33 request и корректно v14-подписанный v33 descriptor отвергаются новым
  runtime. Stable-path downgrade tests прошли 2/2; boundary — 10/10 mutations.
- Readiness расширен до 170 gates; release/ZIP/tag, automatic GitHub build и
  second host не использовались, public default остаётся `legacy-only`.

### M1.2.43 — runtime IPC recipient-import-consumption migration preflight: выполнено

- Postcard command/response extension классифицировано как несовместимое:
  target закреплён как IPC v34 и signed descriptor domain v15;
- exact target variants — nested canonical request bytes и current-Device-signed
  response bytes; outer IPC не интерпретирует внутренний canonical payload;
- migration — hard cut co-installed binaries без dual reader, sniffing или
  silent downgrade; legacy descriptor/request отклоняются до connect/dispatch;
- атомарный activation-set включает enum/version/domain, completion/schema,
  compatibility registry и все 58 найденных live-binding gates; historical
  manifests не переписываются задним числом;
- boundary отклонил 10/10 mutations, readiness расширен до 169 gates;
- executable runtime остаётся v33/domain v14: handler, CLI/desktop, network,
  release/ZIP/tag, automatic GitHub build и second host отсутствуют; default
  `legacy-only`.

### M1.2.42 — bounded process-local consumption Plan session cache: выполнено

- В памяти процесса хранится только opaque M1.2.41 dispatcher Plan stage,
  exact plan summary и digest исходного Plan request; codec/database отсутствуют.
- Не более 32 живых stages; expiry purge, invalid capacity, collision и
  переполнение fail closed без вытеснения unexpired церемонии.
- Restart начинает с пустого cache. Commit без exact same-process Plan, с
  изменённым summary или после TTL отвергается; cache сам не grant-ит authority.
- Same-process Commit и replay проходят через M1.2.41: durable authorization и
  consume receipt остаются обязательными, replay даёт `AlreadyConsumed`.
- Active IPC/CLI/desktop/network не подключены. Stable fixture 1/1; следующий
  отдельный этап обязан явно мигрировать IPC 33/v14 -> 34/v15.

### M1.2.41 — signed dormant import-consumption response dispatcher: выполнено

- Plan канонически перепроверяется против exact runtime descriptor, остаётся
  read-only и не создаёт authorization/mutation state.
- Commit принимает только непрозрачный Plan stage того же dispatcher и того же
  descriptor; stage skip и Device/descriptor substitution fail closed.
- Exact authorization durable до mutation, completed consume receipt durable до
  Device-signed success response; replay возвращает тот же semantic receipt без
  loader и второго consume.
- Active runtime registration, socket/CLI/desktop/network и public-default
  change отсутствуют. Stable signed fixtures 2/2, boundary 10/10, aggregate
  readiness 167/167.

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

### M0.9.45 — offline Root conflict recovery и sibling convergence: выполнено

Реализовано:

- online Device создаёт bounded Device-signed `.pcrq` с exact-current authority
  revision, полным conflict proof, old/new channel summary и byte-exact peer
  replacement ticket, не принимая и не загружая Root secret;
- отдельная `account-publication-conflict-authorize` открывает только offline
  Root и request, независимо проверяет active requester/detector, embedded peer
  ticket, epoch/route/key и выпускает self-contained `.pcrp`;
- Root resolution v2 подписывает request ID вместе с stable evidence, peer,
  old/new keys, ticket digest и authority revision;
- runtime apply принимает один response, сохраняет `.pcf`, использует прежний
  descriptor-first fail-closed commit и остаётся idempotent;
- endpoint-announcement bundle v4 переносит resolution с exact replacement
  endpoint; sibling принимает его только при совпадающем Root roster, local
  proof и old binding, после чего C -> A -> B автоматически сходятся;
- ACK v2 и IPC v18 отдельно считают resolution inventory/insertions;
- Windows incident panel выбирает quarantined channel, создаёт `.pcrq`,
  показывает offline CLI-команду и применяет `.pcrp`, но не имеет Root path или
  Root-signing action;
- regression проверяет tampering, wrong Root, idempotence и автоматическое
  трёхустройственное распространение;
- contract зафиксирован в
  [`../docs/RFC-0067-offline-root-conflict-recovery.md`](../docs/RFC-0067-offline-root-conflict-recovery.md).

### M0.9.46 — live publication incident lifecycle: выполнено

Реализовано:

- IPC v19 добавляет authenticated actor-команды для live rotation собственной
  publication capability, создания `.pcrq` и применения `.pcrp`;
- live rotation разрешена только для exact primary peer текущего listener,
  сохраняет Iroh Endpoint/route/certificate/device directory, transactionally
  пишет signed epoch rotation и атомарно заменяет public ticket;
- state-first порядок имеет retry reconciliation: committed epoch без
  опубликованного ticket переиздаётся как `AlreadyPresent`, не создавая лишний
  epoch; running state ahead of durable state отклоняется;
- request/apply используют текущий actor state, absolute bounded artifact
  paths, не принимают Root secret и возвращают typed reports с
  `runtime_restart_required=false`;
- повторный apply остаётся `Unchanged`, исходный `.pcf` не удаляется, а
  resolution продолжает распространяться существующим bundle v4;
- Windows incident panel объединяет peer-side rotation, request, offline Root
  handoff и response apply и показывает `quarantined` / `authorization pending`
  / `resolved locally` без требования остановить runtime;
- live regressions проверяют сохранение endpoint ID, atomic ticket replacement,
  crash/retry reconciliation boundary, durable epoch, online request/apply,
  idempotence и IPC ping после mutation;
- contract зафиксирован в
  [`../docs/RFC-0068-live-publication-incident-lifecycle.md`](../docs/RFC-0068-live-publication-incident-lifecycle.md).

### M0.9.47 — hardened offline publication-conflict ceremony: выполнено

Реализовано:

- публичные `publication-conflict-request-inspect` и
  `publication-conflict-response-inspect` bounded-проверяют подписи, evidence,
  embedded replacement ticket и показывают exact account/peer/channel/epoch/
  digest данные без Root или runtime state;
- full `.pcrq`/`.pcrp` остаются на removable media, а no-clobber PNG содержит
  только compact versioned claim `kilogram://publication-conflict/v1/`;
- optional `--verification-qr-file` принимает ровно один bounded PNG/JPEG QR и
  требует field-for-field совпадения claim с signed artifact; request/response
  kind и artifact ID не взаимозаменяемы;
- 96-bit KPC1 code domain-separated от request ID, account/revision, evidence,
  peer, old/new channels и ticket digest одинаков для matching request/response;
- offline authorize требует `--confirm-code`, выполняет public inspection и
  optional QR match до resolve/load Root, запрещает untrusted request/response
  внутри Root directory и сохраняет no-clobber output;
- IPC v20 возвращает artifact digest и confirmation code, а Windows incident
  UI проверяет их форму, показывает код, digest и точные inspect/sign/response
  команды, не получая Root material;
- regression проверяет QR round-trip/exact match, cross-kind rejection,
  неправильный код до отсутствующего Root, wrong Root, correct signing,
  одинаковый request/response code, tampering и live idempotent apply;
- contract зафиксирован в
  [`../docs/RFC-0069-hardened-offline-publication-conflict-ceremony.md`](../docs/RFC-0069-hardened-offline-publication-conflict-ceremony.md).

### M0.9.48 — minimal offline publication-conflict appliance: выполнено

Реализовано:

- отдельный `kilogram-offline.exe` имеет ровно три команды: read-only
  `inspect-request`, confirmation-gated `authorize` и read-only
  `inspect-response`; Root загружается только второй командой;
- normal dependency graph не содержит Iroh, Tokio, Reqwest,
  `kilogram-runtime-ipc`, `kilogram-protocol`, `kilogram-session`,
  `kilogram-state`, `kilogram-store` или `kilogram-transport-iroh`;
- offline ticket verifier сохраняет byte-exact raw JSON endpoint при проверке
  v11 Device signature, отдельно bounded-разбирает public endpoint summary и
  полностью проверяет certificate, current prekey directory и write key;
- `.pcrq`, `.pcrp`, Root resolution v2, KPC1 и QR claim v1 не изменились;
  cross-component Three-Device regression принимает online request новым
  decoder, подписывает его offline Root и применяет response прежним runtime;
- `scripts/verify-kilogram-offline-boundary.ps1` fail-closed отклоняет появление
  запрещённых network/runtime packages в normal dependency graph;
- `scripts/package-kilogram-offline.ps1` использует locked release build,
  запрещает чистовую упаковку dirty worktree, не перезаписывает output и создаёт
  stable-name directory/ZIP с README, build provenance и SHA-256 manifest;
- ZIP entries имеют детерминированные порядок и timestamp; две упаковки одного
  source/toolchain/binary дали byte-identical archive hash;
- Windows incident UI теперь показывает команды `kilogram-offline`, оставляя
  совместимые general-CLI команды только fallback/debug surface;
- Windows test workflow больше не полагается на ошибочное предположение, что
  loopback bind никогда не вызывает Firewall prompt: обычный запуск по
  hash-suffixed Cargo path запрещён, а
  `scripts/run-cargo-tests-stable.ps1` по умолчанию только компилирует и копирует
  harness под постоянным именем; реальный запуск требует явного `-Run`;
- contract зафиксирован в
  [`../docs/RFC-0070-minimal-offline-publication-conflict-appliance.md`](../docs/RFC-0070-minimal-offline-publication-conflict-appliance.md).

### M0.9.49 — shared publication-conflict artifacts: выполнено

Реализовано:

- новый `kilogram-publication-conflict` является единственным владельцем
  signed publication/observation, proof/evidence, Device request, Root
  resolution/response и compact KPC1 claim;
- прежние реализации удалены из `kilogram-cli` и `kilogram-offline`; transport,
  HTTP/HPKE, QR image, filesystem и runtime adapters остаются снаружи;
- wire versions, serde/postcard field order, signature domains, IDs, bounds и
  KPC1 не изменены; online `RoutePolicy` переходит в transport-independent
  artifact enum через явное сопоставление;
- locked dependency gate подтверждает отсутствие Iroh/Tokio/Reqwest,
  image/QR, runtime/state/store/session/protocol/transport crates в shared
  boundary и отклоняет duplicate wire definitions вне неё;
- offline dependency gate требует общий crate и по-прежнему не допускает
  network/runtime surface;
- focused network-free regression строит полную conflict chain и требует
  одинаковые online/offline IDs, digests, KPC1 и byte-exact response round-trip;
- network-bearing harnesses только скомпилированы под stable names и намеренно
  не запускались без согласованного окна для Windows Firewall;
- contract зафиксирован в
  [`../docs/RFC-0071-shared-publication-conflict-artifacts.md`](../docs/RFC-0071-shared-publication-conflict-artifacts.md).

### M0.9.50 — reproducible offline release и bounded Cargo cache: выполнено

Реализовано:

- exact clean Git source set дважды копируется в независимые source roots с
  отдельными Cargo target roots и одним SHA-256 manifest;
- обе `kilogram-offline` сборки используют `--frozen --release`, pinned Rust
  1.98.0/MSVC target, disabled incremental, commit `SOURCE_DATE_EPOCH`, общий
  path remap и `/Brepro`;
- bounded `REPRODUCIBILITY.json` создаётся только при exact равенстве размера и
  SHA-256 двух EXE; отдельный verifier повторно проверяет inputs, безопасные
  filenames, build boundary и оба artifacts;
- clean package fail closed требует record exact текущего `HEAD` и включает
  source manifest, `Cargo.lock`, `rust-toolchain.toml` и
  `reproducibility_verified=true`; dirty development package остаётся явным
  unverified escape hatch;
- record честно сообщает `same-host-separate-clean-roots`: отдельный host/
  builder image и signed public provenance пока не заявлены;
- новый `verification` Cargo profile сохраняет line tables, но отключает
  incremental; stable harness script использует его и по умолчанию не запускает
  EXE;
- heavy Cargo scripts запускают parent с BelowNormal и по умолчанию оставляют
  половину logical processors свободной; `-CargoJobs`/environment позволяют
  explicit override;
- cache maintenance read-only по умолчанию показывает debug/incremental,
  verification, release и total, автоматически вызывается stable harness
  workflow, предупреждает после 40 GiB и имеет guarded explicit prune/reset
  modes;
- разовая exact очистка удалила 145,230 файлов / 105.66 GiB; после первичного
  warmup target занимал 3.99 GiB;
- contract зафиксирован в
  [`../docs/RFC-0072-reproducible-offline-release-and-bounded-cargo-cache.md`](../docs/RFC-0072-reproducible-offline-release-and-bounded-cargo-cache.md).

### M0.9.51 — blind mailbox contract и durable bounded store: выполнено

Реализовано:

- отдельный network-free `kilogram-mailbox` crate владеет opaque asynchronous
  delivery boundary и не зависит от runtime/session/state/event store/transport;
- mailbox address состоит из независимых Ed25519 read/write public capabilities,
  а registration-free Mailbox ID является domain-separated hash этих keys;
- write authorization подписывает exact mailbox/item, TTL, ciphertext length и
  digest; list/delete требуют отдельной read capability;
- recipient-encrypted HPKE envelope AAD связывает mailbox/item и bounded
  creation/expiry, plaintext не появляется в storage API;
- Redb store pin-ит store signing identity, атомарно поддерживает live counters,
  TTL и per-mailbox/global item/byte limits;
- exact put replay возвращает прежнюю signed stored receipt, conflicting reuse
  отклоняется, conditional delete требует receipt ID и оставляет signed
  tombstone до исходного expiry;
- address, authorization и receipt ingress имеют bounded decode-and-verify;
- fail-closed boundary script запрещает network/runtime/image зависимости и
  Account/Device/conversation/event/message identifier types;
- три network-free regression tests проверяют HPKE binding, capability failure,
  expiry, quota, idempotence, conditional deletion, tombstone replay,
  persistence и store-key pinning;
- release builds и ZIP packaging для обычных внутренних этапов не запускаются:
  используются debug `check`/Clippy и отдельный unoptimized verification profile;
- contract зафиксирован в
  [`../docs/RFC-0073-blind-mailbox-contract-and-durable-store.md`](../docs/RFC-0073-blind-mailbox-contract-and-durable-store.md).

### M0.9.52 — bounded mailbox HTTP и crash-safe client ledger: выполнено

Реализовано:

- `kilogram-mailbox` теперь владеет versioned bounded Postcard wire objects для
  `put`, paged `list` и conditional `delete`; signed list authorization включает
  fresh nonce, exclusive cursor и limit не более 8 ciphertext items;
- existing `kilogram-ticket-store` обслуживает `/v1/mailboxes/...` в том же
  loopback-only процессе за обязательным HTTPS reverse proxy; второй EXE не
  создан;
- сервис создаёт exclusive 32-byte store signing secret, запрещает symlink/
  неверную длину, pin-ит public key в отдельной mailbox Redb и сохраняет key
  между restart;
- `kilogram-mailbox-client` запрещает redirects и remote plain HTTP, bounded
  читает ответы и проверяет positive stored/delete receipts по заранее
  известному store key;
- durable client ledger хранит exact encrypted pending request, signed outbound
  receipt, application commit marker и signed deletion receipt в четырёх
  отдельных bounded Redb tables; mailbox/read/HPKE secret keys туда не пишутся;
- delete request невозможно получить до записи stable application commit ID;
  logical receipt/commit replay идемпотентен при новом local timestamp, cleanup
  удаляет expired pending ciphertext и receipts;
- fail-closed boundary gate запрещает client library зависеть от Iroh,
  identity/protocol/ratchet/runtime/session/state/store/transport и запрещает
  application identifier types или новый binary;
- network-free tests: 4 mailbox + 2 client прошли; отдельный filtered pure test
  проверил все три server routes без bind/listener. Сетевые harnesses не
  запускались, release и ZIP не создавались;
- contract зафиксирован в
  [`../docs/RFC-0074-bounded-mailbox-http-and-client-ledger.md`](../docs/RFC-0074-bounded-mailbox-http-and-client-ledger.md).

### M0.9.53 — recipient-bound mailbox provisioning: выполнено

Реализовано:

- новый network-free `kilogram-mailbox-provisioning` crate разделяет локальную
  full read/write capability и экспортируемую peer write-only capability;
- локальная capability Device-подписана и HPKE-sealed к собственному encryption
  key; peer offer Device-подписан и HPKE-sealed к exact recipient Device;
- подпись/AAD связывают обе Account/Device стороны, owner encryption key,
  opaque conversation scope, mailbox address, canonical HTTPS URL, pinned store
  key, creation и bounded expiry;
- импорт требует exact local recipient, current enrolled peer endpoint,
  pinned Root-signed peer authority high-water, exact conversation/contact scope
  и неистёкший offer;
- runtime append-only vault state получил bounded local/peer mailbox records;
  секреты не попадают в tickets, endpoint publications или mailbox-client
  ledger;
- CLI `runtime-mailbox-offer-create`, `runtime-mailbox-offer-import` и
  `runtime-mailbox-status` создают no-clobber artifact, идемпотентно импортируют
  exact replay и честно сообщают `provisioned-not-yet-enabled`;
- fail-closed boundary script запрещает provisioning crate network/runtime/
  protocol/event-store dependencies и новый executable;
- 4 network-free tests прошли; CLI/workspace network-bearing harnesses не
  запускались, release и ZIP не создавались;
- contract зафиксирован в
  [`../docs/RFC-0075-recipient-bound-mailbox-provisioning.md`](../docs/RFC-0075-recipient-bound-mailbox-provisioning.md).

### M0.9.54 — runtime mailbox fallback и reverse ACK: выполнено

Реализовано:

- verified peer mailbox binding подключён к existing materialized outbox только
  после bounded failure всех direct/relay endpoint candidates;
- Device-signed append-only dispatch связывает queue/contact/peer,
  conversation, binding/mailbox/item/event и expiry; deterministic item ID и
  exact retained PUT делают crash/retry идемпотентным;
- crash между dispatch commit и ledger enqueue автоматически восстанавливает
  pending envelope после повторной проверки exact queue/event/current binding;
- store-signed receipt переводит очередь только в `mailbox-stored`; delivered
  marker создаётся исключительно после peer-signed acknowledgement;
- local receive bindings опрашиваются bounded страницами по 8 items, payload
  HPKE-open-ится и проверяется против exact source/recipient/conversation/
  binding до обычной membership/ratchet/event проверки;
- AuthorizedEvent, local projection и ACK либо acknowledgement+delivered marker
  коммитятся existing state transaction до client-ledger commit и conditional
  delete; повтор после crash безопасен;
- reverse acknowledgement получает собственный deterministic mailbox item,
  durable enqueue до network PUT и exact retry при сбое;
- serialized actor сначала обслуживает live delivery, затем не чаще одного
  mailbox действия за 5 секунд; network waits не удерживают state lock и ошибка
  mailbox не завершает runtime;
- authenticated runtime IPC v21 и Windows projection различают
  `mailbox-pending`, `mailbox-stored`, `mailbox-expired`, `mailbox-failed`, а
  MailboxStatus показывает binding/dispatch и ledger received/deleted counts;
- fail-closed script проверяет live-before-fallback, commit-before-delete,
  reverse ACK, signed state, IPC contract и отсутствие нового EXE;
- network-free mailbox-client tests прошли; network-bearing harnesses только
  компилируются, release и ZIP не создаются;
- contract зафиксирован в
  [`../docs/RFC-0076-runtime-mailbox-fallback-and-ack.md`](../docs/RFC-0076-runtime-mailbox-fallback-and-ack.md).

### M0.9.55 — authenticated mailbox capability lifecycle: выполнено

Реализовано:

- network-free provisioning contract получил Device-signed ordered update с
  exact owner/recipient Account+Device, opaque scope, generation, predecessor,
  binding и activate/revoke action;
- first activation, rotation и revocation fail closed проверяют contiguous
  chain, gap/fork, wrong target и повторный binding;
- `runtime-mailbox-offer-create` теперь atomic persist-ит generation one вместе
  с local binding; `runtime-mailbox-rotate` и `runtime-mailbox-revoke` делают
  последующие явные transitions;
- serialized runtime раз в 30 секунд выбирает максимум один unacknowledged
  update и отправляет его exact current recipient endpoint только после ACK
  predecessor;
- wire transfer идёт через existing authenticated Device session; bounded
  request/ack/rejection не помещают capability в public discovery;
- recipient сверяет update с authenticated session, exact enrolled contact и
  full retained chain, затем atomic persist-ит peer binding/update до
  session-bound recipient-signed ACK;
- current non-revoked chain head стал обязательным для receive polling,
  fallback selection и retained dispatch; при pending rotation previous
  acknowledged receive binding временно poll-ится как safe handover overlap,
  следующий transition до ACK запрещён, old append-only records не удаляются;
- runtime state получил bounded `.lmu`/`.pmu`/`.mua`, а IPC v22 показывает
  local acknowledged/unacknowledged updates и local/peer revoked heads;
- отдельный fail-closed gate проверяет chain, ordering, ACK, public discovery
  boundary и отсутствие нового executable;
- network-free provisioning/protocol regressions проходят; complete runtime
  path compile/clippy-verified, но не запускался по сети без controlled test
  window, release/ZIP не создавались;
- contract зафиксирован в
  [`../docs/RFC-0077-authenticated-mailbox-capability-lifecycle.md`](../docs/RFC-0077-authenticated-mailbox-capability-lifecycle.md).

### M0.9.56 — transport-independent mailbox capability convergence: выполнено

Реализовано:

- `MailboxCapabilityConvergence` вынесен в network-free provisioning crate и
  полностью реконструируется из signed update/ACK records без clock, socket,
  filesystem или hidden mutable state;
- recipient-signed session-bound ACK codec перенесён из CLI в shared contract,
  а protocol session отображается в opaque 32-byte boundary type;
- один автомат проверяет owner predecessor-ACK progression, выбирает максимум
  один eligible retry, классифицирует inbound append/idempotent replay и
  определяет current/rotation-overlap/inactive binding states;
- runtime snapshot loading, automatic control retry, recipient apply, peer
  write selection и owner receive overlap используют этот общий автомат;
- deterministic network-free regression проходит activation, lost ACK,
  encode/decode restart, idempotent retry, rotation overlap, второй restart,
  revocation и final convergence;
- fail-closed gate запрещает duplicate CLI ACK codec, network/runtime
  dependency, потерю runtime wiring и новый executable;
- provisioning tests: 6 passed; network-bearing processes не запускались,
  release и ZIP не создавались;
- contract зафиксирован в
  [`../docs/RFC-0078-transport-independent-mailbox-capability-convergence.md`](../docs/RFC-0078-transport-independent-mailbox-capability-convergence.md).

### M0.9.57 — authenticated mailbox desktop control: выполнено

Реализовано:

- IPC v23 добавляет typed activation/rotation/revocation commands и secret-free
  lifecycle result; `MailboxStatus` теперь включает current managed heads,
  generations, ACK/revocation и convergence state по exact peer Device;
- activation/rotation принимают только public HTTPS service URL, pinned public
  store key и validity; ни read/write capability, ни Root/device secret не
  возвращаются в GUI;
- serialized runtime actor выполняет mutation под existing state lock,
  persistent commit предшествует success response, state change публикуется;
- desktop IPC path не создаёт manual offer artifact и не запускает отдельный
  CLI mailbox subprocess;
- Windows panel выбирает exact enrolled endpoint Device, даёт Activate/Rotate/
  Revoke/Refresh и показывает aggregate delivery плюс per-device lifecycle
  convergence;
- integrated authenticated IPC test покрывает mapping четырёх desktop actions,
  а fail-closed gate запрещает secret fields, обход single owner/manual export и
  новый executable;
- network-bearing tests/processes не запускались; targets compile/clippy-
  verified, release и ZIP не создавались;
- contract зафиксирован в
  [`../docs/RFC-0079-authenticated-mailbox-desktop-control.md`](../docs/RFC-0079-authenticated-mailbox-desktop-control.md).

### Следующий этап

### M0.9.58 — controlled mailbox lifecycle field harness: выполнено

Реализовано:

- debug runtime получил explicit one-shot fault только через process env:
  recipient durable применяет authenticated capability update, не подписывает
  ACK, завершает vault mirror и автоматически останавливается на точной restart
  boundary;
- non-debug build отвергает fault setting до открытия endpoint; hook нельзя
  передать через IPC/profile/GUI, он не сохраняется в state и не добавляет EXE;
- foreground PowerShell helper запускает existing debug CLI from profile,
  no-clobber пишет phase log и восстанавливает process environment;
- отдельный helper снимает только secret-free authenticated IPC mailbox status
  в фиксированные evidence filenames;
- fail-closed evidence verifier связывает exact Account/Device/conversation,
  update/binding IDs и generations для lost ACK, `AlreadyPresent` retry,
  rotation и revocation, требует direct+relay route evidence;
- тот же verifier требует store-signed `mailbox-stored`, recipient
  `deleted-after-commit`, durable ledger counters и `opaque-redb-v1` startup log,
  отклоняет application identifiers в предоставленном store output;
- network-free verifier self-test принимает полный synthetic evidence set и
  отклоняет добавленную metadata leak; static gate проверяет debug/release
  boundary, apply/drop/sign ordering, vault mirror stop и отсутствие package/
  нового executable;
- реальный Alice/Bob прогон, перенос debug EXE и GUI observation отложены до
  согласованного test window; сеть не запускалась, release/ZIP не создавались;
- contract и процедура зафиксированы в
  [`../docs/RFC-0080-controlled-mailbox-lifecycle-field-test.md`](../docs/RFC-0080-controlled-mailbox-lifecycle-field-test.md)
  и
  [`../docs/M0.9.58-MAILBOX-LIFECYCLE-FIELD-TEST-RU.md`](../docs/M0.9.58-MAILBOX-LIFECYCLE-FIELD-TEST-RU.md).

### M0.9.59 — independent builder и signed provenance foundation: выполнено

Реализовано:

- добавлен только ручной `workflow_dispatch` для GitHub-hosted `windows-2025`;
  push/PR/schedule/release не запускают тяжёлую сборку;
- оператор обязан передать exact clean commit и SHA-256, уже подтверждённый
  двумя M0.9.50 local clean-root builds; выбранный workflow commit обязан
  совпасть с input;
- independent builder повторяет pinned Rust target, commit epoch, disabled
  incremental, source path remap и `/Brepro`, после locked fetch строит только
  network-free `kilogram-offline.exe` двумя Cargo jobs;
- GitHub attestation создаётся только при byte-identical совпадении; divergent
  artifact сохраняется на 14 дней только как bounded diagnosis и job падает;
- actions pinned к exact commits, checkout не сохраняет credentials, job имеет
  только read/OIDC/attestation/artifact-metadata permissions;
- production verifier требует оба M0.9.50 artifacts, external executable и
  record одного commit/hash/length, затем через `gh` проверяет attestation обоих
  external subjects, exact repository, signer workflow, source digest и запрет
  self-hosted runner;
- network-free self-test принимает coherent synthetic evidence и отклоняет
  tampered EXE; отдельный static boundary запрещает automatic triggers,
  floating actions, packaging и production attestation bypass;
- последующий manual run exact commit успешно скомпилировал target на GitHub,
  но fail closed обнаружил расхождение 1024 bytes: local MSVC linker 14.44,
  hosted 14.51; attestation не создавался, а strict cross-environment
  reproducibility перенесена в public-release hardening и не блокирует M1;
- contract зафиксирован в
  [`../docs/RFC-0081-independent-builder-and-signed-provenance.md`](../docs/RFC-0081-independent-builder-and-signed-provenance.md).

### M0.9.60 — portable M1 acceptance kit и HTTPS mailbox preflight: выполнено

Реализовано:

- explicit clean-HEAD builder создаёт обычную no-clobber папку, а не ZIP;
  собирает только debug `kilogram-cli.exe`/`kilogram-windows.exe` с bounded
  Cargo jobs и stable names, не запускает network process;
- `BUILD-INFO.json` связывает exact commit, debug profile, длину и SHA-256 обоих
  EXE; ordered driver проверяет эти значения перед каждым действием;
- переносимый driver перечисляет 23 фиксированных шага M0.9.58 и вызывает
  existing foreground runtime/status/evidence helpers с exact phase filenames;
- actual Alice/Bob role config, runtime profile, bearer-bearing IPC descriptor,
  state и keys обязаны находиться вне shared kit/evidence; записи evidence
  остаются no-clobber;
- mailbox preflight до phase 01 проверяет absolute HTTPS URL, default Windows
  TLS trust, disabled redirects, exact bounded `ok\n`, startup log,
  `opaque-redb-v1` и pinned store key, отклоняя application IDs;
- documented store-on-Alice topology оставляет Alice PC, loopback-only store и
  HTTPS proxy online при остановленном messenger; whole-PC-off delivery честно
  требует external always-on store/peer;
- network-free preflight self-test отвергает wrong key и remote HTTP; новый
  static gate проверяет trust/no-redirect, stable hashes, private-state
  boundary, debug/no-archive policy, полный step inventory и отсутствие нового
  executable;
- negative dirty-worktree generator guard прошёл до build; clean generator
  path, Cargo build и реальная сеть на этом этапе не запускались, поэтому
  release/ZIP/Firewall surface не создавались;
- contract и operator guide зафиксированы в
  [`../docs/RFC-0082-m1-acceptance-kit-and-mailbox-preflight.md`](../docs/RFC-0082-m1-acceptance-kit-and-mailbox-preflight.md)
  и
  [`../docs/M0.9.60-M1-ACCEPTANCE-RU.md`](../docs/M0.9.60-M1-ACCEPTANCE-RU.md).

### M0.9.61 — default embedded volunteer blind storage: выполнено

Реализовано:

- новый runtime launch profile включает volunteer storage по умолчанию для
  новых пользователей; desktop позволяет отключить роль и изменить каталог и
  лимиты;
- default policy разделяет 200 MiB общего ciphertext storage и 30-дневный
  application-payload traffic: Ethernet 500 MiB, Wi-Fi 500 MiB, mobile 0,
  unknown 0;
- обычный `kilogram-cli.exe` runtime владеет embedded `MailboxOnly` store и
  останавливает его вместе с собой; нового EXE, Windows service или Task
  Scheduler нет;
- blind mailbox capacity реально ограничен значением runtime profile, а
  request/response body traffic хранится durable по network-class scope и не
  сбрасывается после restart;
- volunteer data обязаны жить вне protected client state; standalone HTTPS
  store явно переопределён как optional bootstrap/reference/test fixture, а не
  целевая централизованная инфраструктура;
- текущий ingress честно остаётся ephemeral loopback adapter без peer
  advertisement, discovery и replication; это рабочий embedded storage engine,
  но ещё не распределённая сеть хранения;
- network-free tests проверяют profile round trip/unsafe paths, durable
  30-day quota и mailbox-only fail-closed route; static boundary фиксирует
  defaults, desktop controls, same-runtime ownership и отсутствие нового EXE;
- contract зафиксирован в
  [`../docs/RFC-0083-default-volunteer-blind-storage-role.md`](../docs/RFC-0083-default-volunteer-blind-storage-role.md).

### M0.9.62 — dedicated Iroh ingress и signed volunteer offer: выполнено

Реализовано:

- обычный runtime условно добавляет отдельный
  `kilogram/m0/blind-mailbox/1` ALPN на тот же Iroh endpoint только когда
  volunteer policy разрешает текущий network class;
- bounded peer frames переносят existing capability-authenticated
  PUT/LIST/DELETE; outer response фиксирует operation и domain-separated digest
  exact request, а inner successful response сохраняет store-signed receipts;
- Iroh remote endpoint identity используется для per-peer rate limit, но не
  даёт mailbox authority; blind wire не получает Account/Device/conversation
  IDs;
- provider connections исполняются отдельно от messenger sessions в bounded
  tasks с semaphore и одним bidirectional request на connection, не блокируя
  основной accept loop;
- peer path использует тот же durable network-class application-payload budget
  и реальный blind-store capacity из M0.9.61;
- runtime создаёт 15-minute store-signed base64url offer с exact serialized
  Iroh endpoint, store key, `BoundedVolunteer`, capacity/max-record hints,
  issue/expiry и random nonce;
- offer distribution пока manual-only; automatic discovery, provider selection
  и replication не заявлены;
- pure tests проверяют offer tamper/expiry, Iroh EndpointAddr round trip,
  capability frame/request digest, durable PUT/idempotent replay и peer rate
  limit; static gate запрещает потерю ALPN/limits/no-new-EXE boundary;
- contract зафиксирован в
  [`../docs/RFC-0084-volunteer-storage-iroh-ingress-and-offers.md`](../docs/RFC-0084-volunteer-storage-iroh-ingress-and-offers.md).

### M0.9.63 — bounded provider registry и deterministic selection: выполнено

Реализовано:

- `kilogram-mailbox-client` получил отдельный durable Redb registry для exact
  signed provider offers: default 256, hard max 4096, без Account/Device/
  conversation/mailbox/capability linkage;
- import проверяет protocol size, current-time store signature, canonical
  encoding и валидный Iroh endpoint; exact replay идемпотентен, replacement
  обязан иметь строго больший signed issue time, same-generation fork/rollback
  отклоняются;
- expired authenticated records prune-ятся до capacity check; full live set
  fail closed не делает attacker-controlled eviction;
- deterministic BLAKE3 selection принимает opaque 32-byte per-item salt,
  ограничен 1..=8 и выбирает максимум один store key на parsed Iroh endpoint
  identity; это diversity floor, но не Sybil resistance;
- authenticated runtime IPC v24 и диагностический CLI добавили import/select,
  возвращая только public offer metadata; raw endpoint остаётся внутри actor,
  capabilities и social IDs не проецируются;
- provider startup теперь честно сообщает bounded manual import/registry и
  deterministic transport-distinct selection; automatic peer gossip и actual
  replication пока false;
- network-free tests и новый static gate проверяют tamper/expiry/canonical,
  replay/replacement/capacity/prune, stable selection/dedup, privacy projection
  и отсутствие нового executable;
- contract зафиксирован в
  [`../docs/RFC-0085-bounded-volunteer-provider-registry-and-selection.md`](../docs/RFC-0085-bounded-volunteer-provider-registry-and-selection.md).

### M0.9.64 — authenticated bounded volunteer provider gossip: выполнено

Реализовано:

- existing Kilogram Iroh ALPN и Root-authorized Device handshake переносят
  отдельный bidirectional provider-gossip request/response без нового listener,
  executable или глобального directory;
- canonical short-lived frame ограничен 8 offers, 2 KiB/offer, 24 KiB wire,
  60 seconds;
  каждый offer повторно проверяется по store signature, current expiry,
  15-minute age, unique store key и parsed Iroh endpoint;
- Redb registry получил совместимую отдельную hop-provenance table: direct=0,
  transmission increments, max=2; gossip replay не может снизить сохранённый
  hop, direct observation может;
- subset выбирается fresh random entropy и domain-separated BLAKE3, максимум
  один store key на transport identity; response связан с exact request digest;
- runtime автоматически обменивается offers с authority-current contact не чаще
  раза в пять минут, использует endpoint failover, импортирует собственный provider offer
  и обновляет его каждые пять минут;
- payload не содержит Account/Device/conversation/mailbox IDs или mailbox
  capabilities; hop остаётся honest-client bound, не cryptographic path proof;
- unit/runtime/protocol tests и static gate проходят без запуска endpoint;
  actual mailbox replication остаётся false;
- contract зафиксирован в
  [`../docs/RFC-0086-authenticated-bounded-volunteer-provider-gossip.md`](../docs/RFC-0086-authenticated-bounded-volunteer-provider-gossip.md).

### M0.9.65 — resumable volunteer mailbox replication: выполнено

Реализовано:

- exact encrypted mailbox PUT связан с digest полного Device-signed durable
  outbox dispatch и fresh random 32-byte provider-selection salt;
- отдельный backwards-compatible Redb ledger сохраняет immutable plan, exact
  request, independent store-signed receipts и durable attempt timestamps;
- default policy выбирает до 3 remote transport-distinct providers и требует 2
  receipts; own endpoint, duplicate transport identity, retained store key и
  provider с малым max-record исключаются;
- PUT идёт по existing `kilogram/m0/blind-mailbox/1`, outer response связан с
  exact request digest, inner receipt проверяется против request и store key;
- incomplete plans достраиваются после restart и после завершения primary
  upload с 60-second cooldown; expiry удаляет plan/attempt/receipts вместе;
- direct delivery остаётся первым. HTTPS upload пока сохраняется для recipient
  compatibility и не блокируется volunteer failures; recipient Iroh read ещё
  не заявлен;
- новый EXE/listener/service/global directory не добавлен; contract зафиксирован
  в
  [`../docs/RFC-0087-resumable-volunteer-mailbox-replication.md`](../docs/RFC-0087-resumable-volunteer-mailbox-replication.md).

### M0.9.66 — bounded volunteer mailbox retrieval: выполнено

Реализовано:

- recipient runtime выбирает fresh bounded sample из verified provider registry,
  исключает own endpoint и одновременно опрашивает не более 3 transport-distinct
  providers по existing blind-mailbox Iroh ALPN;
- каждый LIST просит ровно 1 item; outer reply, store key, stored receipt,
  scoped read capability и E2EE runtime payload проверяются до локальной записи;
- общий commit path для HTTPS и volunteer source сначала durable добавляет event
  в append-only history и лишь затем создаёт delete eligibility;
- отдельные Redb inbound commit/deletion tables связывают exact
  mailbox/item/store/transport/stored receipt с application commit и signed
  delete receipt; interrupted DELETE возобновляется после restart;
- одинаковая реплика на разных stores учитывается независимо; cleanup bounded
  исходным receipt expiry;
- HTTPS mailbox остаётся compatibility fallback до внешнего evidence. Random
  local-registry sampling не заменяет exact replica locator и не гарантирует
  немедленное обнаружение в большой сети;
- новый EXE/listener/service/directory не добавлен; contract зафиксирован в
  [`../docs/RFC-0088-bounded-volunteer-mailbox-retrieval.md`](../docs/RFC-0088-bounded-volunteer-mailbox-retrieval.md).

### M0.9.67 — no-archive volunteer mailbox field test: выполнено с recovery evidence

Реализовано:

- clean-HEAD generator создаёт обычную debug-папку без ZIP, release build и
  автоматического запуска сетевого executable;
- provider bootstrap создаёт два независимых private профиля с разными Account
  Root, Device/state, endpoint и volunteer store key; secrets не копируются в
  shared evidence;
- no-clobber PowerShell driver покрывает provider offers, Alice/Bob import,
  sender two-of-two receipts, явную остановку Alice, Bob Iroh LIST/commit/
  signed DELETE и restart без повторной volunteer delivery;
- fail-closed verifier связывает один run manifest с двумя store keys, двумя
  transport identities, Alice offline boundary, двумя Bob commits/deletes и
  единственным marker в локальной history;
- два providers можно запустить на одном operator host, но такой результат
  доказывает только protocol/transport/store mechanics, а не независимость
  операторов, физических отказов или Sybil resistance;
- HTTPS mailbox остаётся обязательной compatibility-копией для этого теста:
  field kit не заявляет полностью server-free offline delivery;
- добавлен простой three-folder harness: `1`=Alice, `2`=два providers,
  `3`=Bob; шесть scripts автоматически создают disposable identities,
  membership, contacts, loopback compatibility mailbox и весь evidence;
- live state автоматически хранится в `%LOCALAPPDATA%`, чтобы Yandex Disk не
  синхронизировал открытые Redb-файлы; оператор не вводит ID и пути вручную;
- stale bootstrap IPC descriptor больше не принимается за готовый runtime:
  scripts требуют успешный ping, а pre-queue Alice failure можно безопасно
  повторить без удаления identities/state и без риска duplicate queue;
- первый real provider PUT выявил response-flush race и runtime-fatal offline
  sync; provider теперь подтверждает доставку полного response stream до close,
  sync failure остаётся non-fatal, а post-queue run возобновляет exact durable
  replication plan без второго сообщения;
- contract и русская инструкция зафиксированы в
  [`../docs/RFC-0089-volunteer-mailbox-field-kit.md`](../docs/RFC-0089-volunteer-mailbox-field-kit.md)
  и
  [`../docs/M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md`](../docs/M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md).
  Упрощённый operator contract зафиксирован в
  [`../docs/RFC-0090-three-folder-m0967-harness.md`](../docs/RFC-0090-three-folder-m0967-harness.md).
- Реальный Bob runtime durable записал тестовое событие и acknowledgement, но
  не дошёл до replica ledger/DELETE: подготовка необязательного reverse mailbox
  acknowledgement вернула `no current peer mailbox binding` и ошибочно прервала
  уже состоявшийся application commit. Runtime теперь сохраняет этот сбой как
  `unavailable-after-commit`, после чего продолжает exact replica commit/delete;
  harness распознаёт единственный durable post-commit event и возобновляет его
  idempotently без второго сообщения. Долгое ожидание печатает прогресс каждые
  15 секунд.
- Recovery build `b095d7a` развернут в existing shared kit без нового имени EXE;
  те же два provider states перезапущены, а Bob idempotently продолжил уже
  durable application commit. Обе volunteer replicas были прочитаны по Iroh,
  записаны в store-specific inbound ledger и удалены только после commit;
  restart не доставил их повторно, history содержит marker ровно один раз.
- Финальный fail-closed verifier вернул `result=verified` для run
  `20260918-231823`: два разных store key/transport identity, sender 2/2
  receipts, Alice-offline boundary, Bob 2 commits/2 deletes и отсутствие
  redelivery подтверждены. Поскольку run продолжался через несколько repair
  builds, sender acceptance и завершённая replication связаны как
  `sender_evidence=recovered-log-chain` через один exact durable queue ID;
  это квалифицированный recovery field evidence, а не clean single-revision
  release attestation.
- Harness теперь печатает текущую Bob stage и heartbeat не только при ожидании
  inbound events, но также во время IPC readiness, provider import/retry и
  restart observation. Финальная проверка идемпотентно принимает уже записанный
  идентичный boundary log после исправления verifier, но fail closed отвергает
  отличающийся файл.

### M0.9.68 — authenticated volunteer replica-set locator: выполнено

Реализовано:

- mailbox owner выбирает до трёх transport-distinct volunteer stores из уже
  проверенных offers и включает их canonical store keys в новый
  `ActivateWithReplicaSet` action существующей Device-signed capability chain;
- commitment содержит только 2–8 store keys, связан domain-separated ID с
  mailbox binding и не несёт Account/Device/conversation/mailbox IDs или
  capability secret; полный набор не отправляется providers;
- legacy `Activate` wire variant не изменён; capability, созданная до появления
  минимум двух offers, явно остаётся в `legacy-random-fallback`;
- sender до сетевого PUT durable сохраняет exact commitment в отдельной Redb
  locator table рядом с replication plan; конфликтующий replay fail closed,
  restart сохраняет маршрут, expiry удаляет locator вместе с plan;
- sender и recipient разрешают committed store keys индексированными lookup в
  bounded provider registry и используют fresh signed endpoint offer того же
  store; unrelated random providers в locator-capable path не сканируются;
- recipient по-прежнему спрашивает максимум один opaque item у каждого из не
  более чем трёх stores и сохраняет application-commit-before-signed-DELETE;
- endpoint gossip остаётся нужен, но только как transport resolution для точно
  известного store key; global directory, новый listener/EXE/server не добавлен;
- provider/queue/replication idle inspection переведён на настоящий Redb
  read-only режим; отсутствующая DB не создаётся, byte-exact vault shadow не
  дрейфует от одного open/close, а expiry cleanup остаётся явной mirrored write;
- HTTPS upload пока остаётся compatibility copy до clean external evidence
  exact-locator path и решения для legacy bindings;
- contract зафиксирован в
  [`../docs/RFC-0091-authenticated-volunteer-replica-set-locator.md`](../docs/RFC-0091-authenticated-volunteer-replica-set-locator.md),
  fail-closed static gate —
  `scripts/verify-kilogram-volunteer-replica-locator-boundary.ps1`.

### M0.9.69 — clean external exact-locator field evidence: завершено

Реализовано:

- новый three-folder/six-launch harness сохраняет привычные роли `1`=Alice,
  `2`=два providers, `3`=Bob, но provider runtimes и hash-consistent fresh
  offer publication теперь обязательно существуют до первой mailbox activation;
- Bob импортирует оба transport-distinct offers через authenticated runtime IPC,
  затем создаёт `exact-authenticated` capability с двумя store keys; static gate
  проверяет причинный порядок import-before-activation;
- Bob и Alice одновременно запускают runtimes до отправки сообщения: Alice
  durable применяет exact signed capability update, Bob получает session-bound
  acknowledgement, и только после этого prepare scripts завершаются;
- перед convergence оба клиента требуют, чтобы final runtime заменил bootstrap
  ticket, и через authenticated live IPC проверяют fresh descriptor того же
  enrolled Account/Device; stale ticket или identity substitution fail closed;
- sender import-ит fresh offer snapshot, durable ставит одно сообщение, доказывает
  exact commitment/resolution `2/2`, attempts и store-signed receipts ровно от
  precommitted stores, затем останавливает Alice runtime и loopback HTTPS fixture;
- Bob использует тот же commitment, адресно poll-ит ровно два committed store,
  durable commit-ит и signed-delete-ит обе replicas; restart не доставляет их
  повторно, history содержит marker один раз;
- fail-closed evidence verifier связывает fresh endpoint validation, activation, capability apply/ACK,
  sender и recipient одним commitment ID, запрещает `legacy-random-fallback`,
  partial resolution и provider substitution; synthetic self-test отдельно
  доказывает отклонение stale ticket, legacy marker и чужого poll key;
- Yandex Disk получает только bounded public artifacts/evidence; live Redb/state
  остаётся в `%LOCALAPPDATA%`. Offers читаются из immutable local snapshot только
  после совпадения двух SHA-256/expiry с одним publication manifest;
- generator требует clean HEAD, создаёт stable-name debug binaries с двумя Cargo
  jobs по умолчанию, не создаёт ZIP/release и не запускает сеть;
- HTTPS остаётся loopback compatibility copy и не используется Bob после
  sender-offline boundary. Удалять compatibility path до успешного внешнего run
  и решения legacy upgrade policy нельзя;
- exact store keys для sender/recipient берутся из закрытого Bob
  pre-activation import evidence, а не из открытых live provider logs, которые
  Yandex Disk вправе не синхронизировать до остановки providers;
- после аварийного завершения runtime специальный Redb `RepairAborted` при
  read-only mailbox-ledger inspection запускает ровно один mirrored writable
  repair под state lock; остальные ошибки по-прежнему fail closed;
- clean2 подтвердил корректные identity/membership/fresh-ticket/exact-capability
  фазы, но автоматически выбранный `euc1` повторил известный connection timeout
  в обе стороны. Следующий одноразовый kit сохраняет `auto`, явно pin-ит все
  четыре runtime profile к ранее проверенному `aps1` и fail closed проверяет
  relay URL во всех retained phases; direct upgrade остаётся разрешён;
- contract зафиксирован в
  [`../docs/RFC-0092-clean-external-exact-locator-field-run.md`](../docs/RFC-0092-clean-external-exact-locator-field-run.md),
  gates — `scripts/verify-kilogram-m0969-exact-locator-kit-boundary.ps1` и
  `scripts/verify-kilogram-m0969-exact-locator-evidence.ps1`.

Полевой recovery run `20260919T005959Z` подтвердил функциональный путь после
прерванного Bob runtime: оба exact pre-activation store key совпали с poll и
source sets, получены и committed две volunteer-Iroh replicas, обе удалены,
restart redelivery отсутствует, history содержит marker один раз. HTTPS fixture
в этот момент был offline. Из-за repair/retry chain этот результат не закрывает
требование clean single-revision attestation, поэтому статус этапа не меняется.

Clean2 run `20260919-141905` не является protocol failure: оба provider offer,
fresh peer tickets и exact `2/2` commitment были созданы и проверены, после чего
Alice sync и Bob capability update одновременно завершались timeout через
`euc1`. Открытые provider logs и меняющийся publication manifest объясняют
постоянный статус Yandex synchronization, но consumers их не читают. Provider
processes штатно остановлены marker-ом; этот run не возобновляется.

Из clean revision `6cea6074fbb4116639db3f79dddc0bb4bf373fe9`
создан одноразовый `C:\Users\Paul\YandexDisk\!M\M0.9.69-clean3` с pinned
`aps1`. Все 11/11 artifacts совпали с BUILD-INFO по SHA-256/length,
PowerShell parse errors — 0, до запуска `1\shared` отсутствует, ZIP — 0,
профиль debug, Cargo jobs — 2, generator network execution — false.

Clean external run `20260919-144857` успешно завершён на разных
Alice/Bob hosts/networks. Машинный verifier вернул `result=verified`:
activation=`providers-before-capability`, capability convergence=
`recipient-applied-owner-acknowledged`, sender resolution/receipts=`2/2`, Alice
offline до receive, recipient resolution/commit-delete=`2/2`, legacy fallback
отсутствует, restart redelivery отсутствует, message marker встречается в
history ровно один раз. Все runtime phases подтвердили `route_policy=auto` и
pinned `aps1`; markers `alice-sent`, `bob-complete` и `providers-stopped`
присутствуют. M0.9.69 закрыт.

### M0.9.70 — automatic legacy mailbox upgrade: завершено

Реализовано:

- long-running runtime каждые 30 секунд ищет не более одной локальной receive
  capability с активным `legacy-random-fallback` head;
- автоматическая ротация разрешена только после durable recipient-signed ACK
  текущего head и при наличии минимум двух active transport-distinct volunteer
  providers; повторная проверка внутри provisioning fail closed запрещает
  сохранить ещё одну legacy generation при race/expiry;
- миграция переиспользует ordered `ActivateWithReplicaSet`: новый binding
  получает exact signed locator, старый остаётся `RotationOverlap` до ACK новой
  generation, а exact/revoked/unacknowledged heads повторно не вращаются;
- migration не создаёт outbox item, не меняет event/history и не переотправляет
  сообщение; после restart exact head исключает duplicate rotation;
- authenticated runtime IPC v25 и CLI/GUI mailbox status раскрывают для каждого head discovery mode,
  commitment ID и store count без capability secret;
- HTTPS mailbox descriptor/copy сохранён как compatibility path; его retirement
  остаётся отдельным следующим решением;
- integration test проходит полный legacy -> ACK -> exact 2-provider rotation,
  predecessor overlap, no-message-requeue и no-duplicate checks;
- contract зафиксирован в
  [`../docs/RFC-0093-automatic-legacy-mailbox-upgrade.md`](../docs/RFC-0093-automatic-legacy-mailbox-upgrade.md),
  static gate —
  `scripts/verify-kilogram-mailbox-legacy-upgrade-boundary.ps1`.

### M0.9.71 — exact mailbox HTTPS-copy retirement: завершено

Реализовано:

- sender с authenticated exact locator теперь сначала выполняет volunteer Iroh
  replication, а не HTTPS PUT;
- suppression разрешён только после durable plan/dispatch binding, exact
  equality commitment/store keys и минимум двух signed transport-distinct
  receipts именно из committed set;
- legacy receipt от прежнего random provider не считается qualifying после
  установки exact locator;
- pending HTTP upload и новый `ReplicatedOutboundCommit` меняются одной
  Immediate-durable Redb transaction; restart до commit повторяет только
  локальный переход по сохранённым receipts, а после commit не воскрешает HTTP;
- старый формат HTTPS stored receipt сохранён byte-compatible в той же таблице
  через однозначный framed тип новой записи;
- legacy capability, incomplete exact threshold и reverse acknowledgement
  сохраняют HTTPS compatibility; уже созданная HTTP copy задним числом не
  удаляется;
- runtime evidence явно различает `http_put=not-attempted` и `attempted`, а
  durability — `exact-volunteer-replication` и `https-compatibility`; IPC v25
  не менялся;
- mailbox-client tests проверяют atomic transition, replay, expiry и запрет двух
  receipts от одного transport; replication test исключает legacy receipt из
  exact threshold; CLI test фиксирует три policy branches;
- contract зафиксирован в
  [`../docs/RFC-0094-exact-mailbox-https-copy-retirement.md`](../docs/RFC-0094-exact-mailbox-https-copy-retirement.md),
  static gate —
  `scripts/verify-kilogram-mailbox-https-retirement-boundary.ps1`.

### M0.9.72 — clean no-HTTPS volunteer field harness: готово к внешнему запуску

Реализовано:

- существующий clean exact-locator harness расширен новым milestone mode без
  копирования второго набора сложных PowerShell orchestration helpers;
- M0.9.72 kit не содержит `kilogram-ticket-store.exe`, не запускает loopback
  mailbox fixture и дважды fail closed проверяет недоступность inert endpoint
  `http://127.0.0.1:18787` — до identity/capability setup и перед send;
- совместимый capability tuple пока сохраняет валидный публичный Ed25519 key из
  RFC 8032 test vector и unreachable loopback URL, поскольку удаление самого
  legacy descriptor требует отдельной wire migration;
- sender требует ровно по одной строке suppression,
  `runtime_mailbox_http_put=not-attempted` и
  `runtime_mailbox_delivery_durability=exact-volunteer-replication`; attempted
  HTTP, compatibility durability, retained-copy reason или exact completion
  failure останавливают run;
- post-send offline boundary повторно доказывает отсутствующий binary,
  недоступный endpoint, no-PUT и остановленный Alice runtime до Bob retrieval;
- Bob сохраняет проверенный M0.9.69 контракт: тот же exact commitment/store-key
  set, две volunteer-Iroh replicas, application commit перед двумя signed
  DELETE, отсутствие redelivery после restart и один marker в history;
- общий exact-locator evidence verifier теперь принимает строго ограниченный
  label prefix `m0969|m0972`; новый verifier сначала применяет весь старый
  контракт, затем дополнительные no-HTTPS условия;
- synthetic negative tests отвергают attempted HTTP PUT и reachable
  compatibility endpoint;
- generator сохраняет три папки и шесть запусков, собирает только stable-name
  debug `kilogram-cli.exe` с двумя Cargo jobs, не создаёт release/ZIP и не
  запускает сеть;
- contract зафиксирован в
  [`../docs/RFC-0095-clean-no-https-volunteer-field-run.md`](../docs/RFC-0095-clean-no-https-volunteer-field-run.md),
  gates — `scripts/verify-kilogram-m0972-no-https-kit-boundary.ps1` и
  `scripts/verify-kilogram-m0972-no-https-evidence.ps1`.

Внешний two-host run ещё не выполнен, поэтому M0.9.72 пока не имеет статуса
`завершено`. Прерванный или неуспешный запуск не возобновляется как clean
evidence: для новой попытки создаётся новый kit/run.

Из clean revision `36a4a6c76a02d9f8d6ff70adc5a259951b8b2885` создан
локальный комплект `.tmp\m0972-no-https-36a4a6c`. Все 11/11 записанных payload
artifacts совпали по SHA-256/length, PowerShell parse errors — 0,
`kilogram-ticket-store.exe`/ZIP/предсозданный `1\shared` отсутствуют, профиль —
debug, Cargo jobs — 2, generator network execution — false.

Первый внешний запуск `20260919-170816` выполнен между Alice на проводном ПК и
Bob через мобильную сеть, без VPN. Он не дошёл до send и не является clean
evidence. Обе стороны получили `relay_status=online` и один и тот же pinned
`aps1`; Bob разрешил exact provider set `2/2` и успешно открывал direct sessions
к обоим providers. При этом Alice -> Bob automatic sync шесть раз завершился
30-second timeout, Bob -> Alice capability update также завершался timeout, а
обе стороны позднее зарегистрировали соответствующие входящие Initial/handshake
attempts как timeout. Это исключает ошибку порядка запуска и отсутствие provider
publication; рабочая гипотеза — accept starvation/symmetric lockstep внутри
runtime actor: tick handler последовательно await-ит долгие outbound операции и
в это время не poll-ит сохранённый accept future. Нужен детерминированный
regression и исправление responsiveness accept loop до нового field kit.

### M0.9.73 — cooperative runtime outbound scheduling: локально завершено

Реализовано:

- deterministic mutual-auto-sync regression сначала воспроизвёл прежний
  symmetric outbound/accept timeout, поэтому полевая гипотеза стала локально
  подтверждённым scheduling defect;
- runtime держит не более одной owned outbound-cycle task, а основной actor во
  время её network waits продолжает poll постоянного Iroh accept, shutdown и
  завершение task;
- authenticated IPC mutation принимается только между outbound cycles, а
  существующий state-directory lock сериализует background mutation с accepted
  application session;
- reciprocal automatic sync выбирает ровно одного initiator по Account ID, а
  для устройств одного аккаунта — по Device ID; passive peer обслуживает
  elected inbound session;
- wire formats, device/account authority, executables и network services не
  изменились;
- новый regression стал green, прежний durable outbox/automatic-sync test
  сохранён, полный `kilogram-cli` suite дал 79 passed, 0 failed за 291.41s при
  `--jobs 2 --test-threads=2`;
- граница описана в
  [`../docs/RFC-0096-cooperative-runtime-outbound-scheduling.md`](../docs/RFC-0096-cooperative-runtime-outbound-scheduling.md)
  и закреплена `scripts/verify-kilogram-runtime-cooperative-scheduling.ps1`,
  включённым в M1 acceptance kit.
- replacement harness получил отдельный wrapper
  `new-kilogram-m0973-no-https-kit.ps1`, milestone/label/private-root
  `M0.9.73`/`m0973`/`M0973` и новый fail-closed boundary. Он наследует полный
  M0.9.72 no-HTTPS evidence contract, но также требует cooperative scheduling и
  отклоняет milestone mismatch; шесть запусков, debug-only build, два Cargo job,
  отсутствие ZIP/release и server fixture сохранены.

Run `20260919-170816` остаётся неуспешным и не принимается задним числом.
M0.9.73 локально завершён, но его исправление ещё требует свежего clean
двухсетевого no-HTTPS evidence run.

Из clean harness revision `fe7f46df25c7389291dfb52d06d897d933b7ee15`
собран fresh комплект `.tmp\m0973-no-https-fe7f46df25c7` и без изменений
скопирован в `C:\Users\Paul\YandexDisk\!M\M0.9.73`. Его 11/11 manifest
artifacts совпадают, PowerShell parse errors — 0, 13 файлов занимают 57,741,367
bytes, ZIP/ticket-store/precreated shared отсутствуют. Внешний run ещё не
выполнен и должен начинаться только после полной синхронизации этой новой папки.

Внешний M0.9.73 run `20260919-192354` подтвердил scheduling исправление: Alice
и Bob завершили setup/convergence, Alice получила exact signed receipts `2/2`,
зафиксировала `exact-volunteer-replication`, не пыталась выполнить HTTPS PUT и
ушла offline. Run не принят, потому что Bob не получил ни одной replica. Его
runtime повторял `open mailbox replication ledger read-only: Database repair
aborted`: принудительная остановка предыдущей фазы оставила Redb recovery
marker, а read-only inspection принципиально не мог его снять.

### M0.9.74 — crash recovery mailbox replication ledger: завершено

Реализовано:

- нормальная idle inspection остаётся read-only и не меняет byte-authenticated
  state;
- только typed `redb::DatabaseError::RepairAborted` разрешает один writable
  recovery open под runtime state-directory lock и `VaultDualWriteGuard`;
- после recovery обязательно повторяется исходная read-only inspection;
  permission/I/O/corruption не маскируются, ledger не удаляется и не
  пересоздаётся;
- regression запускает child test process, реально открывает Redb и завершает
  его через `process::exit`, подтверждает первоначальный read-only failure,
  recovery и последующий чистый read-only reopen;
- targeted crash regression, strict clippy и полный `kilogram-cli` suite прошли:
  81 passed, 0 failed за 299.75s при `--jobs 2 --test-threads=2`;
- контракт зафиксирован в
  [`../docs/RFC-0097-replication-ledger-crash-recovery.md`](../docs/RFC-0097-replication-ledger-crash-recovery.md)
  и `scripts/verify-kilogram-runtime-mailbox-replication-recovery.ps1`, который
  включён в M1 acceptance-kit builder/boundary;
- existing exact-locator harness расширен milestone-aware mode без копирования
  orchestration: wrapper `new-kilogram-m0974-no-https-kit.ps1`, label/private
  root `m0974`/`M0974` и новый fail-closed boundary. Evidence verifier требует
  recovery + inherited cooperative/no-HTTPS границы и отвергает milestone
  mismatch; debug-only build, два Cargo job, шесть запусков, отсутствие
  release/ZIP/ticket-store/generator network execution сохранены.

M0.9.73 run остаётся полезным diagnostic evidence, но не clean acceptance.
Replacement field kit должен иметь отдельные milestone/label/private root
`M0.9.74`/`m0974`/`M0974` и не переиспользовать старое состояние. Harness
готов. Из clean revision `aa295dfd183cc4edaf1347f0555916285a9a9cb0`
собран `.tmp\m0974-no-https-aa295dfd183c` и byte-identical скопирован в
`C:\Users\Paul\YandexDisk\!M\M0.9.74`: 11/11 manifest artifacts, 13 файлов,
57,748,745 bytes, PowerShell parse errors=0, ZIP/ticket-store/precreated shared
отсутствуют, profile=debug, Cargo jobs=2, generator network execution=false.
`BOUNDARIES.log` содержит inherited M0.9.72/M0.9.73, cooperative scheduling,
replication recovery и M0.9.74 gates.

Clean two-network run `20260919-203722` независимо перепроверен по полностью
синхронизированному evidence и принят с `result=verified`. Exact source revision
совпала с kit: `aa295dfd183cc4edaf1347f0555916285a9a9cb0`. Итог:

- provider resolution и signed receipts на sender — `2/2`;
- `runtime_mailbox_delivery_durability=exact-volunteer-replication` и
  `runtime_mailbox_http_put=not-attempted`;
- Alice offline до Bob retrieval;
- Bob получил обе replicas через volunteer-Iroh и для каждой записал
  `deleted-after-commit`;
- restart Bob не дал redelivery, history содержит один message occurrence
  (`event_count=2` вместе со служебным событием);
- сохранённые boundaries включают no-HTTPS, cooperative scheduling,
  replication recovery и M0.9.74 kit gate.

Fresh-state field run не потребовал repair marker; сам recovery branch доказан
отдельным real child-process crash regression. Вместе эти проверки закрывают и
узкую crash boundary, и полный внешний no-HTTPS lifecycle.

### M0.9.75 — service-free exact mailbox capability v2: завершено локально

Реализовано:

- новый v2 local binding и recipient offer больше не сериализуют HTTPS URL,
  `MailboxServiceDescriptor` или singleton central store key; применяются
  отдельные v2 signature/HPKE/content-ID domains;
- `ActivateExactVolunteer` связывает v2 offer с canonical Device-signed exact
  replica-set commitment; создание/ротация fail closed требуют минимум двух
  active transport-distinct volunteer providers;
- bounded v1 decode/read/revoke и explicit legacy CLI/IPC commands сохранены
  для совместимости; acknowledged active v1 head автоматически ротируется в
  v2 без message replay, а chain validation разрешает `v1 -> v2`, сохраняет v2
  revocation и запрещает `v2 -> v1` downgrade;
- v2 outbound message и reverse acknowledgement используют только exact set;
  неполный threshold остаётся durable pending и не имеет HTTPS fallback,
  recipient v2 polling также никогда не создаёт HTTP client;
- authenticated runtime IPC поднят до v26, secret-free mailbox status показывает
  `v1-legacy-https`/`v2-exact-volunteer`; desktop default create/rotate больше не
  спрашивает URL/store key и использует только exact commands;
- targeted provisioning и automatic migration regressions проходят; static
  `verify-kilogram-service-free-mailbox-capability-v2.ps1` включён в M1
  acceptance builder/boundary;
- контракт зафиксирован в
  [`../docs/RFC-0098-service-free-exact-mailbox-capability-v2.md`](../docs/RFC-0098-service-free-exact-mailbox-capability-v2.md).

Граница этапа: новый protocol path больше не несёт фиктивную central mailbox
authority. Legacy v1 остаётся только совместимым read/migration surface. Новый
external v2 field run, Sybil-resistant provider diversity и access-correlation
privacy в этот этап не входят.

### M0.9.76 — fresh service-free v2 field lifecycle: завершено и externally verified

Реализовано:

- existing M0.9.69/M0.9.74 three-folder/six-launch orchestration расширена
  отдельными `M0.9.76`/`m0976`/`M0976` branches без копирования harness;
- Bob создаёт receive capability только через
  `runtime-mailbox-exact-offer-create`; Alice public coordination, manifest,
  activation/import evidence и generated `BUILD-INFO.json` не содержат
  compatibility URL или central store key;
- успешная v2 отправка теперь честно сообщает
  `runtime_mailbox_https_compatibility_copy=absent-v2-exact-volunteer`, а не
  legacy-семантику `suppressed`;
- новый fail-closed verifier наследует полный exact-locator lifecycle и
  дополнительно отвергает central tuple, compatibility endpoint, legacy
  retained/suppressed copy и attempted HTTP PUT;
- acceptance по-прежнему требует exact receipts `2/2`, Alice offline до Bob
  retrieval, два commit-before-delete, одно сообщение и отсутствие redelivery
  после restart;
- generator использует stable debug EXE, Cargo jobs=2, не создаёт ZIP/release,
  не запускает сеть и не добавляет executable/server;
- M0.9.72/73/74 boundaries, service-free v2 gate, M1 boundary, PowerShell parse,
  targeted Rust test, strict Clippy и `cargo fmt --check` проходят;
- контракт зафиксирован в
  [`../docs/RFC-0099-service-free-v2-field-run.md`](../docs/RFC-0099-service-free-v2-field-run.md).

Граница этапа: локально доказана корректность harness и fail-closed evidence
contract. Реальный service-free v2 lifecycle считается внешне подтверждённым
только после fresh two-network запуска этого комплекта.

Из clean revision `c39f5ce9ec6effd49cf355036119e28d830c9d28`
собран `.tmp\m0976-service-free-v2-c39f5ce9ec6e`. Проверено: 11/11 manifest
artifacts, 9 PowerShell файлов без parse errors, 13 файлов/57,863,721 bytes,
ZIP=0, `kilogram-ticket-store.exe`=0, compatibility endpoint в BUILD-INFO
отсутствует. Bundled verifier self-test отвергает central tuple и legacy
suppressed-copy marker. Все 13 файлов затем byte-identical скопированы в
`C:\Users\Paul\YandexDisk\!M\M0.9.76`; перед запуском нужно дождаться полной
синхронизации этой fresh папки на ноутбук.

Первый внешний запуск дошёл до capability convergence, но был отклонён до
Alice runtime/queue/message: `02_SEND_ALICE.ps1` ошибочно трактовал false
`legacyNoHttpsCompatibility` как необходимость запустить отсутствующий
`kilogram-ticket-store.exe`. Исправление отделяет явный
`if (-not $noHttpsCompatibility)` legacy start; AST-based gate теперь требует,
чтобы единственный store-start структурно принадлежал именно этому guard.
Старый run не возобновляется; требуется fresh `M0.9.76-retry1`.

Исправление зафиксировано revision
`ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae`. Новый local kit
`.tmp\m0976-service-free-v2-ff38e1b89dc5` прошёл 11/11 artifact hashes,
bundled negative verifier и generated AST guard; 13 файлов/57,863,913 bytes
byte-identical скопированы в
`C:\Users\Paul\YandexDisk\!M\M0.9.76-retry1`; принятый retry использовал
только эту полностью синхронизированную папку.

Fresh retry run `20260920-001614` из exact revision
`ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae` независимо перепроверен bundled
verifier-ом и принят с `result=verified`. Подтверждены:

- `v2-exact-volunteer`, отсутствие central service descriptor и compatibility
  endpoint;
- sender exact resolution/receipts `2/2`, exact volunteer durability и
  `runtime_mailbox_http_put=not-attempted`;
- Alice runtime offline до Bob retrieval;
- два `volunteer-iroh` inbound commit и два `deleted-after-commit`;
- restart без redelivery, history `event_count=2` и ровно одно вхождение
  message marker;
- service-free v2, cooperative scheduling, replication recovery и M0.9.76 kit
  boundaries в retained evidence.

Этап доказывает controlled two-host mechanism/lifecycle, но не физическую или
операторскую независимость двух providers, поскольку оба provider process в
этом тесте работали на Alice host. Sybil resistance и access-correlation
privacy остаются отдельными задачами.

### M0.9.77 — controlled toolchain-bundled LLD reproduction: local gate verified

Реализовано:

- same-host clean-root builder и manual GitHub Windows builder больше не
  выбирают mutable MSVC `link.exe`; оба явно используют `rust-lld.exe` из
  target-specific `bin` закреплённого Rust 1.98.0 toolchain;
- стабильные rustc flags задают explicit linker path, `lld-link` flavor,
  `/Brepro` и прежний source path remap; нестабильный
  `linker-features=+lld` запрещён boundary gate-ом;
- `kilogram-offline` отдельно включает штатный BLAKE3 feature `pure`; resolved
  target graph использует Rust intrinsics и не линкует BLAKE3 C/ASM objects,
  что убирает MSVC `cl.exe` как ещё один artifact input;
- первый clean LLD release pair имел одинаковый размер и отличался ровно 20
  host-local PDB-derived bytes: COFF timestamp, два debug-directory timestamps
  и часть CodeView GUID; code/layout/imports/RVAs совпали;
- parser-bounded PE normalization v1 валидирует DOS/PE32+/sections/debug
  bounds и обнуляет только COFF/debug timestamps и 16-byte RSDS GUID; retained
  divergent fixtures после неё byte-identical, любое иное отличие остаётся;
- reproducibility records подняты до format v3 и фиксируют mode/source/file,
  SHA-256, размер, flavor и reproducibility flag линкера, а также exact
  PE-normalization policy;
- production verifier требует exact equality linker identity между local и
  GitHub records до artifact/attestation acceptance;
- self-test отдельно отвергает mismatched linker hash и tampered EXE;
- real debug `kilogram-offline.exe` успешно собран и запущен через bundled LLD,
  PE header показывает linker version 14.00;
- ZIP/network process на этапе реализации не создавались; exact external match
  остаётся отдельным явным post-push workflow run;
- clean commit `788a3c4ff0152c78a43e8875315a93eba51925a8` собран в двух
  независимых source/target roots; format-v3 verifier подтвердил одинаковые
  2,433,536-byte artifacts с SHA-256
  `25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`,
  после чего тяжёлые временные roots удалены;
- контракт зафиксирован в
  [`../docs/RFC-0100-toolchain-bundled-lld-independent-reproduction.md`](../docs/RFC-0100-toolchain-bundled-lld-independent-reproduction.md).

External workflow run `35469391445` для pushed commit
`4711dd337dce4ba82c19d50970dc29f75b6bd260` подтвердил exact Rust/Cargo и
bundled-LLD identity, но правильно завершился fail closed. Local normalized
artifact: 2,433,536 bytes,
`25b24880f3f7f8ee34da48b4239b0dd730f5799e22275ef508640747cb9b6540`;
GitHub: 2,434,560 bytes,
`3dd8e4b78e4326663393c126a3a1533dc1fdd7f9a5dcf45751e80034b96253ff`.
Размеры `.rdata`/subsequent sections и entry point отличаются, поэтому это не
metadata-only divergence и расширять normalization нельзя.

### M0.9.78 — exact native link-input provenance: implemented locally

Реализовано:

- final `kilogram-offline` link выполняется через `cargo rustc --bin` с
  bounded LLD `/reproduce:<tar>`; flag не распространяется на произвольные
  dependency targets;
- из TAR извлекаются только реально переданные LLD `.lib` под распознанными
  Windows SDK и `VC/Tools/MSVC` layouts; unsafe/unclassified/duplicate/empty
  inputs fail closed;
- retained `NATIVE-LINK-INPUTS.sha256` содержит exact SHA-256, byte length и
  path-independent logical platform name, без username/absolute host path;
- large reproduction TAR и guarded extraction root удаляются сразу, в Git и
  GitHub artifact не попадают;
- same-host builder независимо снимает manifest для обоих clean roots и
  требует exact equality;
- local/external evidence подняты до format v4; manifest входит в checksums,
  GitHub evidence и successful attestation subjects;
- production verifier требует exact local/external native manifest до
  artifact acceptance; self-test отдельно отвергает substituted native-input
  identity;
- реальный local probe нашёл 10 native libraries: MSVC `14.44.35207`
  `msvcrt`/`vcruntime`, Windows SDK `10.0.28000.0` UCRT и семь UM import libs;
  manifest SHA-256
  `182c33c504ac2bce811459acd1a9f3fcd35fcb414be1b710b32651c9c794d61c`;
- 157,723,136-byte probe TAR удалён; `/reproduce` не изменил normalized EXE
  hash или 2,433,536-byte length;
- контракт зафиксирован в
  [`../docs/RFC-0101-exact-native-link-input-provenance.md`](../docs/RFC-0101-exact-native-link-input-provenance.md).

External result:

- manual GitHub run `35471719378` для exact revision
  `c92d25fce192be1ed5094098581ccb082ffa7a53` корректно fail closed;
- local manifest: MSVC `14.44.35207` + Windows SDK `10.0.28000.0`, SHA-256
  `182c33c504ac2bce811459acd1a9f3fcd35fcb414be1b710b32651c9c794d61c`;
- GitHub `windows-2025` manifest: MSVC `14.51.36231` + Windows SDK
  `10.0.26100.0`, SHA-256
  `08807e748ddb05aee3278621b17b9024f5278b2de754b2a0868ab86c54f97fbf`;
- GitHub EXE остался 2,434,560 bytes/
  `3dd8e4b78e4326663393c126a3a1533dc1fdd7f9a5dcf45751e80034b96253ff`;
  attestation не создана, bounded evidence скачано локально в ignored `.tmp`.

### M0.9.79 — hash-locked Windows native toolchain: implemented locally

Реализовано:

- repository `WINDOWS-NATIVE-LINK-INPUTS.lock` закрепляет SHA-256, размер и
  logical path exact ten-library set: MSVC `14.44.35207` и Windows SDK
  `10.0.19041.0`;
- helper находит любую Visual Studio edition с exact toolset, проверяет каждый
  installed file byte-for-byte и не допускает newest-version fallback;
- final binary получает ordered explicit `-L native` для MSVC/UCRT/UM; отдельный
  probe доказал, что `LIB` недостаточен, а explicit paths действительно
  переключают LLD с SDK 28000 на SDK 19041;
- observed transient LLD manifest обязан line-for-line совпасть с repository
  lock; canonical lock/observed SHA-256
  `e478c6daf61551607f8502c7ef97537403f4da24ddbab0b7a6a8fcb94d835027`;
- Microsoft libraries не vendored и не входят в artifacts/packages; retained
  evidence содержит только их hashes/lengths/logical identities;
- records подняты до format v5 с `native_toolchain`; package, local verifier,
  external verifier, GitHub evidence и attestation subjects включают lock;
- workflow закреплён на `windows-2022`, но его mutable label не является trust
  boundary: exact file hashes и post-link manifest остаются обязательными;
- PowerShell parse, static independent-builder boundary и network-free verifier
  self-test прошли; self-test отдельно отвергает mismatched toolchain lock;
- dirty development two-clean-root pair прошёл: оба EXE 2,433,536 bytes,
  SHA-256 `3e5419e14fadeca66068ffdccda7a4cde7584cae03e0430b745e2c3ca700e878`,
  observed manifest равен lock, temporary roots/TAR удалены;
- контракт зафиксирован в
  [`../docs/RFC-0102-hash-locked-windows-native-toolchain.md`](../docs/RFC-0102-hash-locked-windows-native-toolchain.md).

External result:

- manual GitHub run `35473329174` для exact revision
  `a7c5fe0ab7372958959985b54fc2b813c5c4f620` нашёл и использовал exact
  repository-locked MSVC/SDK set; native lock и observed manifest совпали;
- local EXE: 2,433,536 bytes/
  `3e5419e14fadeca66068ffdccda7a4cde7584cae03e0430b745e2c3ca700e878`;
  GitHub EXE: 2,434,560 bytes/
  `bea3fd37a9190f33e30fe31c3ea1c92416d47367cb739b7c5c6ecaf65c85bd78`;
- workflow корректно fail closed, attestation не создана, bounded artifact
  сохранён на 14 дней;
- binary inspection установил новый exact input: local dependency diagnostics
  содержат `C:\Users\Paul\.cargo\registry\src`, hosted diagnostics —
  `C:\Users\runneradmin\.cargo\registry\src`.

### M0.9.80 — canonical Cargo registry source paths: accepted externally

Реализовано:

- reproducible rustflags теперь remap-ят и clean source root в
  `Z:/kilogram-source`, и реально используемый Cargo registry source root в
  `Z:/cargo-registry-src`;
- explicit absolute `CARGO_HOME` поддерживается; default root берётся из
  `%USERPROFILE%`, отсутствие directory и reparse-point fail closed;
- после normalization каждый retained EXE обязан содержать canonical Cargo
  marker и не содержать raw `.cargo\\registry\\src`, checkout root или resolved
  Cargo registry root;
- local/external evidence поднято до format v6 со structured `path_remap` и
  двумя результатами PE leak check;
- production verifier сравнивает полный path-remap contract и повторно сканирует
  local/external artifacts; self-test отвергает synthetic hosted-user Cargo
  path;
- runtime/protocol/network surface не изменён, ZIP/release package не создан;
- контракт зафиксирован в
  [`../docs/RFC-0103-canonical-cargo-registry-path-remapping.md`](../docs/RFC-0103-canonical-cargo-registry-path-remapping.md).

External result:

- clean exact revision `4e9054ab2ccc6a4c542fb37d486b70e53027dd08`
  локально дал byte-identical format-v6 pair: 2,432,000 bytes,
  SHA-256 `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`;
- manual GitHub run `35474774356` завершился успешно за 3m49s: independent
  artifact совпал с local SHA-256, native lock/observed manifest и dual-remap
  boundary прошли;
- attestation `48691093` создана только после equality gate; M0.9.80 принят и
  расследование cross-host divergence закрыто.

### M0.9.81 — M1 candidate evidence composition: принято

Реализовано:

- root `M1-CANDIDATE.json` pin-ит accepted M0.9.76 run/revision и M0.9.80
  GitHub run/attestation/artifact identity без копирования private field state
  или GitHub ZIP;
- field-tested runtime/protocol surface фиксируется 83-entry canonical
  `git ls-tree` manifest SHA-256
  `a67580165a166e22d6c52b52e93d29c562f1bb6289e2497e220fe3ff1e788a3d`;
- independently reproduced offline surface фиксируется 63-entry manifest
  SHA-256 `4ada929ce28d5e152923f746d339d168c5620767f8270443e2a63baf4553e294`;
- production verifier требует обе accepted revisions ancestors текущего HEAD,
  пересчитывает оба baseline/current manifests и отвергает tracked, staged или
  untracked drift в protected paths;
- JSON schema fail closed связывает exact run/revisions/hash/attestation,
  stage boundary и пять честных residual risks; self-test отвергает tampered
  field revision, artifact hash, narrowed runtime surface и omitted risk;
- prerequisite gates повторно проверяют M0.9.76 service-free field contract,
  independent builder и M1 acceptance kit;
- отдельный static boundary запрещает network/download, Cargo build/release,
  ZIP, background process и verification bypass; stage не создаёт Git tag;
- контракт зафиксирован в
  [`../docs/RFC-0104-m1-candidate-evidence-composition.md`](../docs/RFC-0104-m1-candidate-evidence-composition.md).
- committed candidate `485857ec29c838ddcea7cc61fc74ecae629d2d84`
  прошёл production verifier: обе evidence revisions являются ancestors,
  обе protected surfaces неизменны, итог
  `m1_candidate_status=verified`;
- новый network run, release build, ZIP, background service и Git tag не
  создавались.

### M0.9.82 — Sybil-costed volunteer provider admission: принято локально

Реализовано:

- `SignedMailboxStorageOffer` получил domain-separated admission-work digest,
  bounded generator и дешёвую verification; default 18 bits, разрешённый
  generation range 1..20 bits, максимум `2^24` attempts;
- proof связан с store key, полным endpoint, policy/capacity, issue/expiry и
  nonce, после чего весь content подписывается существующим Ed25519 store key;
- embedded `BlindMailboxStore` создаёт qualified offers по умолчанию;
- registry сохраняет low-work offers для compatibility/exact committed lookup,
  но new exact provisioning, automatic legacy rotation, diagnostic selection
  и дальнейший gossip используют только admission-qualified offers;
- transport-identity deduplication применяется после cost filter; extra work
  не повышает rank и не создаёт mining auction;
- runtime логирует фактический work score и policy threshold; IPC остаётся v26,
  новый EXE не добавлен;
- unit/runtime regressions покрывают generation/verification/content binding,
  low-work retention и exclusion из selection/gossip;
- fail-closed static contract добавлен в
  `scripts/verify-kilogram-provider-admission-work-boundary.ps1`, архитектурная
  граница — в
  [`../docs/RFC-0105-sybil-costed-provider-admission.md`](../docs/RFC-0105-sybil-costed-provider-admission.md);
- это ограниченная цена identity churn, а не доказательство разных операторов,
  физических failure domains или полная Sybil resistance;
- stage network-free/debug-only: без release build, ZIP, field network process,
  background service и нового executable;
- все 11 mailbox/provider/service-free static boundaries прошли совместно;
  30 library tests, 3 focused runtime tests и Clippy `-D warnings` прошли при
  `-j 2` в debug profile.

### M0.9.83 — local authenticated provider observation provenance: принято локально

Реализовано:

- Device-authenticated inbound и reply-bound outbound provider gossip передают
  registry только local pseudonymous observer tag, выведенный через BLAKE3
  `derive_key` + keyed mode из local Device secret и authenticated peer
  Account/Device IDs;
- raw social IDs не сохраняются в observation table, а tag не входит в offer,
  gossip frame, protocol request, log или runtime IPC v26;
- separate redb table хранит exact-offer-ID + observer-tag, first/last time;
  repeated session refreshes одну запись вместо увеличения diversity;
- на exact offer сохраняется максимум 8 distinct observations; capacity
  outcome не отклоняет сам signed offer и не создаёт unbounded graph;
- expiry и signed-offer replacement атомарно удаляют старые observations;
  legacy registry без новой таблицы читается с count 0;
- manual/local import не создаёт authenticated provenance;
- `MailboxProviderOffer` локально показывает observation count, но IPC его не
  экспортирует и selection/ranking пока не меняется;
- regression сравнивает byte-identical gossip frames для одинакового wire
  state при разных local observation sets;
- fail-closed static contract —
  `scripts/verify-kilogram-provider-observation-boundary.ps1`, архитектурная
  граница —
  [`../docs/RFC-0106-local-authenticated-provider-observation-provenance.md`](../docs/RFC-0106-local-authenticated-provider-observation-provenance.md);
- это local corroboration signal, не transferable endorsement, proof of person
  или доказательство operator/network/physical independence;
- local Device-secret rotation начинает новый derivation epoch; будущая
  migration обязана очистить или явно re-key observation table, не смешивая
  старые и новые tags как независимых observers;
- acceptance: 16 mailbox-client tests, 3 focused CLI runtime tests, все 12
  mailbox/provider/service-free static boundaries, formatting и Clippy
  `-D warnings` прошли с Cargo `-j 2`;
- stage network-free/debug-only: без release build, ZIP, field network process,
  background service, external publication и нового executable.

### M0.9.84 — bootstrap-safe locally corroborated provider selection: принято локально

Реализовано:

- new replica-set provisioning, automatic acknowledged legacy upgrade и
  diagnostic provider selection используют единый `select_bootstrap_safe`;
- 18-bit admission work остаётся hard floor до применения provenance;
- exact offer с минимум одним local authenticated observation получает binary
  preferred class; count 2..8 rank не повышает и Devices не объявляются
  independent operators;
- внутри preferred и fallback classes сохраняется salted deterministic
  rendezvous rank; итоговый набор остаётся transport-identity-distinct;
- если preferred class не заполняет запрос, unobserved admission-qualified
  offers всегда заполняют оставшиеся места, поэтому fresh install не deadlock;
- exact committed replica sets продолжают indexed resolution без observation
  threshold или re-ranking; retrieval/delete/gossip wire не менялись;
- preference decay привязан к exact offer: replacement/expiry удаляет local
  provenance и replacement должен быть снова замечен через authenticated gossip;
- IPC остаётся v26 и не экспортирует observer tags, raw social IDs, observation
  rows/counts; logs показывают только constant policy/threshold/fallback;
- library regression проверяет observed-first ordering, deterministic fallback,
  admission exclusion, transport deduplication и отсутствие rank benefit от
  extra observers; runtime regression проверяет policy и secret-free projection;
- fail-closed static contract —
  `scripts/verify-kilogram-bootstrap-safe-provider-selection-boundary.ps1`,
  архитектурная граница —
  [`../docs/RFC-0107-bootstrap-safe-provider-selection.md`](../docs/RFC-0107-bootstrap-safe-provider-selection.md);
- acceptance: 17 mailbox-client tests, 7 mailbox tests, 3 focused CLI runtime
  tests, все 13 mailbox/provider/service-free static boundaries, formatting и
  Clippy `-D warnings` прошли с Cargo `-j 2`;
- это preference signal, не proof of operator/network/physical independence;
- stage network-free/debug-only: без release build, ZIP, field network process,
  background service, external publication и нового executable.

### M0.9.85 — local verified provider path-domain provenance: принято локально

Реализовано:

- Iroh selected path получает typed local domain: exact direct remote IP без
  ephemeral port либо canonical relay URL origin; custom/unknown path evidence
  не создаёт;
- только после authenticated peer response и operation-specific verified
  PUT/LIST/DELETE результата runtime выводит BLAKE3 keyed local pseudonym;
- отдельный Device-secret-derived subkey и local derivation epoch не позволяют
  смешивать старые/new tags после смены локального Device secret;
- registry хранит одну latest verified запись на exact signed offer: kind,
  epoch, 32-byte tag и bounded first/last timestamps; raw IP/port/relay URL не
  сохраняются;
- late response fail closed не может прикрепить provenance к replacement:
  store key и exact offer ID проверяются в той же immediate redb transaction;
- same domain refreshes, changed path replaces; exact-offer replacement/expiry
  удаляет row, legacy registry без таблицы читается как unknown;
- optional evidence-write failure не отменяет уже verified receipt/application
  commit и не ломает delivery;
- `MailboxProviderOffer` поддерживает только local equality/co-location query;
  different/missing tags не объявляются independent operators/networks;
- provider ranking, exact replica commitments, gossip/protocol wire и IPC v26
  не менялись; path kind/tag/epoch/count не экспортируются;
- unit regression проверяет direct-port stripping, relay-origin normalization,
  installation scope, redacted Debug, exact binding, refresh/change, key epoch,
  replacement cleanup и unchanged gossiped offer bytes;
- fail-closed static contract —
  `scripts/verify-kilogram-provider-path-domain-boundary.ps1`, архитектурная
  граница —
  [`../docs/RFC-0108-local-provider-path-domain-provenance.md`](../docs/RFC-0108-local-provider-path-domain-provenance.md);
- acceptance: 18 mailbox-client tests, 6 transport tests, 7 mailbox tests, 2
  focused CLI runtime tests, все 16 mailbox/provider/service-free fail-closed
  boundaries, formatting и Clippy `-D warnings` прошли с Cargo `-j 2`;
- stage network-free/debug-only: без release build, ZIP, field network process,
  background service, external publication и нового executable.

### M0.9.86 — positive provider path co-location avoidance: принято локально

Реализовано:

- прежний 18-bit admission floor, binary authenticated-observation preference,
  salted rendezvous rank и transport-identity deduplication сохранены;
- first pass откладывает exact offer только если он positive-equal по kind,
  derivation epoch и local path-domain tag уже выбранному offer;
- path-unknown candidate остаётся first-pass eligible и поэтому может заменить
  второй known co-located offer без bootstrap deadlock;
- если non-conflicting/unknown candidates не заполняют request, deferred offers
  возвращаются исходным ranked порядком, поэтому preference не уменьшает
  доступный transport-distinct fan-out;
- unequal tags не получают independence bonus и не объявляются разными
  operators/networks/hosts;
- new exact provisioning, automatic legacy upgrade и diagnostic selection
  используют один policy; существующие exact commitments продолжают indexed
  resolution без re-ranking;
- provider offer/gossip/protocol wire, receipt threshold и IPC v26 не менялись;
  path tag/kind/epoch не экспортируются, logs показывают только constant policy
  booleans;
- unit regression принудительно назначает одинаковый domain двум highest-ranked
  offers, проверяет выбор path-unknown alternative, deterministic повтор и
  second-pass restoration deferred offer при запросе полного набора;
- fail-closed static contract —
  `scripts/verify-kilogram-provider-colocation-avoidance-boundary.ps1`,
  архитектурная граница —
  [`../docs/RFC-0109-positive-provider-path-colocation-avoidance.md`](../docs/RFC-0109-positive-provider-path-colocation-avoidance.md);
- acceptance: 19 mailbox-client tests, 6 transport tests, 7 mailbox tests, 2
  focused CLI runtime tests, все 17 mailbox/provider/service-free fail-closed
  boundaries, formatting и Clippy `-D warnings` прошли с Cargo `-j 2`;
- stage network-free/debug-only: без release build, ZIP, field network process,
  background service, external publication и нового executable.

### M0.9.87 — independent-provider field contract: реализовано локально, внешний run pending

Реализовано:

- новый debug-only generator создаёт четыре роли: `1` Alice, `2` Provider 1,
  `3` Provider 2, `4` Bob; минимум нужны три физических hosts, идеально четыре;
- Provider 1 и Provider 2 запускаются раздельно и публикуют независимые atomic
  offer/publication/attestation files до mailbox activation;
- run-scoped SHA-256 pseudonym Windows MachineGuid и digests введённых
  operator/network labels позволяют fail closed отклонить известное совпадение,
  не сохраняя raw MachineGuid или labels;
- закрытая evidence schema отвергает дополнительные raw поля; неравенство
  трактуется только как controlled self-attestation, не protocol proof;
- private account/device/Redb/IPC/provider state и открытые runtime logs живут
  только в `%LOCALAPPDATA%`; synchronized evidence получает final logs лишь
  после остановки процесса;
- каждый provider после stop ждёт marker второго, поэтому delayed Yandex sync
  не оставляет Alice ждать aggregate marker после выхода обоих процессов;
- Alice/Bob наследуют service-free capability v2, exact commitment, receipts
  `2/2`, sender-offline, commit-before-delete и restart-no-redelivery contract;
- новый evidence self-test отвергает same-machine, same-operator, same-network
  и injected raw-field fixtures;
- RFC-0110, русская field-инструкция и fail-closed
  `verify-kilogram-m0987-independent-provider-kit-boundary.ps1` фиксируют
  границы;
- stage не меняет Rust/runtime/wire/IPC, не создаёт release, ZIP, HTTPS mailbox,
  новый EXE, background service или network process.

Следующий этап: после commit собрать чистый M0.9.87 kit и провести внешний
трёх-/четырёхмашинный run; только его полный `result=verified` закроет field
acceptance.

Дополнительный операторский профиль `two-host-reduced` позволяет провести
доступный прогон на ПК и ноутбуке: Alice совмещается с Provider 1, Bob — с
Provider 2. Он сохраняет два разных физических provider-хоста и весь
service-free lifecycle, но допускает совпадающие operator/network claims и
явно выдаёт `independent_provider_field_acceptance=false`; строгий pending
field acceptance этим результатом не закрывается.

Диагностический SSH-прогон `20261014-063703` на двух Windows-хостах отклонён
до создания сообщения. Он подтвердил direct Alice/Bob path по LAN и
session-bound capability ACK, но выявил две orchestration-границы:

- non-interactive OpenSSH logon не имеет рабочего current-user DPAPI context;
  удалённые роли должны запускаться с отдельным loaded user profile;
- оба provider пытались писать общий aggregate manifest, и Provider 2 упал на
  Windows atomic replace во время синхронизации.

После диагностики общий manifest/ready marker закреплён за Provider 1 как
single writer, Provider 2 ждёт его публикацию, а atomic shared-file replace
получил bounded retry. Старый run и его private state не переиспользуются;
нужен fresh generated kit и чистый повтор.

### Следующий этап

1. Собрать свежий M0.9.87 debug kit и провести controlled field run exact
   replicas на двух физических и операторски разделённых provider hosts.
2. Не считать успешный controlled run доказательством Sybil resistance:
   colluding offers, observers и dishonest labels остаются residual risks.
3. Отдельным явным действием можно поставить M1 tag на accepted baseline;
   последующие M0.9.82+ изменения намеренно не переписывают M1 evidence.
4. Optional autostart/background mode оставить отдельной явной настройкой, не
   обязательным Windows Task Scheduler step.
5. Добавить macOS/Linux/mobile providers той же platform boundary.
6. Спроектировать privacy-preserving gossip и first-contact freshness;
   M0.9.29 скрывает payload/явные IDs, но не access correlation.
7. Membership removal и group governance проектировать вместе с ordered
   security events и MLS epoch; compact Merkle/range summary и независимый
   криптографический аудит остаются до публичного выпуска.

### M0.9.87 — reduced two-host service-free run: подтверждён

Fresh run `20261014-075145` на ПК и Windows-ноутбуке завершил весь reduced
contract после добавления узкого inbound UDP firewall rule для стабильного
remote `field-kit-current\kilogram-cli.exe` и Alice physical IP:

- Provider 1 и Provider 2 имели разные machine pseudonyms и transport/store
  identities, опубликовали два exact authenticated offers и закрытые final logs;
- Alice/Bob capability convergence прошёл по direct LAN path; локальный relay
  Alice был недоступен, но peer ticket сохранил canonical APS1 target relay;
- Alice получила два signed volunteer receipts, записала offline boundary и
  полностью остановила runtime до запуска Bob receive;
- Bob получил обе volunteer-Iroh replicas, commit-нул одно сообщение, удалил
  обе replicas и после restart не наблюдал redelivery;
- patched exact-locator verifier вернул `result=verified`, M0.9.87 verifier —
  `result=verified-reduced-two-host` и
  `independent_provider_field_acceptance=false`;
- старый verifier дал только false negative на отсутствие `relay_home_url` в
  direct convergence; regression теперь принимает exact `target_relay_url`
  APS1 и fail closed отвергает divergent target relay.

Это закрывает доступный двухмашинный field proof без central HTTPS mailbox, но
не закрывает строгий independent-provider acceptance: для него по-прежнему
нужны минимум три физических host и независимые operator/network claims.

Повторный clean run `20261014-080905` был сгенерирован уже из revision
`162836bb7a54180c3a00915749cf17fe238019e2` с исправленным verifier. Он без
ручного вмешательства прошёл prepare, 2/2 replication, Alice-offline receive,
delete-after-commit, restart-no-redelivery и штатный `03_VERIFY`, который вывел
`M0.9.87 TWO-HOST REDUCED SERVICE-FREE TEST COMPLETED SUCCESSFULLY`. Это
подтверждает, что результат первого run не был следствием post-hoc ослабления
evidence: canonical target relay regression входил в проверяемый kit до старта.

### M0.9.88 — host-neutral Cargo identity verification: принято externally

- Independent run `38506048297` для revision `bb4b7a0` воспроизвёл exact
  2,432,000-byte offline artifact с прежним SHA-256 `d9f9f450...cacb4` и создал
  attestation `55680587`;
- production verification выявил, что full-string `cargo -vV` ошибочно считает
  Windows 11 и Windows Server 2022 частью pinned Cargo identity;
- новый closed parser сохраняет exact equality version, commit, date, target,
  libgit2 и libcurl, требует 64-bit Windows `os:` line, но сравнивает Windows
  release отдельно от tool identity;
- self-test принимает только host OS-release difference и fail closed отвергает
  changed Cargo commit; independent-builder static boundary закрепляет оба
  результата;
- реальный downloaded evidence успешно прошёл исправленный production verifier
  вместе с четырьмя GitHub attestations;
- messenger runtime/protocol/wire/IPC не менялись, ZIP/background service/tag
  не создавались.

Acceptance:

- exact fix revision `68804c93f43b405008cfecf35dc8660db360a3a3`
  прошла новую local two-clean-root reproduction;
- GitHub run `38507114425` независимо получил тот же 2,432,000-byte artifact;
- equality gate создал attestation `55683374`, production verifier проверил
  executable, builder record, native lock и observed native manifest;
- итог `attestation_verified=true`; M0.9.88 принят.

Следующий этап: обновить M1 candidate record на M0.9.87 run
`20261014-080905` и принятую M0.9.88 attestation.

### M0.9.89 — current M1 candidate evidence refresh: принято локально

- `M1-CANDIDATE.json` format v2 pin-ит M0.9.87 clean run
  `20261014-080905`/revision `162836bb...` как
  `verified-reduced-two-host` и не допускает claim strict provider acceptance;
- field runtime/protocol surface: 83 entries, SHA-256
  `1aacaca665c9a2ab4dbc43107c2556128274d7fb9f8727d2dcc78681026bb320`;
- independent evidence pin-ит M0.9.88 revision `68804c93...`, GitHub run
  `38507114425`, attestation `55683374`, artifact SHA-256
  `d9f9f450...cacb4`;
- independent protected surface: 63 entries, SHA-256
  `2a7db2c5cacd0419a8ece61b0dc8386075a558d4f052b44c0eedd81cd9354d68`;
- schema/self-test fail closed отвергают changed revisions/hash/path set,
  omitted risk и попытку установить
  `independent_provider_field_acceptance=true`;
- residual risks обновлены после M0.9.82: strict provider field acceptance и
  operator/network independence pending, Sybil resistance incomplete, access
  correlation visible, field evidence Windows-only, public security release
  не заявлен;
- composition stage network/build/archive/service/tag-free; upstream field и
  release evidence не повторяются при unchanged protected surfaces.

Acceptance gate: commit candidate control surface, затем production
`verify-kilogram-m1-candidate.ps1` на clean HEAD и отдельный docs acceptance
commit.

Acceptance:

- candidate/control commit: `d9142003eeb33f4e9ff44f100d88edd556c058c1`;
- production verifier запущен на clean `HEAD` и сообщил
  `m1_candidate_status=verified`;
- field runtime/protocol и independent artifact surfaces unchanged;
- `network_retest_required=false`; network/build/archive/tag не выполнялись;
- M0.9.89 принят как актуальная evidence composition Windows technical POC,
  без M1 tag и без повышения reduced field result до strict acceptance.

### M0.9.90 — complete M1 readiness gate: принято локально

- полный ручной запуск выявил один скрытый Windows PowerShell 5.1 parser bug в
  старом volunteer field-kit boundary; после explicit UTF-8/ASCII-safe fix все
  31/31 прежних gates прошли;
- новый `verify-kilogram-m1-readiness.ps1` содержит closed inventory всех 32
  boundaries, запускает каждый отдельно и требует exact success marker;
- затем обязательны production M1 candidate verifier, `cargo fmt --check` и
  locked/offline workspace strict Clippy с двумя jobs;
- self-test отвергает missing success marker и changed boundary inventory;
- Rust test executables, network, release build, archive, background process и
  Git tag не запускаются/не создаются;
- compile-only pass всех 24 Rust test harnesses прошёл, disposable 6.52-GiB
  verification cache после этого удалён без затрагивания debug cache.

Acceptance gate: commit readiness scripts/docs, затем production
`verify-kilogram-m1-readiness.ps1` на clean HEAD.

Acceptance:

- implementation/fix revision:
  `56c93aba38648fa5dbe57b22bbf2640c26590c2f`;
- production output: `m1_readiness_status=verified`,
  `boundary_gates=32/32`, candidate/format/offline strict Clippy verified;
- test EXE/network/release/archive/tag flags remained false;
- M0.9.90 принят; M1 tag/publication остаётся отдельным explicit decision.

### M0.9.91 — continuous Windows readiness and tests: externally diagnosed

- added read-only `.github/workflows/windows-ci.yml` for every master push/PR
  plus manual dispatch;
- full-history checkout preserves candidate ancestry/selected-tree proof;
- locked fetch, then Cargo offline readiness and all stable-path Rust test
  harnesses with two jobs/serial execution;
- readiness closed inventory expands from 32 to 33 boundaries and includes the
  CI contract itself;
- action references are full-SHA pinned; `pull_request_target`, write
  permissions, release/archive/upload/attestation/tag paths are forbidden;
- hosted CI is not physical-device or independent-provider field evidence.
- first hosted run `38509447420` passed the complete readiness gate and every
  non-CLI harness; aggregate CLI result was 79 passed / 2 deadline failures;
- exact failures were restart-after-revocation (10 s) and pairwise ticket
  discovery (25 s), both late in the same 770.71-second CLI process;
- production Rust code remains unchanged. A closed format-v1 manifest proves
  both exact selectors exist, excludes them only from aggregate execution and
  runs each once in a fresh process; unknown/duplicate/failed tests fail closed;
- ordinary hosted tests use unoptimized Cargo `test` profile and a bounded
  38-minute step; local compile-only validation resolved both exact selectors.

Second hosted run `38513111049` passed 79 aggregate CLI tests and the isolated
revocation/restart test. The isolated pairwise locator test exposed Windows
`os error 3` while persisting a content-addressed runtime record; its following
HTTP 404 was downstream. M0.9.91 therefore established the hosted gate but did
not produce a green baseline.

### M0.9.92 — Windows extended-length runtime journal: реализовано локально

- `persist_runtime_record` creates and canonicalizes the existing parent before
  joining the final non-existing file name, so Windows uses extended-length
  filesystem paths for read/temp/no-clobber-persist operations;
- full append-only names, collision checks and transaction receipts are kept;
  there is no registry/global-policy dependency or hash truncation;
- Windows regression proves insert, identical idempotent write and byte readback
  for a requested path longer than 260 UTF-16 code units;
- deterministic pairwise locator test returns to aggregate coverage; only the
  confirmed process-history-sensitive revocation/restart test remains isolated;
- hosted tests run after an evidence-continuity readiness failure when locked
  fetch succeeded, while the job remains failed and no bypass is introduced;
- runtime protected surface changed, so current M1 candidate correctly becomes
  stale and fresh physical field evidence is mandatory before candidate refresh.

Acceptance gate: local format/regression/static checks, committed push and
hosted full Rust confirmation. Then prepare fresh field evidence; no M1 tag is
allowed from the old M0.9.89 candidate.

Field evidence:

- first clean run `20261014-103458` completed delivery/restart but was rejected
  because Provider 1 relay registration timed out and its closed log could not
  prove the pinned relay boundary;
- clean retry `20261014-104506` from exact revision
  `d33dd0950484665ee397d161e37ce74743ee8f93` proved both provider relay homes,
  exact receipts `2/2`, sender offline, two commit-before-delete results and no
  restart redelivery;
- unchanged in-kit verifier reported `verified-reduced-two-host` and
  `independent_provider_field_acceptance=false`.

### M0.9.93 — fresh field evidence and M1 candidate refresh: реализовано локально

- `M1-CANDIDATE.json` now pins run `20261014-104506` and post-fix revision
  `d33dd095...` instead of the correctly invalidated pre-fix field baseline;
- exact 83-entry runtime/protocol surface SHA-256 is
  `95c1771de3ca71e3912ab028d50fde3facaa8d05cab81873bdefbb95804f2d34`;
- the accepted M0.9.88 independent reproduction remains pinned because its
  exact 63-entry offline surface has no drift;
- strict provider independence, operator/network separation, complete Sybil
  resistance, correlation hiding, non-Windows evidence and public release all
  remain explicit residual risks;
- composition performs no network/build/archive/service/tag mutation.

Acceptance gate: commit the candidate control surface, run production
`verify-kilogram-m1-candidate.ps1` from a clean tree, then run the complete M1
readiness gate. Hosted full Rust confirmation remains separately required.

Acceptance:

- candidate/control revision:
  `d8252c660dcde49d93ad32c16104c46e91ac8c84`;
- clean production candidate verifier reported
  `m1_candidate_status=verified`, both exact surfaces unchanged and
  `network_retest_required=false`;
- complete readiness reported `boundary_gates=33/33`, verified format and
  locked/offline strict Clippy with two jobs;
- no test executable, network action, release build, archive, background
  service or Git tag was created by composition/readiness;
- M0.9.93 is accepted locally; hosted full Rust confirmation remains pending.

### M0.9.94 — hosted completion and empty-cache accounting: принято локально

- GitHub run `38519726403` на revision `3887e06...` прошёл полный readiness,
  все aggregate Rust harnesses и isolated revocation/restart test;
- единственный job failure произошёл после `status=passed`: read-only cache
  reporter получил существующий, но пустой Cargo profile directory, а
  Windows PowerShell StrictMode попытался прочитать отсутствующий `Sum`;
- `Get-TreeMeasure` теперь явно различает zero и one summary object, принимает
  missing/empty tree как exact zero и отвергает неожиданный multiple result;
- `-SelfTest` в уникальной временной папке проверяет missing, empty и exact
  three-byte trees; Windows CI boundary требует и реально запускает regression
  через Windows PowerShell;
- граница остаётся частью существующего 33-го Windows CI gate; `apps/`,
  `crates/`, field runtime surface и independently reproduced surface не
  менялись, поэтому сетевой field/reproduction rerun не требуется.

Acceptance gate: clean production M1 readiness, commit/push и fresh green
GitHub-hosted Windows CI run. Run `38519726403` остаётся доказательством полного
test pass, но не считается зелёным baseline задним числом.

Local acceptance:

- implementation revision `520793ff17d35d22323f984765b43f4ad8e1c5d3`;
- Windows PowerShell 5.1 и PowerShell 7 self-tests passed;
- normal 37.48-GiB cache report и Windows CI boundary passed;
- clean production readiness: `boundary_gates=33/33`, candidate/format/offline
  strict Clippy verified, mutation flags false;
- fresh hosted green run остаётся единственным pending acceptance condition.

### M0.9.95 — hosted isolated-test scheduling: реализовано локально

- GitHub run `38523647852` на revision `75ad3d8...` прошёл readiness и все
  276 aggregate Rust tests; isolated revocation/restart test один пересёк
  внутренний 10-second IPC-ready deadline после `relay_status=skipped`;
- предыдущий hosted run прошёл exact isolated invocation за 69.84s, а два
  local normal-priority запуска прошли за 59.82s и 59.85s;
- timestamped local run показал restart IPC descriptor через 8.66s после
  завершения первого runtime, поэтому inherited `BelowNormal` после полного
  aggregate suite оставлял слишком малый scheduler margin;
- Windows CI теперь явно выбирает `Normal` и `BeforeAggregate`; test остаётся
  exact, fresh-process, single-attempt и mandatory;
- локальный exact hosted-mode прогон прошёл: isolated `1/1` за 60.64s, затем
  aggregate CLI `81/81` за 530.22s, cache report завершён;
- `apps/`, `crates/`, field runtime surface и independent offline surface не
  изменены; physical field/reproduction rerun не нужен.

Local acceptance:

- implementation revision `c6793136ef0839c6c854227929fa9dfe32cc3f74`;
- Windows CI boundary verified explicit priority/order policy;
- clean full readiness: `boundary_gates=33/33`, candidate/format and
  locked/offline strict Clippy verified;
- readiness mutation flags all false; fresh hosted green run still pending.

Acceptance gate: Windows CI boundary, commit, clean full M1 readiness, push и
fresh green GitHub-hosted run. M1 tag остаётся отдельным явным решением.

Hosted disposition:

- run `38531172733` на revision `a45ce0c...` подтвердил, что CI действительно
  применил `Normal` и `BeforeAggregate`, однако isolated test снова пересёк
  тот же внутренний 10-second restart IPC deadline;
- после isolated failure все 276 aggregate tests прошли, поэтому scheduling
  полезен как явная политика, но недостаточен как исправление;
- M0.9.95 не становится green hosted baseline и superseded этапом M0.9.96.

### M0.9.96 — task-aware runtime test readiness: реализовано локально

- initial и recovered runtime readiness в isolated lifecycle test используют
  общий test-only helper;
- bounded deadline увеличен до 60 секунд, но helper одновременно ждёт
  `JoinHandle`: реальная runtime error или неожиданный clean exit возвращаются
  немедленно, без retry и masking;
- production runtime/network/IPC timeout policy не изменена;
- exact test прошёл локально `1/1` за 61.02s, `cargo fmt --check` passed;
- normal debug EXE сохранил размер `57974784`, но Windows PE build/debug
  identity изменила полный SHA-256, поэтому byte identity не заявляется;
- coarse fail-closed field surface включает весь `apps/kilogram-cli`, поэтому
  до candidate refresh обязателен fresh physical field run exact revision;
- independent `kilogram-offline` surface не менялась, но continuity всё равно
  должен подтвердить существующий exact verifier.

Acceptance gate: technical commit, fresh physical field evidence, candidate
refresh, clean full M1 readiness и fresh green hosted Windows CI. M1 tag не
разрешён автоматически.

### M0.9.97 — configured relay evidence: fresh reduced field verified

- clean two-host run `20261014-135100` на revision `78164eca...` завершил
  exact 2-of-2 volunteer mailbox lifecycle, sender-offline ordering,
  commit-before-delete и restart без redelivery;
- run отклонён fail-closed verifier: Provider 1 был настроен на APS1, но
  `route_policy=auto` продолжил direct после bounded relay registration timeout,
  поэтому старый diagnostic contract не отличал configured pin от observed
  relay home;
- runtime теперь печатает `relay_config_url` для explicit relay configuration,
  а `relay_home_url` остаётся только фактом успешной online registration;
- verifier требует canonical configured URL у обоих providers и отвергает
  любой observed home вне pin; timeout не превращается в online success;
- verifier self-tests и exact isolated lifecycle test (`1/1`, 60.64s) прошли;
- routing, timeout, retry, mailbox и crypto behavior не менялись, но production
  diagnostic surface изменилась и потребовала нового exact-revision field run;
- debug/two-job kit revision `be80d28...` прошёл physical run
  `20261014-141058`: receipts 2/2, Alice offline до receive, оба provider
  replica удалены после commit, Bob history содержит сообщение ровно один раз,
  restart не дал redelivery;
- verifier теперь одинаково требует exact `relay_config_url` в provider,
  convergence, send, receive и restart logs, допускает отсутствие observed home
  и отвергает любой divergent observed home/target;
- final wrapper ждёт оба закрытых provider stdout/stderr после stop marker,
  поэтому асинхронная доставка маркера раньше логов больше не создаёт ложный
  incomplete evidence;
- strict result: `verified-reduced-two-host`,
  `independent_provider_field_acceptance=false`.

Acceptance gate: candidate refresh, clean full readiness and fresh green hosted
Windows CI. Strict independent-provider run и решение об M1 tag остаются
отдельными условиями.

### M0.9.98 — current M1 candidate refresh: accepted locally

- `M1-CANDIDATE.json` pin-ит fresh run `20261014-141058`, product revision
  `be80d28...` и `verified-reduced-two-host` без повышения
  `independent_provider_field_acceptance=false`;
- exact runtime/protocol selected tree: 83 entries, SHA-256
  `b5cb9bc9e06d24c73e63289e662ba019f88b7b052638c6a4b30099470f558095`;
- exact independent offline selected tree от accepted M0.9.88 остаётся
  byte-identical: 63 entries, SHA-256 `2a7db2c5...d68`, поэтому run
  `38507114425` и attestation `55683374` сохраняются;
- self-test отвергает tampered field revision/artifact hash, narrowed runtime
  surface, escalated provider-independence claim и omitted residual risk;
- composition network-free/build-free и не создаёт EXE, archive, service или
  Git tag;
- candidate-control revision `bc3e8248b2f3b278ed86792260fd20147ced08ac`
  прошла production verifier на clean committed tree;
- полный M1 readiness прошёл `33/33`, format и locked/offline strict Clippy с
  двумя jobs; test executable/network/release/archive/service/tag не запускались.

Acceptance gate: push и fresh green hosted Windows CI. M1 tag остаётся отдельным
явным решением.

### M0.9.99 — hosted Windows readiness acceptance: принято

- GitHub Actions run `38588961507` выполнил exact revision
  `9d7895f28853d43eaecad15897b2ca5d1c279713` и завершился success за 38m41s;
- hosted readiness: boundary gates `33/33`, candidate/format/offline strict
  Clippy verified;
- task-aware isolated restart lifecycle прошёл первым: `1/1`, 96.61s,
  `81 filtered out`, без retry;
- aggregate harnesses затем прошли `276/276`, общий executed count с isolated
  test равен 277;
- runner завершил `status=passed`, `isolated_test_count=1`,
  `cache_above_warning=false`;
- workflow read-only и не создал release/archive/attestation/tag.

Hosted gate текущего Windows technical candidate закрыт. Strict
independent-provider field acceptance и решение об M1 tag остаются отдельными
явными пунктами; public security release не заявляется.

### M1.0.0 — Windows technical proof of concept: принято

- пользователь явно принял milestone с уже описанными ограничениями;
- scope остаётся `windows-technical-proof-of-concept`, а не production или
  public security release;
- canonical acceptance record — `M1-CANDIDATE.json` вместе с RFC-0121,
  RFC-0122 и RFC-0123;
- exact acceptance commit обязан пройти production candidate verifier,
  readiness `33/33` и fresh hosted Windows workflow до создания тега;
- canonical tag — annotated `M1.0.0`; перемещать или заменять опубликованный
  tag запрещено;
- Cargo workspace version остаётся `0.1.0`: milestone name не является
  обещанием стабильного Rust API;
- GitHub Release, binary attachment, ZIP/release build и новая attestation не
  требуются и не создаются.

Ограничения сохраняются: strict independent-provider field acceptance pending,
operator/network independence unproven, Sybil resistance incomplete, access
correlation visible, Windows-only field evidence, independent security audit
не проводился. M1.0.0 нельзя рекомендовать для sensitive/high-risk traffic.

### Post-M1 governance — лицензия и приём security reports

- выбран `AGPL-3.0-or-later` без отдельных project copyright notices;
- license grant явно распространяется на source snapshot `M1.0.0`, но
  опубликованный immutable tag не перемещается;
- Cargo workspace и каждый package публикуют единый SPDX license expression;
- `SECURITY.md` фиксирует experimental/no-audit/no-SLA границу и приватный
  канал vulnerability reporting;
- отдельный post-M1 commit не меняет protocol/runtime behavior и не становится
  новым security claim.

Следующий version tag для governance-only изменения не создаётся автоматически;
если понадобится self-contained tagged snapshot с `LICENSE` и `SECURITY.md`,
использовать новый тег, а не переписывать `M1.0.0`.

### Post-M1 public collaboration baseline

- `CONTRIBUTING.md` фиксирует scope, evidence discipline, AGPL inbound terms и
  безопасный bounded Windows test workflow;
- typed bug/feature forms не принимают public security disclosure, secrets или
  production-security claims;
- pull-request template требует явно описать security/privacy/compatibility,
  проверки и stale evidence;
- Dependabot проверяет Cargo и pinned GitHub Actions раз в месяц, ограничивает
  число PR и не включает auto-merge;
- изменение governance-only: runtime/protocol source, accepted M1 evidence и
  immutable `M1.0.1` tag не меняются.

### Post-M1 immutable acceptance and continuous CI

- push governance commit вызвал ожидаемый fail-closed результат старого
  pre-M1 candidate verifier: Cargo manifests уже отличаются от accepted
  candidate из-за license metadata;
- historical evidence не переписывается и не расширяется на новый код;
- новый read-only verifier pin-ит exact annotated tag object, commit, tree и
  acceptance blobs `M1.0.0`, а также требует его ancestry в текущем `HEAD`;
- полный readiness получил explicit `-AcceptedTag M1.0.0` mode для обычного
  post-M1 CI; default mode по-прежнему проверяет текущий candidate без
  ослабления;
- Windows CI проверяет immutable M1 acceptance отдельно от current boundaries,
  format, offline strict Clippy и всех stable-path Rust tests;
- workflow не создаёт и не перемещает tag, не обновляет evidence, не собирает
  release и не публикует archive/attestation.
- local production readiness прошёл: `boundary_gates=33/33`,
  `verification_mode=accepted-tag`, exact `M1.0.0`, format и locked/offline
  strict Clippy verified; test executable/network/release/archive/tag не
  запускались.

### M1.1.0 — threat model and protocol compatibility baseline

- RFC-0125 формально разделяет assets, adversaries, trust boundaries, текущие
  claims и explicit non-guarantees для endpoint, Root, relay, volunteer
  mailbox, local IPC, availability и supply chain;
- компрометация device честно считается полной компрометацией доступных этому
  устройству plaintext/keys/capabilities до наблюдённого revoke; одинаковые
  recovery factors дают злоумышленнику те же полномочия, что владельцу;
- `PROTOCOL-COMPATIBILITY.json` создаёт machine-readable closed inventory из
  20 experimental families и 155 exact source bindings;
- ALPN/version/signature/ID/KDF/HPKE/derivation literals больше нельзя менять
  незаметно: verifier требует exact source match, explicit migration policy и
  stable closed set;
- negative self-test отклоняет changed value, removed binding, path escape,
  unknown policy property и undeclared wire-change claim;
- readiness inventory расширен с 33 до 34 gates, исторический `M1.0.0` tag и
  `M1-CANDIDATE.json` не переписываются;
- полный accepted-tag readiness прошёл `34/34`: exact `M1.0.0`, format и
  locked/offline strict Clippy verified при `CargoJobs 2`/BelowNormal;
- wire/runtime поведение не изменено; Postcard остаётся provisional, поэтому
  следующий protocol step — normative canonical schema и golden
  byte/signature/ID vectors, а не объявление cross-language stability.

### M1.1.1 — canonical wire schema and golden vectors: принято локально

- выбран узкий `kilogram-deterministic-cbor-v1` профиль RFC 8949: только
  shortest-form unsigned integers, definite byte strings и positional arrays;
  maps, text, negative integers, floats, tags, simple и indefinite items
  fail closed;
- `CANONICAL-WIRE-SCHEMA.json` фиксирует пять первых inactive schemas для
  acknowledgement-event и sync-inventory, строгую bytewise ordering и
  отдельные `cbor-v1` signature/ID domains;
- `CANONICAL-WIRE-VECTORS.json` фиксирует два reproducible vectors: exact CBOR,
  signing preimage, deterministic Ed25519 public key/signature и BLAKE3 event
  ID на явно публичном test seed;
- Rust encoder/profile validator прошёл 5/5 targeted tests, включая rejection
  non-minimal, indefinite, unsupported, trailing, duplicate и unsorted forms;
- PowerShell boundary независимо перекодировал оба logical input в те же
  content/signed-envelope bytes и preimages; self-test отверг activation без
  migration, removed object, changed bytes и uppercase hex;
- closed readiness inventory расширен с 34 до 35 gates; accepted-tag прогон
  прошёл `35/35`, exact immutable `M1.0.0`, format и locked/offline strict
  Clippy с `CargoJobs 2`/BelowNormal;
- active runtime остаётся Postcard, wire/network/field behavior не изменён,
  network/release/archive/tag не запускались; перед activation обязательны
  complete schema coverage, new explicit version/identifier, dual-reader
  downgrade tests, independent non-Rust crypto conformance, decoder fuzzing и
  fresh evidence.

### M1.1.2 — canonical authority and session foundation: принято локально

- RFC-0128 и `CANONICAL-AUTHORITY.cddl` формализуют Account/Device IDs,
  Root-signed certificate/revocation/authority snapshot и Device-signed
  session authorization по RFC 8610;
- exact JSON schema фиксирует 11 objects, четыре отдельные inactive `cbor-v1`
  signature domains, максимум 2 capabilities и 32 revocations;
- Rust strict encoder/decoder/verifier проверяет shortest-form CBOR, exact
  positional shape, capability/revocation ordering, nested Account linkage,
  authority sequence bounds, revoked Device и все четыре Ed25519 signatures;
- deterministic nested vector фиксирует public test Root/Device seeds, exact
  contents, domains, signing preimages, signatures и signed envelopes;
- независимый JavaScript/Node reference без сторонних пакетов заново выводит
  Ed25519 public keys из seed, кодирует CBOR и воспроизводит все signatures;
- fail-closed boundary/self-test отклоняет activation без migration, removed
  object, changed Rust binding, uppercase hex, tampered signature и unknown
  capability; active Postcard wire и runtime не изменены;
- closed readiness inventory расширен с 35 до 36 gates. Network, laptop,
  release, archive, tag и active protocol migration не требуются.
- full `kilogram-protocol` regression прошёл `25/25`; accepted-tag readiness
  прошёл `36/36` с format и locked/offline strict Clippy при двух Cargo jobs.

### M1.1.3 — canonical ratchet bootstrap artifacts: принято локально

- RFC-0129, `CANONICAL-RATCHET.cddl` и exact JSON schema формализуют только
  публичные asynchronous bootstrap artifacts: Device-signed ratchet identity,
  prekey bundle, bounded signed pool и authority-complete account directory;
- secret Double Ratchet state, message/skipped keys и `vodozemac` serialization
  намеренно не канонизируются и active Postcard runtime не меняется;
- вложенные signed objects переносятся как byte strings с exact canonical CBOR,
  поэтому внешняя подпись связывает ровно одно внутреннее представление;
- Rust codec проверяет Ed25519, contiguous sequences, unique one-time keys,
  30-day validity, bounds 64/32, Root snapshot/certificate linkage, exact
  certificate/pool coverage, ordering и revocation;
- golden vector фиксирует Root-authorized Device, два prekeys, signed pool,
  BLAKE3 pool ID и полный directory; independent JavaScript/Node заново строит
  CBOR и воспроизводит все Ed25519 signatures без сторонних пакетов;
- independent non-Rust BLAKE3 остаётся явно pending, не подменяется самодельной
  криптографией; его закрывает следующий recovery/digest conformance этап;
- readiness inventory расширен с 36 до 37 gates. Network/laptop/release/archive,
  tag и active protocol migration не требуются.

### M1.1.4 — canonical history recovery checkpoints: принято локально

- RFC-0130, CDDL и exact JSON schema задают inactive deterministic-CBOR
  checkpoint chain для bounded history recovery, не меняя active Postcard;
- recipient Device подписывает exact consent scope, inventory и progress;
  стабильный recovery ID отделён от ID полного signed checkpoint;
- closed-array optionals исключают неоднозначные null/map encodings, page size
  ограничен 256 events, successor обязан ссылаться на exact predecessor,
  строго продвигаться и сохранять identity/inventory;
- golden vector фиксирует initial и successor CBOR, Ed25519 signatures,
  checkpoint IDs и общий recovery ID; Rust regression и независимый Node
  encoder/signature verifier проверяют exact bytes;
- независимая BLAKE3 проверка остаётся явным следующим gate M1.1.5; readiness
  inventory расширен с 37 до 38 gates. Runtime/network/tag не менялись.

### M1.1.5 — independent digests and bounded decoder mutations: принято локально

- dependency-free JavaScript BLAKE3 reference сначала воспроизводит шесть
  official upstream vectors на границах block/chunk/tree, затем независимо
  проверяет 5 Kilogram event/pool/recovery/checkpoint IDs;
- ratchet и recovery Node references теперь проверяют не только CBOR/Ed25519,
  но и exact BLAKE3 IDs; runtime Rust crypto остаётся неизменным;
- closed mutation manifest связывает 5 golden fixtures с exact strict decoder,
  7 классами повреждений и offline bounds 500..2000 iterations;
- integration regression детерминированно отклоняет 1305 truncation/bit-flip/
  non-minimal/indefinite/reserved/oversized mutations примерно за 20 секунд;
- это bounded malformed corpus, а не continuous coverage-guided fuzzing или
  audit; sustained fuzzing остаётся условием activation;
- readiness inventory расширен с 38 до 39 gates; network/release/tag не нужны.

### M1.1.6 — canonical dual-reader migration contract: принято локально

- machine-readable state machine проводит будущую миграцию через `legacy-only`,
  advertise, два dual-reader этапа и только затем `cbor-required`;
- выбор CBOR требует mutual authenticated capability, связанной с current
  authority/session, и нового explicit identifier; существующие Postcard
  version/ALPN переиспользовать нельзя;
- format sniffing и fallback на Postcard после CBOR parse error запрещены;
  unknown version, binding mismatch и downgrade fail closed;
- rollback возможен только explicit local/release policy с monotonic epoch и
  recorded reason, но никогда вследствие network/parse error;
- identifier намеренно `unassigned`, runtime не менялся: complete schema
  coverage, dual reader, sustained fuzzing, downgrade tests, rollback drill и
  fresh hosted/field evidence остаются pending activation requirements;
- readiness inventory расширен с 39 до 40 gates; network/release/tag не нужны.

### M1.1.7 — canonical schema coverage audit: принято локально

- `CANONICAL-COVERAGE.json` exact-join-ит все 20 compatibility surfaces с
  созданными canonical artifacts и не позволяет выдать subset vectors за
  полный протокол;
- complete означает schema + strict decoder + golden vector + independent
  reference для всех objects/preimages/semantic relations одной поверхности;
- результат: 1 complete (`device-session-authorization`), 5 partial, 12
  not-started и 2 explicit non-Postcard codecs вне этой миграции;
- connection ticket и desktop bootstrap сохраняют JSON и не считаются
  пробелами Postcard-to-CBOR migration; любое их будущее изменение всё равно
  требует explicit versioned migration;
- activation остаётся false, пока все targeted surfaces не complete и не
  выполнены отдельные требования migration contract;
- readiness inventory расширен с 40 до 41 gates; runtime/network не менялись.

### M1.1.8 — canonical account collaboration artifacts: принято локально

- canonical device list Root-подписывает exact authority snapshot и 1..32
  strictly ordered certificates, вложенные как bounded canonical-CBOR bytes;
- каждый certificate обязан принадлежать Account, предшествовать snapshot,
  иметь обе messaging capabilities и не быть revoked;
- canonical membership Root-подписывает conversation/revision/owner и полный
  ordered unique список до 4096 Account IDs, обязательно включая owner;
- Rust strict codecs и independent Node/Ed25519 reference воспроизводят exact
  bytes/signatures; два новых decoder расширили corpus с 1305 до 1827 cases;
- `account-authority` остаётся partial только из-за device-link authorization;
  active Postcard/runtime не менялись;
- readiness inventory расширен с 41 до 42 gates; network/release/tag не нужны.

### M1.1.9 — canonical device-link authorization: принято локально

- Root signature связывает Account ID, exact 32-byte request ID, exact canonical
  device certificate и exact canonical resulting device list;
- nested certificate/list остаются отдельно проверяемыми byte strings с
  ограничением 512 KiB, Account linkage обязателен;
- list должен содержать exact certificate, а не только совпадающий Device ID;
- independent Node/Ed25519 reproduces exact bytes/signature, strict decoder
  добавлен в bounded corpus: 2088 cases при ceiling 2500;
- `account-authority` теперь complete (2 complete / 4 partial overall), но
  aggregate activation остаётся false;
- readiness inventory расширен с 42 до 43 gates; active Postcard не менялся.

### M1.1.10 — canonical authorized events: принято локально

- deterministic-CBOR schema покрывает acknowledgement и ratchet-text payload,
  signed event envelope, BLAKE3 Event ID и author authorization evidence;
- parents и recipients имеют strict unique ordering, ciphertext bounded до
  128 KiB, nested canonical objects — до 512 KiB;
- recipient set обязан exact совпадать с Root-signed device list, sender
  ratchet identity — с author Device; certificate/snapshot проверяют Account,
  sequence, `SignEvents` и revocation;
- membership остаётся внешним Root-signed object и проверяет conversation плюс
  наличие author Account, не раздувая каждый event;
- exact vectors и independent JavaScript/Node verifier заново строят CBOR,
  Ed25519 signatures и BLAKE3 IDs; два decoders доводят corpus до 2610 cases;
- `authorized-event` становится complete (3 complete / 3 partial overall), а
  ratchet surface теперь ждёт только prekey-selection transcript;
- readiness inventory расширен с 43 до 44 gates; active Postcard v5,
  network/release/archive/tag не менялись.

### M1.1.11 — canonical prekey selection transcript: принято локально

- exact signed prekey pool входит в bounded transcript; его pool ID становится
  частью canonical selection input вместе с initiator Device ID;
- independent BLAKE3 digest использует explicit domain и CBOR input, первые
  восемь bytes трактуются как unsigned big-endian u64 без host/Rust ambiguity;
- selected index и sequence пересчитываются и fail closed, nested pool сначала
  проходит signature/structure verification;
- transcript не является доказательством possession initiator key, freshness
  также остаётся обязанностью authenticated session/caller;
- active selector с generation little-endian остаётся неизменным и требует
  нового identifier при будущей activation;
- strict decoder доводит corpus до 2871/3000; `ratchet-public-artifacts`
  становится complete (4 complete / 2 partial overall);
- readiness inventory расширен с 44 до 45 gates; network/release/tag не нужны.

### M1.1.12 — canonical synchronization frames: принято локально

- deterministic-CBOR schema теперь покрывает signed inventory/diff, event batch,
  complete, pause/paused/rejected и closed client/server variant frames;
- inventory подписывает requester Device, diff — responder Device; обе подписи
  связывают exact transport-derived session binding;
- Event ID lists strictly ordered/unique, nested authorized events проверяются и
  обязаны принадлежать conversation, boolean values имеют closed encoding 0/1;
- object/frame payload bounded до 8 MiB, inventory до 4096 IDs, round batch до
  64 events; unknown variants и rejection codes fail closed;
- independent Node Ed25519 reference воспроизводит exact bytes/signatures, три
  strict decoder fixture расширяют corpus до 3654/4000;
- `sync` остаётся partial только из-за multi-round session state; active Postcard
  v6 не менялся, readiness inventory расширен с 45 до 46 gates.

### M1.1.13 — canonical durable sync checkpoints: принято локально

- Device-signed checkpoint связывает conversation, обоих Devices, exact session
  binding, round, cumulative received/sent Event IDs, availability, next action
  и paused/terminal state;
- sequence начинается с 1, каждый successor ссылается на BLAKE3 ID exact
  предыдущего signed checkpoint и не может откатывать cumulative evidence;
- round increment разрешён только `receive-complete` -> `send-inventory` при
  наличии ещё работы; pause/resume сохраняют всю semantic state;
- complete/rejected terminal, unpaused, без availability и successor;
  signatures, identities, chain continuity и transitions fail closed;
- independent Node Ed25519/BLAKE3 reference воспроизводит 4 exact checkpoint,
  Rust golden/negative tests проверяют chain и state machine;
- strict decoder доводит corpus до 3915/4000; `sync` становится complete
  (5 complete / 1 partial overall), readiness расширен с 46 до 47 gates;
- active Postcard v6, ALPN, runtime/network/release/archive/tag не менялись.

### M1.1.14 — canonical history recovery exchange: принято локально

- source-signed link canonical-связывает endpoint coordinates, exact source
  certificate, exact Root-signed Account device list, recipient, conversation,
  approved range, page size, route policy и bounded validity;
- recipient-signed plan подписывает exact link, independently derived SAS,
  explicit Ethernet/Wi-Fi/mobile/unknown/power policy и bounded validity;
- page request recipient-signed, exact plan/recovery identity и live-session
  bound; source-signed response имеет только closed page/rejected variants и
  включает exact signed request;
- page переносит rewrapped content как opaque bounded bytes: отдельная
  `history-rewrap` HPKE/event schema намеренно остаётся `not-started`;
- coordinate roles/order, exact certificate membership, SAS, plan/session/range,
  inventory coverage, booleans, variants, signatures и substitutions fail closed;
- independent Node Ed25519/BLAKE3 reference и Rust golden/negative tests
  воспроизводят exact link/plan/request/page/rejection bytes, signatures и IDs;
- три strict decoder fixtures доводят corpus до 4698/5000;
  `history-recovery` становится complete (6 complete / 0 partial overall),
  readiness расширен с 47 до 48 gates;
- verification: protocol unit tests 50/50, mutation corpus 4698/4698,
  accepted-tag readiness 48/48, rustfmt и locked/offline strict Clippy green;
- active Postcard/base64url recovery, history-rewrap wire, ALPN, runtime/network,
  release/archive/tag не менялись.

### M1.1.15 — canonical blind-mailbox capability artifacts: принято локально

- deterministic-CBOR schema покрывает public capability address/mailbox ID,
  HPKE context/AAD и opaque envelope, write/read authorizations, volunteer
  storage offer/admission work, stored/delete receipts и receipt IDs;
- mailbox store по-прежнему не видит Account, Device, conversation или Event
  IDs; ciphertext остаётся opaque, а HPKE cross-language crypto conformance
  явно остаётся activation requirement;
- address/envelope/authorization/receipt semantic links, TTL, pagination,
  capacity/lifetime, closed variants, Ed25519 signatures, BLAKE3 digests/IDs и
  substitutions проверяются fail closed;
- independent Node reference воспроизводит exact CBOR, public keys, signatures,
  digests, IDs и bounded admission-work vector без вызова Rust/CBOR package;
- шесть signed fixtures расширяют mutation corpus до 6264/7000;
  `blind-mailbox-capability` становится complete (7 complete / 0 partial),
  readiness расширен с 48 до 49 gates;
- verification: protocol unit tests 52/52, mutation corpus 6264/6264,
  accepted-tag readiness 49/49, rustfmt и locked/offline strict Clippy green;
- `mailbox-provisioning`, provider gossip, mailbox transport и runtime mailbox
  envelope остаются отдельными `not-started` surfaces;
- active Postcard v1 domains, HPKE AAD, ALPN, runtime/network/release/archive/tag
  не менялись.

### M1.1.16 — canonical blind-mailbox transport: принято локально

- deterministic-CBOR schema композиционно покрывает put/list/delete requests,
  все operation responses, peer forwarding envelope и closed rejection codes;
- peer response exact-связан с canonical peer request через отдельный BLAKE3
  domain; operation/request/response mismatch и digest substitution fail closed;
- nested address, authorization, envelope и receipts повторно проходят strict
  decoding/signature verification, store key и receipt ID сверяются с request;
- list page bounded до 8 strictly ordered items; item/envelope/receipt ID и
  expiry, cursor и `more_available` проверяются как единое состояние;
- independent Node Ed25519/BLAKE3 reference воспроизводит три requests, success
  responses, terminal status variants и representative rejected responses;
- три strict peer-request fixtures расширяют mutation corpus до 7045/8000;
  `blind-mailbox-transport` становится complete (8 complete / 0 partial),
  readiness расширен с 49 до 50 gates;
- active Postcard wire и `kilogram/m0/blind-mailbox/1` ALPN не менялись;
  provisioning, provider gossip и runtime mailbox envelope остаются отдельными
  `not-started` surfaces.

### M1.1.17 — canonical blind-mailbox provider gossip: принято локально

- deterministic-CBOR schema покрывает exact provider offer ID, общий selection
  transcript, отдельные normal/gossip ranks, bounded gossip entry/frame и
  request/reply frame-ID binding;
- selection transcript связывает fresh salt, store key, transport-derived
  identity и exact canonical signed offer ID; transport identities
  дедуплицируются до выбранного размера;
- gossip amplification ограничена 8 offers, 2 hops, 900 seconds age, 2048-byte
  offer, 60-second frame и 20480-byte total frame; store keys уникальны;
- nested signed storage offer повторно проходит canonical signature/validity
  verification, а gossip selection отбрасывает low-work, stale, oversized и
  hop-exhausted candidates;
- frame намеренно не получает новую application signature: active дизайн
  опирается на authenticated peer session, а persisted/correlated frame
  проверяется через exact expected frame ID;
- observer tags, positive path-domain evidence, IP и relay origin остаются
  installation-local и не становятся глобально linkable wire metadata;
- independent Node Ed25519/BLAKE3 reference воспроизводит exact offer,
  selection и frame IDs; ID-aware strict decoder расширяет corpus до 7306/8000;
- `blind-mailbox-provider-gossip` становится complete (9 complete / 0 partial),
  readiness расширен с 50 до 51 gates; active Postcard runtime не менялся.

### M1.1.18 — canonical mailbox provisioning: принято локально

- deterministic-CBOR family покрывает explicit legacy-HTTPS/exact-volunteer
  mode, canonical core и binding ID, signed owner-local binding, signed exact
  recipient peer offer и purpose-specific HPKE header/AAD/info;
- legacy mode требует normalized HTTPS descriptor, exact-volunteer mode его
  запрещает и требует bounded replica set при activation; это migration target,
  а не возврат central mailbox server в целевую архитектуру;
- read/write seeds проверяются против public address; peer получает только
  write seed, local receive capability остаётся sealed owner-local state;
- replica set содержит 2..8 distinct strictly ordered Ed25519 store keys и
  имеет binding-specific commitment ID;
- Device-signed generation chain закрывает activate/rotate/revoke: generation
  one без predecessor, successor exact-ссылается на previous update ID,
  rotation меняет binding, revocation сохраняет active predecessor binding;
- recipient-signed ACK exact-связан с authenticated session, update ID, обеими
  Account/Device парами, generation, binding и revoked state;
- independent Node Ed25519/BLAKE3 reference и Rust golden/negative tests
  воспроизводят legacy/exact cores, local/offer envelopes, replica commitment,
  activation/revocation IDs и ACK;
- четыре signed fixtures расширяют deterministic mutation corpus до 8350/9000;
  `mailbox-provisioning` становится complete (10 complete / 0 partial),
  readiness расширен с 51 до 52 gates;
- verification: protocol unit tests 60/60, mutation corpus 8350/8350,
  rustfmt, workspace-wide offline strict Clippy и accepted-tag readiness 52/52
  green; active Postcard v1/v2 domains, HPKE runtime, ALPN,
  network/release/archive/tag не менялись; next priority —
  `runtime-mailbox-envelope`.

### M1.1.19 — canonical runtime mailbox envelope: принято локально

- deterministic-CBOR family закрывает signed owner-local binding, signed
  imported peer binding, authorized-event payload и signed durable dispatch;
- nested provisioning/event objects проходят strict decode и signature/link
  verification, а не принимаются как opaque trusted bytes;
- payload-to-dispatch binding связывает обе Account/Device пары,
  conversation, Event ID и mailbox binding ID;
- item ID зависит от queue/event/binding, event item ID — от event/binding,
  replication binding коммитит полный signed dispatch;
- dispatch TTL bounded 60..604800 seconds; outer records bounded 64 KiB,
  1 MiB и 8 KiB соответственно;
- independent Node Ed25519/BLAKE3 reference совпадает с Rust golden vectors;
- четыре fixtures расширяют deterministic mutation corpus до 9394/10000;
  `runtime-mailbox-envelope` становится complete (11 complete / 0 partial);
- mutation harness проверяет unsigned AEAD payload только вместе с verified
  signed dispatch: отдельная битовая мутация binding metadata была обнаружена
  corpus и теперь fail-closed на обязательной cross-object границе;
- verification: protocol unit tests 62/62, mutation corpus 9394/9394,
  independent Node reference/self-tests, rustfmt, workspace-wide offline strict
  Clippy и accepted-tag readiness 53/53 green;
- active Postcard v1 domains, runtime queue, network/release/archive/tag не
  менялись; next priority — `endpoint-announcement`.

### M1.1.20 — canonical ticket-publication capability: принято локально

- deterministic-CBOR family закрывает self-authenticating public write
  capability и signed generation/length/body-digest authorization;
- epoch zero сохраняет legacy BLAKE3 derive-key context и pseudonymous channel,
  positive epoch сохраняет rotated context с `0 || epoch.be64` suffix;
- новый CBOR signature domain не позволяет reinterpret legacy HTTP-header
  authorization как canonical envelope;
- private Device secret и opaque peer scope остаются только KDF inputs и не
  сериализуются; scope, capability, authorization и body имеют явные bounds;
- independent dependency-free JavaScript BLAKE3 reference теперь проверяет
  hash и derive-key mode, Node crypto независимо воспроизводит Ed25519 public
  keys/signature, CBOR и точные golden vectors;
- два strict fixtures расширяют deterministic mutation corpus до 9800/10000;
  `ticket-publication-capability` становится complete (12 complete / 0 partial),
  readiness расширен с 53 до 54 gates;
- verification: protocol unit tests 64/64, mutation corpus 9800/9800,
  independent Node reference/self-tests, rustfmt, workspace-wide offline strict
  Clippy и accepted-tag readiness 54/54 green;
- active Postcard artifact, lowercase-hex HTTP headers, legacy authorization
  domain, network/release/archive/tag не менялись; next dependency —
  `publication-conflict`, после него `endpoint-announcement`.

### M1.1.21 — canonical publication-conflict evidence: принято локально

- deterministic-CBOR family покрывает publisher-Device signed publication,
  local-Device signed observation и detector-Device signed conflict proof;
- publication и observation имеют независимые exact hash chains: generation
  one без predecessor, successor сохраняет identities, exact predecessor ID и
  monotonic time/high-water semantics;
- conflict proof принимает только две валидные observations одной Account,
  channel, publisher identity и generation с различающимися publication ID
  или ticket digest;
- observations в proof строго упорядочены по canonical ID, поэтому proof и
  length-delimited evidence IDs не зависят от порядка доставки;
- exact UTF-8 ticket bytes bounded 8 MiB и связаны сохранённым active
  ticket-digest domain; outer publication bounded 16 MiB;
- independent Node Ed25519/BLAKE3 reference и Rust golden/negative tests
  воспроизводят две conflicting publications, две observations и proof;
- три strict fixtures расширяют deterministic mutation corpus до 10583/12000;
  `publication-conflict` становится partial (12 complete / 1 partial /
  5 not-started), readiness расширен с 54 до 55 gates;
- verification: protocol unit tests 66/66, mutation corpus 10583/10583,
  independent Node reference/self-tests, rustfmt, workspace-wide offline strict
  Clippy и accepted-tag readiness 55/55 green;
- Device resolution request, Root resolution, response и portable claim
  намеренно остаются remaining для M1.1.22; active Postcard/base64url URI,
  runtime/network/release/archive/tag не менялись.

### M1.1.22 — canonical publication-conflict resolution: принято локально

- detector Device подписывает bounded request, который exact-связывает conflict
  proof, Account authority revision, peer identity, old/new write keys,
  increasing channel epoch, route policy, replacement ticket и request time;
- request authority намеренно не самодостаточна: verifier требует внешний
  current Root-signed device list exact revision и membership requester/detector;
- Root-signed resolution связывает request/evidence IDs и все rotation fields,
  response добавляет exact replacement ticket и проверяет Root-signed digest;
- request/response portable claims кодируются deterministic CBOR и unpadded
  base64url URI; artifact IDs различаются, общий 12-byte code отображается как
  `KPC1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`;
- claim не является authority; mutation corpus проверяет её только вместе с
  exact Root-signed response, что было отдельно обнаружено первым полным run;
- independent Node reference воспроизводит CBOR, Ed25519, BLAKE3, URI и display
  code; Rust golden/negative tests проверяют authority/root/ticket/cross-kind
  substitutions;
- четыре contextual strict fixtures расширяют deterministic mutation corpus до
  11627/12000; `publication-conflict` становится complete (13 complete /
  0 partial / 5 not-started / 2 not-targeted);
- verification: protocol unit tests 68/68, mutation corpus 11627/11627,
  independent reference/boundary/coverage/conformance self-tests, rustfmt,
  workspace-wide offline strict Clippy и accepted-tag readiness 55/55 green;
- active Postcard/base64url runtime, identifiers, network, release, archive и
  tags не менялись; next canonical priority — `endpoint-announcement`.

### M1.1.23 — canonical endpoint-announcement plaintext: принято локально

- deterministic-CBOR family покрывает signed plaintext bundle, accepted
  observation evidence и closed publication state для endpoint candidates;
- embedded exact current Root-signed device list авторизует source/recipient,
  но bundle не может авторизовать собственный signing Device;
- accepted evidence сохраняет исходную Device signature и добавляет подпись
  importing Device с exact source bundle/time binding;
- contacts строго упорядочены по peer Account/conversation ID, endpoints — по
  Device ID, и ровно один endpoint на contact является primary;
- direct/accepted observations, conflict proof и Root resolution проверяются
  через соответствующие canonical schemas и exact channel/peer/authority/
  write-key/route/ticket-digest bindings;
- bounds: 16 MiB bundle, 64 KiB accepted evidence, 256 contacts, четыре
  endpoints/contact, 4 KiB label, 8 MiB ticket, один час validity и пять минут
  future skew;
- independent Node Ed25519/BLAKE3/CBOR reference совпадает с Rust vectors;
  два strict fixtures расширяют deterministic mutation corpus до 12149/13000;
- coverage честно становится 13 complete / 1 partial / 4 not-started /
  2 not-targeted, readiness — 56 gates;
- recipient-only HPKE envelope и session-bound recipient ACK остаются M1.1.24;
  active Postcard v4/HPKE v1/ACK v2 runtime, network, release, archive и tags не
  менялись.

### M1.1.24 — canonical endpoint-announcement transport: принято локально

- deterministic-CBOR envelope несёт exact canonical header и HPKE ciphertext;
  header целиком используется как associated data и связывает Account,
  source/recipient Device IDs и bundle ID; bundle ID, в свою очередь,
  коммитит authority revision, validity window и весь plaintext bundle;
- HPKE recipient public key берётся только из exact current Root-signed device
  list и обязан совпасть с header recipient Device ID; recipient-only open
  повторно проверяет весь plaintext bundle и его Root/Device authority context;
- recipient-signed acknowledgement exact-связывает session ID, bundle ID,
  applied count, derived contact/endpoint/observation/conflict/
  resolution totals и acknowledgement time;
- независимый Node reference проверяет deterministic CBOR, plaintext Ed25519 и
  BLAKE3 bundle ID, заново строит recipient ACK и отвергает session/bundle/
  count substitutions; Rust golden/negative tests дополнительно выполняют
  реальный HPKE seal/open и tamper/replay rejection;
- два contextual strict fixtures расширяют deterministic mutation corpus до
  12671/14000; endpoint-announcement становится complete (14 complete /
  0 partial / 4 not-started / 2 not-targeted), readiness — 57 gates;
- active Postcard v4 bundle, HPKE v1 envelope и ACK v2 не менялись. Candidate
  остаётся inactive: identifier не назначен, а downgrade tests, sustained
  fuzzing, independent cross-language HPKE и fresh field evidence обязательны
  до activation; следующая canonical priority — `history-rewrap`.

### M1.1.25 — canonical history-rewrap: принято локально

- deterministic-CBOR family отделяет encrypted history payload от M1.1.14
  consent/pagination/checkpoint ceremony и не выдаёт одно за другое;
- exact current Root-signed Account device list авторизует distinct source и
  recipient Devices и поставляет recipient HPKE key; Root-signed conversation
  membership доказывает участие Account в беседе;
- recipient-signed request связывает conversation, roles, external 32-byte
  session binding, bounded contiguous range и SAS от exact roster/roles;
- manifest коммитит полный strictly ordered Event ID inventory до 4096 событий
  и страницу до 256 entries; полный range повторно проверяет inventory digest;
- каждая source-signed entry несёт fully verified authorized RatchetText event,
  manifest/index, real HPKE encapsulated key/ciphertext; AAD exact-связывает
  manifest ID, inventory index и Event ID;
- source-signed transfer встраивает exact request и bundle, а contextual
  verifier требует внешний expected request и session, не доверяя одной
  self-consistency transport artifact;
- Node reference независимо воспроизводит deterministic CBOR, Ed25519,
  BLAKE3, roster/membership/event bindings; real HPKE fixture создаётся и
  открывается Rust, cross-language HPKE честно остаётся pending;
- verification: protocol unit tests 74/74 и mutation corpus 13715/13715 green;
  четыре strict/contextual fixtures, coverage 15 complete / 0 partial /
  3 not-started / 2 not-targeted, readiness 58 gates;
- active Postcard v1 request/bundle/transfer, runtime, network, release,
  archives и tags не менялись; next canonical priority — `account-recovery`.

### M1.1.26 — canonical Account recovery package/witness/quorum: принято локально

- Root-signed deterministic-CBOR recovery package связывает exact Account ID,
  authority snapshot, device list с byte-equivalent embedded authority и
  strictly ordered Root-signed conversation membership heads;
- independent Root-signed witness коммитит exact package ID и authority
  revision, поэтому пакет нельзя подменить при сохранении self-consistency;
- bounded unsigned approval request exact-встраивает verified package/witness,
  fresh nonzero challenge и окно не более 1800 секунд; его portable identity —
  domain-separated BLAKE3 полного canonical request;
- Device-signed approval связывает request/package/state-vector/current roster,
  challenge, expiry, approver и previous approval head. Contextual verifier
  требует exact current certified Device, request и time window;
- quorum report принимает только strictly ordered unique approvals и требует
  strict majority current roster; смена roster и joint old/new majority
  намеренно остаются отдельным M1.1.27 policy-transition boundary;
- independent Node reference заново строит authority/device-list/membership,
  deterministic CBOR, Ed25519 и BLAKE3 package/witness/request/approval IDs;
- protocol regression прошёл 76/76; mutation corpus 14759/14759 за 598.78 s,
  debug/offline, `CargoJobs=2`, BelowNormal. Unsigned request mutation fixture
  exact-связана с expected request ID вместо некорректного требования, чтобы
  никакой другой валидный request вообще не декодировался;
- accepted-tag readiness прошёл 59/59, format и workspace locked/offline strict
  Clippy; network/test EXE/release/archive/tag не запускались и не создавались;
- coverage: 15 complete / 1 partial / 2 not-started / 2 not-targeted. Active
  Postcard Account recovery/runtime и immutable `M1.0.0` не менялись.

### M1.1.27 — canonical Account recovery policy transition: принято локально

- deterministic-CBOR transition request exact-встраивает independently verified
  old/new Root recovery package+witness pairs, fresh challenge, epoch и previous
  transition ID;
- successor обязан сохранить старые revocations, не откатывать memberships и
  не удалять старый Device certificate без explicit new revocation; authority
  revision строго растёт, recovery roster действительно меняется;
- Device-signed approvals exact-связывают request, оба roster digests, epochs,
  challenge, expiry и per-Device previous approval head;
- certificate принимает strictly ordered unique approvals и требует одновременно
  strict majority старого и нового roster; overlap Device считается один раз в
  каждом roster, duplicate не даёт дополнительного голоса;
- portable certificate отдельно требует local DB-authenticated anchor check по
  Account, old epoch, old roster digest и previous transition ID;
- независимый Node reference заново строит old/new packages, Root/Device
  Ed25519 signatures, deterministic CBOR и все BLAKE3 IDs;
- protocol library regression прошёл 78/78; mutation corpus 15542/15542 за
  748.84 s, debug/offline, `CargoJobs=2`, BelowNormal;
- accepted-tag readiness прошёл 60/60, rustfmt и workspace locked/offline strict
  Clippy; network/test EXE/release/archive/tag не запускались и не создавались;
- coverage: 16 complete / 0 partial / 2 not-started / 2 not-targeted. Следующий
  canonical priority — `runtime-ipc`, затем `transport-sync`; active Postcard
  recovery/runtime и immutable `M1.0.0` не менялись.

### M1.1.28 — canonical local runtime IPC core: принято локально

- inactive deterministic-CBOR family начинает `runtime-ipc` с signed loopback
  descriptor, bounded request и signed exact response, не меняя active reader;
- descriptor разрешает только `127.0.0.1`/`::1`, nonzero port и 32-byte bearer,
  связывает Account, Device, server instance и validity до 24 часов и
  подписывается exact Device;
- request exact-связывает descriptor ID, bearer, caller request ID, окно до 30
  секунд и core command; request обязан полностью лежать в validity descriptor;
- response подписывается descriptor Device и коммитит descriptor ID, request
  ID и domain-separated digest полного request, закрывая stale-port rebind race;
- Ping/Pong, WaitForChange/ChangeState, Shutdown/ShutdownAccepted и bounded UTF-8
  Error имеют fail-closed command/result semantics; unchanged revision обязан
  быть exact requested revision, а changed revision — строго больше;
- independent Node reference воспроизводит CBOR, две Ed25519 signatures и все
  BLAKE3 identifiers; Rust golden/substitution/command-matrix tests прошли 3/3;
- первый mutation run корректно выявил, что caller-chosen request ID сам по себе
  может быть другим валидным значением. Fixture теперь проверяет unsigned request
  одновременно через descriptor/bearer и digest в signed response;
- финальный mutation corpus прошёл 16311/16311 за 737.63 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 81/81;
- accepted-tag readiness прошёл 61/61, rustfmt и workspace locked/offline strict
  Clippy; network/test stable EXE/release/archive/push/tag не запускались;
- coverage: 16 complete / 1 partial / 1 not-started / 2 not-targeted. Следующий
  шаг — последовательно покрыть оставшиеся runtime IPC command/result families,
  после чего перейти к parallel CBOR ALPN/negotiation `transport-sync`.

### M1.1.29 — canonical runtime IPC read-only model: принято локально

- inactive deterministic-CBOR IPC получил две read-only команды без подключения
  к active reader: paginated conversation list и snapshot-bound history page;
- conversation cursor состоит из exact UTF-8 label и peer Account ID, page limit
  ограничен 64; ordering и next cursor проверяются fail closed;
- conversation summary коммитит peer Account/Device, exact endpoint counters,
  максимум четыре deterministic candidates, ровно один primary, route/health,
  message count и bounded latest-message preview;
- history page всегда возвращает snapshot ID, total count и reverse page до 100
  сообщений; cursor arithmetic и aggregate body bound 192 KiB запрещают drift,
  overlap, gap и unbounded local response;
- общий signed response bound равен 256 KiB, подпись Device exact-связывает
  descriptor ID, caller request ID и domain-separated digest полного request;
- independent Node reference воспроизводит deterministic CBOR, Ed25519 и BLAKE3
  для list/history vectors и отвергает substitutions/signature tampering;
- первый полный mutation run отверг все cases и остановился только на старом
  inventory 16311; после фиксации exact count corpus прошёл 17337/17337 за
  774.07 s, debug/offline, `CargoJobs=2`, BelowNormal;
- protocol library regression прошёл 85/85; accepted-tag readiness прошёл 62/62,
  rustfmt и workspace locked/offline strict Clippy;
- network, stable-path test EXE, release build, archive, push и tag не создавались;
  active Postcard IPC v26/JSON descriptor v7 и immutable `M1.0.0` не менялись;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — contact/device directory либо durable
  message/outbox; затем mailbox/publication/automation/recovery families.

### M1.1.30 — canonical runtime IPC contact and directory status: принято локально

- inactive deterministic-CBOR IPC получил exact `AddContact` и read-only
  `OwnDeviceDirectoryStatus`; authority-changing `ApplyOwnDeviceDirectory`
  намеренно оставлен отдельному trust boundary;
- contact request связывает signed descriptor, bearer, fresh caller request ID,
  bounded label, expected peer Account и bounded descriptor path; peer Account
  обязан отличаться от local Account;
- canonical contact ID коммитит local Account, peer Account и label-derived
  Conversation ID. Он намеренно отличается от active legacy ID и требует
  explicit mapping/migration при будущей активации;
- Device-signed response exact-коммитит digest полного request, peer identity,
  bounded endpoint count и inserted/endpoint-added semantics;
- own-device status exact-связывает descriptor Account/Device, positive
  authority revision, roster 1..32, coupled receipt ID/generation, nonzero
  device-list digest, applied/launch paths и согласованные convergence/restart
  states;
- independent Node reference воспроизводит deterministic CBOR, Ed25519, BLAKE3,
  contact identity и request/response IDs; Rust golden/substitution/invariant
  tests прошли;
- полный mutation corpus прошёл 18357/18357 за 779.37 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 89/89;
- accepted-tag readiness прошёл 63/63, rustfmt и workspace locked/offline strict
  Clippy; network, stable-path test EXE, release build, archive, push и tag не
  запускались и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — отдельный apply-own-device-directory authority
  transition, затем durable message/outbox и остальные runtime families.

### M1.1.31 — canonical runtime IPC own-device directory apply: принято локально

- inactive deterministic-CBOR IPC получил отдельный authority-changing
  `ApplyOwnDeviceDirectory`, не подключённый к active Postcard reader;
- request embeds exact Root-signed successor list и одновременно anchors exact
  current authority revision и digest полного signed current list, поэтому
  pathname не может стать TOCTOU authority substitution;
- live transition допускает только exact idempotent replay либо monotonic
  removal: running Device остаётся byte-identical, новый/replaced certificate
  запрещён, old revocations retained, каждый removed Device Root-revoked;
- Device-signed result коммитит previous/new revisions и digests, exact sorted
  removed set, retirement/pending counters, ticket/profile decisions и полный
  resulting directory status;
- closed result semantics явно различают future recipient exclusion и
  невозможность remotely rewrite/erase preexisting ciphertext; existing local
  copies остаются readable;
- independent Node reference воспроизводит CBOR, Root/Device Ed25519 и BLAKE3;
  Rust golden, transition, expiry, substitution и invariant tests прошли;
- финальный mutation corpus прошёл 18879/18879 за 794.93 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 95/95;
- accepted-tag readiness прошёл 64/64, rustfmt и workspace locked/offline strict
  Clippy; network, stable-path test EXE, release, archive, push и tag не
  запускались и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — idempotent durable message queue, затем
  snapshot-bound paginated outbox projection.

### M1.1.32 — canonical runtime IPC durable message queue: принято локально

- inactive deterministic-CBOR IPC получил bounded `QueueMessage`, не
  подключённый к active Postcard reader;
- caller request ID является exact durable queue ID: одинаковый request
  идемпотентен, а повтор ID с отличающимся label, peer или plaintext должен
  завершаться conflict до создания второй записи;
- success Device-подписан и exact-коммитит descriptor, полный request digest,
  queue ID, canonical contact ID, label-derived Conversation ID, peer Account и
  `inserted`; последний не является delivery receipt;
- eventual adapter обязан в одной state lock/transaction проверить enrolled
  contact и current conversation membership, seal/sign durable record и только
  затем вернуть success; portable CBOR не подменяет local authority state;
- plaintext допускается только в signed-loopback bounded request, ограничен
  65536 bytes и не должен сохраняться в evidence/log;
- independent Node reference воспроизводит deterministic CBOR, Ed25519 и
  BLAKE3, а request/response включены в закрытый mutation corpus;
- финальный mutation corpus прошёл 19401/19401 за 793.87 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 99/99;
- accepted-tag readiness прошёл 65/65, rustfmt и workspace locked/offline strict
  Clippy; network, stable-path test EXE, release, archive, push и tag не
  запускались и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — snapshot-bound paginated outbox projection.

### M1.1.33 — canonical runtime IPC snapshot-bound outbox: принято локально

- inactive deterministic-CBOR IPC получил bounded read-only
  `OutboxStatusPage`, не подключённый к active Postcard reader;
- content-derived snapshot ID коммитит profile, exact global counters и полный
  строго отсортированный по Queue ID набор items, а не только текущую страницу;
- state projection связывает queue/contact/peer/conversation IDs, delivery
  state, optional ACK и retry count; ACK допустим только для delivered, а все
  global counters exact-пересчитываются из полного snapshot;
- cursor удерживает snapshot ID и exact next start index; page size ограничен
  128, полный snapshot — 4096 queue items / retry records и 256 contacts;
- eventual active adapter обязан удерживать exact immutable view либо явно
  вернуть stale snapshot после mutation/eviction/restart; silent rebuild под
  прежним cursor запрещён;
- independent Node reference воспроизводит deterministic CBOR, content-derived
  BLAKE3 snapshot ID, request/response IDs и Ed25519 signature; item/count/cursor
  substitutions отклоняются;
- финальный mutation corpus прошёл 19905/19905 за 823.63 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 103/103 с
  тремя explicit vector generators ignored;
- accepted-tag readiness прошёл 66/66, rustfmt и workspace locked/offline
  strict Clippy; network, stable-path test EXE, release, archive, push и tag не
  запускались и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC family — mailbox provisioning и volunteer-provider
  control commands; mutation maximum перед ним нужно безопасно расширить выше
  текущего почти заполненного 20000.

### M1.1.34 — canonical runtime IPC volunteer-provider control: принято локально

- inactive deterministic-CBOR IPC получил exact импорт одного подписанного
  volunteer storage offer и capability-free выбор до восьми providers;
- импорт независимо проверяет подпись, current validity и минимум восемь bits
  admission work до любой будущей registry mutation;
- projection сохраняет исходный signed offer, nonzero transport identity и
  явно local-only observation time. Mailbox read/write capabilities и social
  identifiers в ответ не входят;
- selection exact-связывает salt/requested count, запрещает duplicate store
  keys/offer IDs и возвращает не больше requested и active counts;
- import и selection имеют отдельные Rust/Node golden request/response vectors
  и отдельные mutation fixtures;
- финальный mutation corpus прошёл 20947/20947 за 826.67 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 106/106 с
  четырьмя explicit vector generators ignored;
- accepted-tag readiness прошёл 67/67, rustfmt и workspace locked/offline
  strict Clippy; active Postcard IPC v26/JSON descriptor v7, network, stable
  test EXE, release/archive/push/tag не менялись и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — paginated read-only mailbox capability status,
  отдельно от create/rotate/revoke mutation boundary.

### M1.1.35 — canonical runtime IPC mailbox capability status: принято локально

- inactive deterministic-CBOR IPC получил bounded paginated read-only
  `MailboxCapabilityStatusPage`, не подключённый к active Postcard reader;
- content-derived snapshot ID коммитит полный строго отсортированный набор
  capability heads и global counts; cursor сохраняет exact snapshot ID и next
  start index, а Device-signed response связывает полный request digest;
- каждый head содержит exact signed capability update и optional exact signed
  recipient ACK. Local binding, mailbox plaintext и read/write secrets в
  projection отсутствуют; recipient-sealed ciphertext внутри подписанного
  update сохраняется как opaque protocol evidence;
- descriptor Account+Device обязан быть exact owner либо recipient каждого
  head. Full-page counts и snapshot пересчитываются, а partial-page buckets не
  могут превышать signed global counts;
- request/response bounded до 64 KiB / 640 KiB, полный snapshot — 1024 heads,
  одна page — 16 heads;
- independent Node reference воспроизводит deterministic CBOR, Ed25519 и
  BLAKE3; Rust golden, substitution и snapshot tests прошли 3/3, один explicit
  vector generator ignored;
- финальный mutation corpus прошёл 21451/21451 за 849.44 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 109/109 с
  пятью explicit vector generators ignored;
- accepted-tag readiness прошёл 68/68, rustfmt и workspace locked/offline
  strict Clippy; active Postcard IPC v26/JSON descriptor v7, network, stable
  test EXE, release/archive/push/tag не менялись и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — create/rotate/revoke mailbox capability
  mutations с отдельной authority/secret boundary.

### M1.1.36 — canonical runtime IPC mailbox capability mutations: принято локально

- inactive deterministic-CBOR IPC получил create/rotate/revoke lifecycle,
  отдельный от read-only status projection и active Postcard runtime;
- create/rotate exact-связывают Device-signed provider selection, target roster
  digest и bounded validity; rotate/revoke дополнительно требуют exact active,
  recipient-acknowledged predecessor head;
- signed result содержит exact capability update, provider selection ID и
  provider-bounded expiry только для activation, а также честное состояние
  `pending-recipient-acknowledgement`;
- local mailbox binding, read secret и write secret через IPC не передаются.
  Eventual adapter обязан atomic compare-and-commit durable state и replay-ить
  exact idempotency request без повторной ротации;
- independent Node reference проверяет deterministic CBOR, Ed25519, BLAKE3,
  admission work, provider order/transport uniqueness, chain links и replica
  equality; Rust golden/substitution/round-trip tests прошли 3/3, один explicit
  vector generator ignored;
- финальный mutation corpus прошёл 23013/23013 за 932.66 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 112/112 с
  шестью explicit vector generators ignored;
- rustfmt, workspace locked/offline strict Clippy и accepted-tag readiness
  прошли 69/69. Active Postcard IPC v26/JSON descriptor v7, network, stable
  test EXE, release/archive/push/tag не менялись и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — publication-channel rotation и conflict
  ceremony; independent HPKE и реальная adapter transaction остаются
  activation gates.

### M1.1.37 — canonical runtime IPC publication-channel rotation: принято локально

- inactive deterministic-CBOR IPC получил exact-current-state
  `RotatePublicationChannel`, не подключённый к active Postcard runtime;
- request связывает contact, peer Account, digest durable peer scope, current
  epoch/public capability и последний publication head; следующий epoch обязан
  быть ровно `current + 1`;
- trusted helper проверяет signed descriptor, bearer и срок request, exact
  Device secret, scope digest, а также заново выводит и сравнивает текущий
  capability до вывода successor; caller не поставляет successor key;
- Device-signed transition возвращает только public capabilities и честные
  состояния `awaiting-first-publication`/`pending-peer-delivery`. Device secret,
  plaintext peer scope и reusable private material в payload отсутствуют;
- independent Node reference воспроизводит deterministic CBOR, Ed25519 и
  BLAKE3 hash/derive-key; Rust golden/round-trip/substitution tests прошли 3/3,
  один explicit vector generator ignored;
- финальный mutation corpus прошёл 23535/23535 за 932.48 s, debug/offline,
  `CargoJobs=2`, BelowNormal; protocol library regression прошёл 115/115 с
  семью explicit vector generators ignored;
- accepted-tag readiness прошёл 70/70, rustfmt и workspace locked/offline
  strict Clippy; active Postcard IPC v26/JSON descriptor v7, network, stable
  test EXE, release/archive/push/tag не менялись и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — exact network publication и peer-delivery
  evidence; Root-authorized conflict ceremony остаётся отдельной границей.

### M1.1.38 — canonical runtime IPC publication network: принято локально

- inactive deterministic-CBOR IPC получил exact-state `PublishOwnTicket` и
  `RefreshContactTicket`, не подключённые к active Postcard runtime;
- publish request связывает current publication channel, recipient roster
  digest/count, canonical root-only HTTPS origin и TTL. Signed ticket,
  recipient HPKE slots и opaque envelope формируются только внутри trusted
  runtime, а caller не поставляет successor publication head;
- publish result возвращает только publication/ticket/envelope digests,
  envelope length и exact write authorization. `confirmed-https-success`
  остаётся `pending-peer-observation` и не объявляется доказательством remote
  durability или eventual peer delivery;
- refresh exact-связывает contact, peer publisher Device, channel, observation
  и publication high-water marks; success требует независимо проверяемое
  Device-signed observation и atomic install ticket+observation до ответа;
- ticket plaintext, HPKE ciphertext/slots, publication write secret, Device
  secret, plaintext peer scope и filesystem paths через IPC не передаются;
- independent Node reference воспроизводит deterministic CBOR, Ed25519,
  BLAKE3/KDF, publication authorization и observation. Rust golden,
  round-trip и substitution tests прошли 3/3, один explicit generator ignored;
- decoder mutation corpus прошёл 24579/24579 за 961.89 s, debug/offline,
  `CargoJobs=2`, BelowNormal. Protocol library regression прошёл 118/118,
  восемь explicit vector generators ignored;
- accepted-tag readiness прошёл 71/71, rustfmt и workspace locked/offline
  strict Clippy прошли. Active Postcard IPC v26/JSON descriptor v7, network,
  stable test EXE, release/archive/push/tag не менялись и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — Root-authorized publication-conflict commands;
  independent HPKE и real adapter transaction evidence остаются activation
  prerequisites.

### M1.1.39 — canonical runtime IPC publication conflict: принято локально

- inactive deterministic-CBOR IPC получил exact-state
  `CreatePublicationConflictRequest` и `ApplyPublicationConflictResponse`, не
  подключённые к active Postcard runtime;
- обе команды связывают current Root authority revision/device-list digest,
  retained proof/evidence, peer Account/Device, old publication channel/epoch
  и content-addressed current ticket. Apply дополнительно exact-связывает
  pending request claim и staged Root response;
- create возвращает success только после durable staging portable Device
  request. Apply независимо проверяет current Root signature и возвращает
  success только после atomic replacement endpoint/resolution/quarantine/proof
  commit; sibling propagation остаётся явно pending;
- portable request/response и replacement ticket остаются в trusted bounded
  artifact staging. Root secret, publication write key, artifact plaintext и
  local filesystem paths через IPC не проходят;
- independent Node reference воспроизводит deterministic CBOR, Ed25519,
  BLAKE3 request/response/artifact IDs; Rust golden, round-trip и substitution
  tests прошли 3/3, один explicit generator ignored;
- decoder mutation corpus прошёл 25623/25623 за 958.12 s, debug/offline,
  `CargoJobs=2`, BelowNormal. Protocol library regression прошёл 121/121,
  девять explicit vector generators ignored;
- accepted-tag readiness прошёл 72/72, rustfmt и workspace locked/offline
  strict Clippy прошли. Active Postcard IPC v26/JSON descriptor v7, network,
  stable test EXE, release/archive/push/tag не менялись и не создавались;
- coverage остаётся 16 complete / 1 partial / 1 not-started / 2 not-targeted.
  Следующий bounded IPC slice — own-device discovery/automation, затем
  recovery/device-link commands и остальные noncore results.

### M1.1.40 — canonical own-device announcement automation: принято локально

- inactive deterministic-CBOR IPC получил foreground-only configure/status
  policy для отправки endpoint announcement одному sibling Device;
- policy exact-связывает Root-signed roster, content-addressed recipient ticket,
  interval/validity, network mask и bounded retry/backoff, не передавая ticket
  plaintext или local path через IPC;
- Device-signed status честно фиксирует `restart-not-required`,
  `foreground-runtime-only` и отсутствие OS background service;
- decoder mutation corpus прошёл 26667/26667 за 985.34 s, protocol library
  regression — 124/124, accepted-tag readiness — 73/73;
- active Postcard IPC v26/JSON descriptor v7, network, release/archive/push/tag
  не менялись. Следующий bounded slice — pairwise ticket discovery.

### M1.1.41 — canonical pairwise own-device ticket discovery: принято локально

- inactive deterministic-CBOR IPC получил foreground-only configure/status
  policy для двунаправленного discovery между двумя active sibling Devices;
- exact Root-signed roster, distinct public channel IDs, canonical HTTPS
  origin, TTL/refresh, announcement timing, network mask и retry связаны с
  append-only Device policy generation и predecessor;
- private publication capabilities выводятся только внутри trusted runtime;
  verified content-addressed locator и child announcement policy появляются
  атомарно, а ticket plaintext, secrets и filesystem paths через IPC не идут;
- Device-signed status детерминированно различает disabled/network-blocked,
  publish/fetch/announcement, backoff и synchronized; OS background service и
  restart не требуются;
- independent Node reference, CDDL/schema/vectors и fail-closed boundary
  согласованы. Mutation corpus прошёл 27711/27711 за 1026.61 s, protocol
  regression — 127/127, readiness — 74/74;
- active Postcard IPC v26/JSON descriptor v7, network, release/archive/push/tag
  не менялись. Следующий bounded slice — roster-wide orchestration.

### M1.1.42 — canonical roster-wide own-device automation: принято локально

- inactive deterministic-CBOR IPC получил foreground-only configure/status
  parent policy для всего current Root-signed sibling roster;
- exact active children и retained permanently revoked tombstones выводятся в
  строгом Device-ID порядке; retired child принудительно disabled, workflow
  выполняются с max parallelism 1;
- полный pairwise discovery child переносится отдельно проверенным canonical
  CBOR `bstr`, сохраняя глобальный decoder depth limit 8;
- ticket plaintext, derived private capabilities, reusable secrets и local
  paths через IPC не проходят; OS background service и restart не требуются;
- independent Node reference, CDDL/schema/vectors и fail-closed boundary
  согласованы. Mutation corpus прошёл 28755/28755 за 1023.72 s, protocol
  regression — 130/130, readiness — 75/75;
- active Postcard IPC v26/JSON descriptor v7, network, release/archive/push/tag
  не менялись. Следующий bounded slice — recovery-plan/device-link commands.

### M1.1.43 — canonical history-recovery plan custody: принято локально

- inactive deterministic-CBOR IPC получил approval/status/cancel commands для
  одного history-recovery plan без подключения к active Postcard runtime;
- approval принимает exact source-signed recovery link, independently
  пересчитывает полный SAS digest, связывает explicit network/power policy и
  создаёт recipient-Device-signed bounded plan;
- local custody сохраняет content-addressed record. Cancellation требует exact
  current record ID, сохраняет plan byte-identical и добавляет immutable
  recipient-Device-signed `user-requested` tombstone;
- status/transition results exact-связаны с request digest, Account и recipient
  Device и явно утверждают `foreground-runtime-only`, `restart-not-required`,
  `os-background-service-enabled=false`;
- Root/pre-enrollment authority, Device-link bootstrap, recovery payloads,
  private capabilities, decrypted history и filesystem paths через routine IPC
  не проходят;
- CDDL/schema/vectors, independent Node reference, RFC-0169 и dedicated
  fail-closed boundary согласованы. Mutation corpus прошёл 29799/29799 за
  3716.29 s, protocol regression — 133/133, readiness — 76/76;
- active Postcard IPC v26/JSON descriptor v7, network, test EXE,
  release/archive/push/tag не менялись. Следующий bounded slice — recovery-plan
  execution и scheduler results.

### M1.1.44 — canonical history-recovery execution: принято локально

- inactive deterministic-CBOR IPC получил start/status commands для уже
  одобренного exact recovery plan; request ID служит durable idempotency key;
- recipient Device подписывает append-only scheduler chain: initialized,
  attempt-started, attempt-failed, attempt-progressed, cancelled и completed;
  generation, total/consecutive attempts, bounded lease, retry policy и exact
  recovery checkpoints проверяются на каждом переходе;
- retry delay обязан совпадать с детерминированным equal-jitter exponential
  backoff. Counter overflow, нулевой failure code и произвольный bounded delay
  отвергаются fail closed даже для корректно подписанного состояния;
- caller не передаёт network/power class, attempt outcome или checkpoint.
  Readiness детерминированно выводится из plan policy, lifecycle, времени и
  runtime observation; новый attempt возможен только в foreground runtime;
- Root/Device secrets, recovery capabilities, decrypted history, payload pages,
  remote delete commands и filesystem paths через IPC не проходят. Успешный
  status сохраняет `restart-not-required` и отсутствие OS background service;
- CDDL/schema/vectors, independent Node Ed25519/BLAKE3 reference, RFC-0170 и
  dedicated fail-closed boundary согласованы. Mutation corpus прошёл
  30843/30843 за 2674.96 s; protocol unit regression — 137/137, четырнадцать
  explicit generators ignored; accepted-tag readiness — 77/77;
- rustfmt и workspace offline strict Clippy прошли. Active Postcard IPC
  v26/JSON descriptor v7, network, test EXE, release/archive/push/tag не
  менялись. Следующий приоритет — explicit migration активного unpaged
  conversation list, затем оставшиеся mutation-result contracts.

### M1.1.45 — active runtime IPC conversation pagination: принято локально

- active loopback IPC заменил unpaged `ConversationList` на
  `ConversationListPage { cursor, limit }`; Postcard version повышен с 26 до
  27, signed JSON descriptor domain — с v7 до v8;
- cursor exact-связывает UTF-8 conversation label и peer Account ID, порядок
  bytewise-детерминирован, limit ограничен 1..64, continuation допустим только
  для полной страницы и обязан совпасть с последним summary;
- runtime сортирует authenticated contacts и выбирает не более одной страницы
  до загрузки membership/history/endpoint preview. Duplicate cursor keys,
  non-monotonic page и несогласованные endpoint counters fail closed;
- Windows desktop автоматически собирает страницы, повторно валидирует каждый
  cursor/page и явно падает после safety bound 1024 страниц (65 536
  conversations), не выдавая усечение за успех;
- migration — deliberate exact-version hard cut для co-installed binaries:
  legacy v26 descriptor/request отвергаются до actor dispatch, downgrade и
  dual reader отсутствуют. Peer, relay, mailbox, sync wire и inactive
  canonical CBOR не менялись;
- IPC unit regression прошёл 17/17, desktop multi-page test — 1/1, локальная
  direct-only двух-runtime CLI integration — 1/1. Dedicated migration boundary,
  aggregate canonical/compatibility gates, rustfmt и strict offline Clippy
  прошли;
- accepted-tag readiness прошёл 78/78 против immutable `M1.0.0`. Default
  current-candidate mode ожидаемо отвергает protected-surface drift и не
  переиспользует старое field evidence. Release/archive/push/tag не создавались;
- canonical coverage остаётся 16 complete, 1 partial, 1 not-started и 2
  not-targeted. В partial `runtime-ipc` остались только associated
  mutation-result contracts.

### M1.1.46 — exhaustive runtime IPC completion inventory: принято локально

- добавлен нормативный `CANONICAL-RUNTIME-IPC-COMPLETION.json`, который
  перечисляет 32 canonical commands, 31 results и 32 exact command/result
  mappings с owning module/kind;
- тот же manifest исчерпывающе disposition-ит все 34 command и 31 response
  variants активного Postcard v27 adapter непосредственно против Rust enums;
- vague `all-associated-mutation-results-not-listed-above` удалён. Остались три
  точных класса: ticket automation policy/status, endpoint-announcement
  bootstrap helper disposition и legacy central-HTTPS mailbox command removal;
- семь Postcard-only activation blockers перечислены явно. Dedicated boundary
  отвергает пропущенный active variant, unpaired result, missing blocker и
  premature completion claim;
- все 19 runtime-IPC boundaries, aggregate coverage/compatibility и accepted-tag
  readiness прошли 79/79; rustfmt и workspace locked/offline strict Clippy
  также прошли;
- canonical coverage остаётся 16 complete / 1 partial / 1 not-started / 2
  not-targeted. Canonical CBOR не активирован; active local control, peer wire,
  network, release/archive/push/tag не менялись.

### M1.1.47 — canonical runtime IPC ticket automation: принято локально

- добавлены deterministic-CBOR `configure-ticket-automation` и
  `ticket-automation-status` с Device-signed configured/status/error responses,
  domain-separated policy/snapshot/request/response IDs и checked-in golden
  vectors;
- configuration относится только к already-enrolled contact и root-only HTTPS
  opaque ticket-publication origin. Contact ID пересчитывается из exact
  descriptor Account, peer Account и conversation label; ticket plaintext,
  capabilities, secret keys, IP/network names и filesystem paths через IPC не
  проходят;
- policy replacement exact-связывает predecessor generation/ID. Status содержит
  fixed publish/refresh actions, strict contact-ID order, exact success recheck
  и capped exponential failure deadline; publication conflict quarantine
  допускается только для refresh и не планирует автоматический retry;
- execution scope остаётся foreground runtime only, restart не требуется,
  Windows Task Scheduler/service и другие OS background agents не включаются;
- независимый Node reference воспроизвёл CBOR/BLAKE3/Ed25519 bytes. Полный
  bounded decoder corpus прошёл 31851/31851 mutations за 2647.50 s; protocol
  unit regression: 141 passed, 0 failed, 15 explicit vector generators ignored;
- accepted-tag readiness прошёл 80/80 вместе с rustfmt и workspace offline
  strict Clippy. Completion ledger теперь содержит 34 commands, 33 results и
  34 pairs; остаются ровно endpoint-announcement bootstrap-helper disposition и
  legacy central-HTTPS mailbox-command removal. Active Postcard v27/descriptor
  v8, peer wire, network, release/archive/push/tag не менялись.

### M1.1.48 — active runtime IPC endpoint bootstrap retirement: принято локально

- из публичного CLI и активного loopback Postcard IPC удалены ручные
  `ExportEndpointAnnouncements`, `ImportEndpointAnnouncements` и
  `PushEndpointAnnouncements` вместе с их тремя ответами;
- authenticated peer `EndpointAnnouncementPush`, foreground own-device/contact
  automations и внутренние export/import helpers сохранены: удалён только
  устаревший operator-driven bootstrap path, а не transport capability;
- из-за сдвига Postcard enum discriminants выполнен exact-version hard cut:
  IPC v28 и signed descriptor domain v9 отвергают legacy v27/v8 до
  connect/actor dispatch; downgrade/dual reader не добавлялись;
- dedicated retirement boundary self-test-ит forged current version, пропуск
  retired command и повторное появление CLI subcommand. Completion inventory
  теперь исчерпывает 31 active command, 28 response и оставляет только два
  legacy central-HTTPS mailbox blockers;
- `kilogram-runtime-ipc` regression прошёл 17/17, focused CLI regression — 4/4,
  все canonical IPC gates и accepted-tag readiness — 81/81; rustfmt и workspace
  locked/offline strict Clippy прошли с двумя Cargo jobs и BelowNormal priority;
- default current-candidate readiness ожидаемо не переиспользует старое physical
  field evidence после protected runtime drift. Peer/relay/mailbox/sync wire,
  network, release/archive/executable/push/tag не менялись.

### M1.1.49 — active runtime IPC legacy mailbox retirement: принято локально

- из активного loopback Postcard IPC удалены legacy central-HTTPS
  `CreateMailboxCapability` и `RotateMailboxCapability`; canonical create/rotate
  обслуживаются только exact volunteer-provider variants. Revoke сохранён;
- историческое encrypted state по-прежнему может быть прочитано и односторонне
  upgraded, а foreground CLI recovery adapters остаются вне routine desktop IPC.
  Возврата к central-HTTPS mailbox path не добавлено;
- из-за сдвига Postcard enum discriminants выполнен exact-version hard cut:
  IPC v29 и signed descriptor domain v10 отвергают legacy v28/v9 до actor
  dispatch; downgrade/dual reader отсутствуют;
- completion inventory теперь исчерпывает 34 canonical commands, 33 results и
  34 pairs против всех 29/28 active Postcard variants. Blockers и remaining work
  пусты; `runtime-ipc` повышен из partial в complete;
- `kilogram-runtime-ipc` regression прошёл 17/17, `kilogram-cli` — offline
  compile check; accepted-tag readiness прошёл 82/82 вместе с rustfmt и
  workspace locked/offline strict Clippy в два Cargo job;
- canonical coverage теперь 17 complete / 0 partial / 1 not-started / 2
  not-targeted. Canonical target и текущий adapter complete, но canonical CBOR
  остаётся inactive и ещё не имеет protocol identifier/negotiation;
- dedicated retirement, completion, aggregate canonical IPC, compatibility и
  readiness boundaries проходят. Peer/relay/mailbox/sync wire, network,
  release/archive/executable/push/tag не менялись;
- следующий приоритет — `transport-sync`: parallel CBOR ALPN/negotiation с
  explicit identifiers, downgrade resistance и сохранением Postcard fallback до
  отдельного доказанного activation решения.

### M1.1.50 — canonical sync ALPN allocation: принято локально

- зарезервирован отдельный exact ALPN `kilogram/m1/sync/cbor/1` для inactive
  deterministic-CBOR sync candidate; старый `kilogram/m0/sync/8` остаётся
  единственным зарегистрированным и dialed sync ALPN;
- `SyncWireProtocol` принимает только два exact identifier и fail closed
  отвергает unknown, truncated и future versions. Format sniffing и fallback
  после canonical connect/decode failure запрещены;
- `CANONICAL-TRANSPORT-SYNC.json` отдельно фиксирует allocation и activation:
  candidate allocated, но `registered=false`, `dialed=false`; active wire и
  compatibility registry не изменены;
- canonical migration, sync schema и session schema теперь ссылаются на exact
  allocated identifier. Allocation requirement verified, остальные activation
  prerequisites остаются pending;
- coverage: 17 complete / 1 partial (`transport-sync`) / 0 not-started / 2
  not-targeted. Readiness расширен до 83 fail-closed gates;
- transport regression прошёл 7/7, accepted-tag readiness — 83/83 вместе с
  rustfmt и workspace locked/offline strict Clippy в два Cargo job;
- network, release build, archive, stable-path EXE, push и tag не выполнялись.
  Следующий slice должен аутентифицировать listener capability до ALPN choice,
  а не принимать unauthenticated support hint.

### M1.1.51 — authenticated sync transport capability: принято локально

- connection ticket обновлён exact hard cut с v11/signature-domain v11 до
  v12/domain v12; legacy v1-v11 и unknown future artifacts fail closed, dual
  reader и silent downgrade отсутствуют;
- ticket содержит отдельно Device-signed capability v1, которая связывает
  listener Account/Device, exact authority revision, allowed requester Account,
  endpoint ID, publication channel epoch и общий prekey-directory validity
  window;
- capability объявляет active только `kilogram/m0/sync/8`, candidate только
  `kilogram/m1/sync/cbor/1` и `canonical_dial_enabled=false`. Canonical ALPN
  остаётся unregistered/undialed, active sync frame остаётся Postcard v6;
- online и Iroh-free offline verifier используют общий
  `kilogram-sync-capability` crate и сначала проверяют outer ticket signature,
  затем nested signature/context/freshness. Valid capability, перенесённый на
  другой endpoint, отвергается;
- dedicated boundary self-test отвергает forged v11 current version, включённый
  canonical dial, удалённый endpoint binding, undeclared dual reader и
  premature canonical registration. Protocol registry содержит 159 exact
  bindings, readiness inventory расширен до 84 gates;
- focused capability regression прошёл 2/2, CLI connection-ticket regression —
  6/6, offline dependency boundary подтверждает отсутствие Iroh/Tokio/Reqwest;
  accepted-tag readiness прошёл 84/84 вместе с rustfmt и workspace
  locked/offline strict Clippy; network, release/archive/stable EXE/push/tag не
  выполнялись;
- следующий slice — active parallel canonical reader и явное включение signed
  capability с mixed-version, downgrade и no-fallback regression.

### M1.1.52 — canonical sync historical-signature bridge: принято локально

- до activation обнаружен и зафиксирован loss-of-history blocker: active
  `AuthorizedEvent`, Device certificate, authority snapshot и ratchet artifacts
  подписаны над Postcard-specific preimages, поэтому native canonical event
  нельзя получить без приватных ключей каждого исторического автора;
- добавлен bounded deterministic-CBOR `CanonicalSyncLegacyEventBridgeV1`:
  он сохраняет exact active Postcard bytes, прогоняет active decode/signature
  verification, требует exact re-encoding и сверяет conversation ID, active
  Event ID, author Account ID и author Device ID;
- committed vector воспроизводит реальный валидный active acknowledgement
  event. Rust regression 2/2 отвергает metadata/payload tamper, trailing data и
  non-minimal CBOR; независимый Node reference воспроизводит outer CBOR bytes;
- `kilogram/m1/sync/cbor/1` не меняется молча: его closed native-event schema
  отмечен как installed-history-incompatible и blocked-before-activation.
  Следующий generation должен получить новый explicit identifier, встроить
  bridge в closed frames и попасть в signed ticket capability;
- migration/transport manifests получили verified bridge prerequisite и новый
  pending `history-compatible-canonical-sync-generation`; canonical dial всё ещё
  disabled, единственный active ALPN — `kilogram/m0/sync/8`;
- accepted-tag readiness прошёл 85/85 вместе с rustfmt и workspace
  locked/offline strict Clippy в два Cargo job; network, release build, archive,
  stable-path EXE, push и tag не выполнялись.

### M1.1.53 — typed history-compatible sync event carrier: принято локально

- выделен отдельный inactive ALPN `kilogram/m1/sync/cbor/2`; active
  `kilogram/m0/sync/8` остаётся единственным registered/dialed, а несовместимый
  с установленной историей `/cbor/1` не меняется;
- введён `CanonicalSyncEventKeyV2 = (namespace, Event ID)`: namespace `1`
  относится к active Postcard-v5 Event-ID domain, namespace `2` — к native
  canonical-CBOR-v1 domain. Даже одинаковые 32 байта теперь являются разными
  логическими ключами;
- closed `CanonicalSyncEventCarrierV2` принимает ровно verified legacy bridge
  или verified native canonical event. Variant выбирает один decoder; sniffing,
  parse-error fallback, unknown namespace и variant/payload mismatch fail closed;
- CDDL/schema/vector и независимый Node reference воспроизводят оба typed key и
  carrier. Focused Rust regression 2/2 и семь связанных boundary contracts с
  self-tests прошли;
- accepted-tag readiness прошёл 86/86 вместе с rustfmt и workspace
  locked/offline strict Clippy в два Cargo job;
- full v2 inventory/diff/batch/complete frames, typed session checkpoints и
  Device-signed ticket capability для exact `/cbor/2` остаются pending. Active
  wire, ticket bytes, network, release/archive/stable EXE/push/tag не менялись.

### M1.1.54 — history-compatible canonical sync v2 frames: принято локально

- добавлен полный inactive `/cbor/2` codec: signed inventory/diff, event batch,
  complete, pause/paused/rejected и closed client/server frames;
- inventory/diff/complete переносят только typed `(namespace, Event ID)` keys,
  event arrays — только closed verified legacy/native carriers; порядок,
  duplicate, conversation, signature, boolean, variant и size invariants
  проверяются fail closed;
- независимый Node oracle воспроизводит exact v2 signature preimages и outer
  frame bytes. Focused Rust regression 3/3 проверяет оба carrier variants и
  негативные случаи;
- migration/transport/coverage ledgers фиксируют frames как verified, но typed
  durable checkpoint и authenticated `/cbor/2` ticket capability остаются
  pending. ALPN не registered/dialed, active Postcard wire не менялся;
- accepted-tag readiness прошёл 87/87 вместе с rustfmt и workspace
  locked/offline strict Clippy в два Cargo job. Network, release/archive,
  stable EXE, push и tag не выполнялись.

### M1.1.55 — typed durable canonical sync v2 checkpoint: принято локально

- добавлен отдельный inactive checkpoint codec для exact
  `kilogram/m1/sync/cbor/2`; signed content включает 23-byte protocol identifier,
  поэтому v1 checkpoint/signature нельзя переиспользовать для v2;
- received/sent cumulative evidence использует только
  `CanonicalSyncEventKeyV2 = (namespace, Event ID)`, вложенный как canonical
  CBOR bytes. Bare 32-byte IDs запрещены, порядок и duplicate проверяются по
  полному typed key;
- hash-linked Device-signed chain сохраняет exact predecessor, immutable
  conversation/Devices/session/protocol, monotonic evidence, bounded round,
  pause/resume и terminal transitions. Rollback namespace-only key fail closed;
- CDDL/schema/golden vectors и independent Node oracle воспроизводят четыре
  checkpoints, Ed25519 preimages/signatures и BLAKE3 IDs. Focused Rust
  regression прошёл 4/4, dedicated boundary и связанные migration/transport
  contracts прошли normal/self-test;
- `/cbor/2` остаётся unregistered/undialed и не advertised connection ticket
  v12. Следующий slice — exact Device-signed `/cbor/2` ticket capability;
  active Postcard v6, network, release/archive/stable EXE/push/tag не менялись.
- rustfmt, workspace-wide locked/offline strict Clippy и полный accepted-tag
  readiness прошли 88/88 против immutable `M1.0.0`; два исторических
  sync/session boundary также синхронизированы с новым closed coverage item и
  их negative self-tests остались зелёными.

### M1.1.56 — authenticated history-compatible sync capability: принято локально

- connection ticket получил exact hard cut v12/domain v12 → v13/domain v13,
  nested Device-signed sync capability — v1/domain v1 → v2/domain v2;
- capability по-прежнему объявляет ровно Postcard v6 как active, но inactive
  candidate теперь exact `kilogram/m1/sync/cbor/2`; history-incompatible
  `/cbor/1`, old version, changed binding, premature dial/registration fail
  closed;
- online CLI и Iroh-free offline verifier используют общий capability crate,
  outer ticket signature проверяется до nested semantic binding к authority,
  requester, endpoint, publication epoch и prekey validity;
- focused capability regression прошёл 3/3, connection-ticket regression —
  6/6, offline crate compile/test — green; все связанные aggregate boundaries
  и negative self-tests синхронизированы;
- rustfmt, workspace-wide locked/offline strict Clippy и полный accepted-tag
  readiness прошли 88/88 против immutable `M1.0.0`; ни один test executable
  или network process readiness-прогоном не запускался;
- `/cbor/2` остаётся unregistered/undialed, active Postcard wire не менялся.
  Network, release/archive/stable EXE/push/tag не выполнялись.

### M1.1.57 — dormant history-compatible sync runtime adapter: принято локально

- в `kilogram-session` добавлен transport-independent client/server
  state machine для exact `kilogram/m1/sync/cbor/2`; каждый переход проходит
  через closed canonical frame encoder и strict decoder;
- signed inventory/diff связывают conversation, session, requester и responder,
  а reconciliation сохраняет полный `(namespace, Event ID)` и требует exact
  requested/completion sets;
- generic `CanonicalSyncV2Store` задаёт atomic/idempotent typed storage
  boundary; обязательный `CanonicalSyncV2EventPolicy` выполняется до любой
  записи принятого carrier;
- focused regression прошёл 3/3: legacy Postcard и native canonical carriers
  сходятся в обоих направлениях, policy rejection, duplicate inventory и
  incomplete store read fail closed; dedicated и связанные aggregate boundary
  normal/self-tests прошли;
- active EventStore, concrete membership policy, durable checkpoint transaction
  и mutual capability selection остаются pending. `/cbor/2` не зарегистрирован
  и не dial-ится, active Postcard v6 не менялся;
- полный `kilogram-session` regression прошёл 8/8; rustfmt, workspace-wide
  locked/offline strict Clippy и accepted-tag readiness прошли 89/89 против
  immutable `M1.0.0`. Readiness не запускал test executable и не выполнял
  network operation;
- release/archive/stable EXE, push и tag на этом этапе не выполнялись.

### M1.1.58 — dormant atomic typed carrier store: принято локально

- `kilogram-store` получил отдельный `CanonicalSyncV2CarrierStore` на redb для
  exact `/cbor/2` carrier bytes; schema marker и имя БД versioned независимо от
  active filesystem `EventStore`;
- storage key содержит conversation ID, namespace и Event ID. Каждый read
  повторно strict-decode/verify carrier и сверяет embedded conversation/key с
  ключом БД;
- `put_batch` canonicalize/validate-ит весь набор, preflight-ит все immutable
  ключи и только затем записывает отсутствующие values одной transaction с
  immediate durability. Byte-identical replay идемпотентен, конфликт и
  corruption fail closed без partial commit;
- concrete store реализует dormant `CanonicalSyncV2Store`; focused
  debug/offline regression прошёл 3/3 в `kilogram-store` и 4/4 в
  `kilogram-session`, включая reopen, обе namespaces, rollback на конфликте и
  trait round trip;
- отдельный manifest/RFC/fail-closed boundary и aggregate migration,
  compatibility, transport и coverage contracts фиксируют, что active runtime
  не использует новую БД;
- import установленной legacy history, concrete membership policy,
  transactional checkpoint coupling и mutual capability selection остаются
  pending. `/cbor/2` по-прежнему не зарегистрирован и не dial-ится;
- финальные полные locked/offline debug regression прошли 14/14 для
  `kilogram-store` и 9/9 для `kilogram-session`; rustfmt, workspace-wide strict
  Clippy и accepted-tag readiness прошли 90/90 против immutable `M1.0.0`;
- network, release/archive/stable EXE, push и tag на этом этапе не выполнялись.

### M1.1.59 — dormant cross-format membership policy: принято локально

- `kilogram-session` получил concrete `CanonicalSyncV2MembershipPolicy` для
  exact `/cbor/2` pre-storage authorization. Policy принимает только явно
  выбранный legacy Postcard либо canonical-CBOR membership snapshot;
- полная матрица 2×2 позволяет каждому snapshot формату авторизовать legacy
  exact-byte carrier и native canonical carrier. Snapshot signature,
  conversation binding, carrier author authorization и membership проверяются
  fail closed без sniffing или parse-error fallback;
- cross-format bridge сохраняет установленную Root-signed membership authority:
  owner не обязан переподписывать старую conversation state только ради
  перехода на новый transport codec;
- focused regression прошёл 2/2, полный `kilogram-session` regression — 11/11.
  Dedicated и девять связанных aggregate boundary прошли normal/self-test;
- отдельный manifest/RFC/boundary и aggregate migration/transport/history/
  coverage contracts фиксируют отсутствие policy в active CLI, Iroh transport
  и wire path. `/cbor/2` остаётся unregistered/undialed;
- rustfmt, workspace-wide locked/offline strict Clippy и accepted-tag readiness
  прошли 91/91 против immutable `M1.0.0`; readiness не запускал test executable
  и не выполнял network operation;
- legacy import, checkpoint/store transaction и authenticated capability
  selection остаются следующими activation prerequisites. Release/archive,
  stable EXE, push и tag на этом этапе не выполнялись.

### M1.1.60 — dormant bounded/resumable legacy-history import: принято локально

- `kilogram-store` получил immutable `CanonicalSyncV2LegacyImportPlan`, exact
  plan-bound cursor и progress result для переноса установленной filesystem
  `EventStore` history в отдельный `CanonicalSyncV2CarrierStore`;
- source inventory ограничен 4096 events, одна page — 1..64 events. Plan ID
  domain-separated BLAKE3 связывает exact verified Postcard membership bytes,
  revision, conversation и каждый строго упорядоченный Event ID с exact
  authorized Postcard bytes;
- import не синтезирует native canonical event и не переподписывает историю:
  каждый исходный event повторно проверяется и сохраняется в exact legacy
  bridge carrier с исходными author/certificate/authority signatures;
- resume fail closed проверяет exact plan ID и весь prefix до cursor: skipped
  entry, changed source claim, changed membership или cross-plan cursor
  отвергаются. Повтор той же cursor идемпотентен;
- каждая page проходит existing redb preflight и одну atomic immediate-durability
  transaction; conflict в одном item не оставляет более поздние page inserts;
- focused regression прошёл 3/3, полный `kilogram-store` regression — 17/17.
  Dedicated boundary и десять связанных aggregate boundaries прошли normal и
  negative self-test modes;
- rustfmt, workspace-wide locked/offline strict Clippy и accepted-tag readiness
  прошли 92/92 против immutable `M1.0.0`; readiness не запускал Rust test
  executable и не выполнял network operation;
- cursor намеренно ещё не хранится durable: следующий этап должен записывать
  plan ID/cursor в одной redb transaction с carrier page и затем связать это с
  signed `/cbor/2` checkpoint. Active CLI/Iroh/wire не изменены, `/cbor/2`
  unregistered/undialed; release/archive/stable EXE, push и tag не выполнялись.

### M1.1.61 — dormant atomic legacy-import checkpoint: принято локально

- отдельная redb table хранит fixed-format 129-byte local checkpoint по exact
  `(conversation ID, immutable import plan ID)` и повторяет plan binding,
  membership revision/digest, total event count, начало последней page и
  следующий source index;
- carrier page и checkpoint advance выполняются одной immediate-durability
  transaction. Conflict или malformed durable state не оставляет partial page
  и не продвигает cursor;
- missing checkpoint принимает только index 0. Stale/skipped cursor и page
  shape, включая другую page с тем же end, fail closed; exact retry после
  потерянного ответа повторно проверяет carrier page и идемпотентно возвращает
  прежний progress;
- regression покрывает reopen, same-cursor replay, prefix без checkpoint,
  stale page, corrupted/cross-plan record и later-page conflict rollback;
- dedicated manifest/RFC/boundary и aggregate migration/transport/history/
  coverage contracts фиксируют, что checkpoint остаётся dormant;
- focused import regression прошёл 5/5, полный `kilogram-store` regression —
  19/19. Dedicated boundary и одиннадцать связанных aggregate boundaries
  прошли normal и negative self-test modes;
- root canonical JSON parsing, rustfmt, workspace-wide locked/offline strict
  Clippy и accepted-tag readiness прошли 93/93 против immutable `M1.0.0`;
  readiness не запускал Rust test executable и не выполнял network operation;
- это не `CanonicalSyncSessionCheckpointV2`: importer не создаёт peer-visible
  Device signature, session/peer/inventory evidence или network state. Следующий
  этап — signed checkpoint/runtime/capability integration;
- active CLI/Iroh/wire не изменены, `/cbor/2` unregistered/undialed;
  release/archive/stable EXE, push и tag не выполнялись.

### M1.1.62 — dormant signed session bootstrap after exact import: принято локально

- `CanonicalSyncV2CarrierStore` получил immutable Device-signed checkpoint
  journal по exact session identity и sequence, а также отдельный atomic head.
  Record локально связывает canonical signed checkpoint и content-derived ID с
  exact legacy-import plan ID;
- `CanonicalSyncV2Client::bootstrap_from_completed_legacy_import` создаёт
  единственный initial shape: sequence/round 1, no predecessor/evidence,
  availability false, `SendInventory`, not paused. До complete durable import
  plan constructor fail closed получает missing/incomplete error;
- journal record и head preflight/insert выполняются одной
  immediate-durability redb transaction. Exact replay идемпотентен;
  orphan/conflicting head/record, corruption и signature/ID/key mismatch
  отвергаются. Reopen возвращает тот же verified checkpoint;
- plan ID остаётся local migration evidence и не добавляется в peer-visible
  signed schema. Интегрирован только sequence 1; successor checkpoint
  persistence/restore после runtime transitions ещё отсутствует;
- focused store regression прошёл 3/3, полный `kilogram-store` — 22/22;
  focused cross-crate session regression — 1/1, полный `kilogram-session` —
  12/12. Dedicated boundary и двенадцать связанных aggregate boundaries
  прошли normal и negative self-test modes;
- все 92 root JSON documents parsed, rustfmt, workspace-wide locked/offline
  strict Clippy и accepted-tag readiness прошли 94/94 против immutable
  `M1.0.0`; readiness не запускал Rust test executable и не выполнял network;
- active CLI/Iroh/wire не изменены, `/cbor/2` unregistered/undialed. Следующий
  bounded slice — signed successor checkpoint persistence/restore во всех
  runtime transitions, затем mutual authenticated capability selection;
  release/archive/stable EXE, push и tag не выполнялись.

### M1.1.64 — checkpoint-bound runtime frame journal: принято локально

- `CanonicalSyncV2CarrierStore` получил отдельную versioned redb table для
  local runtime record: exact content-derived ID текущего verified signed
  checkpoint плюс optional strict canonical client frame;
- bootstrap создаёт empty runtime record, а generic signed-successor append
  rebind-ит сохранённый frame к новому checkpoint ID в той же
  immediate-durability transaction, что immutable record и replacement head;
- `set_session_pending_client_frame` принимает только current durable head.
  Первичная запись write-once, exact lost-response retry идемпотентен, другой
  frame на том же head и stale checkpoint fail closed;
- restore сначала проверяет всю signed checkpoint chain, затем exact runtime
  binding и closed client-frame codec. Inventory дополнительно связан с local
  Device и session binding; malformed presence, wrong conversation, missing,
  orphan или checkpoint-mismatched record отвергаются;
- exact replay старого M1.1.62/M1.1.63 bootstrap может backfill-ить только
  missing empty runtime record, не угадывая pending frame;
- full workspace debug/offline unit regression завершился без failures;
  `kilogram-store` прошёл 26/26, `kilogram-session` — 12/12. Пятнадцать
  связанных boundaries прошли normal и negative self-test (30/30);
- все 94 root JSON documents parsed, rustfmt, workspace locked/offline strict
  Clippy и accepted-tag readiness прошли 96/96 против immutable `M1.0.0`;
- это exact-frame prerequisite, но ещё не transition engine: state machine не
  создаёт successor на каждом принятом frame, received carrier не коммитится
  атомарно с checkpoint, restored action не dispatch-ится. `/cbor/2` остаётся
  unregistered/undialed; network/release/archive/stable EXE/push/tag не
  выполнялись.

### M1.1.63 — dormant signed session successor store: принято локально

- `CanonicalSyncV2CarrierStore::append_session_checkpoint_successor` требует
  exact durable head и проверяет новый Device-signed checkpoint canonical
  protocol verifier: immutable session identity, sequence/predecessor/round,
  typed cumulative evidence, availability, next-action, pause/resume и terminal
  rules. Import-plan binding наследуется только из verified local head;
- immutable successor record и replacement head коммитятся одной
  immediate-durability redb transaction. Exact lost-response replay
  идемпотентен; gap, stale/forked successor, orphan и successor после terminal
  state fail closed;
- `session_checkpoint_chain` bounded до 65,536 records и после reopen проверяет
  exact initial bootstrap, каждый record key/signature/content-derived ID/local
  plan binding и каждую predecessor edge до head. Missing intermediate record,
  corrupt ancestry и oversized head отвергаются без guess/repair/truncation;
- regression восстанавливает exact three-checkpoint chain после reopen и
  покрывает replay, skipped sequence, same-sequence fork, corrupt/missing
  ancestry и missing durable head. Focused store regression прошёл 5/5, полный
  `kilogram-store` — 24/24, полный `kilogram-session` — 12/12;
- dedicated boundary и тринадцать связанных aggregate boundaries прошли normal
  и negative self-test modes. Все 93 root JSON documents parsed, rustfmt,
  workspace-wide locked/offline strict Clippy и accepted-tag readiness прошли
  95/95 против immutable `M1.0.0`;
- это dormant storage/restore primitive: runtime frame transitions ещё не
  создают checkpoints и не продолжаются из restored action. `/cbor/2` остаётся
  unregistered/undialed; следующий bounded slice — runtime transition
  checkpointing/restart continuation, затем mutual authenticated capability
  selection. Network/release/archive/stable EXE/push/tag не выполнялись.
### M1.1.65 — dormant atomic runtime transition transaction: принято локально

- `CanonicalSyncV2CarrierStore::commit_session_runtime_transition` принимает
  exact restored runtime как optimistic concurrency token, verified signed
  successor, replacement pending frame и received typed carriers;
- successor signature/edge, runtime/head/predecessor и closed carrier batch
  проверяются до write. Missing carriers, immutable successor, head и runtime
  record коммитятся одной immediate-durability redb transaction;
- exact lost-response retry повторно проверяет committed checkpoint, frame и
  carrier bytes и возвращает тот же runtime. Stale/mismatched expectation,
  different retry, orphan successor и immutable carrier conflict fail closed;
- regression доказывает reopen/restore и полное rollback: corrupt carrier не
  оставляет новый carrier, successor, head или runtime frame;
- dedicated manifest, RFC-0191 и boundary фиксируют dormant store-only scope:
  adapter/resume/capability selection не активированы, `/cbor/2` остаётся
  unregistered/undialed;
- focused atomic regression прошёл 2/2, полный `kilogram-store` — 28/28,
  связанный `kilogram-session` — 12/12. Dedicated boundary и пятнадцать
  затронутых aggregate boundaries прошли normal и negative self-test modes;
- все 95 root JSON documents parsed, rustfmt, workspace-wide locked/offline
  strict Clippy и accepted-tag readiness прошли 97/97 против immutable
  `M1.0.0`; readiness не запускал test executable и network operation;
- network/release/archive/stable EXE, push и tag не выполнялись.

### M1.1.66 — dormant durable client transition adapter: принято локально

- concrete `CanonicalSyncV2Client<CanonicalSyncV2CarrierStore>` восстанавливает
  exact session runtime и отображает его в однозначное действие: prepare
  Inventory, transmit persisted Inventory/EventBatch, retransmit exact
  EventBatch while awaiting Complete, paused либо terminal;
- `begin_durable_round` подписывает и сохраняет Inventory до transport write.
  `accept_durable_diff` проверяет responder/session/request/policy и одной
  M1.1.65 transaction коммитит received carriers, signed cumulative evidence,
  head и exact pending EventBatch;
- `mark_durable_batch_sent` сохраняет тот же EventBatch при переходе в
  `ReceiveComplete`. `accept_durable_complete` сверяет exact current-round sent
  keys и continuation, затем очищает frame в terminal `Complete` либо ровно на
  один round переходит к `SendInventory`;
- local availability остаётся true, пока EventBatch, включая empty batch,
  доступен для send/retry. Это позволяет корректно завершить уже converged
  empty round; terminal checkpoint очищает оба availability flags;
- regression выполняет два раунда, atomic receive, три exact lost-response
  retry, drop/reopen redb, empty final round и проверяет девять signed
  checkpoints. Полный `kilogram-session` regression прошёл 13/13;
- dedicated manifest/RFC/boundary фиксируют отсутствие вызовов нового adapter
  из active wire/Iroh/CLI. Durable server adapter и mutual authenticated
  capability selection остаются следующими отдельными этапами;
- все 96 root JSON documents parsed. Dedicated и связанные boundaries прошли
  normal/negative проверки; accepted-tag readiness прошёл 98/98 против
  immutable `M1.0.0`, включая rustfmt и workspace-wide locked/offline strict
  Clippy без запуска Rust test executables внутри readiness;
- active Postcard v6 не изменён, `/cbor/2` unregistered/undialed; network,
  release/archive/stable EXE, push и tag не выполнялись.

### M1.1.67 — dormant durable responder transition adapter: принято локально

- добавлен отдельный `CanonicalSignedSyncResponderCheckpointV2`: он подписывает
  conversation, responder/requester Devices, session, sequence/round,
  predecessor, cumulative typed evidence, continuation и responder action;
- responder signature/ID domains и три redb tables отделены от initiator
  journal, поэтому роли нельзя подменить совместимым байтовым представлением;
- concrete server сохраняет signed successor и exact Diff до send, сохраняет
  тот же Diff на uncertain send boundary и восстанавливает его после restart;
- incoming EventBatch проверяется против exact persisted Diff, а verified
  carriers, responder successor/head и exact Complete коммитятся одной
  immediate-durability transaction. Exact retry идемпотентен, stale/fork/gap,
  frame или signed-evidence mismatch fail closed;
- restart regression закрывает Inventory → Diff → EventBatch → Complete,
  drop/reopen redb, lost-response retries и terminal five-checkpoint chain;
- protocol regression прошёл 170/170, store 28/28, session 14/14; 36 запусков
  затронутых boundary normal/self-test прошли, 97 root JSON parsed, rustfmt и
  workspace-wide locked/offline strict Clippy verified; accepted-tag readiness
  прошёл 99/99 против immutable `M1.0.0`;
- оба dormant adapter теперь durable, но mutual authenticated capability
  selection ещё не подключён. Active Postcard v6 не менялся, `/cbor/2`
  unregistered/undialed; network/release/archive/stable EXE/push/tag не
  выполнялись.

### M1.1.68 — dormant mutual authenticated candidate selection: принято локально

- `kilogram-sync-capability` создаёт domain-separated artifact ID listener
  capability, requester-signed exact candidate offer и listener-signed
  acceptance exact offer ID;
- offer exact-связывает conversation, обе Account/Device identity, обе
  authority revision, session binding, migration epoch и только
  `kilogram/m1/sync/cbor/2`; fallback после выбранного canonical failure
  запрещён;
- session integration повторно проверяет ticket binding/freshness, обе
  canonical Device session authorization с `SyncHistory` и exact identity
  durable client/responder checkpoints;
- результат намеренно не является transport permit:
  `active_transport_authorized=false`; canonical ALPN остаётся
  unregistered/undialed, active Postcard v6 не меняется;
- добавлены `CANONICAL-SYNC-V2-MUTUAL-CANDIDATE-SELECTION.json`, RFC-0195 и
  отдельный boundary с негативными self-tests. Следующий bounded slice —
  mixed-version/downgrade matrix до любого dual-reader activation.
- `kilogram-sync-capability` прошёл 5/5, `kilogram-session` 15/15; 20
  затронутых boundaries прошли normal и 19 поддерживающих self-test прошли
  negative mode. Все 98 root JSON parsed, rustfmt и workspace locked/offline
  strict Clippy verified; accepted-tag readiness прошёл 100/100 против
  immutable `M1.0.0` без запуска Rust test executables внутри readiness.

### M1.1.69 — dormant mixed-version and downgrade matrix: принято локально

- добавлены `CanonicalSyncV2MigrationState`, dormant transport plan и pure
  policy evaluator для пяти состояний migration без подключения к active Iroh;
- legacy без selection разрешён в dual-reader состояниях только как outcome со
  старым peer; canonical без exact mutual selection отклоняется;
- после authenticated canonical selection legacy считается downgrade. Epoch
  rollback и любой fallback после canonical failure fail closed до выбора
  состояния; sniffing/parse fallback по-прежнему запрещены;
- successful plan всегда сообщает `active_transport_authorized=false`.
  Отдельно зафиксировано, что будущий runtime должен переносить signed ceremony
  внутри authenticated transport context, поскольку local matrix не решает
  pre-offer stripping сама по себе;
- добавлены manifest, RFC-0196 и dedicated boundary с пятью negative self-tests.
  Новая и 19 затронутых старых boundaries прошли normal и self-test режимы;
- полный `kilogram-session` regression прошёл 16/16 вне Windows sandbox; все
  99 root JSON parsed, rustfmt и workspace locked/offline strict Clippy прошли;
  accepted-tag readiness — 101/101 против immutable `M1.0.0`;
- active Postcard v6 не менялся, `/cbor/2` остаётся unregistered/undialed;
  network/release/archive/stable EXE/push/tag не выполнялись. Следующий bounded
  slice — dormant dual-reader runtime coordinator без transport activation.

### M1.1.70 — dormant dual-reader runtime coordinator: принято локально

- добавлен `CanonicalSyncV2DormantDualReaderCoordinator`, который принимает
  только уже verified `CanonicalSyncV2DormantTransportPlan`;
- legacy branch возвращает символический `LegacyPostcardV6` и не дублирует
  установленную реализацию. Canonical branch требует durable client либо
  responder runtime и exact-сверяет protocol, conversation, роли Device и
  session с mutual authenticated selection;
- canonical dispatch вычисляется только из сохранённого checkpoint и exact
  pending frame. Missing runtime, cross-session identity и action/frame
  mismatch fail closed; крупный responder action хранится косвенно, чтобы не
  раздувать каждый enum dispatch;
- coordinator не выполняет network I/O и всегда сообщает
  `active_transport_authorized=false`; active wire, Iroh и CLI не получили
  call sites, `/cbor/2` остаётся unregistered/undialed;
- новый manifest, RFC-0197 и dedicated boundary добавлены. Новая и 20
  затронутых исторических boundaries прошли normal и negative self-test режимы
  (21/21 пар);
- полный `kilogram-session` regression прошёл 17/17; все 100 root JSON parsed,
  rustfmt и workspace locked/offline strict Clippy с `-D warnings` прошли;
  accepted-tag readiness — 102/102 против immutable `M1.0.0`;
- network/release/archive/stable EXE/push/tag не выполнялись. Следующий bounded
  slice — deterministic rollback drill, затем sustained fuzzing и fresh field
  evidence до отдельного activation решения.

### M1.1.71 — deterministic dormant rollback drill: принято локально

- добавлены strict fixed-width `CanonicalSyncV2RollbackSnapshot` и
  content-addressed `CanonicalSyncV2RollbackDrillRecord`; snapshot хранит
  migration state/epoch и optional IDs selected session, client progress и
  responder progress;
- разрешены только explicit local и release-policy triggers с непустой
  trimmed/control-free причиной не длиннее 256 bytes. Parse/network error,
  stale/skipped epoch, skipped state и rollback из `legacy-only` fail closed;
- rollback движется ровно на одно состояние к legacy, одновременно увеличивая
  migration epoch на единицу. Идентичный запрос порождает byte-identical record;
- regression сохраняет record во временный файл, восстанавливает после
  simulated restart и применяет только к exact previous snapshot. Любая mutual
  canonical selection или durable progress запрещает rollback;
- replacement может вернуть только symbolic legacy dispatch; active store,
  wire, Iroh и CLI не подключены, transport permit всегда false;
- новый manifest, RFC-0198 и dedicated boundary добавлены. Новая и 21
  затронутая boundary прошли normal и negative self-test режимы (22/22 пары);
- полный `kilogram-session` offline debug regression прошёл 18/18; 101 root
  JSON parsed; rustfmt и workspace locked/offline strict Clippy прошли;
  accepted-tag readiness — 103/103 против immutable `M1.0.0`;
- network/release/archive/stable EXE/push/tag не выполнялись. До отдельного
  activation решения остаются sustained coverage-guided fuzzing и fresh
  hosted/physical-device evidence.

### M1.1.72 — bounded canonical sync v2 decoder mutations: принято локально

- добавлен отдельный manifest `CANONICAL-SYNC-V2-DECODER-MUTATIONS.json` с 20
  exact fixtures и 2863 детерминированными mutations для typed history keys,
  обоих carrier variants, inventory/diff/batch/terminal frames, client/server
  wire frames и обоих signed checkpoint domains;
- responder checkpoint получил собственный independently checked golden vector:
  exact content, signing preimage, Ed25519 signature, signed CBOR и BLAKE3 ID
  воспроизводятся из fixed test seed;
- mutation policy различает structural-only unsigned semantic objects и
  authenticated signed/carrier objects. Truncation, trailing data,
  non-minimal version, indefinite array, reserved head и oversized byte string
  fail closed; authenticated low-bit flips обязаны отвергаться;
- focused regression прошёл 2863/2863 mutation cases; dedicated boundary и
  затронутые aggregate boundaries прошли normal и negative self-test режимы;
- полный `kilogram-protocol` offline/debug regression вне Windows sandbox
  прошёл 157 tests при 15 explicit generators ignored: library 155/155,
  исторический corpus 1/1 за 2607.50s и новый corpus 1/1 за 23.71s;
- 103 root JSON parsed, readiness inventory exact-связан со 104 boundary
  scripts;
- rustfmt, workspace locked/offline strict Clippy с `-D warnings` и
  accepted-tag readiness 104/104 против immutable `M1.0.0` прошли;
- corpus явно сообщает `coverage_guided=false` и
  `compiler_instrumentation=false`: он усиливает bounded CI regression, но не
  закрывает sustained coverage-guided fuzzing gate;
- active Postcard v6 и `/cbor/2` transport activation не менялись. Network,
  release build, archive, stable EXE, push и tag не выполнялись.

### M1.1.73 — canonical sync v2 coverage fuzz harness: принято локально

- добавлен отдельный nested `fuzz` workspace со стандартным
  `cargo-fuzz`/libFuzzer target; production crate включает dispatcher только
  через opt-in feature `decoder-fuzzing`;
- 13 selectors покрывают все public strict decoder entry points, а 20
  allowlisted golden fixtures материализуются как raw binary: один harness
  selector byte плюс exact canonical payload;
- Rust regression независимо восстанавливает seed bytes из JSON pointers,
  проверяет размеры и требует acceptance соответствующим decoder;
- campaign manifest и runner фиксируют один worker/job, below-normal priority,
  8-hour / 1,000,000-execution sustained minimum, 8 MiB + selector max input,
  10-second timeout, clean unchanged committed source, zero crashes, coverage
  report и machine record; unmet sustained contract завершается fail closed;
- локальный preflight не устанавливает зависимости и зафиксировал отсутствие
  nightly, cargo-fuzz и Windows MSVC ASan. Поэтому libFuzzer campaign и
  coverage report не заявлены выполненными;
- dedicated и 23 затронутых historical boundary scripts прошли normal и
  negative self-test режимы; readiness boundary, не имеющий `-SelfTest`,
  прошёл normal mode;
- все 104 root JSON parsed; focused Rust regressions прошли 1/1 + 1/1, полный
  `kilogram-protocol --lib --features decoder-fuzzing` — 156/156 при 15
  explicit generators ignored; rustfmt, workspace all-target/all-feature
  locked/offline strict Clippy и
  accepted-tag readiness 105/105 против immutable `M1.0.0` прошли;
- active Postcard v6 не менялся, `/cbor/2` остаётся unregistered/undialed;
  следующий отдельный этап — подготовленный instrumented host и фактическая
  sustained campaign, затем fresh hosted/physical-device evidence. Network,
  release build, archive, stable EXE, push и tag не выполнялись.

### M1.1.74 — pinned instrumented fuzz toolchain: принято локально

- установлен отдельный Rust nightly `1.101.0-nightly` commit
  `5d8fec0669a64607db9ccbf663e7c23125bf4748` с LLVM `23.1.4`; stable toolchain
  остаётся default для обычной workspace-разработки;
- установлен cargo-fuzz `0.13.2`, а nested `fuzz/Cargo.lock` фиксирует
  libfuzzer-sys `0.4.13` и полный dependency closure;
- установлен и обнаружен Visual Studio MSVC runtime component; runner требует
  exact runtime DLL и import library и добавляет DLL directory в PATH только
  на время дочернего процесса;
- `canonical_sync_v2_decoders` реально собран для
  `x86_64-pc-windows-msvc`: 2 178 560 bytes, SHA-256
  `1c53c755571bcdaf2399fd82b1c54c81850253815e6e8a6add9041ee60d5b03a`;
- preflight exact-проверяет compiler commit/date/LLVM, cargo-fuzz version,
  runtime files и 20 seed corpus; mismatch fail closed останавливает запуск;
- Windows PowerShell native stderr сохраняется в ignored log, но успех
  определяется exit code; каждый run безопасно сбрасывает только exact flat
  corpus/artifact directories и вновь материализует 20 seeds;
- instrumented target build не переименован в execution evidence: smoke,
  sustained campaign, coverage report и transport activation остаются false.

### M1.1.75 — instrumented decoder smoke and coverage profile: принято локально

- на clean committed revision
  `f9148093a7476e2d77c4a967b018c4b26a230225` выполнен отдельный
  инструментированный run продолжительностью 301 секунду;
- один job/worker выполнил 5 697 050 iterations, начав ровно с 20 seeds и
  завершив 859 corpus files; process exit 0, generated artifacts 0;
- coverage pipeline создал профиль 226 312 bytes с SHA-256
  `16e801dbd95013ac16e4bc413723a65ef3d243b0552c5160f5013614d253a723`;
  `llvm-profdata show` подтвердил front-end instrumentation, 871 functions,
  1986 blocks и total count 838 948 912;
- tracked `CANONICAL-SYNC-V2-FUZZ-SMOKE.json` хранит exact source/toolchain/run
  metadata, размеры и SHA-256 четырёх ignored evidence outputs; большие логи,
  corpus и binary profile в Git не добавляются;
- новый fail-closed boundary отделяет завершённый smoke gate от незавершённого
  sustained gate и отвергает source drift, dirty source, artifacts, missing
  coverage, parallel workers и premature activation;
- новый boundary и 23 затронутых historical boundary прошли normal и negative
  self-test режимы; readiness inventory содержит 106 gate, все 106 root JSON
  parsed;
- accepted-tag readiness 106/106 против immutable `M1.0.0`, rustfmt и
  workspace all-target/all-feature locked/offline strict Clippy прошли;
- обязательные 28 800 секунд и fresh hosted/physical-device evidence остаются
  pending. Active Postcard v6 не менялся, `/cbor/2` не зарегистрирован и не
  dial-ится; network, release build, archive, stable EXE, push и tag не
  выполнялись.

### M1.1.76 — sustained decoder campaign: принято локально

- на чистом committed revision
  `eaf3d75bb16ce42c8368017a68c6f4183d15ab9f` выполнен единственный
  восьмичасовой instrumented `canonical_sync_v2_decoders` run;
- один job/worker за 28 801 секунду выполнил 469 853 771 iterations, начав с
  20 checked-in seeds и завершив 1043 corpus files; exit 0, generated
  artifacts 0;
- LLVM profile имеет 230 688 bytes и SHA-256
  `34ca49d1a3d4b7ba5cc91969d823527f9370de41282761a0fb923fb87c710a92`;
  `llvm-profdata show` подтвердил front-end instrumentation, 888 functions,
  2010 blocks и total count 885 327 009;
- `CANONICAL-SYNC-V2-FUZZ-SUSTAINED.json` фиксирует exact source/toolchain/run
  и SHA-256 четырёх ignored evidence files; большие логи, corpus и profile не
  добавляются в Git;
- dedicated boundary отвергает short run, artifacts, dirty source, activation
  и aggregate drift; исторические toolchain/smoke records остаются неизменны;
- новый boundary и 25 затронутых historical boundary прошли normal и negative
  self-test режимы; все 107 root JSON parsed; accepted-tag readiness 107/107,
  rustfmt и workspace all-target/all-feature locked/offline strict Clippy
  прошли;
- sustained coverage-guided decoder gate закрыт. Fresh hosted/physical-device
  evidence остаётся pending; active Postcard v6 не менялся, `/cbor/2` остаётся
  unregistered/undialed.

### M1.1.77 — bounded canonical sync v2 evidence contract: принято локально

- добавлен versioned M1.1.80 manifest, который раздельно описывает hosted и
  physical Windows evidence, exact revision, debug binary SHA-256, topology,
  direct/restart/relay/mixed-version/fail-closed/rollback scenarios;
- fail-closed verifier отвергает source drift, fallback после canonical
  selection, недостоверную physical topology и появление secret material;
- positive self-test и четыре negative mutations прошли;
- обычный production CLI не получает feature и остаётся legacy-only.

### M1.1.78 — evidence-only Iroh lane: принято локально

- отдельный `kilogram-sync-v2-evidence` доступен только через Cargo feature
  `sync-v2-evidence`; default features пусты, production main не ссылается на
  candidate lane;
- два независимых процесса используют exact ALPN `kilogram/m1/sync/cbor/2`,
  существующие durable client/responder adapters и typed carrier store;
- предусмотрены direct, restart checkpoint/resume, pre-selection legacy
  control, corrupt selected-frame fail-closed и one-step rollback drills;
- `--advertise-ip` публикует ровно один фактически обнаруженный адрес и порт,
  исключая loopback, VMware и VPN candidates из physical ticket;
- listener принимает evidence-only `--relay-url`; relay-only dialer затем
  pin-ит strict map к тому же URL из ticket. Для физического контроля выбран
  ранее проверенный APS1, без изменения production relay defaults;
- focused adapter regression прошёл 13/13; evidence binary и обычный CLI
  прошли locked/offline strict Clippy, rustfmt и boundary checks.

### M1.1.79 — manual hosted experiment: принято

- добавлен manual-only workflow `canonical-sync-v2-evidence.yml`; push и pull
  request не запускают его автоматически;
- workflow собирает только debug evidence binary с двумя Cargo jobs, запускает
  два свежих процесса на hosted Windows и сохраняет только bounded manifest;
- release, ZIP, tag, attestation и публикация исполняемого файла отсутствуют;
- accepted hosted result получен отдельным ручным запуском exact frozen
  revision; workflow остаётся manual-only.
- manual run `39114897550` на `ad98314...` успешно прошёл checkout, clean debug
  build и оба boundary/self-test шага, но fail-closed остановился до direct
  scenario: runner создавал scenario directory для prepare-log раньше Rust
  `prepare`. После исправления полный локальный hosted matrix зелёный;
  `$LASTEXITCODE` после PowerShell verifier также удалён как неверный контракт.
- эти orchestration fixes не меняют production runtime или active wire, но
  требуют нового frozen commit и нового manual run; run `39114897550` остаётся
  диагностическим, а не принятым hosted evidence.

### M1.1.80 — canonical sync v2 network evidence: принято

- ноутбук `paul_hp_omen` и основной ПК подготовлены для одинакового stable-name
  debug binary; coordination выполняется через SSH/SCP, не через Yandex Disk;
- Windows Firewall разрешает exact stable path на ноутбуке, а основной ПК уже
  имеет узкие правила для workspace debug CLI;
- проверка вне сетевой песочницы подтвердила TCP/22, SSH execution и SCP при
  включённом VPN основного ПК; прежние отрицательные LAN probes были artifact
  sandbox isolation, а не ограничением ноутбука или v2rayN;
- автономный fresh rehearsal на `ae9672b` и одном exact debug binary SHA-256
  `e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625`
  подтвердил direct, restart/resume, pinned APS1 relay, pre-selection legacy,
  zero-fallback corrupt selected-v2 и rollback на двух физических хостах;
- rehearsal выявил fail-closed verifier drift: checkpoint правильно имеет
  Alice `2/SendEventBatch`, Bob `1/ReceiveEventBatch`, а после restart обе роли
  сходятся к count 2 и одному digest. Verifier и Windows PowerShell 5.1 process
  runner исправлены, negative mutations расширены;
- rehearsal не становится финальным canonical evidence, потому что tooling fix
  меняет frozen source revision. После commit и пересборки требуется один final
  physical run, затем manual hosted run ровно той же ревизии.
- второй fresh physical run `m1180-field-20261018T034245Z` на commit
  `402e86861a4b4145d7947df36ea8be246cfd1031` и binary SHA-256
  `e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625`
  повторно подтвердил всю matrix; bounded manifest SHA-256
  `ae873be47c1e755d95c6ba76cb2261dfc7c0b0e3fef0495dc3c78216dc9bedd8`
  принят verifier-ом;
- full readiness затем обнаружил только historical gate drift: ранние
  mailbox/provider boundaries ошибочно считали любой explicit `[[bin]]`
  production-изменением. Они теперь делегируют exact проверку feature-gated
  evidence executable каноническому boundary, сохраняя запрет EXE для
  library/production targets;
- после исправления readiness прошёл `108/108`, locked/offline strict Clippy и
  rustfmt зелёные. Stable regression прошёл без failures: isolated runtime
  `1/1`, bootstrap `14/14`, CLI `82/82`, protocol `155/155` плюс 15 explicit
  vector-regeneration ignores, legacy decoder mutations `31 851/31 851`,
  canonical sync v2 mutations `2 863/2 863` и все оставшиеся workspace
  harnesses;
- historical boundary fixes снова меняют frozen revision, поэтому manifest
  `402e868` остаётся сильным pre-freeze evidence, но не подменяет один последний
  physical rerun и manual hosted run exact final commit.
- fresh physical run `m1180-field-20261018T051713Z` на `ad98314...` прошёл всю
  matrix; manifest SHA-256
  `52f9dbbb54d267617d86f61010e2e3a85c83ebf8437b22b37830b3f199e9ab55`.
  Первый hosted run этой ревизии выявил только orchestration defects после
  successful build/boundaries. Новый runner fix опять требует один final
  physical rerun и hosted run exact следующего commit.
- final frozen revision
  `b1f23c3719a0842979f047d874141953ae93ccac` прошла свежий physical run
  `m1180-field-20261018T093610Z`: два разных Windows host, одинаковый binary
  SHA-256 `e23ba26692caf7fe355f04dda5f8825b0222b32ffe62794f0228bd35e7e1f625`,
  manifest SHA-256
  `3b87c95c9298fead83ab6cf07d2c37d59e1d8e3336db1d85eb6841a8c9831b37`;
- manual hosted run `39116855502` той же revision полностью зелёный; hosted
  manifest SHA-256
  `d6382354c0327bb1aa900e7dc33536b467401f95edade1e9113f33e89e21e535`,
  hosted binary SHA-256
  `6a0add9561ce59e4c9b2d36a0ab3f81ab2fc5287df25905579f94c81ec031f9c`;
- оба topology независимо подтвердили direct, restart/resume, relay,
  mixed-version, zero-fallback fail-closed и rollback; различие hosted и
  physical binary hash допустимо, идентичность обязательна внутри topology;
- `CANONICAL-SYNC-V2-NETWORK-EVIDENCE.json` сохраняет compact audit record,
  последний activation requirement verified, aggregate coverage `18/18`
  targeted complete. Итог — `ready-for-explicit-activation-decision`, не
  activation: `/cbor/2` остаётся evidence-only и выключен по умолчанию.
- итоговое tracked дерево прошло self-tests всех 27 изменённых boundary gates;
  accepted-tag readiness прошёл `108/108`, rustfmt и locked/offline strict
  Clippy зелёные;
- полный stable Rust regression с profile `test`, двумя Cargo jobs, isolated
  test до aggregate harnesses и `-NoFailFast` завершился `status=passed`:
  isolated runtime `1/1`, bootstrap `14/14`, CLI `82/82`, protocol `155/155`
  плюс 15 explicit vector-regeneration ignores и все оставшиеся workspace
  harnesses. Legacy decoder corpus проверил `31 851/31 851` мутацию за
  `2640.20s`, canonical sync v2 corpus — `2 863/2 863` за `24.07s`.

### M1.1.81 — canonical sync v2 activation semantics: принято локально

- добавлен machine-readable
  `CANONICAL-SYNC-V2-ACTIVATION-POLICY.json` и RFC-0205;
- migration-state больше не считается достаточным transport permit: policy
  отдельно определяет legacy reader, capability publication, canonical ALPN
  registration, new accept, new initiate и exact selected-session resume;
- `advertise-cbor-v2` публикует build/read readiness и регистрирует reader, но
  ещё не принимает новую canonical session;
- `dual-reader-prefer-legacy` принимает authenticated peer-initiated v2, но не
  начинает его сам; `dual-reader-prefer-canonical` начинает v2 только после
  mutual authenticated selection;
- emergency containment формализован как overlay: он требует persisted
  canonical evidence, запрещает publication/new accept/new dial, но оставляет
  ALPN для exact resume и никогда не разрешает Postcard fallback;
- focused Rust tests и fail-closed verifier self-test прошли. Active Iroh/CLI
  не используют evaluator, `/cbor/2` по умолчанию остаётся
  unregistered/undialed, network не запускался.

### M1.1.82 — durable local activation authority: принято локально

- добавлены `SignedCanonicalSyncV2ActivationRecord` и signed high-water:
  Device ID, monotonic epoch, exact previous record, state/disposition,
  bounded actor/reason и optional exact canonical evidence входят в подпись;
- chain verifier fail closed отклоняет wrong Device, invalid signature,
  replay/fork/gap, skipped epoch, неодношаговый transition, post-evidence
  rollback и смену/удаление evidence binding;
- `canonical-sync-policy-status` сохраняет отсутствие authority как
  `legacy-only`; `plan` ничего не пишет, а `apply` exact-сверяет ранее
  напечатанный candidate ID;
- append-only record проходит через `StateTransaction` и vault-primary typed
  delta. High-water публикуется atomic replace только после commit; exact
  повторный apply может завершить прерванный marker publish;
- filesystem и migrated-vault CLI lifecycle прошли, включая restart и
  искусственно прерванный record/high-water boundary. Unit tests закрыли
  replay, epoch skip, fork, wrong Device, tail rollback и containment;
- полный regression изменённых crates прошёл: session 25/25 и CLI 83/83;
  targeted all-target/all-feature strict Clippy и rustfmt также зелёные;
- новый manifest/RFC и boundary verifier с negative self-test приняты;
  readiness inventory расширен до 110 gates. Active Iroh transport и public
  default не изменены, network/release/ZIP/tag не выполнялись.

### M1.1.83 — production runtime canonical sync v2 dark launch: принято локально

- обычный `kilogram-cli` теперь строит ALPN list и ticket capability только из
  verified Device-signed activation authority; отсутствие authority оставляет
  endpoint строго на `kilogram/m0/sync/8` без canonical candidate;
- в Postcard control channel добавлена bounded mutually authenticated selection
  ceremony. Requester offer и listener acceptance подписаны Devices, связаны с
  обоими authority snapshots, certificates, conversation, roles, стабильной
  session binding и listener ticket publication epoch;
- exact selection сохраняется append-only до canonical dial/acceptance и
  повторно проверяется при `/cbor/2`; после selection ни decode, network, state
  либо checkpoint error не вызывает legacy fallback;
- production client/responder используют общий Iroh adapter, typed carrier
  store, resumable legacy import и checkpoint-bound frame journal. Завершённые
  legacy carriers материализуются обратно в active history; canonical-native
  materialization пока намеренно выключена;
- один fresh-host локальный rehearsal подтвердил три контроля: default/default
  и opt-in/default сошлись по Postcard v6, opt-in/opt-in выбрал
  `kilogram/m1/sync/cbor/2`; обе стороны сохранили один selection evidence ID,
  шесть event ID совпали, fallback=false;
- CLI regression прошёл 85/85; all-target/all-feature check, rustfmt и новый
  fail-closed production-dark-launch verifier с четырьмя negative mutations
  зелёные. Public default остаётся legacy-only; release/ZIP/tag не создавались.

### M1.1.84 — canonical sync v2 resilience rehearsal: принят

- в штатный debug binary добавлен one-shot stop-after-Diff fault для client и
  responder; release build его отвергает, permission/selection он не выдаёт;
- fresh local matrix прошла 7/7: default/default, opt-in/default,
  opt-in/opt-in, restart/resume, selected responder loss, one-step rollback и
  post-selection containment;
- restart сохранил exact selection evidence и сошёлся без duplicate/loss;
  selected loss завершил client с ненулевым кодом и zero legacy retry;
- containment сохранил ALPN и resume старого evidence, отключил publication,
  new accept/new initiate и отправил новый разговор по Postcard v6;
- первая попытка нового разговора корректно остановилась на ratchet prekey pool
  rollback. Harness выпускает свежий следующий pool; вся matrix после этого
  повторена с нуля и прошла;
- CLI unit regression вне Windows sandbox прошёл 86/86. Успешный development
  manifest находится в ignored `.tmp/m1184-resilience-20261018T152502Z`;
- первый field run выявил ошибочную общую границу: 20-секундный I/O timeout
  включал responder-side state/vault/store/import preparation. Production-path
  probe передал ответы 1, 32, 256, 1024 и 8123 bytes, исключив Iroh/QUIC и
  размер ответа; runtime получил отдельный bounded response deadline 120 s при
  сохранении 20 s для отдельных stream I/O;
- exact-revision run `m1184-exact-20261018T165652Z` на
  `7ee5ef808cb56c3c48d7a0b6dc07c0984b81992c` прошёл всю matrix между
  `PAUL_HP_OMEN` и `DESKTOP-PAUL-RY` по direct Iroh LAN. SHA-256 одного debug
  binary на обоих хостах —
  `be8ca16754c2ad1291d4ea56c0a339b6e6a0f5994e6816c1c3d966871f42c532`;
- field manifest имеет статус
  `canonical-sync-v2-two-host-activation-rehearsal-verified`: legacy controls,
  mutual `/cbor/2`, restart/exact resume, containment, selected loss и rollback
  подтвердили ожидаемые границы без потери/дублирования и без legacy fallback.

### M1.1.85 — canonical sync v2 opt-in dark-launch acceptance: принят

- добавлены aggregate manifest
  `CANONICAL-SYNC-V2-OPT-IN-DARK-LAUNCH.json`, RFC-0209 и fail-closed verifier;
- четыре canonical ledger синхронизированы на
  `canonical-sync-v2-opt-in-dark-launch-verified`,
  `changes_active_wire=true` и `public_default=legacy-only`;
- verifier pin-ит Device-signed append-only authority, default и opt-in ALPN
  lists, mutual authenticated selection, отсутствие fallback после selection,
  exact M1.1.84 two-host evidence и раздельные deadlines 120/20 секунд;
- readiness inventory расширен до 112 gates. Все 112/112 прошли вместе с
  rustfmt и locked/offline strict Clippy против принятого tag `M1.0.0`;
- полный stable Rust runner вне Windows sandbox завершился `status=passed` с
  profile `test`, двумя Cargo jobs, isolated runtime test `1/1` и всеми
  workspace harnesses кроме двух явно пропущенных неизменённых corpora;
- ранее принятые 31 851 legacy и 2 863 canonical sync v2 decoder mutations не
  повторялись: decoder/frame surface не менялась. Восьмичасовой sustained fuzz
  также не повторялся;
- первый in-sandbox regression отклонён как infrastructure-only из-за
  перенаправленного Temp с `os error 5`; идентичный unrestricted запуск прошёл;
- release build, архив, tag и автоматический GitHub build не выполнялись.
  Следующий продуктовый пакет — desktop activation UI без смены public default.

### M1.1.86 — safe desktop canonical sync v2 activation: принят локально

- Windows desktop получил advanced offline-only панель для status, exact plan
  и apply существующей Device-signed append-only activation authority;
- GUI не импортирует Device/Root secrets и не создаёт новый authority: он
  запускает существующий `kilogram-cli` под state lock с actor
  `explicit-local`;
- работающий или connected runtime блокирует церемонию. Live IPC остаётся v29,
  потому что endpoint ALPN list загружается при старте; после apply требуется
  fresh runtime;
- plan обязан сообщить `writes_state=false`, совпасть по Device/head/epoch,
  state/disposition/evidence/reason и пройти независимую сверку всех шести
  policy rights;
- apply открывается только после exact `APPLY <12 hex>`; смена draft сбрасывает
  plan, а повторный status обязан подтвердить exact applied record head;
- unit tests, strict targeted Clippy, fresh CLI status/plan/apply smoke и
  verifier с negative mutations прошли. Readiness inventory расширен до 113
  gates; public default остаётся `legacy-only`;
- release/ZIP/tag/network run и автоматический GitHub build не выполнялись.
  Следующий пакет — read-only runtime observability загруженной policy и
  negotiated sync path после restart.

### M1.1.87 — canonical sync v2 runtime observability: принят локально

- authenticated local IPC получил read-only
  `CanonicalSyncV2RuntimeStatus`; добавление Postcard variants выполнено как
  exact hard cut v29/domain v10 -> v30/domain v11 без dual reader;
- snapshot exact-разделяет verified policy, загруженную при старте endpoint,
  и текущую configured Device-signed chain head, перечитанную под state lock;
  любое расхождение явно требует fresh runtime и не применяет policy live;
- process-local ring хранит максимум 32 наблюдения фактического inbound/outbound
  пути: legacy Postcard v6 либо canonical CBOR v2, outcome и bounded public /
  authenticated identifiers. Plaintext, IP, relay URL, paths и secrets
  отсутствуют; restart очищает telemetry;
- Windows desktop получает snapshot при connect, обновляет его вручную и через
  существующую runtime change-chain, показывает loaded/configured rights,
  restart state и восемь последних observations;
- canonical IPC inventory расширен до 35 commands / 34 results / 35 pairs,
  active Postcard — до 30 commands / 29 responses, schema — до 180 rules;
- IPC regression прошёл 17/17, новый CLI bounded/restart-divergence test 1/1,
  desktop suite 26/26, 32 затронутых исторических boundaries и новый negative
  self-test прошли;
- accepted-tag readiness прошёл `114/114` против immutable `M1.0.0`, включая
  rustfmt и workspace all-target/all-feature locked/offline strict Clippy;
- public default остаётся `legacy-only`; peer/sync wire, release/ZIP/tag,
  network run и automatic GitHub build не менялись.

### M1.1.88 — guided fresh restart and exact activation verification: принят локально

- desktop-owned runtime получает явное действие stop-and-inspect: shutdown ACK
  не считается завершением, offline helper запускается только после
  фактического `Child::try_wait` exit;
- существующая offline status/plan/apply церемония сохранена, но любой apply из
  desktop теперь автоматически запускает fresh runtime и требует exact
  post-restart verification;
- expectation связывает authenticated Device ID, signed record ID, epoch,
  migration state, disposition, evidence и все шесть permission bits с equal
  loaded/configured snapshot и честными restart flags;
- verification выполняется до change subscription и conversation refresh;
  mismatch завершает новый desktop-owned child и оставляет UI disconnected;
- startup exit/timeout fail closed. Уже durable apply не откатывается, exact
  expectation сохраняется для retry без второго authority append;
- desktop suite прошёл 28/28, dedicated boundary negative mutations прошли,
  readiness расширен до 115/115 вместе с rustfmt и locked/offline strict
  Clippy;
- IPC остаётся v30/domain v11, public default `legacy-only`; network/release/
  ZIP/tag и automatic GitHub build не выполнялись.

### M1.1.89 — public rollout decision readiness: принят локально

- создан machine-readable public rollout decision с честным verdict `hold`,
  `eligible=false` и неизменным `public_default=legacy-only`;
- принятые activation/authority, opt-in production, двуххостовые, fuzz/mutation,
  observability и guided-restart evidence связаны в один decision gate;
- определены пять обязательных pending gates: privacy-preserving cohort
  authority, bounded local health evidence, N/N-1 production interop,
  operational containment drill и independent security review;
- cohort contract запрещает Account/Device/contact/conversation/IP как basis и
  требует неэкспортируемый installation-local random secret;
- зафиксированы ordered 1% advertise, 1% accept, 5% initiate, 25% expanded и
  отдельно разрешаемая broad-default фазы с soak/evidence minima;
- containment прекращает new enrollment/accept/initiate, но сохраняет reader,
  durable evidence и exact selected resume без legacy fallback;
- dedicated boundary прошёл восемь negative mutations, readiness расширен до
  116/116 вместе с rustfmt и locked/offline strict Clippy;
- peer/sync wire, runtime IPC, network, release/ZIP/tag и automatic GitHub
  build не менялись.

### M1.1.90 — dormant rollout local authority: принят локально

- добавлен отдельный `kilogram-rollout` без runtime/network dependency;
- signed release policy использует canonical bounded Postcard, Ed25519 с
  caller-pinned public key, monotonic epoch и exact predecessor ID;
- genesis обязан быть legacy-only hold, phase не пропускается и не
  откатывается, containment даёт zero enrollment, broad default отсутствует;
- installation cohort использует один private 32-byte CSPRNG seed и exact
  SHA-256 vector, не принимает Account/Device либо network/content identity;
- immediate-durability redb хранит pinned authority, seed, policy head и не
  более 16 keyed-MAC health aggregates; stale policy и corruption fail closed;
- explicit JSON export не содержит seed/bucket/timestamp/path/user identity;
  uploader, telemetry endpoint и remote attestation отсутствуют;
- cohort selection не даёт runtime authority, crate не подключён к
  executable crates, public default остаётся `legacy-only`;
- 12/12 tests, boundary self-test, readiness 117/117, rustfmt и locked/offline
  strict Clippy прошли; network/release/ZIP/tag/automatic build не запускались.

### M1.1.91 — exact N/N-1 production executable interop: принят

- exact accepted M1.0.0 executable и current revision `6d95cb7` прошли 4/4
  bidirectional direct/relay scenarios на двух физических Windows-хостах;
- каждый scenario подтвердил delivery acknowledgement, listener restart,
  legacy sync и exact history equality; canonical v2 не выбирался;
- один durable state прошёл N-1/N-1 direct, N-1/N direct и N/N relay, финально
  18 events с одинаковым inventory digest;
- ticket v11/signature projection сохранён byte-compatible, новый sync
  capability вынесен в independently signed optional extension;
- current reader принимает две формы одного v11 envelope (extension absent или
  present), но не является version-level dual reader и не принимает v13;
- intermediate response lifecycle совместим с M1.0.0, sync follow-up deadline
  отделён от generic stream open и ограничен 120 seconds;
- tracked evidence не экспортирует raw logs/state, IDs либо message bodies;
  stable executable paths предотвращают новое firewall identity на каждый SHA;
- first-contact stripping optional capability пока неотличим от genuine M1
  peer; durable selected sessions downgrade не принимают, public hold сохранён;
- dedicated negative boundary и readiness расширены до 118 gates. Public
  default остаётся `legacy-only`; release/ZIP/tag не создавались.

### M1.1.92 — monotonic peer capability floor: принят локально

- extension-free first contact теперь явно называется `legacy-unasserted`, а
  не доказательством старой версии peer;
- pure state machine различает legacy-unasserted, exact current invitation,
  canonical pin и quarantine, не выполняя I/O и не выдавая transport authority;
- exact invitation требует committed capability ID; после pin semantic floor
  хранит monotonic Account authority revision и ticket publication epoch;
- исчезновение pinned canonical capability и regression обеих high-water
  величин переводят peer в quarantine без network-driven self-recovery;
- RFC честно фиксирует неразрешимость generic first-contact stripping без
  prior authenticated state либо stronger invitation envelope;
- 10/10 targeted tests, boundary и negative self-test прошли; ticket/sync wire,
  runtime IPC и public `legacy-only` default не менялись.

### M1.1.93 — Device-signed current peer invitation: принят локально

- bounded JSON artifact с deny-unknown-fields подписывается listener Device и
  exact-связывает оба Account, listener Device, authority revision, endpoint,
  publication epoch, validity window и весь base64url connection ticket;
- embedded ticket обязан нести exact independently signed canonical capability,
  ID и protocol которого повторно проверяются против outer invitation;
- ticket substitution, capability stripping/replacement, wrong signer/version,
  invalid time и oversized/non-base64url payload fail closed;
- CLI `current-peer-invitation-create` пишет только новый файл, а
  `current-peer-invitation-inspect` проверяет outer/inner signatures и bindings,
  не раскрывая embedded ticket;
- targeted library tests 13/13, stable-path CLI negative test, boundary,
  self-test, rustfmt и strict Clippy прошли;
- import/contact enrollment/network/durable floor остаются выключены, wire и
  runtime IPC не менялись, public default остаётся `legacy-only`.

### M1.1.94 — durable monotonic peer capability store: принят локально

- новый `kilogram-peer-capability-store` хранит только bounded peer IDs,
  semantic floor, compact evidence и local Device-signed chain records;
- head+record коммитятся immediate-durability redb transaction, restart заново
  проверяет owner, signatures, IDs, peer binding, sequence и ancestry;
- exact retry idempotent, а corruption, wrong owner/requester, gap/cycle,
  retained head rollback/orphan tail и extension stripping fail closed;
- `current-peer-invitation-import` проверяет local Account certificate,
  outer/inner signatures и exact capability, пишет requirement+pin и только
  затем no-clobber экспортирует embedded ticket;
- `peer-capability-status` показывает только semantic state/IDs/high-water и не
  печатает ticket, path, endpoint address либо message data;
- store tests 6/6 и stable-path CLI restart test прошли; runtime/rollout store
  не читают, wire/IPC не менялись, public default остаётся `legacy-only`.

### M1.1.95 — dormant fail-closed rollout controller: принят локально

- pure evaluator объединяет только уже verified signed release policy,
  matching authenticated local health и текущую Device activation state;
- normal rollout предлагает не более одного следующего шага и никогда не
  предлагает `canonical-required`;
- missing/mismatched evidence, outside cohort и legacy-only policy дают hold;
- containment сохраняет текущий target и exact resume evidence, требует уже
  существующее authenticated canonical evidence и не может использоваться для
  enrollment legacy-only installation;
- proposal требует отдельного explicit Device approval; release authority,
  runtime и controller не могут подписать либо применить activation;
- runtime/desktop/CLI consumer, production key и policy delivery отсутствуют;
  wire/IPC/default не менялись, public default остаётся `legacy-only`.

### M1.1.96 — read-only rollout-controller observer: принят локально

- `rollout-controller-status` — единственный bounded offline consumer нового
  crate; runtime и desktop по-прежнему его не вызывают;
- caller явно передаёт pinned Ed25519 release public key; embedded production
  key и policy downloader отсутствуют;
- rollout store открывается redb read-only, все retained bounded health records
  MAC-проверяются, current policy/health exact-связываются;
- missing directory/database не создаётся и даёт ordinary hold; symlink,
  foreign authority, partial/tampered state fail closed;
- Device activation chain проверяется независимо; `canonical-required` вне
  bounded public controller и отвергается;
- output не содержит seed, bucket, paths, contacts, messages или network data;
  proposal не является authority и не может применить activation;
- rollout tests 20/20, missing-store stable CLI smoke, boundary/self-test,
  rustfmt и locked/offline strict Clippy прошли;
- peer/sync wire, runtime IPC и public default не менялись; default
  `legacy-only`.

### M1.1.97 — containment and corruption rehearsal: принят локально

- найден и закрыт implicit containment-clear: healthy normal policy больше не
  может предложить normal advance при текущем emergency containment;
- выход из containment требует отдельной explicit Device recovery ceremony;
- pure controller exact-связывает health с policy ID, release artifact ID,
  phase и cohort upper bound, не полагаясь только на store caller;
- exhaustive 640-case matrix проверяет все phase/disposition/selection/health/
  target/current-disposition/evidence комбинации;
- каждый proposal требует Device approval, не даёт release-policy signing или
  runtime authority; containment сохраняет target и требует evidence;
- missing current health и truncated redb добавлены к foreign-key/tamper
  fail-closed rehearsal; whole-store rollback без witness остаётся non-claim;
- 25/25 tests, boundary/self-test, rustfmt и locked/offline strict Clippy
  прошли; сеть, runtime IPC и default не менялись.

### M1.1.98 — exact-binary two-host rollout portability: принят

- evidence milestone не добавляет wire, runtime IPC либо новую authority;
- один verification-profile rollout harness и один debug CLI скопированы под
  постоянными именами на физический Windows-ноутбук и exact SHA-256 совпали;
- на основном ПК и ноутбуке прошли 56/56 rollout tests и named end-to-end
  rehearsal;
- absent-store observer и operator observer на обоих хостах вернули
  `hold-without-create`, не создав missing state;
- run `m1205-rollout-20261019T071251Z` проверил revision `2f005c1`; compact
  evidence содержит только revision, hashes, byte counts и anonymous labels;
- SSH был только control/transfer channel: peer network и listening socket не
  запускались, Firewall prompt не ожидался;
- release build, archive и tag не создавались; public default `legacy-only`.

### M1.1.99 — offline release trust root and key rotation: принят локально

- long-lived offline root отделён от operational policy-signing keys;
- canonical root-signed complete keyring содержит максимум восемь строго
  упорядоченных сертификатов и exact predecessor-bound monotonic epoch;
- successor не может удалить либо изменить старый key, задним числом добавить
  key, отменить revocation, пропустить epoch или образовать fork;
- normal и containment signing permissions разделены; containment-only key не
  даёт право расширять rollout, unknown/revoked/wrong-purpose key отвергается;
- хотя бы один активный containment signer обязателен в каждом snapshot;
- production root/private keys, filesystem store, policy delivery и runtime,
  desktop либо CLI consumers отсутствуют; full-chain rollback без external
  witness остаётся non-claim;
- rollout tests 31/31, boundary/self-test, rustfmt и locked/offline strict
  Clippy прошли; wire/IPC/default не менялись, public default `legacy-only`.

### M1.2.0 — rotation-aware trusted policy bundle: принят локально

- bounded canonical bundle self-contained хранит exact root-signed keyring,
  keyring ID, operational signer ID, narrow purpose и signed policy bytes;
- verification начинается с explicitly supplied offline trust-root public key,
  повторно вычисляет bindings и требует byte-exact canonical re-encode;
- policy chain может перейти с прежнего authority на новый operational key
  через ровно один verified keyring successor, не теряя policy epoch/previous ID;
- containment-only key может подписать emergency containment, но normal rollout
  expansion им технически не авторизуется; purpose substitution fail closed;
- foreign root, keyring/policy/signing-key substitution, revoked signer,
  keyring fork/skip и stale policy отвергаются без permissive fallback;
- durable store, inbox/download, production root и runtime/desktop/CLI consumers
  отсутствуют; rollout tests 36/36, boundary/self-test, rustfmt и locked/offline
  strict Clippy прошли, public default остаётся `legacy-only`.

### M1.2.1 — atomic trusted-policy inbox and quarantine: принят локально

- rotation-aware bundle принимается только из explicitly requested local file;
- input обязан быть real regular non-symlink file размером 1–8192 bytes и
  сохранять тип/длину на протяжении bounded read;
- root/keyring/signer/purpose/policy/canonical bytes полностью проверяются до
  любого durable write; reject не копирует invalid bytes в store;
- отдельная redb pin-ит offline trust root и в одной immediate transaction
  append-ит immutable bundle record и replacement head;
- exact replay idempotent, concurrent head mismatch fail closed;
- reopen повторно проверяет максимум 32 records как один exact policy/keyring
  chain от genesis до named tip; orphan/gap/fork/corruption отвергаются;
- full coordinated DB rollback без witness остаётся non-claim; download,
  polling, remote admin и activation consumers отсутствуют;
- rollout tests 40/40, boundary/self-test, rustfmt и locked/offline strict
  Clippy прошли; wire/IPC/default не менялись, public default `legacy-only`.

### M1.2.2 — Device-signed rollout approval bridge: принят локально

- controller proposal остаётся advisory и принимается только с exact флагами
  explicit Device approval=true и release signing authority=false;
- подготовка требует явный `UserOrTestOperatorApproved` marker;
- Device-signed approval связывает trusted bundle ID, policy ID/epoch,
  target/disposition, Device ID, exact activation predecessor ID/epoch, fresh
  nonce и полные bytes independently Device-signed activation candidate;
- release root и operational policy keys не способны подписать ни один слой;
- changed bundle/policy/proposal/head, wrong Device и tamper fail closed;
- byte-exact already-applied candidate является единственным idempotent replay;
- containment сохраняет authenticated canonical evidence и не создаётся из
  evidence-free head;
- bridge не пишет filesystem, не вызывает runtime IPC/сеть и не подключён к
  desktop/runtime persistence; public default остаётся `legacy-only`;
- rollout tests 44/44, boundary/self-test, rustfmt и locked/offline strict
  Clippy прошли.

### M1.2.3 — explicit containment recovery ceremony: принят локально

- containment нельзя снять новой policy или healthy status автоматически;
- требуется отдельный `UserOrTestOperatorApproved` и подпись владельца
  текущей Device activation chain;
- root-verified normal policy и exact matching health являются prerequisites,
  но не authority; policy target не может быть ниже contained state;
- health требует successful runtime start и canonical sync, без runtime/sync
  failures, pause/interruption, downgrade, corruption и zero-tolerance events;
- approval связывает bundle, полный fixed-schema health, Device, exact
  containment head/evidence, nonce и Device-signed candidate bytes;
- recovery меняет только disposition на normal, сохраняя target/evidence;
- canonical-required вне public rollout controller и отвергается;
- exact replay idempotent; changed policy/health/head, wrong Device и tamper
  fail closed;
- filesystem/runtime IPC/network side effects и consumers отсутствуют;
  public default остаётся `legacy-only`;
- rollout tests 48/48, boundary/self-test, rustfmt и locked/offline strict
  Clippy прошли.

### M1.2.4 — rollout operator UX without hidden authority: принят локально

- `rollout-operator-status` read-only объединяет Device activation chain,
  root-verified trusted bundle, matching rollout health и controller decision;
- missing trusted/health store не создаётся, а даёт fail-closed hold; read-only
  trusted-policy reopen проверяет exact bytes без изменения redb;
- output не содержит installation seed/cohort bucket, private keys, paths,
  contacts, messages или network metadata;
- Windows desktop показывает только компактный read-only snapshot при явных
  `--rollout-dir`, `--trusted-policy-dir`, `--release-trust-root`; без всех трёх
  view disabled и поведение остаётся legacy-only;
- import требует `--confirm-import`; approval/recovery имеют отдельные plan и
  apply, Device-signed no-clobber artifact и exact confirmation ID;
- desktop не имеет rollout toggle, approval/recovery action либо signing
  authority; auto-enrollment/network apply отсутствуют;
- trusted health теперь root-anchored и rotation-aware: import требует
  explicit `--rollout-dir` и синхронизирует полную verified bundle chain;
- rollout tests 56/56 и новые desktop parser tests 2/2 прошли; peer/sync wire,
  runtime IPC и public default не менялись.

### M1.2.5 — complete rollout rehearsal and restart repair: принят

- один deterministic test проходит пять root-verified policy bundles:
  genesis, advertise, accept, emergency containment и explicit recovery;
- operational signing key rotation проходит через offline-root keyring chain;
  trusted health проверяет уже установленный prefix и добавляет только exact
  missing successors;
- explicit Device approval выполнен для трёх activation transitions; release
  root/operational keys остаются только verification/proposal authority;
- crash после durable activation record, но до high-water, repair-ится только
  тем же exact Device-signed approval/recovery artifact после повторной
  проверки текущих policy, health, Device и predecessor;
- foreign root, stale, truncated и forked chain reject; replay idempotent;
  symlink input отвергается на поддерживающем filesystem host;
- named end-to-end rehearsal и focused CLI interrupted-apply regression
  прошли локально; весь rollout suite содержит 56 tests;
- stable full workspace regression и aggregate readiness 131/131 прошли;
- exact-binary run `m1205-rollout-20261019T071251Z` на двух физических
  Windows-хостах прошёл 56/56 tests и named rehearsal на каждом, оба no-create
  observer checks и exact SHA-256 comparison;
- bounded evidence сохранён в
  `CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-TWO-HOST-EVIDENCE.json`; raw logs,
  state, Account/Device IDs и secrets не отслеживаются;
- public default остаётся `legacy-only`, accept-canary запрещён без отдельного
  решения и независимого security review.

### M1.2.6 — peer capability transport enforcement: принят локально

- real outbound `sync` после проверки ticket/Account Root/membership и до Iroh
  endpoint bind применяет authenticated capability evidence к durable
  Device-signed peer floor;
- signed canonical candidate durable pin-ится до сети; missing extension после
  pin создаёт durable quarantine и fail closed до endpoint creation;
- неизвестный extension-free first contact остаётся
  `legacy-compatible-unasserted`, не создаёт отсутствующий capability store и
  не считается доказанно старым клиентом;
- persisted mutually signed canonical selection используется как более сильный
  resume evidence: stripped optional extension не стирает selection и не
  разрешает legacy fallback; new selection требует current signed candidate;
- добавлены 4 network-free CLI regression tests и fail-closed boundary с
  negative mutation; полный stable-path CLI regression прошёл 96/96 binary и
  1/1 library tests;
- aggregate readiness расширен и прошёл 132/132 вместе с rustfmt и
  workspace-wide locked/offline strict Clippy; wire, runtime IPC и public
  default не изменены, release/ZIP/tag не создавались.

### M1.2.7 — peer capability exact-binary two-host rehearsal: принят

- run `m1207-capability-20261019T101809Z` на revision `120fa77` использовал
  одинаковый debug `kilogram-cli.exe` SHA-256 `1b58772a...`, 61,414,912 bytes,
  на основном ПК и физическом Windows-ноутбуке через stable paths;
- extension-free first contact прошёл legacy sync 6 events с equal histories и
  без создания capability store;
- signed candidate был durable record sequence 1; stripped follow-up записал
  quarantine sequence 2 с reason `pinned-canonical-capability-missing` и не
  создал `transport_endpoint_id`;
- persisted mutually signed selection перекрыл stripped current extension,
  сохранил record sequence 1 и завершил `/cbor/2` sync без legacy fallback;
- tracked evidence не содержит raw logs/state/tickets, endpoint addresses,
  selection/Account/Device IDs или secrets; SSH был только control channel;
- dedicated fail-closed boundary и self-test добавлены, aggregate readiness
  расширен до 133/133; wire, runtime IPC и public default `legacy-only` не
  изменены, release/ZIP/tag не создавались.

### M1.2.8 — automated runtime peer capability enforcement: принят локально

- runtime outbox проверяет все authenticated endpoint candidates общим
  conversation-scoped capability authorizer после membership, но до открытия
  sealed body, ratchet/event materialization, network и mailbox fallback;
- quarantine сохраняет queue item неизменным, не создаёт retry и исключает его
  из повторного выбора до конца процесса; после restart durable floor
  проверяется снова до любых side effects;
- scheduled sync выполняет тот же preflight до вызова `sync`, прекращает
  candidate failover при capability block и process-locally пропускает этот
  contact, не блокируя выбор других due peers;
- persisted canonical selection продолжает быть resume authority; unknown
  extension-free runtime first contact остаётся legacy/no-create даже при уже
  существующей capability DB другого peer;
- bounded diagnostics различают `capability-blocked` и подтверждают
  `network_attempted=false` для preflight block без ticket, endpoint или body;
- focused stable-path tests и fail-closed boundary закрывают materialization,
  restart и multi-contact fairness; полный stable-path CLI regression прошёл
  99/99 binary и 1/1 library test, accepted-tag readiness — 134/134;
- wire, runtime IPC, automatic recovery и public default `legacy-only` не
  изменены; release/ZIP/tag и второй физический хост не использовались.

### M1.2.9 — read-only runtime quarantine observability: принят локально

- `PeerCapabilityReadStore` открывает только существующую capability DB через
  read-only redb API, полностью проверяет metadata/owner/records и не создаёт
  directory, database, table либо repair record; absent store — valid empty
  projection, wrong owner/corruption — fail closed;
- runtime IPC hard-cut обновлён до v31/domain v12 и получил bounded страницу до
  64 opaque contacts при общих limits 256 contacts/4096 queue items;
- durable quarantine, process-local scheduler suppression и ordinary network
  retry имеют разные counters/flags; restart сохраняет первый класс и не
  выдаёт ephemeral suppression за durable state;
- IPC/desktop не содержат raw Account/Device/queue IDs, ticket/endpoint/IP/
  relay URL, message/ciphertext, path либо secret/capability bytes;
- Windows advanced panel явно read-only и имеет только refresh: clear, retry,
  downgrade, re-enrollment и recovery actions отсутствуют;
- stable-path tests покрывают no-create, byte-preserving read, restart,
  multi-peer ordering/pagination/bounds/privacy и authenticated desktop IPC;
  все 157/157 тестов проходят через stable-name harness; fail-closed boundary
  отклоняет 7/7 authority/privacy mutations;
- aggregate readiness расширен до 135/135; peer/sync wire и public default
  `legacy-only` не менялись, release/ZIP/tag/network/second host не нужны.

### M1.2.10 — Device-authorized peer re-enrollment plan: принят локально

- capability store получил строго read-only extraction exact quarantined head,
  bounded reason и непосредственно предшествующего Device-signed floor; вся
  retained chain, owner/key binding, IDs и sequence проверяются до выдачи;
- новый pure offline crate `kilogram-peer-reenrollment` требует current local и
  peer Root-signed device lists, active SyncHistory Devices, fresh exact current
  invitation и non-regressing predecessor high-water;
- same-Device re-enrollment обязан реально продвинуть authority revision либо
  ticket epoch; cross-Device epoch не сравнивается как общий счётчик, но новый
  Device обязан находиться в exact peer authority с revision не ниже high-water;
- план exact-связывает старый quarantine/predecessor, обе authority digest,
  replacement Device, invitation/capability IDs, epoch, fresh 32-byte nonce и
  максимум 15 минут validity; confirmation phrase и explicit enum обязательны;
- локальный Device подписывает bounded canonical tombstone, но crate не имеет
  store writer, apply/clear API, runtime IPC, transport, CLI либо desktop;
  старый quarantine остаётся неизменным и сеть не получает recovery authority;
- stable-path focused regression прошёл 8/8 store + 5/5 re-enrollment tests;
  boundary отклонил 7/7 ослаблений, accepted-tag readiness прошёл 136/136 с
  rustfmt и workspace-wide locked/offline strict Clippy;
- wire, runtime IPC и public default `legacy-only` не менялись; release/ZIP/tag,
  automatic GitHub build, сеть и второй физический хост не использовались.

### M1.2.11 — durable peer re-enrollment acceptance: принят локально

- pure M1.2.10 planner сохранён без redb/apply authority; отдельный crate
  `kilogram-peer-reenrollment-acceptance` владеет отдельной bounded
  `peer-reenrollment-acceptance-v1.redb`;
- current local Device подписывает receipt, exact-связанный с обеими authority
  revision/digest, старым quarantine record/sequence/reason и predecessor,
  полными tombstone bytes/plan ID, replacement Device, invitation/capability
  IDs, publication epoch, successor floor и временем acceptance;
- receipt и единственный per-peer successor head записываются одной
  immediate-durability transaction; injected failure перед commit после
  restart не оставляет ни receipt, ни head;
- exact replay того же committed artifact идемпотентен даже после plan expiry,
  но changed live context, corrupt/foreign store и второй successor fail
  closed без замены первого head;
- старый capability DB открывается CLI read-only и regression подтверждает его
  byte-for-byte сохранность; quarantine не очищается и не переписывается;
- CLI получил отдельные offline no-clobber plan/approve/accept и read-only
  status commands; runtime consumer/hot apply/network/automatic retry/desktop
  one-click отсутствуют, после acceptance явно требуется fresh start;
- stable-path tests прошли 5/5 planner + 5/5 acceptance; boundary отклонил 8/8
  ослаблений, accepted-tag readiness прошёл 137/137 с rustfmt и
  workspace-wide locked/offline strict Clippy;
- wire, runtime IPC и public default `legacy-only` не менялись; release/ZIP/tag,
  automatic GitHub build, сеть и второй физический хост не использовались.

### M1.2.12 — fresh-start peer re-enrollment consumption: принят локально

- добавлен отдельный bounded `kilogram-peer-reenrollment-runtime` с
  `peer-reenrollment-runtime-v1.redb`, current-Device-signed complete transition
  и независимыми prepared/consumed tables;
- transition exact-связывает acceptance/quarantine IDs, старый и replacement
  Devices, stable contact/conversation, predecessor digest, полные successor
  contact/candidate/invitation bytes, их digests/ID и successor floor;
- explicit acceptance готовит transition, но prepared-only snapshot продолжает
  использовать старый quarantined Device; hot apply отсутствует;
- fresh runtime start сначала проверяет все prepared transitions против
  predecessor, exact accepted local/peer authority, descriptor, acceptance и
  retained quarantine, затем идемпотентно публикует consumed heads до Iroh
  endpoint;
- consumed replay допускает дальнейшее монотонное повышение обеих authority,
  но отклоняет rollback/equivocation и отзыв replacement Device в новом peer
  authority snapshot;
- consumed successor применяется как in-memory overlay; original contact file,
  acceptance, quarantine, candidate source и transition не удаляются и не
  переписываются;
- queued item остаётся sealed/unmaterialized и проходит обычную scheduler
  evaluation после startup; consumption не запускает delivery или network;
- stable-path tests прошли 5/5 runtime store, 101/101 CLI binary и 1/1 CLI
  library; boundary отклонил 10/10 ослаблений, aggregate readiness прошёл
  138/138 с rustfmt и workspace-wide locked/offline strict Clippy;
- wire, runtime IPC и public default `legacy-only` не менялись; release/ZIP/tag,
  automatic GitHub build, сеть и второй физический хост не использовались.

### M1.2.13 — two-host fresh-start peer re-enrollment rehearsal: принят

- exact debug `kilogram-cli.exe` revision `d307e96`, SHA-256 `31e3b6e...`,
  62,375,424 bytes выполнен с постоянными именами на основном ПК и физическом
  Windows-ноутбуке; SSH использовался только как control/artifact channel, а
  product path был Iroh direct LAN;
- authenticated predecessor сначала сохранил durable quarantine и блокировал
  sealed queue item до materialization, endpoint и network;
- replacement Device создан тем же Account Root, old Device передал ему шесть
  читаемых local projections через explicit offline history rewrap; прямое
  расшифрование старых recipient ciphertext новым Device не разрешалось;
- offline plan/approve/accept подготовил transition без network; подмена
  accepted descriptor старым ticket была отклонена до endpoint и не потребила
  transition;
- fresh runtime потребил exact successor до Iroh endpoint, ordinary scheduler
  доставил ранее queued message один раз по direct path и получил ACK;
- restart увидел already-consumed transition, выполнил zero outbound actions
  и не создал duplicate; финальный sync дал equal histories из 8 events и одно
  вхождение queued message;
- predecessor contact остался byte-exact, old quarantine retained; tracked
  evidence не содержит raw logs/state/tickets, addresses, IDs, body или secret;
- dedicated boundary отклоняет 6/6 negative mutations, aggregate readiness
  расширен до 139/139; wire, runtime IPC и public default `legacy-only` не
  менялись, release/ZIP/tag и automatic GitHub build не создавались.

### M1.2.14 — canonical runtime IPC peer re-enrollment contract: принят локально

- в `kilogram-runtime-ipc` добавлен отдельный dormant signed codec, который не
  является вариантом active `RuntimeIpcCommand/Response`; active IPC остаётся
  v31/domain v12 и ни runtime actor, ни desktop к ceremony ещё не подключены;
- request строго разделяет `inspect -> plan -> approve -> accept` и отдельный
  `status`; approve требует typed `REENROLL` phrase и explicit `UserConfirmed`,
  а response не может пропустить или перемотать стадию;
- UI projection ограничена opaque contact ID, verified invitation summary,
  exact artifact IDs и confirmation phrase; raw Account/Device IDs, secrets,
  ticket/endpoint, queue/message body и capability bytes отсутствуют;
- request exact-связан с authenticated runtime descriptor, а Device-signed
  response — с request digest, exact descriptor, domain-separated current
  Device binding, predecessor quarantine, successor summary и durable outcome;
- closed states различают missing invitation, unsafe/divergent, ready,
  approval-required, approved-prepared, accepted-restart-required и consumed;
  durable quarantine не подменяется process-local scheduler suppression;
- history recovery является отдельным состоянием: успешный re-enrollment не
  обещает старую читаемую историю, без approved same-account rewrap source
  возвращается `SeparateApprovalRequired`, пустой inventory не считается
  complete;
- targeted stable-name tests прошли 8/8, весь `kilogram-runtime-ipc` — 26/26;
  boundary отклонил 8/8 mutations, accepted-tag readiness прошёл 140/140 с
  rustfmt и workspace-wide locked/offline strict Clippy;
- wire/default `legacy-only` не менялись; network, ноутбук, release/ZIP/tag и
  automatic GitHub build не использовались.

### M1.2.15 — read-only runtime peer re-enrollment adapter: принят локально

- active authenticated IPC сделал exact hard cut с v31/domain v12 на
  v32/domain v13 и получил единственную bounded команду
  `PeerReenrollmentRead`; nested `inspect`/`status` разрешены, а
  `plan`/`approve`/`accept` отклоняются до обращения к store;
- runtime под state lock открывает capability quarantine, invitation staging,
  acceptance и transition stores только read-only: отсутствующий store не
  создаётся, corruption/foreign owner/unexpected entry/oversize fail closed;
- новый `kilogram-peer-reenrollment-staging` принимает приглашение только
  отдельной explicit offline командой, проверяет exact authority/quarantine и
  сохраняет current-Device-signed content-addressed record; staging не даёт
  acceptance, clear, retry, transition либо network authority;
- current Device подписывает response, exact-связанный с request и runtime
  descriptor; CLI и desktop независимо проверяют canonical bytes, signature,
  descriptor/device binding и state invariants;
- privacy-safe projection различает durable quarantine, process-local
  suppression, ready/unsafe/accepted/restart/consumed и отдельный history
  recovery outcome, но не раскрывает raw identity, ticket/endpoint, путь,
  queue/message, secret либо capability bytes;
- Windows panel только показывает состояние и обновляет его вручную; plan,
  approve, accept, clear, retry, hot apply и automatic recovery controls
  отсутствуют;
- dedicated boundary и self-test отклоняют 8/8 mutations; accepted-tag
  readiness прошёл 141/141 с rustfmt и workspace-wide locked/offline strict
  Clippy;
- stable-path regression прошёл isolated runtime 1/1, CLI binary 101/101 и
  library 1/1, staging 3/3, runtime IPC 26/26 и Windows library 30/30; один
  sandboxed desktop IPC запуск ожидаемо получил WSAEACCES 10013, идентичный
  stable EXE вне сетевой песочницы прошёл без изменения исходников;
- connection-ticket/sync wire и public default `legacy-only` не менялись;
  network, ноутбук, release/ZIP/tag и automatic GitHub build не использовались.

### M1.2.16 — fresh-start peer re-enrollment mutation adapter: принят локально

- active authenticated IPC сделал exact hard cut с v32/domain v13 на
  v33/domain v14 и получил отдельный `PeerReenrollmentMutation`; read adapter
  по-прежнему принимает только `inspect`/`status`, mutation adapter — только
  `plan`/`approve`/`accept`;
- Device secret остаётся только внутри foreground runtime. Plan exact-связывает
  contact, staged invitation, quarantine и обе authority; approve требует
  отдельный флаг и точную typed `REENROLL` phrase; accept повторно проверяет
  весь signed context;
- runtime-owned bounded redb journal хранит current-Device-signed monotonic
  фазы `planned -> approved -> acceptance-committed -> complete` и exact IDs
  plan/tombstone/receipt/transition; divergent replay fail closed;
- injected crashes после acceptance commit, journal commit и transition
  prepare остаются честно `acceptance-incomplete`; exact retry завершает ту же
  ceremony, в том числе если повторный prepare получил новое wall-clock time;
- incomplete journal не даёт restart authority и пропускается на fresh start;
  только complete journal разрешает consume до Iroh endpoint. Hot apply,
  quarantine deletion и implicit history recovery отсутствуют, predecessor и
  old readable evidence сохраняются;
- stable-path regression прошёл CLI 102/102 + library 1/1, planner 5/5,
  acceptance 5/5, runtime/journal 7/7, IPC 27/27 и Windows 30/30; dedicated
  boundary отклонил 10/10 mutations, readiness прошёл 142/142 с rustfmt и
  workspace-wide locked/offline strict Clippy;
- wire и public default `legacy-only` не менялись; product network, ноутбук,
  release/ZIP/tag и automatic GitHub build не использовались.

### M1.2.17 — guided desktop peer re-enrollment ceremony: принят локально

- Windows GUI проводит уже существующую runtime ceremony по явным стадиям
  `status -> plan -> approve -> accept`; каждая операция получает fresh request
  ID и проверяет Device-signed response относительно exact descriptor bytes;
- UI state содержит только opaque contact/artifact IDs, bounded replacement
  fingerprint, summaries и confirmation phrase. Device/Root secrets, raw
  identity, filesystem paths, transport ticket/endpoint, message/history body
  и reusable capability в него не попадают;
- exact staged invitation выбирается отдельно; approval требует точной phrase
  и отдельного checkbox, accept остаётся отдельной кнопкой. Changed descriptor,
  signed status, plan либо contact сбрасывает consent;
- `AcceptanceIncomplete` честно остаётся незавершённым и не разрешает restart.
  После complete acceptance desktop может штатно остановить только запущенный
  им runtime, создать fresh process и проверить exact receipt/transition в
  состоянии `Consumed`; hot apply отсутствует;
- mismatch, ошибка signed response или невозможность поставить fresh
  verification в очередь останавливает новый child process. Для точного
  retained expectation доступен только явный retry, без hidden auto-retry;
- пять pure state tests и authenticated loopback всей последовательности вошли
  в полный stable-path Windows regression 36/36. Новый boundary отклонил 10/10
  ослаблений, прежние read/mutation boundaries сохранились;
- aggregate accepted-tag readiness прошёл 143/143 с rustfmt и workspace-wide
  locked/offline strict Clippy. IPC остаётся v33/domain v14, wire и public
  default `legacy-only` не менялись; second host, product network,
  release/ZIP/tag и automatic GitHub build не использовались.

### M1.2.18 — explicit old-history recovery plan: принят локально

- успешный peer re-enrollment больше нельзя трактовать как неявное разрешение
  на чтение старой истории: planner требует отдельное owner-approved consent;
- каждый потенциальный источник публикует bounded Device-signed claim без
  message bodies. Inventory показывает opaque conversation coverage, ranges и
  existing signed checkpoints; несовпадающие claims видимы как `Divergent` и
  не позволяют построить recovery plan;
- plan content-addressed и exact-связан с consumed fresh-start transition,
  acceptance receipt, current Account authority, replacement Device,
  membership, одним source claim, выбранными ranges и checkpoints;
- подтверждение `HistoryOwnerApproved` требует отдельной точной фразы
  `RECOVER HISTORY`, после чего plan подписывает именно source Device. Подпись
  replacement Device, прежний re-enrollment approval или receipt не дают это
  полномочие;
- drift authority, recipient, membership, source, range, checkpoint либо
  consumed transition инвалидирует plan; tamper и expiry fail closed;
- pure stable-path regression прошёл 6/6, dedicated boundary отклонил 10/10
  negative mutations, accepted-tag readiness прошёл 144/144 с rustfmt и
  workspace-wide locked/offline strict Clippy. Planner не имеет store,
  transfer/rewrap, network, desktop или active IPC authority; wire/default
  `legacy-only` не менялись.

### M1.2.19 — durable old-history recovery consent: принят локально

- отдельная `old-history-recovery-consent-v1.redb` exact-привязана к Account и
  source Device и не смешивается с acceptance/runtime/capability stores;
- первая durable запись повторно проверяет signed plan, consumed transition,
  current authority, memberships, body-free inventory, ranges, checkpoints и
  expiry; immutable signed record и transition head записываются одной
  immediate-durability транзакцией;
- exact replay уже committed plan идемпотентен даже после его expiry, но
  competing plan для того же transition, foreign owner, fork/orphan,
  corruption и incomplete metadata отклоняются без восстановления данных;
- read-only open existing не создаёт store и показывает только artifact IDs,
  scoped opaque Device tokens, bounded count/time и expired flag. Message
  bodies, raw identity, paths, endpoints и execution authority отсутствуют;
- stable-path regression прошёл 6/6, dedicated boundary отклонил 10/10
  mutations, accepted-tag readiness прошёл 145/145 с rustfmt и
  workspace-wide locked/offline strict Clippy;
- whole-DB rollback без external monotonic witness остаётся честным non-claim.
  Transfer/rewrap/checkpoint advance, network, IPC и desktop не добавлялись;
  wire/default `legacy-only` не менялись.

### M1.2.20 — bounded single-source old-history execution: принят локально

- execution начинается только из exact durable M1.2.19 consent; перед prepare
  и commit повторно проверяются consent record/head, source signature и полный
  committed live context без повторного превращения истёкшего confirmation
  window в запрет crash/restart resume;
- source использует существующий authenticated `HistoryRewrapBundle` и выдаёт
  максимум 32 events за chunk. Replacement Device отдельно расшифровывает их,
  создаёт encrypted history-rewrap projections и подписывает checkpoint
  transition;
- отдельная owner Account/source Device-bound
  `old-history-recovery-execution-v1.redb` одной `Immediate` транзакцией
  append-ит source-signed record, bundle, before/after checkpoints, encrypted
  projections и единственный consent-selection head;
- exact replay committed checkpoint возвращает те же durable artifacts без
  повторного advance; differently randomized bundle либо другой checkpoint
  для уже consumed input отклоняется. Pre-commit crash оставляет пустое
  состояние, fork/orphan/corruption/context drift fail closed;
- read-only status no-create показывает лишь opaque selection token и bounded
  progress; bodies/plaintext/raw identity/path/network coordinates отсутствуют;
- affected stable-path regression прошёл planner 6/6, consent 6/6, execution
  6/6 и protocol 156 passed / 15 explicit generators ignored; decoder mutation
  corpus прошёл 31851/31851 за 2575.21 s, sync-v2 mutation — 1/1 за 23.87 s.
  Dedicated boundary отклонил 10/10 mutations, accepted-tag readiness прошёл
  146/146 с rustfmt и workspace-wide locked/offline strict Clippy;
- production history stores пока не изменяются. Network transfer,
  multi-source scheduler, active IPC, desktop controls, wire/default change,
  release/ZIP/tag/automatic GitHub build и second host отсутствуют.

### M1.2.21 — atomic recipient history-store import: принят локально

- новый import слой принимает только exact committed M1.2.20 chunk, повторно
  проверяет execution artifacts, membership, Account/Device и требует current
  execution head до первого импорта; stale unimported record отклоняется;
- authorized events, encrypted local projections, signed before/after
  checkpoints и current-recipient-Device-signed receipt публикуются одной
  `StateTransaction` под state-directory lock. При наличии encrypted vault
  применяется существующий DB-primary dual-write protocol;
- receipt exact-связывает execution record/generation, bundle, checkpoints,
  ranges, ordered event IDs, projection digests и previous receipt. Цепь
  линейна и ограничена 16,384 chunks, один chunk — 32 events;
- exact imported replay не мутирует store и повторно проверяет все durable
  artifacts. Injected failure после каждой стадии откатывает страницу; crash
  оставляет active marker, status fail closed, а следующий mutating retry
  выполняет recovery до повторного импорта;
- короткие content-addressed checkpoint filenames устранили реальную Windows
  path-length ошибку без ослабления signed checkpoint content;
- stable-path combined execution/import regression прошёл 10/10; dedicated
  boundary отклонил 10/10 mutations, accepted-tag readiness прошёл 147/147 с
  rustfmt и workspace-wide locked/offline strict Clippy;
- plaintext не сохраняется. Network transfer, multi-source scheduler, active
  IPC, desktop controls, second host, release/ZIP/tag и automatic GitHub build
  не использовались; public default остаётся `legacy-only`.

### M1.2.22 — authenticated history-recovery transfer inbox: принят локально

- source экспортирует только exact current committed execution head в
  canonical postcard envelope до 9 MiB; отдельная source Device signature
  связывает все внутренние artifacts и полный Account/source/recipient,
  consent-selection, generation/predecessor, conversation/range context;
- recipient сначала повторно проверяет source-signed execution record,
  authenticated rewrap bundle, оба signed checkpoints и encrypted projections,
  затем append-ит exact bytes под state-directory lock и `StateTransaction`;
- inbox требует generation zero и линейное продолжение без gaps/forks/stale
  alternatives, но может заранее принять bounded backlog. Exact byte replay
  идемпотентен; foreign recipient, tamper, corruption, symlink и unexpected
  entries отклоняются;
- import bridge требует тот же canonical state directory и exact staged
  transfer. Backlog импортируется строго по recipient-Device-signed receipt
  chain; попытка начать со второго чанка получает `MissingPreviousReceipt`;
- stable-path transfer regression прошёл 4/4, полный execution/import/transfer
  harness — 14/14; dedicated boundary отклонил 10/10 mutations, accepted-tag
  readiness прошёл 148/148 с rustfmt и workspace-wide locked/offline strict
  Clippy;
- envelope содержит ciphertext, а не plaintext. Network session, retry,
  multi-source scheduler, active IPC, desktop controls, second host,
  release/ZIP/tag и automatic GitHub build не использовались; public default
  остаётся `legacy-only`.

### M1.2.23 — authenticated Iroh history-transfer session: принят локально

- отдельный `kilogram-peer-history-recovery-transport` crate владеет dedicated
  ALPN `kilogram/m1/history-recovery/postcard/1`; active runtime, CLI и desktop
  crate его не подключают и не рекламируют;
- canonical source-Device-signed request exact-связывает authenticated source
  и recipient Endpoint IDs, nonzero session nonce, transfer ID и полный
  M1.2.22 envelope. Bounded framing и per-operation timeout fail closed;
- recipient сверяет Iroh peer identity до мутации, повторно проверяет envelope,
  durable-stage-ит exact bytes и только после этого подписывает response с
  request/transfer/Endpoint/Device/disposition binding;
- exact retry с тем же request идемпотентен: первая попытка возвращает `Staged`,
  повторная — `AlreadyStaged`. Endpoint substitution отклоняется до появления
  inbox state;
- stable-path focused regression прошёл 4/4, pre-existing Iroh loopback control
  1/1; dedicated boundary отклонил 10/10 mutations, accepted-tag readiness
  прошёл 149/149 с rustfmt и workspace-wide locked/offline strict Clippy;
- использовался только local `127.0.0.1` loopback. Product Internet/relay,
  second host, durable source outbox/resume, scheduler/multi-source, IPC/GUI,
  release/ZIP/tag и automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.24 — durable history-transfer outbox: принят локально

- `PreparedHistoryRecoveryTransferSession` отделяет preparation от send и даёт
  сохранять exact canonical source-signed request bytes до сети. Request
  повторно связывает Account/source/recipient, Endpoint IDs, nonce и transfer;
- dedicated redb outbox атомарно записывает request и transfer head с
  `Durability::Immediate`, ограничен 16,384 intents и при каждом open проверяет
  owner metadata, signatures, heads, orphan requests/receipts и ID bindings;
- один transfer ID допускает ровно один request. Exact prepare replay
  идемпотентен, а новый nonce или Endpoint для того же transfer отклоняется;
- exact recipient-signed response сохраняется отдельной immediate transaction.
  Exact replay идемпотентен; отличный receipt после commit отклоняется;
- injected failure до request/head commit оставляет `Absent`, до receipt commit
  оставляет `Pending`. Read-only open-existing не создаёт store, status body-free
  и использует opaque recipient token;
- stable-path focused regression прошёл 4/4, полный transport/outbox crate —
  8/8; boundary отклонил 10/10 mutations, accepted-tag readiness прошёл 150/150
  с rustfmt и workspace-wide locked/offline strict Clippy;
- executor/background retry/multi-source/runtime IPC/desktop, product network,
  second host, release/ZIP/tag и automatic GitHub build отсутствуют; whole-DB
  rollback без external witness не заявлен, default `legacy-only`.

### M1.2.25 — explicit restart-safe history-transfer executor: принят локально

- явно вызываемый executor принимает только уже durable pending transfer ID,
  загружает exact signed request и до сети сверяет source Endpoint с локальным
  endpoint, а recipient Endpoint — с caller-supplied target;
- каждая попытка предлагает только `kilogram/m1/history-recovery/postcard/1`
  и отправляет clone тех же canonical bytes. Новый nonce/request подготовить
  через executor невозможно;
- policy ограничена 1..=8 попытками (default 3), connect timeout обязан быть
  ненулевым. Отказ до authenticated connection — `pre-connect`, любой отказ
  после него консервативно `post-connect-ambiguous`;
- exhaustion не мутирует pending intent; отдельный последующий вызов/restart
  повторяет exact request. Verified recipient receipt сохраняется durable до
  success, acknowledged transfer возвращается без connect/stream;
- stable-path executor regression прошёл 6/6, полный crate — 14/14; реальный
  сетевой тест ограничен `127.0.0.1`. Boundary отклонил 10/10 mutations,
  accepted-tag readiness прошёл 151/151 с rustfmt и strict offline Clippy;
- background worker/schedule, recipient accept service, multi-source,
  active runtime/IPC/desktop/import authority, product network, second host,
  release/ZIP/tag и automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.26 — explicit bounded recipient acceptor: принят локально

- явно awaited acceptor заимствует caller-owned endpoint и завершается после
  session budget, idle deadline либо закрытия endpoint; default budget — 1,
  hard max — 8, одновременно обрабатывается ровно одна session;
- разрешён только dedicated history-recovery ALPN. Handshake/request ограничены
  отдельным request deadline, durable stage/response — completion deadline;
  все deadline ненулевые и не больше 300 секунд;
- malformed, wrong-ALPN, rejected и timeout sessions расходуют slot, явно
  закрываются и не лишают оставшийся budget возможности принять следующую;
- результат содержит только bounded counts, closed failure/termination enums и
  константные privacy/authority flags — без Device/Endpoint ID, network
  coordinates, paths либо message bodies;
- stable-path acceptor regression прошёл 5/5, полный transport/outbox/executor/
  acceptor crate — 19/19. Реальный network control — одна authenticated
  `127.0.0.1` session; multi-session state-machine проверен детерминированно;
- boundary отклонил 10/10 mutations, accepted-tag readiness прошёл 152/152 с
  rustfmt и strict offline Clippy;
- spawned/background listener, persistent schedule, active runtime/IPC/desktop,
  multi-source и import authority, product Internet/relay, second host,
  release/ZIP/tag и automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.27 — bounded multi-source recovery scheduler: принят локально

- scheduler принимает 1..=8 caller-supplied уже durable intents; до первой
  сети загружает и проверяет каждый exact request, source/recipient Endpoint,
  единые Account/recipient и уникальные source Device/transfer ID;
- default policy — 3 rounds и 24 total invocations, hard max — 8 и 64.
  В каждом round каждый pending source получает не более одной попытки в
  caller input order, одновременно выполняется ровно одна;
- новый round начинается только при достаточном budget для каждого pending
  source на его старте, поэтому partial round не благоприятствует ранним input;
- каждая попытка делегируется M1.2.25 executor с max_attempts=1. Retryable
  pre-connect/post-connect-ambiguous failure уступает следующему source и не
  меняет exact durable request;
- already acknowledged source не открывает сеть. Round/total exhaustion
  оставляет unfinished intents pending; неожиданная local/outbox ошибка
  fail-closed завершает вызов, а предыдущие receipts остаются durable;
- privacy-safe report содержит только source ordinal, bounded counts, closed
  outcomes/failure classes и не содержит identity/transfer/network/body/path;
- stable-path focused regression прошёл 7/7, полный transport/outbox/executor/
  acceptor/scheduler crate — 26/26. Boundary отклонил 10/10 mutations,
  accepted-tag readiness прошёл 153/153 с rustfmt и strict offline Clippy;
- scheduler не discover-ит источники и не координирует удалённые процессы;
  background/runtime/IPC/desktop/import authority, новый network wire, product
  Internet/relay, second host, release/ZIP/tag и automatic GitHub build
  отсутствуют; default `legacy-only`.

### M1.2.28 — durable recipient recovery coordinator: принят локально

- 1..=8 inputs выводятся только из durable owner-approved consent selection;
  все принадлежат одному Account/recipient, source Devices distinct;
- read-only `plan` проверяет existing linear transfer inbox и recipient-signed
  import receipt chains, но не создаёт coordinator DB, не открывает сеть и не
  импортирует историю;
- explicit `apply` берёт общий state-directory lock, повторно наблюдает exact
  heads и одной immediate redb transaction пишет canonical append-only plan,
  все selected source heads и единый monotonic chain head;
- durable DB хранит только opaque content-addressed IDs и monotonic counts/
  ranges. Device IDs, ciphertext artifacts, message bodies и plaintext туда не
  копируются; whole-DB rollback без external witness не заявлен;
- stale observation, equal-generation head fork, skipped/forked plan sequence,
  noncanonical/corrupt record, progress regression и возврат complete source в
  pending fail closed. Exact replay идемпотентен, precommit failure не меняет
  plan progress;
- focused stable-path regression прошёл 4/4, полный execution crate — 18/18;
  boundary отклонил 10/10 mutations, accepted-tag readiness прошёл 154/154 с
  rustfmt и workspace-wide locked/offline strict Clippy;
- coordinator не вызывает transfer/import operations и не подключён к active
  runtime/CLI/desktop. Product network/relay, second host, release/ZIP/tag и
  automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.29 — explicit recipient recovery import plan/apply: принят локально

- read-only planner принимает explicit source ordinal только из exact
  durably-applied M1.2.28 coordinator plan и не создаёт state;
- для backlog выбирается `transfer ordinal == imported chunk count`, а не
  newest staged transfer. Так generation нельзя пропустить;
- plan связывает coordinator/source, selected transfer/execution record,
  current inbox head, expected import receipt head и digest/revision verified
  membership; changed coordinator/head/membership fail closed;
- explicit apply импортирует ровно один transfer существующим M1.2.21 atomic
  pipeline. Coordinator и inbox повторно проверяются под тем же state lock,
  затем после durable receipt строится и применяется fresh coordinator plan;
- окно после import commit и до coordinator refresh честно возобновляется:
  тот же plan проверяет exact imported artifacts, получает `AlreadyImported`
  и завершает refresh без дубликата;
- focused stable-path regression прошёл 4/4, полный execution crate — 22/22;
  boundary отклонил 10/10 mutations, accepted-tag readiness прошёл 155/155 с
  rustfmt и workspace-wide locked/offline strict Clippy;
- batch/background/network/runtime/IPC/desktop authority, product network,
  second host, release/ZIP/tag и automatic GitHub build отсутствуют; default
  `legacy-only`.

### M1.2.30 — bounded explicit multi-source recipient import round: принят локально

- round принимает 1..=4 sources по умолчанию и максимум 8; event budget —
  128 по умолчанию и 256 hard max. Порядок задаёт caller, duplicates forbidden;
- до первого import проверяются exact applied coordinator, каждый ordinal,
  membership digest/revision и весь M1.2.29 candidate. Invalid late input не
  оставляет импорт раннего source;
- каждый source получает максимум один transfer за round, даже если backlog
  длиннее. Импорты идут последовательно, после каждого используется fresh
  coordinator и exact candidate revalidation;
- replay принимает только exact caller-order durable prefix, повторно проверяет
  recipient-signed receipts и все imported artifacts. Import-before-refresh
  сначала reconciles coordinator; non-prefix, unplanned или >1 generation
  progress fail closed;
- focused stable-path regression прошёл 5/5, включая два реальных source
  Devices в reverse order `[1, 0]`; полный execution crate — 27/27. Boundary
  отклонил 10/10 mutations, readiness прошёл 156/156 с rustfmt и workspace-wide
  locked/offline strict Clippy;
- whole-round atomicity не заявлена: concurrent mutation после первого import
  может оставить проверенный prefix для resume. Discovery/background/network/
  runtime/IPC/desktop authority, product network, second host, release/ZIP/tag
  и automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.31 — durable explicit recipient import-round intent: принят локально

- read-only plan принимает только exact unstarted M1.2.30 round над durable
  coordinator и не создаёт state. Recipient Device подписывает owner/round,
  coordinator baseline, ordered ordinals, все import plan/candidate IDs,
  budgets и membership digest/revision;
- explicit commit пишет canonical Postcard record в отдельную immediate redb
  под общим state lock. Store bounded до 4096 records / 64 KiB record и ровно
  одного active unconsumed intent; consumed audit history сохраняется;
- durable state не содержит membership snapshot, bodies, plaintext или
  ciphertext artifacts. Timestamp не влияет на semantic exact-replay, но все
  authority-bearing поля content-addressed и подписаны;
- read-only progress допускает только exact caller-order prefix, проверяет
  receipts/imported artifacts, coordinator chain baseline и неизменность всех
  unplanned sources. Состояния закрыты: `not-started`, `partial`, `complete`;
- import-before-coordinator-refresh виден как complete с обязательным refresh,
  но consume до него fail closed. Финальный consumption отдельно подписан
  recipient Device и exact-связан с final coordinator;
- focused stable-path regression прошёл 4/4, включая два реальных sources,
  restart partial prefix, crash-before-consume и pre-refresh block; полный
  execution crate — 31/31. Boundary отклонил 10/10 mutations, readiness прошёл
  157/157 с rustfmt и workspace-wide locked/offline strict Clippy;
- intent не исполняет import и не добавляет discovery/background/network/
  runtime/IPC/desktop authority. Product network, second host, release/ZIP/tag
  и automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.32 — explicit durable recipient import-round executor: принят локально

- read-only execution plan открывает ровно один active unconsumed M1.2.31
  intent, exact-сопоставляет caller-supplied live memberships с signed digest,
  revision/order и не создаёт state;
- `apply_next` выполняет максимум один source, `apply_bounded` — default 1 и
  hard max 8. Каждый импорт делегируется существующему M1.2.29 pipeline,
  выполняется последовательно и требует полного совпадения fresh candidate со
  signed intent binding;
- после каждого source active intent заново открывается, current coordinator и
  exact ordered prefix перепроверяются. Progress обязан увеличиться ровно на
  один; stale/fork/changed membership/candidate/unplanned source fail closed;
- crash после import до refresh сначала exact-reconciles coordinator. Complete
  replay возвращает zero-step receipt, а intent не consume-ится автоматически;
- focused stable-path regression прошёл 3/3, включая два реальных source
  Devices в reverse order и crash-window resume; полный execution crate —
  34/34. Boundary отклонил 10/10 mutations, readiness прошёл 158/158 с
  rustfmt и workspace-wide locked/offline strict Clippy;
- новый durable store, discovery/background/network/runtime/IPC/CLI/desktop
  authority, product network, second host, release/ZIP/tag и automatic GitHub
  build отсутствуют; default `legacy-only`.

### M1.2.33 — dormant cross-process recipient import execution contract: принят локально

- отдельный canonical Postcard contract ограничен 8 KiB request, 16 KiB signed
  response и максимум 8 membership references; active IPC остаётся v33/domain
  v14 и не получает нового command variant;
- стадии разделены на read-only `plan`/`status`, explicit `confirm` и mutation
  `apply-next`/`apply-bounded`. Exact confirmation phrase, confirmation ID,
  confirmation nonce и отдельный mutation nonce обязательны;
- каждая стадия exact-связана с request/descriptor/current Device, intent,
  execution plan, coordinator ID+sequence и digest caller-ordered references.
  Response подписывает current descriptor Device;
- IPC не переносит membership snapshot. Opaque authority/context references
  обязаны заново reload-иться будущим adapter из caller-authorized live context;
  reorder/duplicate/substitution/partial decode и stage skip fail closed;
- focused stable-path regression прошёл 8/8, полный runtime IPC crate — 35/35,
  boundary отклонил 10/10 mutations, readiness прошёл 159/159 с rustfmt и
  workspace-wide locked/offline strict Clippy;
- active adapter/server dispatch, CLI/desktop, import/consume authority,
  product network, second host, release/ZIP/tag и automatic GitHub build
  отсутствуют; default `legacy-only`.

### M1.2.34 — dormant recipient import read adapter: принят локально

- dependency bridge вынесен в отдельный
  `kilogram-peer-history-recovery-runtime`; offline execution crate не получил
  production dependency на runtime IPC, а exact adapter source проверяется его
  real signed fixtures только под `cfg(test)`;
- transport-independent adapter принимает только M1.2.33 `Plan` и `Status` и
  не зарегистрирован в active `RuntimeIpcCommand`/server/CLI/desktop;
- `Plan` exact-сверяет active unconsumed intent и coordinator, затем через
  caller-owned callback reload-ит каждый authority/context reference,
  проверяет membership digest и строит transient M1.2.32 source list;
- успешный M1.2.32 plan возвращается только как ephemeral in-process context.
  Membership snapshots не сохраняются и не попадают в privacy-safe outcome;
- unplanned status разрешён только при original coordinator. Known-plan status
  reload-ит те же references, replans с тем же budget и требует exact plan ID;
  foreign/stale plan блокируется до loader;
- missing/ambiguous/substituted membership и stale coordinator fail closed.
  `Confirm`, `ApplyNext`, `ApplyBounded` всегда отвергаются; intent/import/
  confirmation/consume state не создаётся;
- focused stable-path regression прошёл 8/8, полный execution crate — 42/42,
  boundary отклонил 10/10 mutations, readiness прошёл 160/160 с rustfmt и
  workspace-wide locked/offline strict Clippy;
- product network, second host, release/ZIP/tag и automatic GitHub build не
  использовались; default `legacy-only`.

### M1.2.35 — durable recipient import confirmation ledger: принят локально

- отдельный ledger в dormant runtime-integration crate принимает только exact
  M1.2.34 `Planned` dispatch и validated M1.2.33 `Confirm` request;
- intent/execution/coordinator IDs, exact ordered membership references и
  membership-set digest должны совпасть. Current owner Device подписывает
  candidate с caller nonce, sequence, previous ID и expiry максимум 900 секунд;
- planning читает ledger без создания каталога/БД. Explicit commit под общим
  state lock повторно открывает exact head и атомарно пишет record, nonce index
  и single head с immediate durability;
- ledger ограничен 4096 records. Exact replay возвращает прежний record;
  reused nonce, stale concurrent plan, expiry, foreign Device, orphan/fork/
  corrupt chain fail closed. Whole-database rollback без external witness не
  заявлен;
- записи не содержат membership snapshot, bodies/plaintext, ciphertext history
  artifacts, filesystem paths или transport authority. Confirmation не
  вызывает import и не consume-ит intent;
- focused stable-path regression прошёл 8/8, boundary отклонил 10/10
  mutations, readiness прошёл 161/161 с rustfmt и workspace-wide
  locked/offline strict Clippy;
- active runtime/server/CLI/desktop/background/network, product network,
  second host, release/ZIP/tag и automatic GitHub build отсутствуют; default
  `legacy-only`.

### M1.2.36 — explicit bounded recipient import mutation adapter: принят локально

- dormant integration adapter исполняет только `ApplyNext`/`ApplyBounded` при
  exact M1.2.35 confirmation, current owner Device, live membership reload и
  fresh exact M1.2.32 execution plan;
- отдельный immediate-durability ledger пишет Device-signed `Prepared` до
  import и `Completed` receipt до ответа, ограничен 4096 mutations/8192 records
  и проверяет exact nonce index и единую content-addressed chain;
- exact completed replay не вызывает loader/import. Rebound nonce, parallel
  mutation, stale authority, fork/orphan/corrupt state fail closed;
- prepared retry принимает только monotonic progress внутри исходного budget,
  reconciles import-before-coordinator-refresh без duplicate и не начинает
  новый import после confirmation expiry;
- stable-path ledger regression прошёл 3/3, real signed mutation fixture 1/1,
  crash-reconciliation fixture 1/1; boundary отклонил 10/10 mutations,
  readiness прошёл 162/162 с rustfmt и workspace locked/offline strict Clippy;
- automatic intent consume, active runtime/server/CLI/desktop/background/
  network, product network, second host, release/ZIP/tag и automatic GitHub
  build отсутствуют; default `legacy-only`.

### M1.2.37 — explicit recipient import-consumption ceremony contract: принят локально

- отдельный dormant contract содержит только read-only `Plan` и explicit
  `Commit`; active `RuntimeIpcCommand` и IPC v33/domain v14 не изменены;
- evidence допустим лишь из complete M1.2.36 receipt и связывает exact
  completed record ID/receipt digest, все upstream IDs, ordered membership-set
  digest, final coordinator, closed source counts и imported event count;
- оба stage повторяют ordered opaque membership refs и fail closed при digest
  substitution; membership snapshot через contract не переносится;
- content-addressed plan требует fresh nonce, TTL максимум 300 секунд и
  derived `CONSUME` phrase; commit требует explicit `UserConfirmed`;
- current Device подписывает exact request/descriptor-bound response. Plan не
  может claim consume, commit receipt обязан очистить active intent внутри TTL,
  `AlreadyConsumed` допускается только для exact binding;
- focused stable-path regression прошёл 8/8, полный runtime IPC — 43/43,
  boundary отклонил 10/10 mutations, readiness прошёл 163/163 с rustfmt и
  workspace locked/offline strict Clippy;
- adapter, durable consumption-plan ledger, active runtime/CLI/desktop/
  background/network, product network, second host, release/ZIP/tag и automatic
  GitHub build отсутствуют; default `legacy-only`.

### M1.2.38 — recipient import-consumption read adapter: принят локально

- dormant integration adapter обрабатывает только `Plan`; `Commit` отвергается
  до чтения mutation ledger, intent store либо вызова membership loader;
- exact mutation nonce read-only перечитывает durable record без создания
  missing DB и различает `Absent`, `Prepared`, `Completed`; только completed
  проецирует signed record ID, membership-set digest и receipt;
- canonical completed evidence целиком реконструируется и exact-сравнивается с
  request до проверки current coordinator и active unconsumed intent;
- caller-owned loader заново загружает ordered live memberships, после чего
  progress обязан быть `Complete`, без coordinator refresh, с exact counts,
  imported event count и final coordinator;
- success возвращает только ephemeral content-addressed plan; state не пишется,
  intent не consume-ится, network/background не открываются;
- stable-path real signed fixtures прошли 4/4, mutation-ledger projection 1/1,
  boundary отклонил 10/10 mutations, readiness прошёл 164/164 с rustfmt и
  workspace locked/offline strict Clippy;
- active runtime/server/CLI/desktop, product network, second host,
  release/ZIP/tag и automatic GitHub build отсутствуют; default `legacy-only`.

### M1.2.39 — durable recipient import-consumption authorization: принят локально

- отдельный dormant ledger принимает только exact M1.2.38 ephemeral plan и
  validated M1.2.37 `Commit` с `UserConfirmed` и derived `CONSUME` phrase;
- original plan TTL максимум 300 секунд переносится без продления. Planning
  read-only/no-create; current owner Device подписывает exact completed
  evidence, plan, membership-set digest, nonce, sequence, previous ID и expiry;
- explicit commit под state lock атомарно пишет bounded 4096-record redb с
  immediate durability, exact nonce index и одной content-addressed chain;
- exact replay idempotent; rebound nonce, stale concurrent head, expiry,
  foreign owner, pre-commit crash, fork/orphan/corrupt state fail closed;
- ledger хранит no membership snapshot/message/history artifact/path/token и
  только разрешает будущую exact consumption mutation, но сам intent не
  consume-ит;
- stable-path focused tests прошли 7/7, boundary отклонил 10/10 mutations,
  readiness прошёл 165/165 с rustfmt и locked/offline strict Clippy;
- active runtime/server/CLI/desktop/background/network, product network,
  second host, release/ZIP/tag и automatic GitHub build отсутствуют; default
  `legacy-only`.

### M1.2.40 — crash-safe recipient import-consumption mutation: принят локально

- dormant adapter принимает только exact M1.2.37 `Commit` с durable M1.2.39
  authorization текущего owner Device и заново проверяет completed mutation,
  coordinator, caller-owned membership и active complete intent;
- отдельная bounded redb хранит immutable Device-signed `Prepared` до consume
  и `Completed` receipt до success. Разрешена одна pending ceremony, nonce
  rebound и broken prepared/completed linkage fail closed;
- intent-store атомарно записывает signed consumption и снимает лишь exact
  active pointer. Intent, membership и импортированная история сохраняются;
- crash после consume до completed receipt восстанавливается по exact signed
  consumption timestamp/coordinator/counts без второго consume; completed
  replay не вызывает membership loader и не мутирует state;
- stable-path ledger tests прошли 2/2, real signed fixtures 3/3, boundary
  отклонил 10/10 mutations, readiness прошёл 166/166 с rustfmt и
  locked/offline strict Clippy;
- active runtime/server/response dispatcher/CLI/desktop/background/network,
  product network, second host, release/ZIP/tag и automatic GitHub build
  отсутствуют; default `legacy-only`.
