# Открытые вопросы и риски

## P0 — до реализации протокола

M0.5.1 зафиксировал минимальную однопроцессную модель authority:
`AccountId` — Ed25519 public root key, root подписывает device certificate и
постоянный отзыв конкретного ключа. M0.5.2 применяет certificate и известные
caller-supplied revocations до сетевых данных. M0.6.1 заменил их полным
root-signed snapshot и max-seen anti-rollback. M0.6.2 добавил owner-signed
add-only conversation membership и проверку Account/Device proof каждого
event. M0.7.1 добавил два static-key HPKE box для тела сообщения. M0.7.2 удалил
sender box из replicated event и ввёл local-only encrypted history projection.
M0.7.3 заменил оставшийся peer box на persistent `vodozemac::olm` Double
Ratchet с device-signed one-time prekey. M0.7.4 добавил root-signed полный
device list, точный prekey directory и отдельный ciphertext slot каждого
устройства peer account. M0.7.5 добавил same-account history rewrap с signed
source inventory/range, HPKE на новый device и local projection provenance v2.
M0.7.6 добавил signed prekey pools, per-device max-seen freshness и bounded
active/retained resolution для двух crossed outbound sessions.
M0.7.7 добавил exclusive device state lock и crash-consistent M0 filesystem
transaction для ratchet/sequence/projection/event/prekey state.
M0.7.8 добавил same-account network history rewrap с двусторонним consent/SAS,
session-bound request, source-signed transfer и local multi-source claim
reconciliation без обещания глобальной полноты.
M0.7.9 добавил signed append-only checkpoint chain, authenticated pagination
через fresh session на каждую страницу, safe retry и выбор inventory только при
согласии минимум двух явно опрошенных sources.
M0.9.1 переносит до 64 смежных страниц по одному authenticated connection,
сохраняя atomic checkpoint каждой страницы и explicit source/SAS consent.
M0.9.2 добавляет compact expiring source-signed link, offline verification и
exact recipient/conversation/SAS preflight. M0.9.3 добавляет bounded QR image
render/decode; M0.9.4 добавляет default-off bounded LAN descriptor discovery без
connection/consent. Live camera/clipboard, GUI ceremony и wide-area
privacy-preserving discovery ещё не реализованы.
M0.8.1 добавил обратимый encrypted shadow snapshot всего device state в `redb`,
а M0.8.2 — authenticated intent, versioned generation и recoverable mirror
после каждой live CLI-команды. M0.8.3 добавил typed exact shadow reads и
атомарный encrypted delta: DB writes стали `O(changed + deleted)`, хотя полный
scan/decrypt остаётся `O(state)`. M0.8.4 перевёл read-only history, M0.8.5 —
manual/network history-rewrap source inventory, M0.8.6 — mixed sync reads на
DB-primary event/projection snapshot, а M0.8.7 — vault commit barrier для всех
journaled writes. M0.8.8 добавил typed journal delta без полного filesystem
payload scan и первый mutable DB-primary adapter для `next-sequence`.
Filesystem пока остаётся compatibility shadow. M0.8.9 переводит ratchet на
DB-sourced workspace и
заменяет второй append directory walk явным write-set. M0.8.10 заменяет CLI
path reconstruction repository receipts и вводит encrypted manifest index:
schema-v2 direct commit не decrypt-ит unchanged DB payload, schema-v1 rebuild
выполняется один раз. Initial rollback baseline, pre-command/final shadow gates
и цельный metadata index всё ещё `O(state)`. M0.8.11 переводит authority,
contact pin и membership reads/writes на DB-primary trust repository и удаляет
неявный filesystem ingress из direct commit. M0.8.12 хранит Windows vault key
только в DPAPI CurrentUser envelope и автоматически rewrap-ит прежний raw key;
non-Windows provider, key recovery и rollback witness ещё не решены. Retained
shadow пока обязателен.
M0.9.17 добавил existing-account enrollment без seed transfer: новый device
подписывает short-lived request, Root после exact SAS атомарно публикует полный
device list и шифрует authorization exact recipient. Это делает device
допустимым получателем уже существующих resumable recovery plans, но не выбирает
источники автоматически и не доказывает полноту восстановленной истории.
M0.9.19 добавил portable Root-signed authority package и exact independently
retained witness: phrase restore больше не обнуляет sequence/revocations/list/
membership heads, а old package с latest witness отклоняется. Совместный rollback
старого matching package+witness остаётся неразрешимым без внешнего monotonic
источника или current-device quorum. M0.9.20 добавил desktop ceremony, exact
inspected package ID/revision gate и stdin-only phrase handling, но explicit
checkbox newest witness является только human assertion, не freshness proof.
M0.9.21 добавил local exact-export receipt/status и поэтому делает пропущенное
обновление видимым, но rollback всего Root откатывает и receipt. RFC-0048 выбрал
fresh challenge-bound current-device quorum. M0.9.22–24 реализовали exact
roster, DB-primary anti-equivocation heads и joint-majority transition старого
и нового epoch; wide-area witness и exceptional loss-of-majority recovery пока
остаются открыты.
Следующие вопросы относятся к production recovery, rotation, asynchronous
sessions, distribution, removal и key epochs и не решены этим прототипом.

