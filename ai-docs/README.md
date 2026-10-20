# Память проекта Kilogram

Актуально на: 2026-10-20.

Эта папка — краткая проектная память и дорожная карта. Подробная техническая
спецификация находится в [`docs/RFC-0001-core-architecture.md`](../docs/RFC-0001-core-architecture.md).

Последний рабочий блок описан в
[`next-stage-plan.md`](next-stage-plan.md): M1.2.17 добавил guided desktop peer
re-enrollment поверх authenticated IPC с exact staged plan, раздельными
approve/accept, честным incomplete state и обязательной fresh-process проверкой
consumed transition. Публичный default остаётся legacy-only; hot apply и
неявный доступ replacement Device к старой истории отсутствуют.

## Текущее состояние

Архитектура остаётся в стадии проектирования. Rust workspace теперь содержит
`kilogram-identity`, `kilogram-protocol`, `kilogram-ratchet`, `kilogram-state`,
`kilogram-store`, `kilogram-runtime-ipc`, `kilogram-bootstrap-contract`,
`kilogram-session`, `kilogram-ticket-publication`, `kilogram-transport-iroh`,
`kilogram-publication-conflict`, network-free `kilogram-mailbox`, bounded
`kilogram-mailbox-client`, `kilogram-cli`, purpose-built
`kilogram-offline`, отдельный
`kilogram-bootstrap` и первый GUI package
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
К M0.9.51 поверх этого уже реализованы recoverable seed/Account Root lifecycle,
device link/removal, current-device quorum, desktop/runtime IPC, wide-area
ticket publication, multi-Device endpoint convergence и полный
publication-conflict incident flow. Purpose-built `kilogram-offline` оставляет
на Root host только inspect/confirm/sign surface, dependency gate исключает
network/runtime crates, а deterministic portable package несёт provenance и
checksums. Общий `kilogram-publication-conflict` теперь является единственным
владельцем signed publication/observation/proof/request/response/KPC1 codec для
online и offline сторон без network/runtime/image dependencies.
`kilogram-mailbox` теперь отдельно фиксирует network-free blind asynchronous
delivery contract: unrelated read/write capabilities, recipient-HPKE envelope,
bounded Redb store, signed receipts и replay-safe conditional deletion без
прикладных identifier types. Network adapter и client outbox/inbox integration
ещё не реализованы. Production non-Windows/mobile Root provider,
hardware-backed signer, independently
reproducible builder, global
freshness/completeness witness, privacy-preserving mailbox/gossip, membership
removal semantics и MLS-группы ещё не реализованы.

## Цель продукта

Создать публичный, открытый P2P-мессенджер с удобством и функциональной
полнотой уровня Telegram, но с true end-to-end encryption и без доверия
оператору инфраструктуры в вопросах чтения или незаметной модификации
переписки.

UX должен быть максимально знаком пользователю Telegram: те же основные
навигационные и чатовые паттерны без копирования бренда, assets и кода. Там,
где P2P/E2EE semantics отличаются, интерфейс обязан показывать честное
состояние доставки и безопасности, а не имитировать невозможную гарантию.

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
- Корневая идентичность не является будущим публичным глобальным User ID.
  Контакты, группы, каналы и боты должны видеть pairwise/context/service-scoped
  pseudonyms, не связываемые между scopes без явного решения пользователя.
- В M0.5.1 `AccountId` является отдельным Ed25519 public root key; root secret
  подписывает только device certificate/revocation. Любой валидный отзыв
  навсегда запрещает повторное использование конкретного device key. Это
  реализованный precursor, который предстоит скрыть за contextual identity до
  production-совместимости публичных групп, каналов и ботов.
- Полученное и committed сообщение не может быть физически удалено или
  переписано удалённым собеседником. Edit/retract являются новыми подписанными
  событиями; local owner сохраняет доступ к original/version history и сам
  решает, удалять ли свою локальную копию.
- Anti-spam/Sybil не должен возвращать один глобальный traceable ID. Admission,
  reputation и rate limits задаются по контексту; для более сильной защиты
  используются invitations/vouching, bounded proof of work, payment или
  privacy-preserving scarce credentials.
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

M0.9.76 уже имеет принятое внешнее service-free v2 evidence. M0.9.77 закрепил
bundled `rust-lld.exe`, BLAKE3 pure-Rust codegen и bounded PE normalization, а
M0.9.78 добавил exact native-input capture. External run `35471719378` для
`c92d25f...` точно подтвердил причину оставшегося structural divergence: local
использовал MSVC `14.44.35207`/SDK `10.0.28000.0`, GitHub `windows-2025` — MSVC
`14.51.36231`/SDK `10.0.26100.0`; attestation правильно не создана.
M0.9.79 закрепляет repository lock из десяти exact libraries: MSVC
`14.44.35207` и non-preview Windows SDK `10.0.19041.0`. Builders проверяют
hash/length до build, передают ordered `-L native` только final binary и затем
требуют observed LLD manifest равным lock. Библиотеки не vendored; workflow
использует `windows-2022`, но доверяет hashes, а не mutable image label.
Run `35473329174` подтвердил exact native match, но затем fail closed обнаружил
новый 1,024-byte mismatch. В retained EXE остались разные user-profile paths к
Cargo registry: `C:\Users\Paul\.cargo\registry\src` против
`C:\Users\runneradmin\.cargo\registry\src`. M0.9.80 добавил второй rustc
remap в `Z:/cargo-registry-src`, post-build PE leak check и format-v6 evidence.
Clean exact revision `4e9054ab...` дал локальный artifact 2,432,000 bytes с
SHA-256 `d9f9f450...cacb4`; GitHub run `35474774356` воспроизвёл его
byte-for-byte и создал attestation `48691093`. Cross-host divergence закрыт.

1. M0.9.81 принят: fail-closed `M1-CANDIDATE.json` связал принятое M0.9.76
   service-free field evidence с успешной M0.9.80 attestation; production
   verifier подтвердил exact committed candidate `485857ec...d2d84`, ancestor
   relations и отсутствие drift в обеих protected Git surfaces.
2. M0.9.82 принят локально и добавляет первый честный слой Sybil-cost: новые provider offers
   платят bounded 18-bit BLAKE3/Hashcash-style work, а new selection/gossip
   fail closed исключают дешёвые identities. Это cost floor, не доказательство
   независимых операторов и не полная Sybil resistance.
3. M0.9.83 добавляет bounded local authenticated observation provenance:
   exact offer получает до восьми дедуплицированных pseudonymous observer tags,
   выведенных из local Device secret и уже authenticated peer session. Tags и
   social IDs не входят в offer/gossip/IPC.
4. M0.9.84 принимает bootstrap-safe policy: admission-qualified offers с хотя
   бы одним authenticated observation идут первыми, количество сверх одного
   не повышает rank, а unobserved candidates всегда заполняют остаток запроса.
5. M0.9.85 после verified PUT/LIST/DELETE сохраняет local-only keyed tag
   фактически выбранного direct remote IP без порта либо relay origin. Tag
   привязан к exact offer и local Device-secret derivation epoch; raw адреса,
   tags и epochs не входят в offer/gossip/protocol/IPC.
6. M0.9.86 использует только положительный co-location signal: второй exact
   offer с тем же known local path domain откладывается, пока есть
   non-conflicting или path-unknown candidate. Deferred offers возвращаются
   вторым проходом при нехватке альтернатив, поэтому availability не падает;
   разные tags не считаются доказательством независимых операторов.
7. M0.9.87 clean two-host run `20261014-080905` подтвердил полный service-free
   lifecycle и два разных physical provider hosts, но был superseded после
   runtime change. Fresh M0.9.93 run `20261014-104506` на revision
   `d33dd095...` повторил lifecycle после MAX_PATH fix и сохранил
   `independent_provider_field_acceptance=false`: operator/network separation
   требует будущего strict three-/four-host run.
8. M0.9.88 принят externally: revision `68804c93...` локально и в GitHub run
   `38507114425` дала byte-identical offline artifact, attestation `55683374`
   проверена production verifier; Windows release исключён только из Cargo
   tool identity equality, остальные поля остаются exact.
9. M0.9.89 принят локально: candidate revision `d9142003...` прошёл production
   verifier на clean tree; обе protected surfaces неизменны, network retest и
   release build не требуются. Это ещё не M1 tag/public security release.
10. M0.9.90 принят локально на revision `56c93aba...`: closed inventory всех 32
   boundary gates, exact markers, production candidate continuity, format и
   offline strict Clippy verified. Rust test EXE/release/archive/tag не запускались.
11. M0.9.91 добавляет read-only GitHub-hosted Windows CI на каждый master
   push/PR: readiness inventory расширен до 33 gates, затем выполняются все
   stable-path Rust test harnesses с двумя jobs и Cargo offline после fetch.
   Первый run `38509447420` прошёл readiness, но выявил два runtime-теста
   (79/81 aggregate passed). Run `38513111049` подтвердил один как
   process-sensitive, а у второго обнаружил реальную MAX_PATH ошибку.
