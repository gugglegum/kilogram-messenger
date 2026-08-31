# Память проекта Kilogram

Актуально на: 2026-08-31.

Эта папка — краткая проектная память и дорожная карта. Подробная техническая
спецификация находится в [`docs/RFC-0001-core-architecture.md`](../docs/RFC-0001-core-architecture.md).

## Текущее состояние

Архитектура остаётся в стадии проектирования. Rust workspace теперь содержит
`kilogram-identity`, `kilogram-protocol`, `kilogram-store`,
`kilogram-session`, `kilogram-transport-iroh` и `kilogram-cli`.
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
передаёт и создаёт её только после ratchet decrypt. Один OTK/session на device
pair пока не решает simultaneous initiation или account-wide device fan-out.
Seed/recovery, history rewrap, защищённое хранение root/local/ratchet keys,
production-grade sync summaries, membership removal и группы ещё не реализованы.

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

1. Добавить signed device-list fan-out, prekey pool и разрешение concurrent
   pairwise session initiation.
2. Спроектировать membership removal вместе с ordered security log и MLS epoch;
   отдельно — gossip/witness для first-contact freshness.
3. Спроектировать seed/recovery authority, protected root storage, root
   rotation и конфликтующие authority operations.
4. Подготовить ADR по Iroh против rust-libp2p и проверить мобильные платформы.
5. Спроектировать финальный wire format подписанного события и алгоритм
   линеаризации.
6. Спроектировать compact Merkle/range summary и переносимый signed cursor.
7. Добавить мультиустройство и затем небольшие MLS-группы.
8. Перед публичным выпуском провести независимый криптографический аудит.

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
- [`../docs/M0.4-RESUMABLE-SYNC-TEST-RU.md`](../docs/M0.4-RESUMABLE-SYNC-TEST-RU.md) —
  внешний тест pause/reconnect со сменой интерфейса.
