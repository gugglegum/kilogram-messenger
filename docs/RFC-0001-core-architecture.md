# RFC-0001: Базовая архитектура Kilogram

- Статус: **Draft**
- Дата: **2026-08-24**
- Область: identity, E2EE, transport, storage, synchronization, groups
- Связанные материалы: [`../ai-docs/README.md`](../ai-docs/README.md),
  [`../ai-docs/decisions.md`](../ai-docs/decisions.md),
  [`../ai-docs/open-questions.md`](../ai-docs/open-questions.md)

## 1. Резюме

Kilogram — публичный open-source P2P-мессенджер с end-to-end encryption,
локальной постоянной историей и интерфейсом, ориентированным на функциональную
полноту Telegram. Инфраструктура может помогать в discovery, NAT traversal,
relay и временной доставке ciphertext, но не является доверенной стороной и не
получает возможность прочитать или незаметно изменить сообщения.

Первая выбранная архитектура использует независимое ядро на Rust, Iroh как
предварительный P2P transport, OpenMLS для группового E2EE и собственный
подписанный causal event DAG для истории и синхронизации.

Этот RFC задаёт направление, границы доверия и основные инварианты. Он пока не
фиксирует окончательный wire format и конкретные ciphersuites.

## 2. Мотивация

Централизованные cloud-чаты требуют доверия оператору сервера относительно
конфиденциальности, целостности и доступности истории. Kilogram должен убрать
возможность инфраструктурного оператора:

- читать содержимое переписки;
- создавать правдоподобные сообщения от имени пользователя;
- незаметно изменять существующие сообщения;
- единолично уничтожать все экземпляры истории.

Последний пункт является best-effort: если исчезли все клиентские и временные
реплики, протокол не может восстановить данные из ничего.

## 3. Цели

### 3.1. Обязательные свойства

1. True E2EE для личных чатов, групп, каналов, файлов и звонков.
2. Отсутствие постоянно хранимого сервером plaintext и серверных message keys.
3. Подписанные события и проверяемая целостность истории.
4. Forward secrecy и post-compromise security.
5. Несколько устройств одного аккаунта с отдельными ключами и отзывом.
6. P2P-синхронизация истории и явное отображение неполного восстановления.
7. Direct P2P, NAT traversal и зашифрованный relay fallback.
8. Временная офлайн-доставка через слепое хранение ciphertext с TTL.
9. Группы и каналы без блокчейна и без обязательного центрального sequencer.
10. Публичная спецификация, open-source клиенты и воспроизводимые сборки как
    целевое свойство релизного процесса.

### 3.2. Не цели первой версии

- абсолютная сетевая анонимность;
- гарантированная неотличимость трафика от любого обычного HTTPS;
- защита plaintext на полностью скомпрометированной конечной точке;
- бесконечная офлайн-доставка без доступных реплик;
- мгновенный строгий total order при любом сетевом разделении;
- токен или экономическая система вознаграждения relay-узлов;
- собственные криптографические примитивы.

## 4. Термины

**Account Root Identity** — стабильная корневая криптографическая идентичность
аккаунта. Её публичная часть или её хеш образует Account ID.

**Recovery seed** — мнемонический секрет, позволяющий восстановить корневую
власть аккаунта. Точная схема derivation остаётся открытой.

**Device identity** — отдельная пара ключей конкретной установки клиента,
авторизованная Account Root Identity.

**Event** — канонически сериализованная, подписанная запись: сообщение,
редактирование, reaction, receipt, membership change, revocation и так далее.

**Conversation DAG** — причинный граф событий разговора.

**Relay peer** — узел, пересылающий уже зашифрованный поток между endpoints.

**Storage peer** — узел, временно хранящий непрозрачный ciphertext с TTL.

**Indexer/checkpoint peer** — уполномоченный участник, подтверждающий
финализированный префикс группового журнала. Это не глобальный blockchain miner.

## 5. Модель угроз

### 5.1. Предполагаемый атакующий

Атакующий может:

- контролировать один или несколько relay/storage/bootstrap-узлов;
- читать, задерживать, переупорядочивать, повторять и удалять сетевые пакеты;
- подменять ответы discovery;
- создавать множество Sybil-узлов;
- быть участником группы;
- получить локальную БД или device key украденного устройства;
- наблюдать IP, время, объём и направления трафика в доступной ему части сети.

### 5.2. Границы защиты

- AEAD защищает конфиденциальность и целостность ciphertext.
- Подпись устройства связывает событие с авторизованным device key.
- Causal links и event IDs обнаруживают подмену истории и пропущенные зависимости.
- Репликация уменьшает способность одного узла уничтожить историю.
- Padding и будущая relay-схема уменьшают, но не устраняют metadata leakage.
- Активно скомпрометированное устройство видит предназначенный ему plaintext.
- Обладатель recovery seed неотличим от законного владельца без дополнительного
  фактора или recovery policy.