12. M0.9.92 исправляет append-only runtime journal для extended-length Windows
   paths без сокращения content-addressed имён. Локальный >260 regression и
   fresh physical run `20261014-104506` прошли. GitHub run `38519726403`
   подтвердил прохождение полного Rust suite, но job покраснел уже после
   тестов на empty-cache accounting.
13. M0.9.93 обновляет candidate на fresh field revision `d33dd095...`, run
   `20261014-104506` и runtime surface SHA-256 `95c1771d...`; independent
   M0.9.88 attestation остаётся применимой, поскольку её protected surface не
   менялась. Candidate commit `d8252c6...` прошёл clean production verifier и
   полный readiness `33/33`; fresh green hosted baseline ещё ожидается.
14. M0.9.94 исправляет только verification tooling: missing и empty Cargo
   profile trees считаются exact zero, а Windows CI boundary выполняет
   missing/empty/non-empty self-test. Field/runtime и independent surfaces не
   менялись. Revision `520793f...` прошла clean readiness `33/33`; нужен fresh
   green CI, но не новый сетевой run.
15. M0.9.95 исправляет hosted scheduling без Rust/runtime drift: isolated
   restart test запускается один раз до aggregate suite и при explicit Normal
   priority. Local hosted-mode run прошёл isolated `1/1` и aggregate CLI
   `81/81`; revision `c679313...` прошла clean readiness `33/33`. Hosted run
   `38531172733` подтвердил политику, но снова пересёк test-local 10-second
   restart deadline, поэтому scheduling-only fix признан недостаточным.
16. M0.9.96 заменяет этот хрупкий test-local wait на bounded 60-second
   task-aware readiness: IPC ждётся одновременно с runtime task, поэтому
   реальная ранняя ошибка видна сразу и не маскируется timeout. Exact local
   test прошёл за 61.02s; coarse field surface требует fresh physical rerun и
   candidate refresh перед новым hosted acceptance run.
17. M0.9.97 разделяет exact configured relay pin и observed registration.
   Fresh two-host run `20261014-141058` revision `be80d28...` подтвердил exact
   2-of-2 volunteer lifecycle и прошёл strict reduced verifier; результат не
   повышается до independent-provider acceptance. Следующие обязательные шаги:
   candidate refresh, clean readiness и fresh green hosted Windows CI.
18. M0.9.98 обновляет `M1-CANDIDATE.json` на fresh run `20261014-141058` и
   runtime surface `b5cb9bc9...`; unchanged independent M0.9.88 surface
   сохраняет принятую attestation. Revision `bc3e824...` прошла production
   candidate verifier и полный readiness `33/33`; остался fresh hosted CI.
19. M0.9.99 принят: GitHub Windows CI run `38588961507` на revision
   `9d7895f...` прошёл hosted readiness `33/33`, isolated lifecycle `1/1` и
   aggregate harnesses `276/276`; job завершился зелёным за 38m41s. Это закрывает
   hosted gate, но не разрешает M1 tag автоматически.
20. Пользователь явно принял M1.0.0 как Windows technical proof of concept с
   неизменёнными residual risks. Exact acceptance commit должен пройти fresh
   hosted Windows CI до создания annotated tag `M1.0.0`; release binaries,
   archive и GitHub Release не создаются, Cargo workspace остаётся `0.1.0`.
21. Optional autostart/background mode оставить отдельной явной настройкой;
   Windows Task Scheduler не является обязательной частью мессенджера.
4. Уже реализованный M0.9.49 выделяет единый
   `kilogram-publication-conflict`: online/offline используют один signed codec,
   dependency gate запрещает network/runtime/image crates и отдельный guard
   отклоняет повторное появление дублирующих wire definitions.
5. Уже реализованный M0.9.48 выделяет `kilogram-offline` с тремя командами,
   dependency deny gate и deterministic portable package; cross-component
   regression доказывает byte compatibility с online `.pcrq/.pcrp`.
6. Уже реализованный M0.9.47 публично проверяет полные `.pcrq`/`.pcrp`, создаёт
   compact exact-match QR claim и требует 96-bit KPC1-код до загрузки Root;
   IPC v20 и Windows показывают digest, код и точные команды церемонии.
7. Уже реализованный M0.9.46 вращает собственный publication channel через
   authenticated live actor, атомарно переиздаёт ticket на том же endpoint и
   создаёт/применяет conflict request/response без restart; Windows показывает
   единый incident lifecycle, а IPC v19 возвращает typed результаты.
7. Уже реализованный M0.9.45 разделяет online Device-signed `.pcrq` и offline
   Root-signed self-contained `.pcrp`, автоматически переносит resolution через
   bundle v4 к exact-current siblings и даёт Windows incident UI без Root path;
   ACK v2 и IPC v18 отдельно считают resolution evidence.
8. Уже реализованный M0.9.44 переносит conflict proof через bundle v3,
   конвергирует разные local proof ID в stable evidence ID, вращает настоящий
   ticket v11 channel epoch и применяет exact Root-signed `.pcr` без удаления
   исходного `.pcf`; IPC v17 отдельно считает conflict evidence.
9. Уже реализованный M0.9.43 сохраняет canonical signed observation conflict
   в одном append-only `.pcf`, карантинит channel для delivery/sync/refresh и
   показывает proof/generation/time через IPC v16 и Windows UI. Это local
   conflict evidence, не peer equivocation proof и не global consensus.
10. Уже реализованный M0.9.42 транзитивно переносит direct или accepted
   publication high-water через bundle v2, отклоняет same-generation конфликт
   до mutation и compact-ит `.aeo` до checkpoint-защищённого head.
11. Уже реализованный M0.9.41 заменяет ручную настройку каждой пары одной
   local-Device-signed roster policy: active recipients получают существующие
   discovery/announcement schedules, revoked recipients немедленно
   выключаются, а actor выполняет не более одного foreground workflow.
12. Уже реализованный M0.9.40 даёт exact-current sibling Devices directional
   pairwise store capabilities, recipient-HPKE публикацию свежего runtime ticket
   и автоматический fetch в runtime-managed path без общей папки.
13. Уже реализованный M0.9.39 даёт одному listener primary peer и exact-current
   own-account аудитории, stable own-device ticket и Device-signed restart-safe
   foreground schedule с network permissions, backoff и compaction.
14. Уже реализованный M0.9.38 переносит M0.9.37 recipient-encrypted bundle по
   authenticated same-account Device session, вызывает общий import gate и
   возвращает recipient-signed session-bound replay acknowledgement.
15. Уже реализованный M0.9.37 переносит bounded endpoint candidates и signed
   publication high-water между exact-current authorized own Devices через
   source-signed recipient-HPKE file, не принимая bundle как Root authority.
16. Уже реализованный M0.9.36 хранит local-device-signed expiry-independent
   publication binding и обновляет long-offline endpoint без принятия
   просроченного transport/prekey ticket.
17. Уже реализованный M0.9.35 обновляет все enrolled Device channels одной
   bounded action, хранит independent observation high-water и показывает
   desktop `usable`/`stale` состояния.
18. Уже реализованный M0.9.33 даёт self-authenticating per-peer capability
   channel и exact Ed25519 PUT authorization без Account/Device ID на store.
19. Уже реализованные M0.9.31–M0.9.32 дают opt-in foreground automation и
   crash-safe bounded compaction его signed runtime chains.
20. Уже реализованный M0.9.30 даёт self-hostable loopback-only opaque store за
   HTTPS reverse proxy: durable Redb, fixed retention, monotonic replacement,
   size/channel/connection/rate limits и Internet test procedure.
21. Уже реализованный M0.9.29 заменяет synchronized steady-state ticket refresh
   на explicit HTTPS publication/fetch: device-signed expiring chain, per-device
   HPKE slots, local rollback high-water, IPC v7 и Windows UI. Initial verified
   contact остаётся out-of-band, store traffic metadata видимы.
22. Уже реализованный M0.9.28 сохраняет device-signed receipt применённого live
   roster в vault-primary transaction, восстанавливает его раньше stale launch
   profile и bounded desktop-операцией согласует exact canonical path.
23. Optional autostart/background mode оставить отдельной явной настройкой, а не
   обязательным Task Scheduler этапом.
24. Добавить production macOS/Linux local key provider, согласованный monotonic
   witness и lifecycle обновления portable recovery package; затем mobile
   providers. Расширить pseudonymous M0.9.29 lookup до privacy-preserving
   gossip/mailbox. Live camera/clipboard оставить platform UI.
25. Спроектировать membership removal вместе с ordered security log и MLS epoch;
   отдельно — gossip/witness для first-contact freshness.
26. Спроектировать полное seed/recovery authority с monotonic history/witness,
   root rotation и конфликтующие authority operations.
27. Подготовить ADR по Iroh против rust-libp2p и проверить мобильные платформы.
28. Спроектировать финальный wire format подписанного события и алгоритм
   линеаризации.
