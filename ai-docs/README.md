# Память проекта Kilogram

Актуально на: 2026-09-05.

Эта папка — краткая проектная память и дорожная карта. Подробная техническая
спецификация находится в [`docs/RFC-0001-core-architecture.md`](../docs/RFC-0001-core-architecture.md).

## Текущее состояние

Архитектура остаётся в стадии проектирования. Rust workspace теперь содержит
`kilogram-identity`, `kilogram-protocol`, `kilogram-ratchet`, `kilogram-state`,
`kilogram-store`, `kilogram-runtime-ipc`, `kilogram-bootstrap-contract`,
`kilogram-session`, `kilogram-ticket-publication`, `kilogram-transport-iroh`,
`kilogram-cli`, отдельный `kilogram-bootstrap` и первый GUI package
`kilogram-windows`.
Два процесса обмениваются подписанными событиями через Iroh/QUIC, проверяют
Ed25519-подписи и causal acknowledgement. Прикладная device identity, author
sequence и signed events сохраняются после перезапуска отдельно от эфемерной
transport identity. Bounded signed inventory/diff восстанавливает пропущенные
events в обе стороны. M0.1.4 автоматически продолжает bounded sync rounds в
одном Iroh connection; state machine отделена от transport framing. M0.1.5
показывает фактически выбранный direct/relay path, remote address и RTT. M0.1.6
исправляет Windows event-store paths длиннее 260 символов. Локальные
M0.1–M0.1.6 smoke tests пройдены. M0.2 полностью пройден на двух физических
Windows-PC: delivery и recovery sync использовали direct LAN, пустая история с
прежним device key восстановила 2 подписанных events и правильный frontier.
Подготовка M0.3 добавила подписанную route policy `auto` / `direct-only` /
`relay-only`, ограниченные connection/route/stream/wire timeouts и явную
диагностику выбранного пути. Локальный process smoke подтвердил delivery и
reconnect/sync после перезапуска listener в обоих принудительных режимах;
`relay-only` использовал публичный n0 relay и ticket без IP-адресов. Внешний
direct-only тест между домашней сетью и настоящим cellular hotspot корректно
остался на relay, подтвердив необходимость fallback для жёсткой NAT topology.
Первый внешний relay-only control дал connection timeout до `peer_id`; новая
диагностика подтвердила online relay и соответствие target Endpoint ID текущему
listener. Контрольный `auto` успешно выполнил delivery через relay (`aps1`) при
ticket Bob с `euc1`, локализовав проблему в strict relay-only cross-relay
negotiation. Однако retest с одинаковым pinned `euc1` снова дал timeout.
Listener теперь поддерживает явный `--relay-url`; следующий контроль использует
доказанно рабочий в `auto` relay `aps1`. Внешний test5 через него успешно
выполнил strict relay-only delivery между домашней и cellular сетями с одним
relay path. Это подтвердило cross-network работу `clear_ip_transports` и
локализовало прежние timeout в доступности `euc1` route во время тестов;
финальный restart/sync через `aps1` восстановил 6 недостающих events за один
round и свёл обе стороны к 8 events. M0.3 завершён.
M0.4 завершил проверку resumable sync: CLI умеет штатно остановиться после
заданного числа завершённых rounds через `sync --max-rounds`, а новый
transport-independent тест меняет session binding после первого batch и при
reconnect передаёт только оставшиеся 6 из 70 events. Для bounded full-ID
профиля durable event set принят как correctness checkpoint; отдельный
переносимый signed cursor отложен до compact Merkle/range summary. Внешний
test7 остановился после 64/64 по direct LAN, после смены сети Bob продолжил
через pinned `aps1` только остатком 6/6; обе histories полностью совпали и
содержат 142 events с одинаковым frontier.
M0.5.1 добавил отдельную Account Root Identity: публичный Ed25519 key является
`AccountId`, а root secret подписывает capability-bearing device certificates
и постоянные отзывы device keys, но не обычные сообщения. Реализованы
локальные CLI lifecycle-команды create/show/enroll/authorize/revoke и проверка
чужого account, tampering, capability mismatch и revocation. M0.5.2 встроил
эту модель в сеть: ticket v3 содержит listener certificate и allowed requester
Account ID; до event/inventory клиент предъявляет certificate и device-signed
Endpoint-bound proof. `--allow-device` и known-author удалены, новое устройство
того же account может восстановить пустую историю. Проверяющая сторона принимает
trusted revocations через `--peer-revocation-file`; revoked device отклоняется
до sync inventory. M0.6.1 заменил эти файлы root-signed полным authority
snapshot в ticket v4/session proof. Устройства атомарно сохраняют max-seen
revision каждого account и отклоняют rollback/equivocation. Snapshot доказывает
completeness на своей revision, но не global freshness при первом контакте.
M0.6.2 добавил owner-signed add-only conversation membership и обязательный
`AuthorizedEvent` с certificate/snapshot автора. Delivery, acknowledgement,
history и каждый sync event проверяют цепочку membership → account → device →
event; store требует immutable authorization sidecar. Локальный Alice/Bob Iroh
smoke получил две одинаковые авторизованные истории.
M0.7.1 удалил plaintext `Text` payload и добавил static HPKE baseline. M0.7.2
удалил sender box из replicated event: sender и recipient хранят отдельные
local-only encrypted projections, адресованные Event ID. M0.7.3 заменил
оставшийся peer HPKE box на `vodozemac::olm` Double Ratchet: device-signed
identity/one-time prekey входит в ticket, а persistent session выдаёт
PreKey/Normal ciphertext с per-message key evolution. Sync projection не
передаёт и создаёт её только после ratchet decrypt. M0.7.4 добавил root-signed
полный device list, точный directory prekey bundles и отдельный ratchet
ciphertext каждого устройства peer account в одном Event ID. Offline Bob-2
может получить через sync тот же event, расшифровать свой slot и получить
историю, идентичную Bob-1. M0.7.5 добавил same-account authenticated history
rewrap для устройства, добавленного после старых events: source подписывает
canonical inventory/range и HPKE ciphertexts, а local projection v2 сохраняет
проверяемый provenance. Полный range — это claim source, не глобальный
checkpoint. M0.7.6 заменил сетевой single-OTK directory на signed pools по 16
ключей с generation/sequence/expiry и persistent max-seen anti-rollback.
Crossed outbound sessions теперь временно сосуществуют и детерминированно
сходятся на одном active session ID, сохраняя losing session для in-flight
сообщений. Ticket повышен до v9; event/sync v5/v6 не изменились.
M0.7.7 добавил exclusive OS lock всего device `STATE_DIR` и prepared/committed/
rolled-back filesystem journal. Ratchet tree и `next-sequence` восстанавливаются
из backup, а rollback удаляет только новые immutable event/projection/rewrap
files относительно baseline. Delivery, sync batch, seed/import и prekey
rotation теперь завершаются local commit до сетевого ответа; следующий запуск
автоматически откатывает оставленный prepared journal. Wire версии не менялись.
M0.7.8 добавил двусторонний consent/SAS, session-bound network history rewrap и
multi-source claim reconciliation. M0.7.9 добавил signed append-only checkpoint
  chain и `history-recovery-resume`: каждая страница атомарно коммитит import и
  checkpoint, а смена source inventory claim отклоняется. M0.9.1 добавил bounded
  coordinator: до 64 смежных страниц идут по одному явно подтверждённому
  connection и одному immutable source snapshot. M0.9.2 заменил ручной recovery
  ticket компактной source-signed recipient-specific ссылкой с offline inspect,
  expiry и exact device/conversation/SAS preflight до сети. M0.9.3 добавил
  no-clobber PNG renderer и bounded exact-one PNG/JPEG decoder; inspect/accept
  теперь принимают QR image напрямую. M0.9.4 добавил opt-in signed URI
  publication через bounded IPv4 LAN multicast и exact-recipient discovery без
  Iroh connection. M0.9.5 добавил recipient-signed execution plan: fresh
  endpoint может автоматически продолжить recovery только при exact совпадении
  source/device-list/SAS/conversation/range/page/route и разрешённой
  network/power policy. M0.9.6 сохраняет retry state как recipient-signed
  append-only chain: process restart соблюдает persistent equal-jitter deadline,
  attempt lease не допускает параллельный connection, clock rollback блокируется,
  а explicit cancel является terminal. M0.9.7 добавил platform-neutral context
  provider и Windows-native network/cost/roaming/power snapshot; VPN tunnel
  разрешается только через единственный exact active physical profile,
  metered/roaming используют signed `mobile` bucket, ambiguity остаётся unknown.
  M0.9.8 добавил bounded worker, WinRT network/power change subscriptions,
  signed-deadline wakeup, cross-process cancel polling и policy recheck перед
  discovery/connect без удержания state lock во время wait. M0.9.9 добавил
  long-lived messaging runtime: один endpoint/ticket обслуживает successive
  delivery/sync sessions, network wait не держит state lock, а каждая сессия
  получает отдельную vault-mirrored transaction; restart атомарно публикует
  новый ticket. M0.9.10 добавил signed exact-device contact с обновляемым
  descriptor path, locally encrypted durable outbox, materialize-once event,
  idempotent ACK replay, persistent signed retry/backoff и runtime-owned
  automatic sync. Runtime state стал десятым typed repository kind в vault.
  M0.9.11 вынес versioned local API в reusable `kilogram-runtime-ipc`: runtime
  публикует device-signed bearer descriptor для loopback-only bounded RPC,
  а `Ping`, idempotent `QueueMessage` и structured `OutboxStatus` проходят через
  сериализованный actor без доступа UI к `STATE_DIR`. Descriptor должен быть
  private machine-local. M0.9.12 добавил отдельный safe-Rust desktop client;
  M0.9.13 перенёс signed contact list, preview и paginated decrypted history в
  actor-owned IPC read model. M0.9.14 поднял IPC до v3: GUI импортирует contact
  только через actor, запускает runtime из secret-free no-clobber profile и
  штатно останавливает его authenticated `Shutdown`. M0.9.15 поднял IPC до v4:
  отдельный connection-task long poll будит GUI по actor change revision, а
  unconditional двухсекундный polling удалён. Desktop теперь редактирует и
  атомарно сохраняет public launch profile уже enrolled device, не получая
  seed/device/vault secrets. Autostart/service не устанавливается. M0.9.16
  добавил отдельный one-shot bootstrap process и first-run GUI: 24-word BIP39
  phrase детерминированно кодирует Account Root, Windows root envelope защищён
  DPAPI CurrentUser, первый device/certificate/device-list/prekey pool создаются
  в staging и публикуются только после verified vault migration. Persistent
  receipt и launch draft не содержат phrase; восстановление намеренно требует
  ещё и актуальную authority history. M0.9.17 добавил existing-account link:
  short-lived device-signed request, exact SAS, idempotent Root enrollment,
  atomic complete device list, HPKE-encrypted authorization и DB-primary trust
  accept. M0.9.18 добавил desktop wizard с explicit request/response drop target,
  крупным exact SAS, заполнением нового launch-profile draft и списком нескольких
  recipient-bound recovery plans. Одна bounded attempt показывает signed
  scheduler progress, а reconciliation выводит exact source counts и
  `incomplete`/`single-source`/`agreed`/`divergent`. M0.9.19 добавил Root-signed
  portable authority package, который сохраняет exact sequence/revocations,
  complete device list и все current membership heads. Отдельный Root-signed
  witness фиксирует exact package digest/revision; phrase restore выполняется
  только в новый staged Root и заново использует local platform key provider.
  Old package + latest witness отклоняется, но matching rollback обоих файлов
  всё ещё требует global monotonic witness/current-device quorum. M0.9.20
  добавил desktop export/inspect/restore: stopped-runtime one-shot helper остаётся
  единственным Root writer, phrase идёт только bounded stdin и zeroize-ится после
  enqueue, а restore связан с exact inspected package ID/revision для защиты от
  same-path replacement. После успеха GUI только заполняет device-link поля.
  M0.9.21 добавил обязательный exact-export lifecycle: Root локально фиксирует
  witness только после повторной проверки опубликованного package, а status
  пересобирает current package под authority lock и показывает `current` либо
  `update-required`, включая membership-only mutation без смены authority
  revision. Это локальный receipt, не global freshness proof. RFC-0048 выбрал
  fresh challenge-bound current-device quorum как более сильный serverless
  режим и зафиксировал необходимость joint-consensus смены recovery roster.
  M0.9.22–M0.9.25 реализовали device-signed quorum approvals, one-shot Iroh
  transport, joint-majority recovery-policy epochs и desktop orchestration.
  M0.9.26 добавил permanent Root revocation/removal с exact before/after
  checkpoint. M0.9.27 поднял runtime IPC до v5: работающий actor принимает
  только Root-signed monotonic removal-only device list, одной vault-primary
  транзакцией устанавливает authority и удаляет revoked-device ratchet/prekey
  state, затем атомарно заменяет public ticket. Новый peer fanout исключает
  removed device после наблюдения ticket; уже подписанные recipient slots не
  переписываются, а старые копии истории остаются читаемыми. M0.9.28 поднял
  IPC до v6 и добавил device-signed append-only receipt в тот же vault commit.
  Restart проверяет receipt chain и exact digest применённого Root-signed list,
  поэтому stale profile больше не возвращает revoked roster. Desktop получает
  explicit `current`/`convergence-required` status и может bounded-операцией
  заменить только проверенный `device_list_file` path. M0.9.29 поднял IPC до v7
  и заменил synchronized steady-state ticket refresh на signed expiring HTTPS
  publication/fetch: directional publisher-device channel, HPKE slot каждому
  active recipient device, append-only publisher chain и local signed receiver
  high-water. Store не видит ticket/Account/Device plaintext, но IP, timing,
  size и channel correlation остаются видимыми; first contact требует прежней
  независимой проверки. M0.9.30 добавил отдельный `kilogram-ticket-store`:
  loopback-only HTTP за HTTPS reverse proxy, durable Redb opaque values,
  monotonic conditional replacement, fixed TTL, bounded parser/channel/body/
  connection limits и per-IP/global rate limits. M0.9.31 добавил signed opt-in
  foreground automation с network permissions и bounded backoff; M0.9.32 —
  device-signed crash-safe checkpoint/compaction этих runtime chains. M0.9.33
  поднял connection ticket до v10 и заменил identity-input lookup на
  self-authenticating channel из отдельного per-peer Ed25519 write key. PUT
  подписывает exact channel/generation/body, поэтому знающий channel не может
  записать произвольный высокий generation; store по-прежнему не видит
  Account/Device ID или envelope plaintext. M0.9.34 поднял runtime IPC до v9 и
  добавил до четырёх local-device-signed endpoints на stable contact:
  deterministic delivery/automatic-sync failover, single immutable
  materialization, exact attempted-device ack и newest-authority/prekey
  high-water. IP/timing/size correlation остаётся.
  OS peer credentials остаются дальше. Reconciliation выбирает
  inventory только при совпадении двух или более полных явно собранных claims и
  всё равно не обещает global completeness.