## 6. Компонентная архитектура

```text
Kilogram UI
    |
Kilogram Core API
    +-- Identity and device authority
    +-- Pairwise messaging
    +-- MLS group state
    +-- Signed event DAG and deterministic views
    +-- Local encrypted storage
    +-- History synchronization
    +-- Blind mailbox client
    +-- Transport abstraction
            +-- direct QUIC
            +-- NAT traversal
            +-- encrypted relay
            +-- future HTTPS/privacy transports
```

UI, transport, local database and wire codec не должны содержать единственную
реализацию security policy. Критические проверки выполняются в Rust-ядре.

## 7. Идентичность и устройства

### 7.1. Иерархия

Концептуальная модель:

```text
Recovery seed
    |
Account Root Key / Recovery Authority
    +-- DeviceCertificate(A)
    +-- DeviceCertificate(B)
    +-- DeviceRevoked(C)
```

Account Root Key не используется для подписи обычных сообщений. Каждое
устройство создаёт собственные signing, encryption и session keys.

`DeviceCertificate` должен как минимум связывать:

- Account ID;
- Device ID;
- публичные ключи устройства;
- допустимые capabilities;
- serial/epoch;
- срок действия или правило обновления;
- подпись авторизующей authority.

### 7.2. Чувствительные операции

Обычный device key не должен автоматически обладать всеми корневыми правами.
Политика для добавления устройства, отзыва другого устройства и root rotation
будет определена отдельным RFC. Кандидаты: seed, аппаратный ключ, подтверждение
двух устройств и security modes разной строгости.

### 7.3. Компрометация

При отзыве устройства публикуется подписанное `DeviceRevoked`, увеличивается
account/device epoch, закрываются pairwise sessions и обновляются групповые MLS
epochs. Отозванное устройство не получает будущие ключи.

Протокол не может удалить уже расшифрованную историю с захваченного устройства.
Локальное шифрование и безопасное удаление старых message keys ограничивают
последствия, но не заменяют отзыв.

## 8. Личные чаты

Окончательный pairwise-протокол ещё не выбран. Рассматриваются:

- стандартизованная MLS-группа из двух участников;
- схема уровня PQXDH/X3DH + Double Ratchet с мультиустройством уровня Sesame.

Обязательные свойства независимо от выбора:

- асинхронное установление сессии;
- отдельная адресация устройств;
- forward secrecy и post-compromise security;
- replay protection;
- out-of-order delivery в ограниченном окне;
- key-change notifications и QR verification;
- fan-out на все актуальные устройства собеседника и собственные устройства;
- padding до классов размера.

Собственная криптографическая конструкция не допускается без крайней
необходимости, формального анализа и независимого аудита.

## 9. Формат события

Логическая модель события:

```text
Event {
  protocol_version
  conversation_id
  event_id
  author_account_id
  author_device_id
  author_sequence
  membership_epoch
  parents[]
  logical_time
  payload_type
  ciphertext
  signature
}
```

`event_id` вычисляется из канонической формы подписываемых полей. Точный codec,
domain separation, hash и signature algorithm будут зафиксированы отдельно.

M0.1.1 использует отдельный экспериментальный профиль: Postcard для
детерминированной сериализации Rust-структур, Ed25519 для подписей и BLAKE3 для
идентификаторов с разными domain-separation prefixes. Этот профиль проверяет
инварианты и API, но не фиксирует публичный межъязыковой wire protocol. Device
signing key в прототипе также ещё не имеет сертификата от Account Root Identity.

Все клиенты обязаны отклонять событие, если:

- подпись неверна;
- устройство не авторизовано для указанной эпохи;
- sequence нарушает правила writer log;
- отсутствует обязательная causal dependency;
- payload нарушает state machine разговора;
- событие превышает установленные лимиты.

## 10. История и синхронизация

Каждое устройство ведёт append-only writer log. События разных writers образуют
causal DAG. Локальная materialized view строится чистым детерминированным
reducer и может быть пересоздана из валидных событий.

Синхронизация должна поддерживать:

- обмен heads и компактными Merkle summaries;
- поиск отсутствующих диапазонов;
- sparse/on-demand загрузку вложений;
- дедупликацию по event/content ID;
- возобновление после разрыва;
- bounded storage и bounded processing;
- обнаружение неполной истории;
- синхронизацию между собственными устройствами и участниками разговора.