29. Спроектировать compact Merkle/range summary и переносимый signed cursor.
30. Добавить небольшие MLS-группы.
31. Перед публичным выпуском провести независимый криптографический аудит.

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
- [`../docs/RFC-0063-roster-wide-own-device-availability.md`](../docs/RFC-0063-roster-wide-own-device-availability.md) —
  реализованный M0.9.41 local-Device-signed roster policy, transactional child reconciliation, live revocation disable и serialized foreground availability workflow.
- [`../docs/RFC-0064-convergent-sibling-publication-evidence.md`](../docs/RFC-0064-convergent-sibling-publication-evidence.md) —
  реализованный M0.9.42 transitive accepted publication evidence, fail-closed same-generation divergence и checkpointed per-channel high-water compaction.
- [`../docs/RFC-0065-durable-publication-conflict-quarantine.md`](../docs/RFC-0065-durable-publication-conflict-quarantine.md) —
  реализованный M0.9.43 detector-Device-signed conflict proof, restart-safe publication-channel quarantine и explicit IPC/Windows audit state.
- [`../docs/RFC-0066-sibling-conflict-proof-and-root-channel-rotation.md`](../docs/RFC-0066-sibling-conflict-proof-and-root-channel-rotation.md) —
  реализованный M0.9.44 exact-current sibling proof propagation, stable conflict evidence ID, ticket v11 channel epoch и portable Root-signed resolution без удаления audit proof.
- [`../docs/RFC-0067-offline-root-conflict-recovery.md`](../docs/RFC-0067-offline-root-conflict-recovery.md) —
  реализованный M0.9.45 split Device-request/offline-Root-response flow, automatic resolution propagation через bundle v4 и guided Windows incident UI без Root secret.
- [`../docs/RFC-0068-live-publication-incident-lifecycle.md`](../docs/RFC-0068-live-publication-incident-lifecycle.md) —
  реализованный M0.9.46 authenticated live channel rotation, actor-owned request/apply и единый Windows incident lifecycle без остановки runtime или доступа к Root.
- [`../docs/RFC-0069-hardened-offline-publication-conflict-ceremony.md`](../docs/RFC-0069-hardened-offline-publication-conflict-ceremony.md) —
  реализованный M0.9.47 bounded public request/response viewer, compact exact-match QR claim и mandatory KPC1 gate до загрузки offline Root.
- [`../docs/RFC-0070-minimal-offline-publication-conflict-appliance.md`](../docs/RFC-0070-minimal-offline-publication-conflict-appliance.md) —
  реализованный M0.9.48 purpose-built offline signer/viewer, dependency deny
  gate, cross-component compatibility и deterministic portable package.
- [`../docs/RFC-0071-shared-publication-conflict-artifacts.md`](../docs/RFC-0071-shared-publication-conflict-artifacts.md) —
  реализованный M0.9.49 единый network-free signed artifact/KPC1 codec для
  online и offline сторон с dependency и duplicate-definition gates.
- [`../docs/RFC-0072-reproducible-offline-release-and-bounded-cargo-cache.md`](../docs/RFC-0072-reproducible-offline-release-and-bounded-cargo-cache.md) —
  реализованный M0.9.50 same-host two-clean-root reproducibility gate, clean
  package provenance и bounded Cargo cache policy.
- [`../docs/RFC-0073-blind-mailbox-contract-and-durable-store.md`](../docs/RFC-0073-blind-mailbox-contract-and-durable-store.md) —
  реализованный M0.9.51 network-free opaque capability/envelope contract,
  bounded Redb store, signed receipts и replay-safe conditional deletion.
- [`../docs/RFC-0074-bounded-mailbox-http-and-client-ledger.md`](../docs/RFC-0074-bounded-mailbox-http-and-client-ledger.md) —
  реализованный M0.9.52 bounded paged wire/HTTPS adapter в existing service и
  crash-safe client ledger с application-commit-before-delete invariant.
- [`../docs/RFC-0075-recipient-bound-mailbox-provisioning.md`](../docs/RFC-0075-recipient-bound-mailbox-provisioning.md) —
  реализованный M0.9.53 recipient-bound Device-signed/HPKE mailbox capability
  provisioning с exact current-authority import.
- [`../docs/RFC-0076-runtime-mailbox-fallback-and-ack.md`](../docs/RFC-0076-runtime-mailbox-fallback-and-ack.md) —
  реализованный M0.9.54 direct/relay-first runtime mailbox fallback,
  application-commit-before-delete и reverse-mailbox acknowledgement.
- [`../docs/RFC-0077-authenticated-mailbox-capability-lifecycle.md`](../docs/RFC-0077-authenticated-mailbox-capability-lifecycle.md) —
  реализованный M0.9.55 Device-signed ordered mailbox capability lifecycle,
  authenticated automatic exchange, rotation/revocation и session-bound ACK.
- [`../docs/RFC-0078-transport-independent-mailbox-capability-convergence.md`](../docs/RFC-0078-transport-independent-mailbox-capability-convergence.md) —
  реализованный M0.9.56 pure signed-artifact convergence автомат и
  deterministic lost-ACK/restart/rotation/revocation regression.
- [`../docs/RFC-0079-authenticated-mailbox-desktop-control.md`](../docs/RFC-0079-authenticated-mailbox-desktop-control.md) —
  реализованный M0.9.57 authenticated IPC v23 и Windows mailbox
  activation/status/rotation/revocation без передачи capability secrets в GUI.
- [`../docs/RFC-0080-controlled-mailbox-lifecycle-field-test.md`](../docs/RFC-0080-controlled-mailbox-lifecycle-field-test.md) —
  реализованный M0.9.58 debug-only post-commit lost-ACK hook, controlled
  restart boundary и fail-closed direct/relay/mailbox evidence contract.
- [`../docs/M0.9.58-MAILBOX-LIFECYCLE-FIELD-TEST-RU.md`](../docs/M0.9.58-MAILBOX-LIFECYCLE-FIELD-TEST-RU.md) —
  пошаговый двухмашинный lifecycle test без state/IPC secrets в общей папке.
- [`../docs/RFC-0081-independent-builder-and-signed-provenance.md`](../docs/RFC-0081-independent-builder-and-signed-provenance.md) —
  реализованный M0.9.59 manual-only GitHub-hosted Windows second-builder,
  byte-equality gate и fail-closed signed provenance verifier foundation.
- [`../docs/RFC-0082-m1-acceptance-kit-and-mailbox-preflight.md`](../docs/RFC-0082-m1-acceptance-kit-and-mailbox-preflight.md) —
  реализованный M0.9.60 no-archive debug kit, stable binary hash checks,
  ordered field driver и trusted-HTTPS mailbox preflight.
- [`../docs/M0.9.60-M1-ACCEPTANCE-RU.md`](../docs/M0.9.60-M1-ACCEPTANCE-RU.md) —
  инструкция приёмки M1, включая ограниченный store-on-Alice вариант.
- [`../docs/RFC-0083-default-volunteer-blind-storage-role.md`](../docs/RFC-0083-default-volunteer-blind-storage-role.md) —
  реализованный M0.9.61 default-on embedded volunteer blind store: 200 MiB
  storage, 500 MiB/30 days на Ethernet/Wi-Fi, durable quota, mobile-off и
  честная граница до будущего P2P ingress/discovery.
- [`../docs/RFC-0084-volunteer-storage-iroh-ingress-and-offers.md`](../docs/RFC-0084-volunteer-storage-iroh-ingress-and-offers.md) —
  реализованный M0.9.62 dedicated Iroh mailbox ALPN, capability-authenticated
  PUT/LIST/DELETE, bounded provider dispatch и store-signed expiring manual
  offer без нового EXE или public TCP listener.
- [`../docs/RFC-0085-bounded-volunteer-provider-registry-and-selection.md`](../docs/RFC-0085-bounded-volunteer-provider-registry-and-selection.md) —
  реализованный M0.9.63 durable bounded signed-offer registry и per-item
  transport-distinct deterministic selection без social/capability linkage.
- [`../docs/RFC-0086-authenticated-bounded-volunteer-provider-gossip.md`](../docs/RFC-0086-authenticated-bounded-volunteer-provider-gossip.md) —
  реализованный M0.9.64 automatic short-lived offer exchange через existing
  authenticated peer sessions с count/size/age/hop/rate limits, reply binding,
  automatic own-offer refresh и без глобального directory.
- [`../docs/RFC-0087-resumable-volunteer-mailbox-replication.md`](../docs/RFC-0087-resumable-volunteer-mailbox-replication.md) —
  реализованный M0.9.65 sender-side three-target/two-receipt Iroh replication,
  durable per-dispatch random salt, exact encrypted request, independent signed
  receipts и resumable cooldown при сохранённом HTTPS compatibility delivery.