M0.8.1 добавил первый production-storage bridge: `state-vault-migrate` одной
durable `redb` transaction создаёт encrypted snapshot всего device state,
`state-vault-verify` аутентифицирует records и сверяет retained legacy tree, а
`state-vault-restore` восстанавливает byte-exact snapshot только в новый
каталог. Это пока shadow vault: live repositories продолжают использовать
legacy files; эволюция key provider описана ниже.
M0.8.2 добавил recoverable shadow dual-write: каждая live device-state команда
сначала сохраняет authenticated intent к exact generation/snapshot, а затем
атомарно зеркалирует фактически committed legacy tree. После crash валидный
intent разрешает next-start recovery; drift без intent блокируется. Read-only
command оставляет generation неизменной, changed command увеличивает её.
M0.8.3 применяет только encrypted record delta: changed/new records
перешифровываются, removed records удаляются, unchanged ciphertext не
переписывается. Девять typed repository kinds проходят exact DB/legacy shadow
comparison; CLI публикует per-kind inventory и delta counters. Filesystem пока
остаётся primary read/write path.
M0.8.4 переводит read-only `history` на DB-primary owned snapshot для
events/authorization/local projections. Vault полностью проверяется и exact
сравнивается с legacy shadow; после этого прикладные decoders получают DB bytes.
Инициализированный vault при любой ошибке блокирует history без silent fallback;
writes и остальные reads пока остаются legacy-primary.
M0.8.5 переводит на тот же immutable vault-primary read-set ручной
`history-rewrap-export` и source-side network rewrap. Snapshot захватывается до
command-local authority/prekey/request mutations, а bundle builder зависит
только от read traits. `EventReadRepository` теперь также даёт authorized
inventory/events-by-ID для будущего sync overlay. Mixed read/write sync пока
остаётся legacy-primary.
M0.8.6 переводит sync reads на authenticated immutable vault base плюс
command-local committed overlay. Event/projection batch сначала валидируется и
staged, затем проходит прежнюю crash-consistent filesystem transaction и только
после её commit становится видим следующим rounds. Process smoke передал 73
events двумя rounds 64+9, listener overlay вырос до 73 events/73 projections,
обе vault-primary истории совпали; shadow drift завершился без fallback. Writes
пока остаются legacy-primary.
M0.8.7 делает vault необратимой точкой commit для всех M0.7.7
StateTransaction operations. Filesystem сначала служит staging, затем одна
immediate redb transaction публикует encrypted delta/generation и
authenticated primary-shadow intent. Только после этого journal коммитит legacy
shadow; crash между шагами восстанавливает весь shadow из vault, включая
связанные ratchet/sequence records. Network response следует после обоих
commit/confirmation. Mutable reads и non-transactional trust writes ещё не
полностью переведены.
M0.8.8 заменяет полный filesystem payload checkpoint на typed delta активного
journal. Direct vault transaction принимает changed ratchet/sequence и только
новые append-only records, проверяет canonical kind/path и сохраняет прежний
primary-shadow crash contract. `next-sequence` стал первым mutable DB-primary
adapter: allocator получает authenticated counter из vault, а filesystem
записывает только transactional shadow. Старые history payloads больше не
перечитываются из filesystem при построении pre-commit delta, но exact
post-commit shadow confirmation, DB manifest rebuild и path enumeration ещё
остаются `O(state)`. Пока authority/contact writers не journal-aware, bounded
trust compatibility ingress включает authority/certificate/membership/
peer-authority records в ту же vault transaction и запрещает их неявное
удаление; trust repository всё ещё filesystem-backed.
M0.8.9 делает ratchet read DB-primary на каждой transaction boundary. Active
vault generation гидратирует crash-журналированный staging до открытия
`RatchetState`, direct delta сравнивается с DB baseline, а rollback ratchet и
sequence использует durable primary backup вместо предкомандного shadow.
Append-only writers явно регистрируют event/authorization/projection/rewrap/
recovery paths; второй directory walk при построении delta удалён, committed
append record нельзя удалить или изменить на месте. Начальный rollback baseline,
final exact shadow scan и full DB manifest rebuild пока остаются `O(state)`.
M0.8.10 переносит знание append layout из CLI в repository-owned
`AppendOnlyWriteReceipt`: event/authorization/projection/rewrap/transfer/
checkpoint writers возвращают exact canonical paths, а transaction проверяет
root и kind. Vault schema v2 добавляет AEAD-encrypted path/length/hash index;
normal typed commit обновляет manifest без enumeration/decrypt неизменённых DB
payload records (`vault_payload_records_loaded=0`). Schema v1 проверяется и
перестраивается один раз. Pre-command gate, initial journal baseline и final
exact shadow confirmation всё ещё full-state; index metadata пока цельный
`O(record count)` blob.
M0.8.11 удаляет bounded trust compatibility ingress. Новый
`TrustStateRepository` выбирает certificate/own-authority/peer-authority/
membership records из authenticated schema-v2 index и decrypt-ит только эти
payloads; при initialized vault silent filesystem fallback запрещён. Каждый
trust write явно гидратирует crash-journaled workspace из DB baseline и
коммитит typed `Trust` delta до retained shadow. Rollback и next-start recovery
восстанавливают DB-authoritative trust, а незарегистрированная filesystem
подмена больше не становится authority mutation.
M0.8.12 заменяет raw 32-byte `state-vault.key` versioned envelope. Windows
сборка использует DPAPI CurrentUser и никогда не пишет новый vault master key
на диск открытым; старый development key автоматически атомарно rewrap-ится
без re-encryption DB. Ошибка provider, повреждение envelope или запуск под
другим Windows user/machine fail-closed до открытия redb. На non-Windows пока
остаётся явно диагностируемый `plaintext-development` provider.
M0.8.13 добавляет явный portable recovery package: master key шифруется
Argon2id-derived key и XChaCha20-Poly1305 только во внешнем no-clobber файле.
Package связывает key с generation/snapshot witness. Импорт сначала полностью
аутентифицирует candidate DB, отклоняет rollback и same-generation fork и лишь
затем атомарно создаёт локальный provider envelope. Согласованный rollback DB
вместе со старым package по-прежнему требует независимого monotonic witness.
M0.8.14 переводит device signing/encryption identity на отдельный immutable
`DeviceIdentityStateRepository`. После инициализации vault все production
команды получают ровно две 32-byte записи из authenticated schema-v2 index и
не делают filesystem fallback; missing/invalid/tampered record fail-closed.
M0.8.15 вводит schema-v3 DB-only identity layout. Upgrade сначала атомарно
публикует новую schema и generation, затем удаляет только byte-exact raw
`device-secret.key`/`device-encryption-secret.key`. Exact gate сохраняет DB
identity в effective snapshot, typed shadow показывает для неё 0 records, а
primary-shadow recovery больше не создаёт plaintext keys. Mismatched
повторно появившаяся копия блокируется без импорта.
Полное seed/root recovery с authority history, non-Windows root provider,
защищённое хранение оставшихся ratchet shadows, дальнейшее shadow retirement,
global prekey discovery/witness, автоматический recovery source discovery,
постоянный scheduler, QR/device-link UX, sync summaries, membership removal и
группы ещё не реализованы.