M0.1.3 проверяет базовую reconciliation state machine с bounded full-ID
inventory: requester подписывает inventory application device key и включает
session binding к текущему transport Endpoint ID listener. Стороны обмениваются
не более чем 64 отсутствующими events в каждом направлении за round, сверяют
точные requested IDs, повторно проверяют подписи и сохраняют события
идемпотентно. Inventory ограничен 4096 IDs. Неизвестный локальной истории
requester получает явный отказ.

Connection ticket также подписан application device key listener и связывает
этот ключ с конкретным Iroh Endpoint ID и одним явно разрешённым requester
device ID. Клиент проверяет ticket до отправки inventory; изменение transport
endpoint или любого из device IDs обнаруживается. Listener применяет то же
ограничение к обычной delivery, поэтому посторонний обладатель ticket не может
сначала добавить своё событие, чтобы пройти последующую known-author проверку.

До отправки запрошенных локальных events клиент также проверяет подписанный и
session-bound diff listener. Подписант обязан совпадать с application device ID
из connection ticket; одной transport identity или копии старого signed event
недостаточно, чтобы запросить историю клиента.

M0.1.4 выносит эти проверки и переходы протокола в transport-independent
`kilogram-session`, доступ к событиям задаётся узким `SessionStore` trait.
Iroh-specific ALPN и framing находятся в отдельном
`kilogram-transport-iroh`. CLI автоматически повторяет bounded rounds в одном
аутентифицированном Iroh connection до convergence, но не более 64 rounds.
Детерминированный тест с 70 отсутствующими событиями в каждом направлении
сходится за два rounds (64 + 6). Запись принятого batch в временный файловый
store проверяет существующую conversation history один раз на batch.

Это временный профиль: правило known-author не заменяет Account Root
authorization и revocation, полный список IDs не заменяет compact Merkle/range
summary, а восстановление rounds после разрыва ещё не имеет resumable cursor.
Лимит inventory означает, что этот M0-профиль перестаёт работать после 4096
локальных events и не является масштабируемым алгоритмом истории.

Постоянный plaintext хранится только на устройствах, которым он предназначен.
Локальная БД шифруется отдельным ключом устройства, защищённым средствами ОС,
где они доступны.

Эксперимент M0.1.2 до выбора БД хранит каждый проверенный event отдельным
content-addressed файлом, устанавливаемым атомарно без перезаписи. Он проверяет
deduplication, обнаружение повреждений и вычисление causal frontier. Payload и
device secret в этом development store пока не зашифрованы; это известное
временное несоответствие целевому требованию at-rest encryption, поэтому такое
состояние нельзя использовать для реальной переписки.

## 11. Сетевой транспорт

### 11.1. Первая версия

Первый кандидат — Iroh:

- QUIC/TLS 1.3 connections;
- прямое P2P при доступности;
- hole punching;
- зашифрованный relay fallback;
- peer authentication через публичный Endpoint ID.

Выбор остаётся предварительным до desktop/mobile spike. rust-libp2p является
основной альтернативой.

Ядро должно зависеть от абстракции транспорта, а не от публичных типов Iroh:

```text
connect(peer, route_policy) -> authenticated multiplexed connection
listen() -> incoming authenticated connections
```

### 11.2. Privacy modes

Первая версия допускает раскрытие IP при direct P2P. UI должен сообщать об этом
без обещания анонимности.

Будущие режимы:

- `direct-preferred` — relay fallback;
- `relay-only` — собеседник не видит IP;
- `multi-hop` — endpoints разделены несколькими relay;
- per-contact policy — direct только для доверенных контактов.

Маскировка под HTTPS, padding и cover traffic проектируются отдельно. TLS/QUIC
сам по себе не гарантирует нераспознаваемость приложения для DPI.

## 12. Временная офлайн-доставка

Storage peers могут хранить только E2EE ciphertext с ограниченным TTL.
Предварительная конструкция:

```text
application payload
  -> conversation encryption
  -> author signature
  -> fixed-size padding/chunking
  -> opaque mailbox address
  -> replicated storage peers
```

Адрес слота должен быть псевдослучайным и вычисляться только сторонами, имеющими
mailbox capability, например из epoch и slot counter через PRF/HMAC. Storage peer
получает opaque address, ciphertext, size class и expiry, но не прикладные IDs и
не ключ расшифрования.

Требования:

- AEAD и подпись обнаруживают изменение;
- receipt или TTL запускает удаление;
- повторная выдача безопасна благодаря replay protection;
- несколько реплик или erasure coding уменьшают риск потери;
- quotas/capabilities ограничивают spam и storage exhaustion;
- узел не является открытым произвольным blob host или proxy.

Даже при этой схеме storage peer видит сетевой источник, время и объём операции.
Сокрытие этой связи требует relay/multi-hop режима.