- [`../docs/RFC-0088-bounded-volunteer-mailbox-retrieval.md`](../docs/RFC-0088-bounded-volunteer-mailbox-retrieval.md) —
  реализованный M0.9.66 recipient-side bounded three-provider/one-item Iroh
  LIST/DELETE, durable application-commit-before-delete, store-specific signed
  deletion receipts и restart resume при сохранённом HTTPS fallback.
- [`../docs/RFC-0089-volunteer-mailbox-field-kit.md`](../docs/RFC-0089-volunteer-mailbox-field-kit.md) —
  успешно применённый M0.9.67 no-archive debug field kit для two-provider
  replication, sender-offline boundary, recipient Iroh commit/delete и restart
  verification при сохранённой HTTPS compatibility-копии; итоговый recovery
  evidence имеет `result=verified`.
- [`../docs/M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md`](../docs/M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md) —
  русская пошаговая инструкция внешнего M0.9.67 теста с двумя private provider
  профилями и bounded shared evidence без секретов.
- [`../docs/RFC-0090-three-folder-m0967-harness.md`](../docs/RFC-0090-three-folder-m0967-harness.md) —
  упрощённый six-launch Windows harness: `1`=Alice, `2`=два providers,
  `3`=Bob, automatic disposable identity/contact/mailbox setup и unchanged
  fail-closed M0.9.67 evidence.
- [`../docs/RFC-0091-authenticated-volunteer-replica-set-locator.md`](../docs/RFC-0091-authenticated-volunteer-replica-set-locator.md) —
  реализованный M0.9.68 recipient-selected Device-signed exact store-key
  commitment, durable per-item locator и indexed sender/recipient provider
  resolution без global directory или раскрытия social IDs providers.
- [`../docs/RFC-0092-clean-external-exact-locator-field-run.md`](../docs/RFC-0092-clean-external-exact-locator-field-run.md) —
  реализованный M0.9.69 clean-run harness: providers до mailbox activation,
  fresh live endpoint validation, обязательный capability apply/ACK, один exact
  commitment на sender/recipient, fail-closed запрет stale ticket, legacy
  sampling и provider substitution; после transport-only clean2 failure harness
  pin-ит проверенный `aps1` relay fallback при сохранённом `auto`/direct upgrade;
  clean3 run `20260919-144857` прошёл с `result=verified` и закрыл M0.9.69.
- [`../docs/RFC-0093-automatic-legacy-mailbox-upgrade.md`](../docs/RFC-0093-automatic-legacy-mailbox-upgrade.md) —
  реализованный M0.9.70 automatic upgrade acknowledged legacy receive bindings
  до exact authenticated replica set из минимум двух transport-distinct stores;
  ordered overlap сохраняется до нового ACK, message replay отсутствует,
  IPC v25 показывает locator state, HTTPS compatibility пока сохранена.
- [`../docs/RFC-0094-exact-mailbox-https-copy-retirement.md`](../docs/RFC-0094-exact-mailbox-https-copy-retirement.md) —
  реализованный M0.9.71 per-item retirement HTTPS compatibility copy: exact
  capability сначала получает два transport-distinct signed volunteer receipt,
  затем атомарно фиксируется как replicated и не вызывает HTTP; legacy,
  incomplete exact и reverse-ACK пути сохраняют совместимость.
- [`../docs/RFC-0095-clean-no-https-volunteer-field-run.md`](../docs/RFC-0095-clean-no-https-volunteer-field-run.md) —
  подготовленный M0.9.72 three-folder/six-launch field harness без
  `kilogram-ticket-store.exe`: inert unreachable descriptor, обязательные
  `http_put=not-attempted` и exact volunteer durability `2/2`, Alice offline,
  Bob commit/delete/restart и дополнительный fail-closed evidence verifier;
  первый внешний two-host run остановился до send и evidence не принят.
- [`../docs/RFC-0096-cooperative-runtime-outbound-scheduling.md`](../docs/RFC-0096-cooperative-runtime-outbound-scheduling.md) —
  реализованный M0.9.73 cooperative runtime scheduling: ровно одна owned
  outbound-cycle task, постоянно poll-able Iroh accept, IPC между cycles и
  детерминированный единственный initiator reciprocal auto-sync; replacement
  M0.9.74 run подтвердил полный двухсетевой lifecycle.
- [`../docs/RFC-0097-replication-ledger-crash-recovery.md`](../docs/RFC-0097-replication-ledger-crash-recovery.md) —
  реализованное M0.9.74 восстановление Redb replication ledger после
  принудительной остановки: только typed `RepairAborted`, state lock, vault
  dual-write, обязательный повтор read-only inspection и реальный
  child-process crash regression; clean field run `20260919-203722` принят с
  `result=verified`.
- [`../docs/RFC-0098-service-free-exact-mailbox-capability-v2.md`](../docs/RFC-0098-service-free-exact-mailbox-capability-v2.md) —
  реализованный M0.9.75 exact-volunteer capability v2 без HTTPS URL и central
  store key, с bounded чтением/миграцией v1 и запретом downgrade `v2 -> v1`.
- [`../docs/RFC-0099-service-free-v2-field-run.md`](../docs/RFC-0099-service-free-v2-field-run.md) —
  реализованный и externally verified M0.9.76 fresh three-folder/six-launch
  harness: только
  v2 exact activation, отсутствие compatibility tuple/fixture, обязательные
  exact receipts `2/2`, sender-offline, commit-before-delete и restart без
  redelivery; retry run `20260920-001614` принят с `result=verified`.
- [`../docs/RFC-0100-toolchain-bundled-lld-independent-reproduction.md`](../docs/RFC-0100-toolchain-bundled-lld-independent-reproduction.md) —
  реализованный локально M0.9.77 controlled-linker contract: local/GitHub
  builders используют exact `rust-lld.exe` из pinned Rust toolchain, format-v3
  records связывают linker hash, BLAKE3 pure-Rust codegen и bounded PE metadata
  normalization v1 с artifact, mismatch fail closed; local format-v3 pair для
  `788a3c4...` verified; external run с exact same LLD корректно обнаружил
  оставшееся structural divergence и передал диагностику RFC-0101.
- [`../docs/RFC-0101-exact-native-link-input-provenance.md`](../docs/RFC-0101-exact-native-link-input-provenance.md) —
  реализованный и externally diagnosed M0.9.78 exact native link-input provenance: final
  `cargo rustc` link создаёт transient LLD `/reproduce` archive, сохраняет
  только bounded `NATIVE-LINK-INPUTS.sha256` с exact hashes/lengths реально
  использованных Windows SDK/MSVC libraries и немедленно удаляет большой TAR;
  format-v4 local/GitHub records и verifier требуют identical manifest; run
  `35471719378` доказал exact MSVC/SDK divergence.
- [`../docs/RFC-0102-hash-locked-windows-native-toolchain.md`](../docs/RFC-0102-hash-locked-windows-native-toolchain.md) —
  реализованный локально M0.9.79 hash-locked native-toolchain boundary: exact
  MSVC 14.44/Windows SDK 19041 libraries проверяются до build, явно выбираются
  final `rustc -L native`, повторно подтверждаются LLD manifest и связываются с
  format-v5 evidence без vendoring Microsoft binaries.
- [`../docs/RFC-0103-canonical-cargo-registry-path-remapping.md`](../docs/RFC-0103-canonical-cargo-registry-path-remapping.md) —
  принятый M0.9.80 dual path-remap boundary: checkout и Cargo registry получают
  разные canonical virtual roots, post-build PE gate требует canonical marker
  и запрещает raw host Cargo path; run `35474774356` подтвердил byte-identical
  format-v6 artifact и создал attestation `48691093`.
- [`../docs/RFC-0104-m1-candidate-evidence-composition.md`](../docs/RFC-0104-m1-candidate-evidence-composition.md) —
  M0.9.81 fail-closed composition accepted M0.9.76 field evidence и M0.9.80
  independent attestation: exact Git surfaces, ancestor/dirty-tree guards,
  обязательный residual-risk список и no-network/no-build/no-archive boundary.
- [`../docs/RFC-0105-sybil-costed-provider-admission.md`](../docs/RFC-0105-sybil-costed-provider-admission.md) —
  M0.9.82 bounded 18-bit provider-offer admission work: proof привязан к
  store/endpoint/capacity/lifetime, low-work offers исключаются из новых
  replica sets и gossip, legacy exact resolution сохраняется; cost floor не
  выдаётся за operator independence или полную Sybil resistance.
- [`../docs/RFC-0106-local-authenticated-provider-observation-provenance.md`](../docs/RFC-0106-local-authenticated-provider-observation-provenance.md) —
  M0.9.83 local-only provenance: authenticated peer session превращается в
  keyed pseudonymous tag, exact offer хранит не более восьми distinct tags,
  replacement очищает evidence; wire/IPC не несут tags или social IDs и
  M0.9.83 provider ranking ещё не менял.