## Цель продукта

Создать публичный, открытый P2P-мессенджер с удобством и функциональной
полнотой уровня Telegram, но с true end-to-end encryption и без доверия
оператору инфраструктуры в вопросах чтения или незаметной модификации
переписки.

Рабочее название — **Kilogram**. Оно отсылает к Telegram и может быть изменено.
Идейный ориентир — Keet, но Kilogram не является его форком. Разработка
изначально AI-driven, с упором на более высокую скорость развития и лучший UX.

## Принятые решения

- Базовая архитектура: независимое ядро на Rust.
- Предварительный сетевой фундамент: Iroh (QUIC, hole punching, relay fallback).
- Групповое E2EE: OpenMLS / MLS RFC 9420.
- История и синхронизация: собственный подписанный журнал событий и causal DAG.
- Hypercore, Autobase и HyperDHT используются как референсы, но не являются
  фундаментом первой выбранной архитектуры.
- На первом этапе прямое P2P-соединение допустимо, несмотря на раскрытие IP
  собеседнику.
- В дальнейшем появится privacy mode с обязательным relay или несколькими
  relay; прямой P2P можно будет разрешать доверенным контактам.
- Постоянная читаемая история хранится только на устройствах участников.
- Случайные узлы могут временно хранить непрозрачные E2EE-зашифрованные пакеты
  с TTL, не имея ключей и прикладных идентификаторов.