- Какая точная модель угроз: массовое наблюдение, целевой атакующий, злонамеренные
  relay/storage peers, компрометация bootstrap-инфраструктуры, Sybil и eclipse?
- Как уменьшить раскрытие account metadata при реализованной local/LAN/relay
  транспортировке recovery approvals и какой future external monotonic witness
  реализует тот же verifier interface, когда device quorum недоступен?
- Какой явно reduced-assurance процесс допустим для emergency recovery после
  потери старого majority и как сделать его заметным всем оставшимся devices?
- Какие операции может единолично подписать device key, а какие требуют seed,
  аппаратного ключа или кворума устройств?
- Как разрешать конкурирующие операции восстановления и отзыва при утечке seed?
- Остаётся ли vodozemac/Olm только M0 reference implementation или production
  личный чат использует Signal-style PQXDH + Double Ratchet либо
  двухучастниковый MLS?
- Какой TTL применять к retained losing session, как выполнять authenticated
  session reset после потери state и нужен ли production-протокол сложнее
  проверенного M0.7.6 lexicographic-min разрешения двух crossed sessions?
- M0.8.10 даёт repository-owned append receipts и authenticated incremental
  manifest index без unchanged DB payload scan. Как разбить цельный metadata
  index на transactional pages/Merkle nodes и убрать initial/final full-state
  gates без второго coordinator? Как долго сохранять legacy shadow и какие fault/
  migration критерии разрешают удалить его? Остаётся ли `redb` production
  engine после mobile/load, bounded backup и compaction tests?
- Как дополнить Windows DPAPI CurrentUser provider аппаратным ключом или
  passphrase/seed-derived wrapping на всех платформах; как обнаруживать rollback
  согласованной старой пары DB+key и восстанавливать device-specific key без
  создания общего ключа расшифровки всех устройств?
- Как authenticated discovery/gossip сообщает глобально самую свежую device-list
  revision и prekey pool, выполняет remote atomic OTK reservation и не раскрывает
  лишнюю account metadata? M0.7.6 отклоняет rollback/equivocation после
  наблюдения новой generation, но не доказывает её глобальную свежесть при
  первом контакте.
- M0.9.2 определяет compact signed recipient-specific descriptor, offline
  inspect и explicit SAS-gated accept; M0.9.3 реализует PNG/JPEG image round-trip,
  M0.9.4 — opt-in bounded LAN multicast publication и verified no-connect scan,
  M0.9.5 — recipient-signed retry plan с caller-supplied network/power policy,
  M0.9.6 — signed persistent retry chain, equal-jitter deadline, attempt lease,
  clock high-water и terminal cancellation, M0.9.7 — Windows-native разовый
  network/metered/roaming/power snapshot с conservative VPN mapping, M0.9.8 —
  bounded worker с native change events, deadline/cancel wakeup и policy recheck,
  M0.9.9 — stable multi-session endpoint с per-session state transaction.
  Как сделать wide-area privacy-preserving lookup/gossip, подключать live camera/
  clipboard/GUI, регистрировать deep link, как сделать optional autostart/
  background lifecycle, как реализовать providers на других ОС, как внешний
  witness обнаруживает rollback
  всей scheduler chain; как отдельно
  разрешать recovery от устройства собеседника без неявного расширения
  same-account trust?