- [`../docs/RFC-0107-bootstrap-safe-provider-selection.md`](../docs/RFC-0107-bootstrap-safe-provider-selection.md) —
  M0.9.84 binary locally-corroborated preference поверх 18-bit admission:
  observation count сверх одного не даёт rank, unobserved qualified offers
  сохраняют bootstrap fallback, exact commitments и IPC v26 не меняются.
- [`../docs/RFC-0108-local-provider-path-domain-provenance.md`](../docs/RFC-0108-local-provider-path-domain-provenance.md) —
  M0.9.85 local post-exchange path provenance: verified successful mailbox
  response даёт keyed exact-IP/relay-origin tag, exact-offer replacement и
  local key epoch ограничивают lifetime/comparability; raw addresses/tags не
  передаются, selection и IPC v26 не меняются.
- [`../docs/RFC-0109-positive-provider-path-colocation-avoidance.md`](../docs/RFC-0109-positive-provider-path-colocation-avoidance.md) —
  M0.9.86 positive-only path policy: known equal local domains откладываются в
  первом проходе, path-unknown candidates остаются допустимыми, deferred
  candidates возвращаются для availability; unequal tags не объявляются
  operator independence, exact commitments и IPC v26 не меняются.
- [`../docs/RFC-0110-independent-provider-field-contract.md`](../docs/RFC-0110-independent-provider-field-contract.md) —
  M0.9.87 four-folder controlled field contract: два provider процесса живут
  на разных физических/network/operator domains, публикуют только run-scoped
  hashed attestations и closed logs; exact service-free lifecycle наследуется,
  но external three-/four-host run ещё ожидается.
- [`../docs/RFC-0111-host-neutral-cargo-identity-verification.md`](../docs/RFC-0111-host-neutral-cargo-identity-verification.md) —
  M0.9.88 исправляет cross-Windows verifier boundary: exact Cargo
  version/commit/target/libgit2/libcurl остаются byte-equal, а валидная
  Windows 11/Server OS-release строка проверяется как runner metadata; changed
  Cargo identity fail closed.
- [`../docs/RFC-0112-current-m1-candidate-evidence-refresh.md`](../docs/RFC-0112-current-m1-candidate-evidence-refresh.md) —
  M0.9.89 refresh M1 candidate: current M0.9.87 reduced field run и принятая
  M0.9.88 attestation связаны exact selected-tree continuity, а strict provider
  independence остаётся явным residual risk.
- [`../docs/RFC-0113-complete-m1-readiness-gate.md`](../docs/RFC-0113-complete-m1-readiness-gate.md) —
  M0.9.90 closed inventory всех fail-closed boundaries плюс production
  candidate, format и offline strict Clippy перед решением о M1 tag.
- [`../docs/RFC-0114-continuous-windows-readiness-and-tests.md`](../docs/RFC-0114-continuous-windows-readiness-and-tests.md) —
  M0.9.91 read-only hosted Windows CI для readiness gate и полного запуска
  stable-path Rust tests без release/publication.
- [`../docs/RFC-0115-windows-extended-length-runtime-journal.md`](../docs/RFC-0115-windows-extended-length-runtime-journal.md) —
  M0.9.92 исправляет deterministic Windows MAX_PATH failure append-only runtime
  journal и отделяет stale evidence readiness от обязательного CI test feedback.
- [`../docs/RFC-0116-fresh-field-evidence-and-candidate-refresh.md`](../docs/RFC-0116-fresh-field-evidence-and-candidate-refresh.md) —
  M0.9.93 pin-ит fresh post-fix physical run и новый exact runtime surface,
  сохраняя неизменённую independent offline attestation и все residual risks.
- [`../docs/RFC-0117-hosted-test-completion-and-empty-cache-accounting.md`](../docs/RFC-0117-hosted-test-completion-and-empty-cache-accounting.md) —
  M0.9.94 отделяет полный hosted Rust pass от post-test cache-report failure и
  закрывает Windows PowerShell empty-tree regression динамическим self-test.
- [`../docs/RFC-0118-hosted-isolated-test-scheduling.md`](../docs/RFC-0118-hosted-isolated-test-scheduling.md) —
  M0.9.95 запускает exact isolated restart test до aggregate harnesses и с
  normal hosted priority; run `38531172733` доказал, что этого недостаточно.
- [`../docs/RFC-0119-task-aware-runtime-test-readiness.md`](../docs/RFC-0119-task-aware-runtime-test-readiness.md) —
  M0.9.96 вводит bounded task-aware IPC readiness для lifecycle test без
  изменения production runtime policy или retry masking.
- [`../docs/RFC-0120-configured-relay-evidence.md`](../docs/RFC-0120-configured-relay-evidence.md) —
  M0.9.97 разделяет configured relay pin и фактически наблюдаемый relay home:
  auto runtime всегда доказывает exact конфигурацию, но не выдаёт timeout за
  успешную relay registration.
- [`../docs/RFC-0121-current-m1-candidate-refresh.md`](../docs/RFC-0121-current-m1-candidate-refresh.md) —
  M0.9.98 связывает fresh reduced field run с unchanged independent offline
  reproduction, exact selected-tree manifests и неизменным residual-risk list.
- [`../docs/RFC-0122-hosted-windows-readiness-acceptance.md`](../docs/RFC-0122-hosted-windows-readiness-acceptance.md) —
  M0.9.99 принимает fresh green GitHub Windows CI: readiness `33/33`, isolated
  lifecycle `1/1` и все 276 aggregate tests без release/tag side effects.
- [`../docs/RFC-0123-m1-technical-poc-acceptance.md`](../docs/RFC-0123-m1-technical-poc-acceptance.md) —
  фиксирует M1.0.0 как Windows technical PoC, сохраняет шесть residual risks и
  разрешает annotated tag только на exact green acceptance commit.
- [`../docs/RFC-0124-post-m1-immutable-acceptance-and-continuous-ci.md`](../docs/RFC-0124-post-m1-immutable-acceptance-and-continuous-ci.md) —
  отделяет immutable historical acceptance `M1.0.0` от current regression:
  post-M1 CI pin-ит exact tag и отдельно проверяет актуальные boundaries,
  format, strict Clippy и Rust tests без переписывания старой evidence.
- [`../docs/RFC-0125-m11-threat-model-and-security-claims.md`](../docs/RFC-0125-m11-threat-model-and-security-claims.md) —
  формальная M1.1 threat model: assets, adversaries, trust boundaries, threat
  register, current claims и explicit non-guarantees без production/audit
  переоценки M1.0.0.
- [`../docs/RFC-0126-m11-protocol-compatibility-baseline.md`](../docs/RFC-0126-m11-protocol-compatibility-baseline.md) —
  machine-readable experimental compatibility baseline: 20 families, 155
  exact version/ALPN/domain bindings, explicit migration rules и новый
  fail-closed readiness boundary; active Postcard wire остаётся provisional.
- [`../docs/RFC-0127-m111-canonical-wire-and-golden-vectors.md`](../docs/RFC-0127-m111-canonical-wire-and-golden-vectors.md) —
  inactive deterministic-CBOR candidate: пять positional schemas, два golden
  byte/Ed25519/BLAKE3 vectors, Rust verification и независимая PowerShell
  перекодировка без скрытого переключения active Postcard wire.
- [`../docs/RFC-0128-m112-canonical-authority-and-session.md`](../docs/RFC-0128-m112-canonical-authority-and-session.md) —
  inactive CDDL authority/session chain: Account/Device IDs, Root-signed
  certificate/revocation/snapshot, session-bound Device signature, строгий Rust
  decode/verify и независимое JavaScript/Node воспроизведение Ed25519.
- [`../docs/RFC-0166-m1140-canonical-runtime-ipc-own-device-announcement-automation.md`](../docs/RFC-0166-m1140-canonical-runtime-ipc-own-device-announcement-automation.md) —
  M1.1.40 канонизирует inactive foreground-only endpoint announcement policy
  одному sibling Device без ticket plaintext, local paths или ложного OS
  background service.
- [`../docs/RFC-0167-m1141-canonical-runtime-ipc-own-device-ticket-discovery.md`](../docs/RFC-0167-m1141-canonical-runtime-ipc-own-device-ticket-discovery.md) —
  M1.1.41 канонизирует pairwise own-device ticket discovery, directional
  channels и atomic locator-plus-announcement install без передачи derived
  private capabilities через IPC.
- [`../docs/RFC-0168-m1142-canonical-runtime-ipc-own-device-roster-automation.md`](../docs/RFC-0168-m1142-canonical-runtime-ipc-own-device-roster-automation.md) —
  M1.1.42 канонизирует roster-wide foreground orchestration: exact active
  sibling projection, retained revoked tombstones и serialized child workflow
  без ticket plaintext, local paths или ослабления decoder depth limit.
- [`../docs/RFC-0169-m1143-canonical-runtime-ipc-recovery-plan-custody.md`](../docs/RFC-0169-m1143-canonical-runtime-ipc-recovery-plan-custody.md) —
  M1.1.43 канонизирует approval/status/cancellation локального recovery plan:
  full SAS confirmation, Device-signed plan, content-addressed custody и
  immutable cancellation tombstone без private capabilities или background
  service.