- Seed-фраза восстанавливает корневую идентичность аккаунта. У каждого
  устройства должны быть отдельные ключи и отзывные полномочия.
- В M0.5.1 `AccountId` является отдельным Ed25519 public root key; root secret
  подписывает только device certificate/revocation. Любой валидный отзыв
  навсегда запрещает повторное использование конкретного device key.
- Блокчейн для групп не используется. Базовая модель порядка — причинный DAG,
  детерминированная линеаризация и, при необходимости, кворумные checkpoints.
- Протокол и клиент должны быть open-source; безопасность не должна зависеть
  от секретности реализации.

## Основные свойства

- Конфиденциальность: инфраструктура и узлы хранения не получают ключи
  сообщений.
- Целостность: события подписываются устройствами; подмена обнаруживается.
- Устойчивость к удалению: несколько реплик уменьшают влияние одного узла, но
  абсолютная доступность без хотя бы одной копии невозможна.
- Forward secrecy и post-compromise security являются требованиями протокола.
- Компрометация конечного устройства позволяет читать доступный ему plaintext;
  отзыв и смена эпох защищают только последующее общение.
- Компрометация seed равносильна компрометации высшей recovery-власти, если не
  включён дополнительный фактор.

## План ближайших работ

1. M0.9.41: заменить ручную настройку каждой пары одним bounded roster-wide
   own-device availability policy с безопасной реакцией на live Root roster
   add/remove и без параллельного publish/fetch/push storm.