- M0.9.10 закрывает локальную часть contact/outbox: signed exact-device карточка
  с atomically refreshable ticket path, materialize-once event, idempotent ACK,
  signed retry chain и automatic sync внутри одного runtime writer. Открыто:
  какой wide-area multi-source descriptor gossip/mailbox не раскрывает social
  graph, как witness обнаруживает rollback/equivocation и как versioned contact
  безопасно принимает device/route rotation?
- M0.9.11 дал device-signed bearer IPC только на loopback и сериализованный
  runtime actor. Открыто: Windows named-pipe/Unix-socket peer credentials,
  push/change revision с backpressure, локальные роли нескольких UI clients и
  IPC onboarding/rotation contact без передачи секретного descriptor через
  sync/shared folder.
- Какой финальный межъязыковой canonical wire encoding обеспечивает одинаковые
  подписи и event IDs на всех платформах? Postcard используется только как
  предварительный M0 codec и не закрывает вопрос публичного протокола.
- Какие security properties требуются от линеаризатора при злонамеренных, а не
  только сбойных indexer-узлах?

## P1 — до сетевого MVP

- Подтверждена ли работа Iroh и OpenMLS на Android/iOS в требуемой конфигурации?
- Iroh или rust-libp2p лучше соответствуют требованиям relay, discovery,
  transport pluggability и будущей HTTPS-маскировки?
- Как устроены bootstrap/discovery без центральной точки отказа и без раскрытия
  списка участников комнаты через общий discovery topic?
- Нужен ли TCP/WebSocket/HTTP/3 fallback уже в первой версии?
- Как измерять и публиковать NAT traversal success rate?
- Как защищать relay от превращения в открытый прокси и от amplification?

## P1 — хранение и синхронизация

- Как разделить transactional metadata и большие encrypted attachment blobs,
  какие retention/compaction limits применять и как доказать отсутствие
  plaintext remnants после окончательного ухода от legacy store?
- Какая Merkle/range summary и переносимая signed cursor-схема заменит bounded
  full-ID inventory M0.1.3, не раскрывая лишнюю структуру истории? M0.4 уже
  использует durable event set как correctness checkpoint, поэтому будущий
  cursor должен давать измеримый выигрыш по трафику и иметь явные
  snapshot/staleness semantics.
- Как discovery/gossip/witness доказывает global freshness authority snapshot
  без центрального сервера? M0.6.1 доказывает completeness на подписанной
  revision и запрещает rollback после получения новой, но first contact может
  получить старый корректно подписанный snapshot.
- Как доставлять и обнаруживать самый свежий conversation membership без
  доверия одному peer и без раскрытия social graph? M0.6.2 принимает локально
  установленный owner-signed add-only snapshot и запрещает rollback после его
  наблюдения, но не реализует distribution.
- Как доказать, что импортируемое «историческое» событие было создано до
  device revocation? Embedded authority snapshot сам по себе не даёт trusted
  time; нужны epoch-bound/expiring authorizations, ordered witness или другая
  явная модель исторической валидности.
- Схема blind mailbox: вычисление адресов, TTL, подтверждение получения,
  повторная доставка и unlinkability. M0.9.29 решает только short-lived runtime
  ticket head для already-enrolled contact: deterministic pseudonymous channel
  и access pattern ещё не являются blind capability или mailbox.