- [`../docs/RFC-0170-m1144-canonical-runtime-ipc-recovery-execution.md`](../docs/RFC-0170-m1144-canonical-runtime-ipc-recovery-execution.md) —
  M1.1.44 канонизирует start/status approved recovery plan, Device-signed
  append-only scheduler chain, exact deterministic retry и checkpoint-bound
  progress без caller-supplied outcomes или OS background service.
- [`../docs/RFC-0171-m1145-active-runtime-ipc-conversation-pagination.md`](../docs/RFC-0171-m1145-active-runtime-ipc-conversation-pagination.md) —
  M1.1.45 мигрирует active loopback IPC на Postcard v27/descriptor v8 и
  bounded exact-cursor pagination до дорогой conversation projection, сохраняя
  peer-network wire и inactive canonical CBOR неизменными.
- [`../docs/RFC-0172-m1146-runtime-ipc-completion-inventory.md`](../docs/RFC-0172-m1146-runtime-ipc-completion-inventory.md) —
  M1.1.46 вводит exhaustive command/result и active-adapter inventory, заменяет
  catch-all тремя точными remaining items и fail closed фиксирует семь
  Postcard-only blockers без активации canonical CBOR.
- [`../docs/RFC-0173-m1147-canonical-runtime-ipc-ticket-automation.md`](../docs/RFC-0173-m1147-canonical-runtime-ipc-ticket-automation.md) —
  M1.1.47 канонизирует foreground-only ticket automation policy/status, exact
  predecessor и детерминированные scheduler deadlines без ticket plaintext,
  mailbox semantics или OS background service.
- [`../docs/RFC-0174-m1148-active-runtime-ipc-endpoint-bootstrap-retirement.md`](../docs/RFC-0174-m1148-active-runtime-ipc-endpoint-bootstrap-retirement.md) —
  M1.1.48 удаляет ручные file-path export/import/push endpoint-announcement
  команды из active loopback IPC, переводит Postcard на v28/descriptor v9 и
  сохраняет authenticated peer wire и foreground own-device automation.
- [`../docs/RFC-0175-m1149-active-runtime-ipc-legacy-mailbox-retirement.md`](../docs/RFC-0175-m1149-active-runtime-ipc-legacy-mailbox-retirement.md) —
  M1.1.49 удаляет central-HTTPS mailbox create/rotate из active loopback IPC,
  сохраняет exact volunteer create/rotate и legacy read/upgrade и завершает
  canonical runtime-IPC target через hard cut Postcard v29/descriptor v10.
- [`../docs/RFC-0176-m1150-canonical-sync-alpn-allocation.md`](../docs/RFC-0176-m1150-canonical-sync-alpn-allocation.md) —
  M1.1.50 резервирует отдельный exact ALPN canonical-CBOR sync, но намеренно не
  регистрирует и не dial-ит его до authenticated capability negotiation.
- [`../docs/RFC-0177-m1151-authenticated-sync-transport-capability.md`](../docs/RFC-0177-m1151-authenticated-sync-transport-capability.md) —
  M1.1.51 переносит Device-signed legacy-active/canonical-candidate capability
  в connection ticket v12, exact-связывая authority, endpoint, requester,
  publication epoch и prekey validity без регистрации canonical ALPN.
- [`../docs/RFC-0178-m1152-canonical-sync-historical-signature-bridge.md`](../docs/RFC-0178-m1152-canonical-sync-historical-signature-bridge.md) —
  M1.1.52 сохраняет и повторно проверяет exact Postcard bytes исторического
  `AuthorizedEvent`, не требуя ключей старых авторов, и блокирует activation
  closed canonical sync v1 до нового history-compatible generation.
- [`../docs/RFC-0179-m1153-typed-history-compatible-sync-event-carrier.md`](../docs/RFC-0179-m1153-typed-history-compatible-sync-event-carrier.md) —
  M1.1.53 выделяет inactive `/cbor/2`, вводит namespace-aware Event key и
  closed verified carrier для legacy bridge/native canonical event, оставляя
  frames, ticket capability и runtime activation pending.
- [`../docs/RFC-0180-m1154-history-compatible-canonical-sync-v2-frames.md`](../docs/RFC-0180-m1154-history-compatible-canonical-sync-v2-frames.md) —
  M1.1.54 закрывает полный inactive `/cbor/2` inventory/diff/batch/complete и
  pause/reject frame set поверх typed history, оставляя typed durable checkpoint,
  ticket capability и runtime activation pending.
- [`../docs/RFC-0181-m1155-typed-canonical-sync-v2-session-checkpoint.md`](../docs/RFC-0181-m1155-typed-canonical-sync-v2-session-checkpoint.md) —
  M1.1.55 добавляет namespace-safe durable checkpoint для exact `/cbor/2`:
  protocol identifier подписан, evidence хранит typed Event keys, chain
  fail closed проверяет predecessor/round/pause/terminal transitions; ticket
  capability и runtime activation остаются pending.
- [`../docs/RFC-0182-m1156-authenticated-history-compatible-sync-capability.md`](../docs/RFC-0182-m1156-authenticated-history-compatible-sync-capability.md) —
  M1.1.56 переводит portable ticket v12→v13 и signed sync capability v1→v2,
  аутентифицирует exact `/cbor/2` candidate и fail closed отвергает `/cbor/1`,
  не регистрируя и не dial-я canonical ALPN.
- [`../docs/RFC-0183-m1157-dormant-history-compatible-sync-runtime-adapter.md`](../docs/RFC-0183-m1157-dormant-history-compatible-sync-runtime-adapter.md) —
  M1.1.57 добавляет dormant transport-independent `/cbor/2` client/server
  state machine с full typed-key reconciliation и mandatory pre-storage policy,
  но не подключает active store/checkpoint и не регистрирует/dial-ит ALPN.
- [`../docs/RFC-0184-m1158-dormant-atomic-typed-sync-store.md`](../docs/RFC-0184-m1158-dormant-atomic-typed-sync-store.md) —
  M1.1.58 добавляет отдельную atomic redb БД с full typed storage key, strict
  carrier validation и fail-closed batch conflict, не импортируя active legacy
  history и не подключая `/cbor/2` к runtime.
- [`../docs/RFC-0185-m1159-dormant-cross-format-membership-policy.md`](../docs/RFC-0185-m1159-dormant-cross-format-membership-policy.md) —
  M1.1.59 добавляет dormant concrete membership policy: legacy Postcard и
  canonical-CBOR membership snapshots независимо авторизуют оба closed carrier
  формата без sniffing, fallback и подключения `/cbor/2` к active runtime.
- [`../docs/RFC-0186-m1160-dormant-bounded-legacy-history-import.md`](../docs/RFC-0186-m1160-dormant-bounded-legacy-history-import.md) —
  M1.1.60 добавляет dormant bounded/resumable import из active filesystem
  `EventStore` в отдельный typed carrier store: exact legacy bytes сохраняются
  в bridge, plan/cursor exact-связаны, prefix skip и changed claims fail closed,
  но durable cursor/checkpoint/runtime activation остаются отдельным этапом.
- [`../docs/RFC-0187-m1161-dormant-atomic-legacy-import-checkpoint.md`](../docs/RFC-0187-m1161-dormant-atomic-legacy-import-checkpoint.md) —
  M1.1.61 сохраняет plan-scoped import checkpoint в той же redb transaction,
  что и carrier page: crash/retry не может показать advanced cursor без полной
  page, stale/skipped/corrupt state fail closed. Это локальный migration cursor,
  а не signed peer-session checkpoint; `/cbor/2` всё ещё inactive.
- [`../docs/RFC-0188-m1162-dormant-signed-session-bootstrap.md`](../docs/RFC-0188-m1162-dormant-signed-session-bootstrap.md) —
  M1.1.62 разрешает первый Device-signed `/cbor/2` checkpoint только после
  завершения exact durable import plan, атомарно хранит immutable sequence-1
  record и head и fail closed проверяет signature/ID/key/head relation. Это
  gated dormant bootstrap; successor checkpoints и mutual capability selection
  ещё не подключены, `/cbor/2` не зарегистрирован и не dial-ится.
- [`../docs/RFC-0189-m1163-dormant-signed-session-successor-store.md`](../docs/RFC-0189-m1163-dormant-signed-session-successor-store.md) —
  M1.1.63 атомарно добавляет Device-signed successor checkpoints в тот же
  immutable journal и после reopen проверяет всю bounded chain от exact
  sequence-1 bootstrap до head. Gap/fork/missing/corrupt ancestry fail closed;
  runtime transitions и mutual capability selection ещё не подключены.