2. Уже реализованный M0.9.40 даёт exact-current sibling Devices directional
   pairwise store capabilities, recipient-HPKE публикацию свежего runtime ticket
   и автоматический fetch в runtime-managed path без общей папки.
3. Уже реализованный M0.9.39 даёт одному listener primary peer и exact-current
   own-account аудитории, stable own-device ticket и Device-signed restart-safe
   foreground schedule с network permissions, backoff и compaction.
4. Уже реализованный M0.9.38 переносит M0.9.37 recipient-encrypted bundle по
   authenticated same-account Device session, вызывает общий import gate и
   возвращает recipient-signed session-bound replay acknowledgement.
5. Уже реализованный M0.9.37 переносит bounded endpoint candidates и signed
   publication high-water между exact-current authorized own Devices через
   source-signed recipient-HPKE file, не принимая bundle как Root authority.
6. Уже реализованный M0.9.36 хранит local-device-signed expiry-independent
   publication binding и обновляет long-offline endpoint без принятия
   просроченного transport/prekey ticket.
7. Уже реализованный M0.9.35 обновляет все enrolled Device channels одной
   bounded action, хранит independent observation high-water и показывает
   desktop `usable`/`stale` состояния.
8. Уже реализованный M0.9.33 даёт self-authenticating per-peer capability
   channel и exact Ed25519 PUT authorization без Account/Device ID на store.
9. Уже реализованные M0.9.31–M0.9.32 дают opt-in foreground automation и
   crash-safe bounded compaction его signed runtime chains.
10. Уже реализованный M0.9.30 даёт self-hostable loopback-only opaque store за
   HTTPS reverse proxy: durable Redb, fixed retention, monotonic replacement,
   size/channel/connection/rate limits и Internet test procedure.
10. Уже реализованный M0.9.29 заменяет synchronized steady-state ticket refresh
   на explicit HTTPS publication/fetch: device-signed expiring chain, per-device
   HPKE slots, local rollback high-water, IPC v7 и Windows UI. Initial verified
   contact остаётся out-of-band, store traffic metadata видимы.
11. Уже реализованный M0.9.28 сохраняет device-signed receipt применённого live
   roster в vault-primary transaction, восстанавливает его раньше stale launch
   profile и bounded desktop-операцией согласует exact canonical path.
12. Optional autostart/background mode оставить отдельной явной настройкой, а не
   обязательным Task Scheduler этапом.
