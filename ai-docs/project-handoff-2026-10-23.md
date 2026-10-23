# Передача состояния проекта Kilogram

Дата среза: 2026-10-23 (Asia/Yekaterinburg).

Назначение этого документа — дать следующему исполнителю короткую проверяемую
точку входа. Подробная история остаётся в [индексе проектной памяти](README.md),
архитектура — в [RFC-0001](../docs/RFC-0001-core-architecture.md), а принципы
пользовательского суверенитета — в
[RFC-0193](../docs/RFC-0193-user-sovereignty-and-contextual-identity.md).

Рекомендуемое разделение ролей: GPT-6.1 Sol реализует ограниченные change-set'ы,
GPT-6 Astra независимо проверяет архитектуру, threat boundaries и соответствие
пользовательским сценариям. Архитектурный review не считается реализацией или
пройденным acceptance gate.

## 1. Проверенная точка остановки

- В начале передачи `HEAD` и `origin/master` совпадали:
  `3e82e6380c30d0f9443b6692ea34faccaacddef7`, commit
  `refactor(recovery): retire installation owner API` (M1.2.62).
- Изменений Rust-кода нет. M1.2.63 **не начат**; его утверждённый узкий scope
  сохранён в [next-stage-plan.md](next-stage-plan.md).
- Незакоммиченное состояние до этого handoff:
  - `ai-docs/README.md` — 6 строк со ссылкой на независимый аудит;
  - `ai-docs/development-environment.md` — 26 строк read-only power/availability
    аудита ноутбука, не относящихся к product code;
  - `ai-docs/reviews/` — незакоммиченный архитектурный аудит и компактный
    component reproduction A1 (5 файлов, около 52 KiB).
- Этот handoff и его ссылки также являются документационным изменением поверх
  указанной implementation revision. Перед продолжением нужно снова выполнить
  `git status --short --branch`, а не предполагать чистое дерево.

Последние принятые проверки M1.2.62: combined guided-recovery regression 24/24,
dedicated boundary mutations 18/18 и M1 readiness 188/188 вместе с rustfmt и
locked/offline strict Clippy. Подробности:
[RFC-0286](../docs/RFC-0286-m1262-host-installation-owner-public-surface-retirement.md)
и [development-environment.md](development-environment.md).

Незавершённые или не выполненные проверки:

- во время независимого аудита прошли 218 library tests, 15 специальных
  golden-vector generators остались ignored; повтор полного
  `canonical_decoder_mutations` corpus был остановлен, а следующие protocol
  mutation/fuzz-seed harnesses в том запуске не выполнялись;
- аудит не выполнял новый manual GUI run, полный workspace regression или
  свежий two-host/cross-network acceptance;
- A1 подтверждён component-level reproduction, а не полным
  GUI/runtime/network pipeline;
- сведения и границы проверки зафиксированы в
  [архитектурном аудите](reviews/2026-10-22-architecture-mvp-audit.md) и его
  [инструкции воспроизведения](reviews/repro-2026-10-22/README.md).

Минимальная работа до согласованной точки остановки уже выполнена: M1.2.62
закоммичен и отправлен, исходники не имеют незавершённого change-set. Для
возобновления не требуется «доделывать» частичный M1.2.63; требуется сначала
принять архитектурное решение о приоритетах ниже.

## 2. Фактическая карта готовности

### Работает в реальном, но пока операторски подготовленном пути