- [`../docs/RFC-0190-m1164-checkpoint-bound-runtime-frame-journal.md`](../docs/RFC-0190-m1164-checkpoint-bound-runtime-frame-journal.md) —
  M1.1.64 exact-связывает pending canonical client frame с текущим verified
  signed checkpoint в отдельной versioned redb table. Bootstrap/successor
  атомарно создают или rebind-ят runtime record, frame write-once и strict
  reverified после reopen; runtime transition/resume ещё не подключены.
- [`../docs/RFC-0191-m1165-atomic-runtime-transition-transaction.md`](../docs/RFC-0191-m1165-atomic-runtime-transition-transaction.md) —
  M1.1.65 атомарно коммитит verified received carriers, signed successor,
  session head и exact successor-bound pending frame. Предыдущий runtime служит
  optimistic concurrency token; exact retry идемпотентен, conflict откатывает
  всю transaction. Adapter/resume/capability selection ещё не подключены.
- [`../docs/RFC-0192-m1166-durable-client-transition-adapter.md`](../docs/RFC-0192-m1166-durable-client-transition-adapter.md) —
  M1.1.66 подключает atomic transition к dormant canonical client: Inventory и
  EventBatch сохраняются до send, restart возвращает exact pending frame, а
  continuation/terminal completion опирается на signed cumulative evidence.
- [`../docs/RFC-0193-user-sovereignty-and-contextual-identity.md`](../docs/RFC-0193-user-sovereignty-and-contextual-identity.md) —
  фиксирует продуктовые принципы Telegram-знакомого UX, отсутствия публичного
  вечного User ID, context-scoped anti-abuse и local-owner контроля над уже
  полученной append-only историей, версиями edit и retract.
- [`../docs/RFC-0194-m1167-durable-responder-transition-adapter.md`](../docs/RFC-0194-m1167-durable-responder-transition-adapter.md) —
  M1.1.67 добавляет отдельный responder-signed checkpoint/journal и atomic
  server transitions: exact Diff/Complete переживают restart, а incoming batch
  коммитится вместе с successor/head/frame до ответа.
- [`../docs/RFC-0195-m1168-mutual-authenticated-candidate-selection.md`](../docs/RFC-0195-m1168-mutual-authenticated-candidate-selection.md) —
  M1.1.68 связывает listener capability, requester-signed offer,
  listener-signed acceptance, обе authority/session chains и durable
  client/responder identities, но намеренно не разрешает ALPN registration или
  dial.
- [`../docs/RFC-0196-m1169-mixed-version-downgrade-matrix.md`](../docs/RFC-0196-m1169-mixed-version-downgrade-matrix.md) —
  M1.1.69 формализует все dormant mixed-version outcomes: canonical требует
  mutual selection, legacy после неё считается downgrade, epoch rollback и
  error-driven fallback запрещены; transport activation всё ещё не выдаётся.
- [`../docs/RFC-0197-m1170-dormant-dual-reader-runtime-coordinator.md`](../docs/RFC-0197-m1170-dormant-dual-reader-runtime-coordinator.md) —
  M1.1.70 связывает verified mixed-version plan с exact durable client и
  responder restart actions, сохраняя legacy dispatch символическим и не
  выдавая разрешения на transport activation.
- [`../docs/RFC-0198-m1171-deterministic-rollback-drill.md`](../docs/RFC-0198-m1171-deterministic-rollback-drill.md) —
  M1.1.71 добавляет строгую persisted snapshot/record модель и доказывает
  детерминированный one-step rollback после restart, запрещая automatic
  error fallback и rollback после selected либо progressed canonical session.
- [`../docs/RFC-0199-m1172-canonical-sync-v2-decoder-mutations.md`](../docs/RFC-0199-m1172-canonical-sync-v2-decoder-mutations.md) —
  M1.1.72 закрывает bounded deterministic mutation regression полного
  history-compatible sync v2 decoder surface: 20 exact fixtures и 2863
  structural/authenticated mutations без network, randomness или ложной
  декларации coverage-guided fuzzing.
- [`../docs/RFC-0200-m1173-canonical-sync-v2-coverage-fuzz-harness.md`](../docs/RFC-0200-m1173-canonical-sync-v2-coverage-fuzz-harness.md) —
  M1.1.73 добавляет opt-in cargo-fuzz/libFuzzer harness для всех 13 sync v2
  strict decoders, materialized raw-binary seed corpus и bounded one-worker
  campaign contract, не выдавая отсутствующую instrumented campaign за
  пройденную и не активируя `/cbor/2`.
- [`../docs/RFC-0201-m1174-pinned-fuzz-toolchain.md`](../docs/RFC-0201-m1174-pinned-fuzz-toolchain.md) —
  M1.1.74 закрепляет exact nightly compiler, cargo-fuzz, libFuzzer dependency
  lock и Windows runtime component; instrumented target реально собран, но
  smoke/sustained execution и coverage пока намеренно не заявлены.
- [`../docs/RFC-0202-m1175-instrumented-fuzz-smoke.md`](../docs/RFC-0202-m1175-instrumented-fuzz-smoke.md) —
  M1.1.75 фиксирует clean-source пятиминутный instrumented run, ноль generated
  artifacts и валидный coverage profile, сохраняя восьмичасовой gate и
  transport activation открытыми.
- [`../docs/RFC-0203-m1176-sustained-fuzz-campaign.md`](../docs/RFC-0203-m1176-sustained-fuzz-campaign.md) —
  M1.1.76 фиксирует clean-source восьмичасовой instrumented run: 469 853 771
  executions, ноль generated artifacts и валидный coverage profile. Sustained
  decoder gate закрыт, fresh hosted/physical-device evidence остаётся pending.
- [`../docs/RFC-0204-m1177-m1180-canonical-sync-v2-network-evidence.md`](../docs/RFC-0204-m1177-m1180-canonical-sync-v2-network-evidence.md) —
  M1.1.77–M1.1.80 фиксирует feature-gated evidence lane и принятые hosted и
  двухмашинные Windows manifests одной frozen revision. Полная network matrix
  закрывает последний технический gate, но оставляет `/cbor/2` выключенным до
  отдельного explicit activation decision.
- [`../docs/RFC-0205-m1181-canonical-sync-v2-activation-semantics.md`](../docs/RFC-0205-m1181-canonical-sync-v2-activation-semantics.md) —
  M1.1.81 разделяет publish/register/accept/initiate/resume права и определяет
  emergency containment как сохранение exact canonical resume без новых v2
  sessions либо legacy fallback. Active runtime и default пока не меняются.
- [`../docs/RFC-0206-m1182-durable-canonical-sync-v2-activation-authority.md`](../docs/RFC-0206-m1182-durable-canonical-sync-v2-activation-authority.md) —
  M1.1.82 добавляет Device-signed append-only activation chain, exact
  plan/apply, signed atomic high-water и fail-closed restart/crash/rollback
  boundary. Authority уже durable, но Iroh runtime ещё её не потребляет.
- [`../docs/RFC-0207-m1183-production-runtime-canonical-sync-v2-dark-launch.md`](../docs/RFC-0207-m1183-production-runtime-canonical-sync-v2-dark-launch.md) —
  M1.1.83 подключает canonical adapter к обычному `kilogram-cli` только через
  verified Device policy, сохраняет default legacy-only и запрещает Postcard
  fallback после mutually signed durable selection.
- [`../docs/RFC-0208-m1184-canonical-sync-v2-resilience-rehearsal.md`](../docs/RFC-0208-m1184-canonical-sync-v2-resilience-rehearsal.md) —
  M1.1.84 проверяет restart/resume, selected-v2 loss без fallback,
  pre-evidence rollback и post-selection containment; exact-revision run на
  двух физических Windows-хостах принят, public default остаётся legacy-only.
- [`../docs/RFC-0209-m1185-canonical-sync-v2-opt-in-dark-launch-acceptance.md`](../docs/RFC-0209-m1185-canonical-sync-v2-opt-in-dark-launch-acceptance.md) —
  M1.1.85 агрегирует authority, production integration и двухмашинное evidence,
  фиксирует прошедшие 112 readiness gates и полный regression кроме двух
  неизменённых принятых mutation corpora. Статус — verified opt-in dark launch,
  не public activation.
- [`../docs/RFC-0210-m1186-desktop-canonical-sync-v2-activation.md`](../docs/RFC-0210-m1186-desktop-canonical-sync-v2-activation.md) —
  M1.1.86 добавляет в Windows desktop offline-only status/plan/apply церемонию
  существующей Device-signed authority: exact plan confirmation, независимую
  сверку rights matrix и обязательный restart без изменения public default.
- [`../docs/RFC-0211-m1187-canonical-sync-v2-runtime-observability.md`](../docs/RFC-0211-m1187-canonical-sync-v2-runtime-observability.md) —
  M1.1.87 раздельно показывает loaded/configured activation policy и максимум
  32 process-local наблюдения legacy/canonical sync path через read-only IPC
  v30/domain v11; authority, peer wire и public default не меняются.