13. Добавить production macOS/Linux local key provider, согласованный monotonic
   witness и lifecycle обновления portable recovery package; затем mobile
   providers. Расширить pseudonymous M0.9.29 lookup до privacy-preserving
   gossip/mailbox. Live camera/clipboard оставить platform UI.
14. Спроектировать membership removal вместе с ordered security log и MLS epoch;
   отдельно — gossip/witness для first-contact freshness.
15. Спроектировать полное seed/recovery authority с monotonic history/witness,
   root rotation и конфликтующие authority operations.
16. Подготовить ADR по Iroh против rust-libp2p и проверить мобильные платформы.
17. Спроектировать финальный wire format подписанного события и алгоритм
   линеаризации.
18. Спроектировать compact Merkle/range summary и переносимый signed cursor.
19. Добавить небольшие MLS-группы.
20. Перед публичным выпуском провести независимый криптографический аудит.

## Навигация

- [`decisions.md`](decisions.md) — журнал архитектурных решений.
- [`open-questions.md`](open-questions.md) — нерешённые вопросы и риски.
- [`development-environment.md`](development-environment.md) — состояние
  локального toolchain и требования к сборке.
- [`milestones.md`](milestones.md) — выполненные и следующие технические этапы.
- [`../docs/RFC-0001-core-architecture.md`](../docs/RFC-0001-core-architecture.md) —
  черновик основного RFC.
- [`../docs/RFC-0002-account-device-authority.md`](../docs/RFC-0002-account-device-authority.md) —
  реализованный M0.6.1-контракт Account Root, snapshot, device и session authority.
- [`../docs/RFC-0003-conversation-membership.md`](../docs/RFC-0003-conversation-membership.md) —
  реализованный M0.6.2-контракт membership и авторизации событий.
- [`../docs/RFC-0004-pairwise-hpke-payload.md`](../docs/RFC-0004-pairwise-hpke-payload.md) —
  исторический M0.7.1 HPKE baseline для ciphertext payload двух устройств.
- [`../docs/RFC-0005-local-encrypted-history-projection.md`](../docs/RFC-0005-local-encrypted-history-projection.md) —
  реализованный M0.7.2-контракт recipient-only event и локальной projection.
- [`../docs/RFC-0006-pairwise-double-ratchet.md`](../docs/RFC-0006-pairwise-double-ratchet.md) —
  реализованный M0.7.3-контракт signed prekey и persistent pairwise ratchet.
- [`../docs/RFC-0007-multi-device-ratchet-fanout.md`](../docs/RFC-0007-multi-device-ratchet-fanout.md) —
  реализованный M0.7.4-контракт signed device list и ratchet fan-out.
- [`../docs/RFC-0008-authenticated-history-rewrap.md`](../docs/RFC-0008-authenticated-history-rewrap.md) —
  реализованный M0.7.5-контракт same-account history rewrap и provenance.
- [`../docs/RFC-0009-authenticated-prekey-pools.md`](../docs/RFC-0009-authenticated-prekey-pools.md) —
  реализованный M0.7.6-контракт fresh prekey pools и concurrent initiation.
- [`../docs/RFC-0010-crash-consistent-local-state.md`](../docs/RFC-0010-crash-consistent-local-state.md) —
  реализованный M0.7.7-контракт local state lock, journal и crash recovery.
- [`../docs/RFC-0011-network-history-rewrap.md`](../docs/RFC-0011-network-history-rewrap.md) —
  реализованный M0.7.8-контракт session-bound rewrap, consent/SAS и reconciliation.
- [`../docs/RFC-0012-resumable-history-recovery.md`](../docs/RFC-0012-resumable-history-recovery.md) —
  реализованный M0.7.9-контракт signed checkpoint pagination и safe retry.
- [`../docs/RFC-0013-encrypted-transactional-state-vault.md`](../docs/RFC-0013-encrypted-transactional-state-vault.md) —
  реализованный M0.8.1-контракт encrypted shadow migration, verify и restore.
- [`../docs/RFC-0014-recoverable-shadow-dual-write.md`](../docs/RFC-0014-recoverable-shadow-dual-write.md) —
  реализованный M0.8.2-контракт authenticated intent, generation и crash recovery.
- [`../docs/RFC-0015-typed-incremental-shadow-repositories.md`](../docs/RFC-0015-typed-incremental-shadow-repositories.md) —
  реализованный M0.8.3-контракт encrypted delta и typed shadow equivalence.
- [`../docs/RFC-0016-immutable-vault-primary-read-canary.md`](../docs/RFC-0016-immutable-vault-primary-read-canary.md) —
  реализованный M0.8.4-контракт DB-primary immutable history canary без fallback.
- [`../docs/RFC-0017-vault-primary-history-rewrap.md`](../docs/RFC-0017-vault-primary-history-rewrap.md) —
  реализованный M0.8.5-контракт vault-primary manual/network rewrap source reads.
- [`../docs/RFC-0018-command-local-sync-read-overlay.md`](../docs/RFC-0018-command-local-sync-read-overlay.md) —
  реализованный M0.8.6-контракт immutable vault base и committed sync overlay.
- [`../docs/RFC-0019-vault-primary-transaction-checkpoint.md`](../docs/RFC-0019-vault-primary-transaction-checkpoint.md) —
  реализованный M0.8.7-контракт vault commit barrier и recoverable legacy shadow.
- [`../docs/RFC-0020-typed-journal-delta-and-mutable-sequence.md`](../docs/RFC-0020-typed-journal-delta-and-mutable-sequence.md) —
  реализованный M0.8.8 typed direct delta и первый mutable DB-primary sequence adapter.
- [`../docs/RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md`](../docs/RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md) —
  реализованный M0.8.9 DB-primary ratchet workspace и explicit append write-set.