## 13. Группы и порядок сообщений

### 13.1. Шифрование

Группы используют MLS RFC 9420 через OpenMLS. MLS отвечает за membership,
group epochs и обновление секретов, но не задаёт глобальный порядок прикладных
сообщений и не заменяет delivery/synchronization layer.

Membership/admin events и MLS commits должны применяться согласованно одной
детерминированной state machine. Нельзя принять application event из эпохи или
от устройства, не разрешённых актуальным состоянием.

### 13.2. Причинный порядок

Каждый event ссылается на известные автору parents. Если событие B ссылается на
A, все клиенты отображают A перед B.

Параллельные события упорядочиваются одинаковым total tie-break rule, например
по кортежу из logical time, author ID и event ID. Конкретное правило будет
зафиксировано вместе с wire format.

Клиенты, временно имеющие разные наборы событий, могут показывать разные
нефинализированные хвосты. После получения одинакового DAG их views обязаны
сойтись.

### 13.3. Финализация

Для операций, которым нужен необратимый порядок, вводятся подписанные
checkpoints. Кворум indexer-узлов подтверждает конкретный префикс/набор heads;
события до checkpoint больше не переставляются.

Обычные сообщения могут отображаться оптимистично до checkpoint. Security-
sensitive операции — membership, ban, admin capability и key epoch — требуют
более строгих правил применения.

Точная Byzantine fault model, выбор indexers и quorum formula остаются открытыми.
Простой majority checkpoint нельзя автоматически считать BFT-консенсусом.

## 14. Каналы

Канал моделируется подписанным append-only feed автора или набора редакторов.
Подписчики и зеркала реплицируют события и вложения. Порядок публикаций одного
writer задаётся sequence number; несколько редакторов используют тот же causal
DAG и правила финализации, что и группа.

Удаление публикации является подписанным tombstone. Оно изменяет отображаемое
состояние, но не гарантирует физического стирания уже скачанных копий.

## 15. Relay-узлы пользователей

Добровольный relay является будущей функцией. Начальная безопасная модель:

- функция выключена по умолчанию;
- пользователь задаёт лимит трафика и скорости;
- ограничены число соединений и время жизни;
- capability tokens не позволяют использовать узел как открытый proxy;
- relay работает только с E2EE-потоками Kilogram;
- учёт не требует раскрытия содержимого и социального графа.

Токен-экономика не входит в первую версию из-за Sybil, накрутки и усложнения
metadata privacy.

## 16. Проверка безопасности

До публичного production-релиза необходимы:

- опубликованная protocol specification;
- test vectors для codec, signatures, ratchets/MLS integration и event ordering;
- property-based и fuzz testing парсеров и state machines;
- моделирование потерь, повторов, reorder, partitions и Byzantine inputs;
- независимый криптографический и архитектурный аудит;
- воспроизводимые сборки и подписанный механизм обновлений;
- процедура раскрытия уязвимостей и миграции алгоритмов.

## 17. Этапы реализации

1. Identity и два локальных Rust-процесса.
2. Подписанный writer log и deterministic sync.
3. Direct QUIC connection и повторное соединение.
4. Pairwise E2EE и локальная зашифрованная история.
5. Relay fallback и NAT traversal tests.
6. Мультиустройство, device list и revocation.
7. Blind mailbox с TTL и несколькими storage peers.
8. Небольшие MLS-группы и causal ordering.
9. Checkpoints, роли, moderation и каналы.
10. Mobile clients, calls, privacy relay modes и аудит.

## 18. Нерешённые вопросы

Актуальный список поддерживается в
[`../ai-docs/open-questions.md`](../ai-docs/open-questions.md). В первую очередь
должны быть закрыты recovery authority, pairwise protocol, canonical encoding,
Byzantine model для checkpoints и мобильная пригодность выбранного транспорта.

## 19. Ссылки

- [RFC 9420: Messaging Layer Security](https://www.rfc-editor.org/rfc/rfc9420.html)
- [RFC 8445: Interactive Connectivity Establishment](https://www.rfc-editor.org/rfc/rfc8445.html)
- [RFC 8656: TURN](https://www.rfc-editor.org/rfc/rfc8656.html)
- [RFC 9000: QUIC](https://www.rfc-editor.org/rfc/rfc9000.html)
- [OpenMLS](https://github.com/openmls/openmls)
- [Iroh](https://docs.iroh.computer/)
- [rust-libp2p](https://libp2p.io/)
- [Hypercore](https://github.com/holepunchto/hypercore)
- [Autobase](https://github.com/holepunchto/autobase)
- [Signal specifications](https://signal.org/docs/)