- [`../docs/RFC-0212-m1188-canonical-sync-v2-guided-restart.md`](../docs/RFC-0212-m1188-canonical-sync-v2-guided-restart.md) —
  M1.1.88 требует actual child exit, offline exact apply и authenticated exact
  verification fresh runtime до conversation refresh.
- [`../docs/RFC-0213-m1189-canonical-sync-v2-public-rollout-decision.md`](../docs/RFC-0213-m1189-canonical-sync-v2-public-rollout-decision.md) —
  M1.1.89 удерживает public rollout в `hold` и формализует ordered cohorts,
  health, interop, containment и review gates.
- [`../docs/RFC-0214-m1190-rollout-local-authority.md`](../docs/RFC-0214-m1190-rollout-local-authority.md) —
  M1.1.90 реализует dormant installation-local cohort/release-policy authority
  и bounded explicit-only health export без runtime integration либо user ID.
- [`../docs/RFC-0215-m1191-n-minus-one-production-interop.md`](../docs/RFC-0215-m1191-n-minus-one-production-interop.md) —
  M1.1.91 подтверждает exact M1.0.0/current interop в обоих направлениях по
  direct/relay и forward upgrade одного durable state; public default остаётся
  legacy-only, а полевые EXE используют постоянные firewall paths.
- [`../docs/RFC-0231-m1207-peer-capability-two-host-rehearsal.md`](../docs/RFC-0231-m1207-peer-capability-two-host-rehearsal.md) —
  M1.2.7 подтверждает на двух физических Windows-хостах legacy/no-create,
  durable quarantine до endpoint после stripping и persisted canonical resume
  без fallback одним exact debug binary; public default остаётся legacy-only.
- [`../docs/RFC-0232-m1208-automated-runtime-enforcement.md`](../docs/RFC-0232-m1208-automated-runtime-enforcement.md) —
  M1.2.8 применяет тот же durable peer capability floor к runtime outbox и
  scheduled sync до materialization/network, исключает busy retry и starvation
  других peers, а после restart заново проверяет durable quarantine; wire/IPC и
  public default не меняются.
- [`../docs/RFC-0233-m1209-runtime-quarantine-observability.md`](../docs/RFC-0233-m1209-runtime-quarantine-observability.md) —
  M1.2.9 добавляет bounded read-only IPC v31/domain v12 и Windows-панель,
  раздельно показывающие durable quarantine, process-local suppression и
  обычный network retry без raw peer/queue IDs, ticket, endpoint, message либо
  clear/retry/recovery authority; missing store остаётся no-create.
- [`../docs/RFC-0234-m1210-device-authorized-peer-reenrollment-plan.md`](../docs/RFC-0234-m1210-device-authorized-peer-reenrollment-plan.md) —
  M1.2.10 создаёт bounded offline Device-signed recovery plan из exact
  quarantine, обеих Root authority и fresh current invitation без clear/apply
  или network authority.
- [`../docs/RFC-0235-m1211-durable-peer-reenrollment-acceptance.md`](../docs/RFC-0235-m1211-durable-peer-reenrollment-acceptance.md) —
  M1.2.11 атомарно фиксирует current-Device-signed acceptance receipt и
  единственный successor head, сохраняя старую capability DB byte-exact.
- [`../docs/RFC-0236-m1212-fresh-start-peer-reenrollment-consumption.md`](../docs/RFC-0236-m1212-fresh-start-peer-reenrollment-consumption.md) —
  M1.2.12 потребляет полный accepted successor только на fresh runtime start и
  до Iroh endpoint, применяя его как overlay без перезаписи predecessor.
- [`../docs/RFC-0237-m1213-peer-reenrollment-two-host-rehearsal.md`](../docs/RFC-0237-m1213-peer-reenrollment-two-host-rehearsal.md) —
  M1.2.13 подтверждает весь flow на двух физических Windows-хостах: explicit
  history rewrap, fail-closed descriptor substitution, exactly-once delivery,
  idempotent restart и equal eight-event histories одним exact debug binary.
- [`../docs/RFC-0238-m1214-canonical-runtime-ipc-peer-reenrollment-contract.md`](../docs/RFC-0238-m1214-canonical-runtime-ipc-peer-reenrollment-contract.md) —
  M1.2.14 задаёт dormant bounded signed IPC contract для раздельных
  inspect/plan/approve/accept/status стадий, privacy-safe UI projection и
  отдельного old-history rewrap status, не меняя active runtime IPC v31.
- [`../docs/RFC-0243-m1219-durable-old-history-recovery-consent.md`](../docs/RFC-0243-m1219-durable-old-history-recovery-consent.md) —
  M1.2.19 сохраняет exact source-owner consent в отдельной Account/Device-bound
  immediate-durability redb. First commit повторно проверяет весь live context,
  exact replay после expiry идемпотентен, а competing/fork/orphan/corruption
  fail closed; status body-free и не даёт execution authority.
- [`../docs/RFC-0244-m1220-bounded-single-source-history-recovery-execution.md`](../docs/RFC-0244-m1220-bounded-single-source-history-recovery-execution.md) —
  M1.2.20 выполняет durable consent локальными chunks до 32 events через
  существующий authenticated history-rewrap. Recipient-signed checkpoint и
  source-signed append-only journal дают crash-safe resume/exact replay;
  plaintext не сохраняется, а production import/network/multi-source/IPC/GUI
  остаются за границей этапа.
- [`../docs/RFC-0245-m1221-atomic-recipient-history-store-import.md`](../docs/RFC-0245-m1221-atomic-recipient-history-store-import.md) —
  M1.2.21 атомарно публикует exact committed recovery chunk в обычные event и
  encrypted projection stores получателя. Current recipient Device подписывает
  линейную receipt chain; conflict, stale unimported head, partial crash,
  symlink и corruption fail closed, network/IPC/GUI не добавлены.
- [`../docs/RFC-0246-m1222-authenticated-history-recovery-transfer-inbox.md`](../docs/RFC-0246-m1222-authenticated-history-recovery-transfer-inbox.md) —
  M1.2.22 отделяет source execution journal от recipient state через bounded
  canonical source-Device-signed envelope. Recipient inbox атомарно сохраняет
  exact encrypted artifacts и линейный backlog, а импорт остаётся под властью
  recipient-signed receipt chain; network/IPC/GUI не добавлены.
- [`../docs/RFC-0247-m1223-authenticated-iroh-history-transfer-session.md`](../docs/RFC-0247-m1223-authenticated-iroh-history-transfer-session.md) —
  M1.2.23 добавляет отдельную bounded authenticated Iroh-сессию для exact
  M1.2.22 envelope. Request связывает оба Endpoint ID, Device, nonce и transfer;
  durable stage предшествует signed response, exact retry идемпотентен, а
  active runtime/IPC/desktop по-прежнему не получают эту authority.
- [`../docs/RFC-0248-m1224-durable-history-transfer-outbox.md`](../docs/RFC-0248-m1224-durable-history-transfer-outbox.md) —
  M1.2.24 сохраняет exact signed request до сети и exact signed receipt после
  неё в bounded immediate-durability outbox. Restart не меняет nonce/Endpoint
  bindings; competing intent/receipt и partial commit fail closed, network
  executor/runtime/IPC/desktop ещё не подключены.
- [`../docs/RFC-0249-m1225-explicit-restart-safe-history-transfer-executor.md`](../docs/RFC-0249-m1225-explicit-restart-safe-history-transfer-executor.md) —
  M1.2.25 явно выполняет одну durable pending передачу: до сети сверяет оба
  Endpoint binding, bounded retry отправляет только exact stored request, а
  success возвращается после durable signed receipt. Exhaustion сохраняет
  pending, acknowledged short-circuit не открывает сеть; background/runtime/
  IPC/desktop authority не добавлены.
- [`../docs/RFC-0250-m1226-explicit-bounded-recipient-acceptor.md`](../docs/RFC-0250-m1226-explicit-bounded-recipient-acceptor.md) —
  M1.2.26 добавляет явно вызываемый recipient acceptor над caller-owned
  endpoint: до восьми последовательных dedicated-ALPN sessions, отдельные
  idle/request/completion deadlines и privacy-safe aggregate report. Ошибка
  расходует slot, но не блокирует оставшийся budget; background listener,
  runtime/IPC/desktop и import authority не добавлены.
- [`../docs/M0.9.87-INDEPENDENT-PROVIDER-FIELD-TEST-RU.md`](../docs/M0.9.87-INDEPENDENT-PROVIDER-FIELD-TEST-RU.md) —
  краткая русская инструкция будущего трёх- или четырёхмашинного запуска.
- [`../docs/M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md`](../docs/M0.9.30-OPAQUE-STORE-INTERNET-TEST-RU.md) —
  двухсетевой HTTPS publish/fetch/restart/retention test procedure.
- [`../docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](../docs/M0.4-RESUMABLE-SYNC-TEST-RU.md) —
  внешний тест pause/reconnect со сменой интерфейса.