- [`../docs/RFC-0022-repository-write-receipts-and-manifest-index.md`](../docs/RFC-0022-repository-write-receipts-and-manifest-index.md) —
  реализованный M0.8.10 contract repository receipts и encrypted manifest index.
- [`../docs/RFC-0023-db-primary-trust-repository.md`](../docs/RFC-0023-db-primary-trust-repository.md) —
  реализованный M0.8.11 DB-primary authority/contact trust repository.
- [`../docs/RFC-0024-protected-vault-key-provider.md`](../docs/RFC-0024-protected-vault-key-provider.md) —
  реализованный M0.8.12 Windows DPAPI key envelope и legacy-key migration.
- [`../docs/RFC-0025-portable-vault-key-recovery-and-rollback-witness.md`](../docs/RFC-0025-portable-vault-key-recovery-and-rollback-witness.md) —
  реализованный M0.8.13 portable key recovery и внешний rollback/fork witness.
- [`../docs/RFC-0026-db-primary-device-identity.md`](../docs/RFC-0026-db-primary-device-identity.md) —
  реализованный M0.8.14 immutable DB-primary device identity без filesystem fallback.
- [`../docs/RFC-0027-db-only-device-identity-layout.md`](../docs/RFC-0027-db-only-device-identity-layout.md) —
  реализованный M0.8.15 schema-v3 layout и retirement raw device identity shadow.
- [`../docs/RFC-0028-bounded-multi-page-history-recovery-session.md`](../docs/RFC-0028-bounded-multi-page-history-recovery-session.md) —
  реализованный M0.9.1 coordinator до 64 atomic recovery pages в одном connection.
- [`../docs/RFC-0029-signed-history-recovery-device-link.md`](../docs/RFC-0029-signed-history-recovery-device-link.md) —
  реализованный M0.9.2 compact recipient-specific descriptor, offline inspect и explicit SAS-gated accept.
- [`../docs/RFC-0030-bounded-history-recovery-qr-ceremony.md`](../docs/RFC-0030-bounded-history-recovery-qr-ceremony.md) —
  реализованный M0.9.3 no-clobber PNG renderer и bounded exact-one PNG/JPEG QR import.
- [`../docs/RFC-0031-authenticated-lan-recovery-discovery.md`](../docs/RFC-0031-authenticated-lan-recovery-discovery.md) —
  реализованный M0.9.4 opt-in bounded LAN publication и verified no-connect discovery.
- [`../docs/RFC-0032-consent-bound-history-recovery-scheduler.md`](../docs/RFC-0032-consent-bound-history-recovery-scheduler.md) —
  реализованный M0.9.5 recipient-signed retry plan и bounded fresh-endpoint coordinator.
- [`../docs/RFC-0033-persistent-history-recovery-scheduler-state.md`](../docs/RFC-0033-persistent-history-recovery-scheduler-state.md) —
  реализованный M0.9.6 signed append-only retry state, jitter, lease и terminal cancellation.
- [`../docs/RFC-0034-windows-recovery-platform-context.md`](../docs/RFC-0034-windows-recovery-platform-context.md) —
  реализованный M0.9.7 Windows-native network/metered/roaming/power snapshot и fail-closed provider boundary.
- [`../docs/RFC-0035-bounded-windows-recovery-worker.md`](../docs/RFC-0035-bounded-windows-recovery-worker.md) —
  реализованный M0.9.8 bounded process, native change events, signed deadline/cancel wakeup и повторный policy gate.
- [`../docs/RFC-0036-long-lived-messaging-runtime.md`](../docs/RFC-0036-long-lived-messaging-runtime.md) —
  реализованный M0.9.9 stable endpoint, multi-session listener и per-session state/vault transaction.
- [`../docs/RFC-0037-persistent-runtime-contact-and-outbox.md`](../docs/RFC-0037-persistent-runtime-contact-and-outbox.md) —
  реализованный M0.9.10 signed contact, encrypted durable outbox, persistent retry и automatic sync.
- [`../docs/RFC-0038-authenticated-local-runtime-ipc.md`](../docs/RFC-0038-authenticated-local-runtime-ipc.md) —
  реализованный M0.9.11 device-signed loopback IPC и сериализованный runtime actor API.
- [`../docs/RFC-0039-minimal-desktop-runtime-client.md`](../docs/RFC-0039-minimal-desktop-runtime-client.md) —
  реализованный M0.9.12 safe-Rust desktop GUI поверх authenticated runtime IPC.
- [`../docs/RFC-0040-actor-owned-chat-read-model.md`](../docs/RFC-0040-actor-owned-chat-read-model.md) —
  реализованный M0.9.13 signed chat list и paginated readable local history через runtime actor.
- [`../docs/RFC-0041-desktop-contact-onboarding-and-runtime-lifecycle.md`](../docs/RFC-0041-desktop-contact-onboarding-and-runtime-lifecycle.md) —
  реализованный M0.9.14 IPC contact import, secret-free launch profile и foreground runtime lifecycle.
- [`../docs/RFC-0042-desktop-runtime-setup-and-change-notifications.md`](../docs/RFC-0042-desktop-runtime-setup-and-change-notifications.md) —
  реализованный M0.9.15 desktop launch-profile editor и actor-safe IPC change notifications.
- [`../docs/RFC-0043-desktop-first-account-bootstrap.md`](../docs/RFC-0043-desktop-first-account-bootstrap.md) —
  реализованный M0.9.16 recoverable Account Root, protected key envelope и atomic first-device bootstrap.
