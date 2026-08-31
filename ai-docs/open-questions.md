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
Следующие вопросы относятся к production recovery, rotation, asynchronous
sessions, distribution, removal и key epochs и не решены этим прототипом.

- Какая точная модель угроз: массовое наблюдение, целевой атакующий, злонамеренные
  relay/storage peers, компрометация bootstrap-инфраструктуры, Sybil и eclipse?
- Как seed соотносится с Account Root Key: прямое детерминированное получение или
  расшифрование случайно созданного root key?
- Какие операции может единолично подписать device key, а какие требуют seed,
  аппаратного ключа или кворума устройств?
- Как разрешать конкурирующие операции восстановления и отзыва при утечке seed?
- Остаётся ли vodozemac/Olm только M0 reference implementation или production
  личный чат использует Signal-style PQXDH + Double Ratchet либо
  двухучастниковый MLS?
- Какой TTL применять к retained losing session, как выполнять authenticated
  session reset после потери state и нужен ли production-протокол сложнее
  проверенного M0.7.6 lexicographic-min разрешения двух crossed sessions?
- Какая encrypted production DB/WAL заменит M0.7.7 full ratchet snapshot и
  append-only filename baseline, обеспечит bounded recovery, migrations,
  backups и безопасное удаление без ослабления forward secrecy?
- Как authenticated discovery/gossip сообщает глобально самую свежую device-list
  revision и prekey pool, выполняет remote atomic OTK reservation и не раскрывает
  лишнюю account metadata? M0.7.6 отклоняет rollback/equivocation после
  наблюдения новой generation, но не доказывает её глобальную свежесть при
  первом контакте.
- Как resumable network history recovery выбирает и опрашивает несколько
  sources, переносит signed checkpoint между соединениями и разрешает recovery
  от устройства собеседника без неявного расширения same-account trust?
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

- Какая production database и схема at-rest encryption заменят временный
  content-addressed filesystem store из M0.1.2?
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
  повторная доставка и unlinkability.
- Репликация или erasure coding: сколько случайных узлов и какие гарантии нужны?
- Как выдавать storage capability и ограничивать spam/Sybil без глобального
  аккаунта и утечки социального графа?
- Как синхронизировать новое устройство и честно показывать неполную историю?
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