- Windows desktop управляет отдельным runtime через authenticated loopback IPC:
  показывает чаты/историю/status, импортирует подписанный contact descriptor,
  ставит текст в durable outbox и запускает/останавливает runtime. Это не
  самостоятельный onboarding двух чистых пользователей: форма всё ещё просит
  Account ID и путь к ticket
  ([UI](../apps/kilogram-windows/src/lib.rs#L9062)).
- Подготовленная 1:1 переписка использует device-signed immutable events,
  vodozemac/Olm ratchet ciphertext, локальные encrypted projections, durable
  ACK/retry, direct Iroh path либо relay fallback. Активный public sync default
  остаётся legacy Postcard v6
  ([active ALPN](../crates/kilogram-transport-iroh/src/lib.rs#L60)).
- Account Root, отдельные Device identities, certificates, authority snapshots,
  revocation, encrypted transactional vault и device-link/recovery primitives
  реализованы. Компрометация разблокированного endpoint всё равно раскрывает
  доступный ему plaintext; revocation защищает только будущее.
- Blind volunteer mailbox встроен в runtime и включён в новом desktop profile
  по умолчанию: 200 MiB хранения, 500 MiB/30 дней Ethernet, 500 MiB/30 дней
  Wi-Fi, 0 mobile и 0 unknown-network
  ([desktop defaults](../apps/kilogram-windows/src/lib.rs#L725),
  [runtime constants](../crates/kilogram-runtime-ipc/src/lib.rs#L64)).
  Сохраняется opaque E2EE ciphertext, а не читаемая история.
- Ранее выполнены реальные двухмашинные проверки direct/relay, recovery sync,
  volunteer store и opt-in canonical sync v2. Это историческое evidence, не
  доказательство текущего полного пользовательского acceptance.

### Реализовано отдельно или частично, но не завершено как продуктовый путь

- Canonical sync v2 имеет protocol/store/adapters, durable activation authority,
  dark launch, desktop opt-in и two-host evidence, но public default намеренно
  `legacy-only`; автоматического перехода или error fallback нет. Навигация:
  [RFC-0205](../docs/RFC-0205-m1181-canonical-sync-v2-activation-semantics.md),
  [RFC-0210](../docs/RFC-0210-m1186-desktop-canonical-sync-v2-activation.md).
- Peer re-enrollment после capability quarantine реализован и был проверен на
  двух хостах, но обычный пользовательский onboarding/re-enrollment без
  подготовленных artifacts не завершён:
  [RFC-0237](../docs/RFC-0237-m1213-peer-reenrollment-two-host-rehearsal.md),
  [RFC-0241](../docs/RFC-0241-m1217-guided-desktop-peer-reenrollment-ceremony.md).
- Old-history recovery имеет durable consent, transfer, multi-source scheduler,
  import, explicit consume, IPC v34 envelope и безопасную dormant launch-service
  facade. Однако runtime стартует без configured recovery context; facade не
  подключена к CLI/desktop/background/network. Точная граница:
  [RFC-0268](../docs/RFC-0268-m1244-active-runtime-ipc-recipient-import-consumption-envelope.md),
  [RFC-0283](../docs/RFC-0283-m1259-dormant-process-owned-guided-recovery-launch-service.md).
- Desktop first-run, seed restore, device link, Root package/quorum и removal
  имеют отдельные мастера и механизмы, но clean install -> verify invite ->
  create membership -> first message всё ещё не является единым простым flow.
- Runtime хранит несколько outbound contacts, но inbound admission одного
  запущенного профиля ограничен одним внешним `allowed_requester_account_id`;
  это не multi-contact listener
  ([options](../apps/kilogram-cli/src/main.rs#L2926),
  [admission](../apps/kilogram-cli/src/main.rs#L27077)).

### Только запланировано или не готово для ежедневной эксплуатации

- Согласованный catch-up порядок между ratchet, conversations, direct delivery,
  mailbox и paged sync; lifetime-scalable inventory/history.
- Самостоятельный invite/membership/admission flow, полноценный multi-contact
  inbound runtime, responsive UI при blackhole/offline peer, notifications,
  installer/update/background lifecycle и длительный dogfooding.
- Context-scoped social identities вместо публичного глобального `AccountId`,
  privacy-preserving anti-abuse, edit/retract/version history и mutually
  consented local purge.
- MLS/group membership, deterministic group ordering/checkpoints, channels,
  encrypted files/media, mobile clients и metadata-hiding multi-hop mode.

## 3. Непереговорные инварианты и причины решений

| Инвариант | Почему / что отвергнуто | Подробности |
| --- | --- | --- |
| Инфраструктура не хранит читаемую историю и не получает message keys. Временное bounded хранение чужого ciphertext допустимо. | Отвергнут доверенный центральный mailbox с Telegram-подобной возможностью читать, менять или удалять историю. Relay и volunteer nodes остаются заменяемой доставкой, не источником истины. | [RFC-0001, §12](../docs/RFC-0001-core-architecture.md#12-временная-офлайн-доставка), [RFC-0083](../docs/RFC-0083-default-volunteer-blind-storage-role.md), [RFC-0095](../docs/RFC-0095-clean-no-https-volunteer-field-run.md) |
| Полученное и committed содержимое принадлежит локальному владельцу vault. Remote edit/retract — только новое подписанное событие; original/version history остаётся доступной локально. | Отвергнуто незаметное удаление или переписывание истории на чужом устройстве и Telegram-подобный remote erase. | [RFC-0193, §3](../docs/RFC-0193-user-sovereignty-and-contextual-identity.md#3-user-owned-received-history) |
| Account Root — административная recovery-власть, не публичный вечный User ID. Публичные контакты/комнаты/боты должны получить pairwise/context/service-scoped pseudonyms. | Отвергнут единый неизменяемый идентификатор, позволяющий связывать действия человека между контекстами. Текущий `AccountId` — precursor, не финальная social identity. | [RFC-0193, §2](../docs/RFC-0193-user-sovereignty-and-contextual-identity.md#2-no-permanent-global-social-identifier) |
| Обычные сообщения подписывает Device; Root не является online message key. Device можно отозвать, но прошлые копии и plaintext уже скомпрометированного устройства нельзя «отозвать». Seed compromise означает компрометацию высшей recovery-власти без дополнительного фактора. | Отвергнут один неотзывный ключ аккаунта для всех сообщений и ложное обещание ретроактивной защиты. | [RFC-0001, §7](../docs/RFC-0001-core-architecture.md#7-идентичность-и-устройства) |
| Direct P2P может раскрывать IP; абсолютная анонимность первой версии не обещается. Relay/multi-hop privacy должна быть отдельной политикой, direct допустим для доверенных контактов. | Отвергнута маркетинговая подмена E2EE полной сетевой анонимностью. | [RFC-0001, §11.2](../docs/RFC-0001-core-architecture.md#112-privacy-modes), [RFC-0193, §5](../docs/RFC-0193-user-sovereignty-and-contextual-identity.md#5-threat-boundary) |
| Recovery, capability migration и downgrade-sensitive переходы требуют explicit signed consent, exact binding, restart-safe monotonic records и fail closed; не реконструировать authority из UI projection. | Отвергнуты автоматический destructive recovery, format sniffing и fallback после authenticated selection. | [RFC-0196](../docs/RFC-0196-m1169-mixed-version-downgrade-matrix.md), [RFC-0242](../docs/RFC-0242-m1218-explicit-old-history-recovery-plan.md) |
| Для групп не нужен blockchain. База — causal DAG, deterministic linearization и при необходимости quorum checkpoints; MLS решает group key agreement, но не прикладной порядок. | Отвергнут глобальный blockchain/sequencer как лишняя сложность и централизация. | [RFC-0001, §13](../docs/RFC-0001-core-architecture.md#13-группы-и-порядок-сообщений) |
| Доступность не абсолютна: если нет ни одного online/retained ciphertext экземпляра, восстановить данные невозможно; first-contact/global freshness и whole-database rollback не доказываются без witness/quorum. | Отвергнуты обещания бесконечной offline delivery и глобальной истины без доверия/кворума. | [RFC-0001, §3.2](../docs/RFC-0001-core-architecture.md#32-не-цели-первой-версии) |

Telegram-знакомый UX, anti-gaslighting history, contextual identity и default-on
volunteer limits уже зафиксированы в документации; их нельзя считать
реализованными только потому, что direction принят. Новых неизвестных деталей
в этой передаче не додумывалось.

## 4. Recovery-серия: исходная цель, остаток и конечная точка

Здесь важно различать две вложенные серии.

### Product recovery series (M1.2.14–M1.2.62)

Исходная проблема: capability-aware peer re-enrollment восстанавливает будущую
связь с fresh successor, но replacement Device не получает старые pairwise
ratchet keys; успешный новый ticket не является согласием на history rewrap.
[RFC-0238](../docs/RFC-0238-m1214-canonical-runtime-ipc-peer-reenrollment-contract.md#old-history-remains-a-separate-authority-boundary)
явно отделяет future communication от old-history recovery. Серия строит
recipient-controlled цепочку: inspect/consent -> durable source transfer ->
bounded multi-source selection -> transactional import -> отдельный explicit
consume -> process-owned guided ceremony без экспорта raw authority.

Сделано: все перечисленные primitives, v34 opaque local IPC envelope и dormant
safe launch service. Не сделано и потому обязательно до продуктового завершения:

1. принять отдельное architecture decision — продолжать интеграцию сейчас или
   заморозить dormant facade ради active-path P1 blockers;
2. если продолжать: подключить единственного process owner к уже
   аутентифицированному recovery flow, затем safe IPC/UI ceremony без передачи
   paths/signers/memberships в GUI;
3. проверить restart/cancel/expiry/crash windows и multi-source divergence на
   реальном runtime, а не только copied-module fixtures;
4. выполнить clean two-host acceptance: re-enrollment, источники истории,
   offline/restart, import, explicit consume, одинаковая readable history и
   отсутствие duplicate/silent loss;
5. только после этого объявлять old-history recovery пользовательской функцией.

В документах **нет утверждённого номера финального product milestone** после
M1.2.63. Поэтому нельзя честно писать, что вся recovery-серия заканчивается на
M1.2.63. Её конечная продуктовая точка — описанный acceptance result; номер и
scope должны быть утверждены архитектурным решением.

### Public-surface retirement subseries (M1.2.60–M1.2.63)

Её конечная точка определена точно: raw launch slot, installed host,
installation owner и assembly issuer больше не являются externally nameable;
снаружи остаётся только safe launch-service facade и безопасная command/view
vocabulary. M1.2.60–62 закрыты. M1.2.63 должен сделать
`RecipientImportGuidedRecoveryAssemblyIssuer` crate-private и убрать public
re-export, сохранив atomic staging. Сейчас issuer всё ещё public
([type](../crates/kilogram-peer-history-recovery-runtime/src/recipient_import_guided_session_controller.rs#L267),
[re-export](../crates/kilogram-peer-history-recovery-runtime/src/lib.rs#L111));
никакого частичного исправления в working tree нет.

## 5. Корректировки и дополнения к архитектурному аудиту

Полный текст review: [2026-10-22 architecture MVP audit](reviews/2026-10-22-architecture-mvp-audit.md).
Точечная проверка source не выявила контрпримеров главным A1, A2, A5 и A6;
A1 дополнительно воспроизведён. Но следующие формулировки нужно читать точнее:

1. **A3: exact limit, approximate message count.** Lifetime cap 4096 событий
   реален и дублируется history ordering
   ([wire limit](../crates/kilogram-protocol/src/wire.rs#L13),
   [history rejection](../apps/kilogram-cli/src/main.rs#L14962)). «Около 2048
   подтверждённых сообщений» — типичный text+ACK расчёт, а не универсальный
   порог: unacknowledged/mailbox/recovery histories имеют другое отношение
   message/event. Исправлять всё равно нужно lifetime-scalable paging, а не
   только менять число.
2. **A4: ограничение inbound, не всего contact store.** Runtime умеет хранить и
   исходяще обслуживать несколько contacts; узкое место — launch profile с одним
   `allowed_requester_account_id` и listener admission только этого account
   ([profile](../apps/kilogram-cli/src/main.rs#L2926),
   [check](../apps/kilogram-cli/src/main.rs#L27077)). Пользовательский вывод
   аудита верен, но исправление должно расширять authenticated admission, а не
   переписывать outbound contact model.
3. **A7: «блокирует весь аккаунт» слишком широко.** Complete directory требует
   pool каждого active Device и freshness каждого pool
   ([coverage/freshness](../crates/kilogram-ratchet/src/lib.rs#L444)); runtime
   действительно не соберёт fresh listener directory при отсутствующем или
   expired pool ([startup](../apps/kilogram-cli/src/main.rs#L21925)). Это
   блокирует startup/new-session/send paths, которые требуют directory, но
   source не показывает блокировку локального чтения уже committed history.
   Риск для daily use остаётся P1, область отказа должна быть сформулирована
   точно.
4. **Упущена финальная identity-модель.** Код делает `AccountId` стабильным Root
   public key ([definition](../crates/kilogram-identity/src/account.rs#L63)) и
   текущая membership прямо содержит owner/member Account IDs
   ([membership](../crates/kilogram-identity/src/account.rs#L2118)). Для
   подготовленного 1:1 PoC это допустимо, но публичные группы, каналы и боты не
   могут выйти с этим wire identity: это нарушит принятую unlinkability цель.
5. **Упущена user-owned history semantics.** Текущий protocol знает только
   `RatchetText` и `Acknowledgement`
   ([payload](../crates/kilogram-protocol/src/lib.rs#L166)); edit/retract/delete
   ещё не реализованы. Их будущий дизайн обязан быть append-only и сохранять
   local original/version history, а не копировать remote erase Telegram.
6. **Canonical v2 не является текущим default.** Большой verified v2 stack
   существует, но transport прямо считает active только legacy Postcard v6
   ([selection](../crates/kilogram-transport-iroh/src/lib.rs#L60)). Исправление
   A1/A3 нельзя подменить простым включением v2: у v2 тот же inventory cap, а
   ratchet ordering нужно решить сквозным образом.

Dependency finding A2 опирается на `Cargo.lock` с Iroh 1.0.3 и внешний advisory.
Перед исправлением нужно заново проверить актуальный advisory и совместимую
patched версию; timestamp review не является вечной SCA-истиной.

## 6. Три ближайших приоритета до daily-usable Windows alpha

### P1. Lossless и масштабируемый message/sync backbone

- **Пользовательский результат:** после долгого offline, restart и смены
  direct/relay/mailbox ни одно сообщение не становится нечитаемым, а история не
  перестаёт открываться после нескольких тысяч сообщений.
- **Зависимости:** единый design ratchet domains/order между conversations и
  transport paths; paged/range inventory и history index; Iroh upgrade;
  crash-safe migration и regression на persistent state.
- **Критерий завершения:** 100 000 events в одной истории; backlog не меньше
  256 ratchet messages с перемешанными conversations и direct/relay/mailbox;
  restart между каждой page; exact readable convergence без
  `MissingMessageKey`, lifetime cap и silent fallback; dependency advisory gate
  не содержит известного unaccepted Iroh finding.

### P2. Два чистых клиента -> три одновременных контакта без CLI-оператора

- **Пользовательский результат:** пользователь устанавливает Windows client,
  создаёт/восстанавливает account, обменивается одним проверяемым invite,
  подтверждает контакт и пишет; второй offline contact не замораживает UI.
- **Зависимости:** create/install membership ceremony, bounded multi-contact
  inbound admission, actor/network separation, durable enqueue order,
  per-conversation drafts, понятные delivery states.
- **Критерий завершения:** три clean accounts создают минимум две беседы на
  одном runtime без ручных ticket paths/Account IDs; двусторонняя доставка и
  restart проходят direct и relay; blackhole peer не задерживает local
  status/history/send/shutdown; draft не уходит другому контакту, outbox
  сохраняет пользовательский порядок.

### P3. Честная offline delivery и recoverability в ежедневной эксплуатации

- **Пользовательский результат:** отправитель и получатель могут не быть online
  одновременно; UI честно различает queued/stored/delivered/expired; потерянное
  или заменённое устройство восстанавливает согласованную историю из доступных
  пользовательских источников без передачи Root/message secrets посредникам.
- **Зависимости:** завершённый mailbox TTL/renew/reset/reconciliation lifecycle;
  per-device freshness/debt policy; решение продолжить или заморозить dormant
  recovery series; при продолжении — safe facade -> runtime IPC -> desktop;
  backup/restore, notification и sleep/wake lifecycle.
- **Критерий завершения:** физический two-host/cross-network run с минимум двумя
  volunteer replicas, sender offline до receive, lost ACK, TTL expiry/renew,
  process crash и sleep/wake; затем device replacement и multi-source recovery
  с restart/cancel/divergence. У обеих сторон exact readable history, нет
  duplicate/silent loss, а every terminal status объясним пользователю.

Группы, каналы, media и mobile не входят в эту узкую daily-usable Windows alpha;
они остаются следующими самостоятельными vertical slices. После каждого P1–P3
нужна отдельная Astra-проверка пользовательского результата и threat boundary,
а не только числа tests/gates.