- [`../docs/RFC-0044-existing-account-device-link.md`](../docs/RFC-0044-existing-account-device-link.md) —
  реализованный M0.9.17 short-lived SAS-gated device request, Root enrollment, recipient-encrypted authority transfer и DB-primary accept.
- [`../docs/RFC-0045-desktop-device-link-and-recovery-wizard.md`](../docs/RFC-0045-desktop-device-link-and-recovery-wizard.md) —
  реализованный M0.9.18 desktop enrollment/recovery wizard, bounded helper adapters и honest multi-source reconciliation UI.
- [`../docs/RFC-0046-account-root-authority-recovery.md`](../docs/RFC-0046-account-root-authority-recovery.md) —
  реализованный M0.9.19 portable Root authority package, exact witness и atomic phrase restore.
- [`../docs/RFC-0047-desktop-account-root-recovery.md`](../docs/RFC-0047-desktop-account-root-recovery.md) —
  реализованный M0.9.20 desktop export/inspect/stdin restore и exact inspected-artifact gate.
- [`../docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md`](../docs/RFC-0048-recovery-freshness-and-checkpoint-lifecycle.md) —
  реализованный M0.9.21–M0.9.27 recovery lifecycle, quorum/policy transitions и runtime activation removal.
- [`../docs/RFC-0049-live-runtime-device-directory-refresh.md`](../docs/RFC-0049-live-runtime-device-directory-refresh.md) —
  реализованный M0.9.27 authenticated live roster refresh, crash-consistent ratchet retirement и honest immutable-history boundary.
- [`../docs/RFC-0050-restart-safe-runtime-device-directory.md`](../docs/RFC-0050-restart-safe-runtime-device-directory.md) —
  реализованный M0.9.28 device-signed receipt chain, restart recovery и bounded launch-profile convergence.
- [`../docs/RFC-0051-signed-wide-area-ticket-publication.md`](../docs/RFC-0051-signed-wide-area-ticket-publication.md) —
  реализованный M0.9.29 signed expiring ticket publication, recipient HPKE slots, local rollback high-water и explicit first-contact/privacy boundary.
- [`../docs/RFC-0052-self-hostable-opaque-ticket-store.md`](../docs/RFC-0052-self-hostable-opaque-ticket-store.md) —
  реализованный M0.9.30 loopback-only self-hostable store, durable opaque Redb records, fixed retention и bounded abuse controls.
- [`../docs/RFC-0053-opt-in-ticket-automation.md`](../docs/RFC-0053-opt-in-ticket-automation.md) —
  реализованный M0.9.31 signed opt-in publish/refresh scheduler, durable bounded backoff, network permissions и explicit foreground-only UI state.
- [`../docs/RFC-0054-authenticated-runtime-ticket-compaction.md`](../docs/RFC-0054-authenticated-runtime-ticket-compaction.md) —
  реализованный M0.9.32 device-signed checkpoint, bounded ticket-chain retention и crash-safe vault-primary compaction.
- [`../docs/RFC-0055-unlinkable-ticket-write-capability.md`](../docs/RFC-0055-unlinkable-ticket-write-capability.md) —
  реализованный M0.9.33 self-authenticating per-peer channel и unlinkable Ed25519 PUT authorization.
- [`../docs/RFC-0056-authenticated-endpoint-candidate-failover.md`](../docs/RFC-0056-authenticated-endpoint-candidate-failover.md) —
  реализованный M0.9.34 bounded authenticated multi-device endpoint set, deterministic delivery/sync failover и stable contact compatibility.
- [`../docs/RFC-0057-multi-candidate-ticket-refresh.md`](../docs/RFC-0057-multi-candidate-ticket-refresh.md) —
  реализованный M0.9.35 parallel network/sequential commit refresh всех enrolled Device channels, independent observation high-water и typed `usable`/`stale` desktop state.
- [`../docs/RFC-0058-expiry-independent-publication-binding.md`](../docs/RFC-0058-expiry-independent-publication-binding.md) —
  реализованный M0.9.36 local-device-signed immutable channel binding, legacy backfill from authenticated expired ticket и fresh-only endpoint installation.
- [`../docs/RFC-0059-authenticated-own-device-endpoint-announcements.md`](../docs/RFC-0059-authenticated-own-device-endpoint-announcements.md) —
  реализованный M0.9.37 source-signed recipient-HPKE endpoint bundle, exact own-roster import gate и sibling publication high-water evidence.
- [`../docs/RFC-0060-network-own-device-endpoint-announcements.md`](../docs/RFC-0060-network-own-device-endpoint-announcements.md) —
  реализованный M0.9.38 authenticated same-account network push, общий import gate, session-bound recipient ACK и bounded backpressure.
- [`../docs/RFC-0061-multi-audience-own-device-automation.md`](../docs/RFC-0061-multi-audience-own-device-automation.md) —
  реализованный M0.9.39 dual signed ticket views, bounded multi-audience listener и Device-signed foreground own-device schedule с backoff/compaction.
- [`../docs/RFC-0062-pairwise-own-device-ticket-discovery.md`](../docs/RFC-0062-pairwise-own-device-ticket-discovery.md) —
  реализованный M0.9.40 directional pairwise store capability, source-signed recipient-HPKE runtime-ticket locator и foreground publish/fetch/push без общей папки.
- [`../docs/M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md`](../docs/M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md) —
  двухсетевой HTTPS publish/fetch/restart/retention test procedure.
- [`../docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](../docs/M0.4-RESUMABLE-SYNC-TEST-RU.md) —
  внешний тест pause/reconnect со сменой интерфейса.