- Как синхронизировать M0.9.29 observation high-water между устройствами одного
  аккаунта и обнаруживать valid publisher equivocation между получателями без
  доверенного global log? Первый recipient observation пока защищён только
  signature/expiry. M0.9.34 уже хранит bounded authenticated candidate set и
  делает delivery/automatic-sync failover между явно импортированными Devices.
  M0.9.35 независимо обновляет все их publication channels и показывает local
  per-channel high-water. M0.9.36 хранит проверенный expiry-independent local
  binding. M0.9.37 вручную переносит candidate set/bindings и signed sibling
  observation evidence в recipient-HPKE bundle, требует exact current Root
  roster и не принимает bundle как authority. M0.9.38 переносит exact bundle
  по authenticated same-account session и возвращает session-bound recipient
  ACK с bounded one-shot backpressure. M0.9.39 даёт одному listener bounded
  primary+own Account аудитории, stable own-device ticket и signed durable
  foreground schedule с backoff/network policy/compaction. M0.9.40 убирает
  общую папку: exact-current siblings выводят directional pairwise capability,
  публикуют source-signed recipient-HPKE ticket в opaque store и выполняют
  existing push из runtime-managed path. M0.9.41 проецирует одну
  local-Device-signed exact-roster policy во все pairwise child schedules,
  добавляет recipients после полного startup roster и немедленно выключает их
  после live Root revocation; один runtime не запускает parallel fan-out.
  M0.9.42 переносит higher direct/accepted high-water транзитивно через bundle
  v2, M0.9.43 сохраняет первый mismatch в local-Device-signed `.pcf`, а
  M0.9.44 распространяет canonical proof и разрешает только Root-authorized
  переход на peer-signed ticket v11 с новым channel epoch. M0.9.45 разделяет
  online `.pcrq`/offline `.pcrp`, а bundle v4 автоматически переносит готовый
  resolution к exact-current siblings с тем же local evidence. M0.9.46
  выполняет собственную rotation, request и apply через authenticated live
  actor без restart. M0.9.47 добавляет public evidence viewer, compact QR claim
  и mandatory KPC1 gate до Root load, но всё ещё требует человеческой передачи
  свежего ticket и offline artifacts. Открыты purpose-built minimal offline
  signer/reproducible media image, обнаружение конфликта между устройствами,
  которые никогда не обменивались
  bundle, и действительно распределённая
  cross-machine координация без доверенного scheduler.
- M0.9.32 compact/checkpoint-ит runtime publication, observation, policy и
  attempt chains до exact signed heads за device-signed cumulative checkpoint;
  DB-primary checkpoint/removals атомарны и crash-safe. Открытым остаётся
  внешний witness против rollback всего vault и общая compaction policy для
  outbox/recovery/event repositories; локальная `.rtc` цепочка сама по себе не
  является global freshness proof.
- Репликация или erasure coding: сколько случайных узлов и какие гарантии нужны?
- Как ограничивать spam/Sybil без глобального аккаунта и утечки социального
  графа? M0.9.33 защищает существующий channel от постороннего высокого
  generation через self-authenticating per-peer Ed25519 write capability без
  открытого Account/Device ID. Но capability не мешает распределённому attacker
  создавать собственные channels, занимать общий cap или устраивать volumetric
  DDoS; proof-of-work, invitations/quota или другая admission модель остаётся
  открытой.
- M0.9.18 уже позволяет вручную добавить несколько recipient-bound plans,
  запускает по одной bounded attempt и честно показывает
  `incomplete`/`single-source`/`agreed`/`divergent`. Как desktop безопасно
  автоматизирует wide-area поиск sources и persistent supervision, не сливая
  разные consent scopes и не создавая скрытый background service?
- Как обрабатывать редактирование, удаление, reactions, receipts и исчезающие
  сообщения в append-only модели?
- Что означает удаление: локальное сокрытие, подписанный tombstone или best-effort
  физическое удаление реплик?

## P1 — группы и каналы

- Кто является MLS Delivery Service в P2P-модели и как доставляются KeyPackages,
  Welcome, Proposal и Commit?
- Как выбираются indexer/checkpoint-узлы и что происходит при их недоступности?
- Какой порядок может временно меняться в UI, а какой обязан быть финальным?
- Как обеспечивать согласованность membership/admin events с MLS epoch?
- Какая модель редакторов и подписанного feed используется в каналах?
- Как распространять популярные каналы без раскрытия подписчиков и без перегрузки
  автора?

## P2 — последующие версии

- Single-hop и multi-hop privacy relay, сокрытие IP и защита от traffic analysis.
- Padding, cover traffic и маскировка под обычный HTTPS без ложных обещаний
  нераспознаваемости для DPI.
- Справедливые лимиты добровольных relay/storage nodes без накручиваемой
  токен-экономики.
- Push-уведомления на Android/iOS без утечки содержания и социального графа.
- Воспроизводимые сборки, обновления клиента и защита supply chain.
- Миграция криптоалгоритмов и wire protocol без разделения сети.

## Инварианты при выборе ответов

- Не создавать собственные криптографические примитивы.
- Сервер или случайный пир не должен получать ключ расшифрования сообщения.
- Любая модификация события должна обнаруживаться клиентом.
- Сетевой транспорт, локальная БД и UI должны быть заменяемыми деталями.
- Утверждения об анонимности должны соответствовать реально скрываемым метаданным.
