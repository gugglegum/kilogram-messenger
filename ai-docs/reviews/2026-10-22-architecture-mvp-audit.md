# Архитектура и готовность Kilogram к MVP — независимый обзор

Дата начала: 2026-10-22; воспроизведение завершено 2026-10-23 (Asia/Yekaterinburg).
Проверенная ревизия: `3e82e6380c30d0f9443b6692ea34faccaacddef7` (M1.2.62).

## Вывод

Проект имеет настоящий работающий технический фундамент: E2EE, authority,
durable outbox, локальное зашифрованное хранилище, прямой/relay транспорт и
blind mailbox. Переписывать его с нуля оснований нет. Однако текущая сборка
остаётся стендом для подготовленных участников, а не самостоятельным
пользовательским MVP. Главные риски находятся на стыках криптографии,
синхронизации, runtime и UX. Есть конкретные дефекты, мешающие обычной переписке.

Следующие этапы стоит выбирать по завершённым пользовательским сценариям.
Продолжение серии dormant recovery/rollout abstraction и retirement stages
без такого критерия не даёт предсказуемого срока MVP.

Уточнение после [передачи состояния 2026-10-23](../project-handoff-2026-10-23.md):
recovery-серия решает реальную задачу возврата старой истории replacement Device,
которому не передаются прежние pairwise ratchet keys. M1.2.63 завершает только
узкую API-retirement подсерию; утверждённого номера завершения всей продуктовой
recovery-интеграции нет. Приоритет active-path дефектов не отменяет её authority
и consent requirements и не означает, что dormant код следует удалить.

Аудит первоначально недостаточно учитывал принятый
[RFC-0193](../../docs/RFC-0193-user-sovereignty-and-contextual-identity.md):
нынешний глобальный `AccountId` — precursor, а final social identity должна быть
context-scoped; remote edits/retractions не должны уничтожать оригиналы в чужом
vault. Это принятые требования, не уже реализованные возможности. Перед публичным
выпуском групп/каналов/ботов identity-протокол требует отдельного решения и
проверки. Приведённая ниже оценка MVP остаётся предварительной и должна учитывать
эту работу; её нельзя исключать ради прежнего диапазона сроков.

## Границы обзора и проверки

- Выполнены основной обзор и три параллельных независимых разбора:
  криптография/authority, runtime/delivery/storage, desktop/product/QA.
- Просмотрены реальные пути исполнения, связанные тесты, RFC, roadmap и
  evidence. Это выборочный инженерный обзор, не полный формальный аудит всех
  251 510 строк Rust (174 файла, включая тесты) и не сертификация безопасности.
- На момент обзора: 286 RFC и 207 `verify-*.ps1` scripts. Эти числа показывают
  объём сопровождения, а не процент готовности.
- Повторно выполнен `verify-kilogram-m1-readiness.ps1 -AcceptedTag M1.0.0`:
  **188/188**, rustfmt и workspace/all-targets/all-features locked/offline
  strict Clippy прошли. Этот gate сам сообщает `test_executables_started=false`.
- Дополнительно: **218 library tests passed, 15 ignored** (явная генерация
  golden vectors), component reproduction A1 подтверждён. Полный повтор
  decoder mutation corpus намеренно остановлен; детали в конце отчёта.
- GUI вручную не исследовался; свежего прогона на двух физических хостах и
  эксплуатации в разных сетях в этом обзоре не было. Прежние field/fuzz evidence
  прочитаны как исторические доказательства, не выданы за новые проверки.
- Продуктовый код, зависимости, roadmap следующего этапа, настройки ОС и
  удалённые системы не изменялись. Существовавшее изменение
  `ai-docs/development-environment.md` оставлено нетронутым.

## Приоритетные проблемы

P1 здесь означает «устранить до регулярной пользовательской эксплуатации»;
это не утверждение одинаковой CVSS-серьёзности всех пунктов.
Сокращённые ссылки `main.rs` ниже относятся к `apps/kilogram-cli/src/main.rs`,
названия crate — к каталогу `crates/` этой ревизии.

### A1 — P1: порядок доставки несовместим с окном пропущенных ratchet keys

`plan_sync` выбирает missing events по EventId/hash и обрезает их до 64
(`crates/kilogram-session/src/lib.rs:424`). Получатель сортирует author sequence
только **внутри** полученной страницы
(`apps/kilogram-cli/src/main.rs:35265`). У закреплённого `vodozemac 0.10.0`
на receiving chain остаётся лишь 40 skipped message keys; ранние ключи
удаляются при продвижении дальше этого окна. Ratchet session общая для
всех бесед с одним DeviceId (`kilogram-ratchet/src/lib.rs:936,1314`).

Следствие: первая hash-selected страница большого offline backlog может
успешно расшифроваться и сохраниться, но вытеснить ключи сообщений следующей
страницы. Сохранённый ciphertext этих сообщений больше не расшифровывается
этим ratchet state. Повтор sync/restart сам по себе не помогает. История может
ещё существовать у отправителя и потребовать отдельного history recovery;
это не утверждение уничтожения всех экземпляров plaintext.

Другой разрешённый порядок: 41 сообщение в беседе A, следующее в B,
затем sync B раньше A. Сортировка только внутри A не устраняет проблему.
Mailbox также извлекает первое item по opaque item ID, а не ratchet sequence
(`crates/kilogram-mailbox/src/store.rs:509`, `main.rs:24511`). Для него нужен отдельный
end-to-end regression; компонентное воспроизведение ниже относится к sync.

Исправление: спроектировать согласованный порядок catch-up и ratchet domains,
включая разные беседы, direct delivery и mailbox. Простое увеличение 40 до
другой константы не решает неограниченное переупорядочивание. Нужны реальные
ratchet + persistent state тесты на сотнях сообщений, страницу за страницей,
и на смешивании transport paths.

### A2 — P1: закреплена известная уязвимая версия Iroh

`Cargo.lock:2822` содержит `iroh 1.0.3`, `Cargo.lock:2943` —
`iroh-relay 1.0.3`. Официальное
[GHSA-7cq4-mhxw-xw78](https://github.com/n0-computer/iroh/security/advisories/GHSA-7cq4-mhxw-xw78)
описывает affected Iroh `>=0.91.0, <=1.0.3`, patched `1.1.0`.
При знании EndpointId специально сформированный relay frame может остановить
входящий relay traffic до restart и занять CPU core. Это отказ в обслуживании,
не раскрытие переписки. Эксплойт в ходе обзора не запускался.

Нужны обновление до исправленной совместимой версии, transport regressions и
регулярная проверка dependency advisories. Зафиксированный lockfile обеспечивает
повторяемость, но не актуальность безопасности.

### A3 — P1: пожизненный предел 4096 событий на беседу

`kilogram-protocol/src/wire.rs:13` ограничивает полный inventory 4096 events.
`kilogram-session/src/lib.rs:113` передаёт inventory всей беседы. Canonical v2
сохраняет такой же предел (`canonical_sync_v2.rs:26`). Это не лимит страницы.

Дополнительно `order_runtime_history` прямо отклоняет историю длиннее 4096
(`apps/kilogram-cli/src/main.rs:14962`), а вызывают его и conversation index,
и history page (`15197,15330`). Обычный обмен добавляет text+ACK, что подтверждает
существующий runtime integration test (`41270`). Поэтому примерно после 2048
обычно подтверждённых сообщений этот предел достигается; 2049 таких сообщений
дают 4098 событий. История/список бесед и sync могут перестать открываться,
хотя direct delivery ещё работает.

Нужны инкрементальный/paged inventory и индекс истории, ограничения ресурсов
на страницу/операцию вместо lifetime cap, тесты хотя бы на 100 000 событий.
Одна смена wire encoding этого не исправляет.

### A4 — P1: один runtime допускает только одного внешнего собеседника

`accept_device_authorization` разрешает `primary_allowed_account` либо свой
аккаунт (`apps/kilogram-cli/src/main.rs:27094`). Значение берётся из launch
options; `AddContact` сохраняет запись, но не расширяет audience (`17127`).
Диагностика runtime честно сообщает audience 1 или 2 (`21178`).

Несколько outbound контактов возможны, но одновременная полноценная
двусторонняя переписка с несколькими внешними аккаунтами в одном runtime
не обеспечена. Это намеренное ограничение PoC, которое нельзя считать готовой
multi-contact архитектурой. Нужен bounded authenticated admission по контактам
и инвайтам. Просто убрать проверку аккаунта нельзя.

### A5 — P1: GUI не завершает создание новой беседы

`+ Contact` просит label, Account ID и путь к ticket
(`apps/kilogram-windows/src/lib.rs:9081`). Runtime ещё до чтения ticket требует
уже установленный signed conversation membership
(`apps/kilogram-cli/src/main.rs:9750`). Desktop не предоставляет обычного
create/install membership flow. Launch profile также предполагает подготовленные
device list/prekey файлы (`kilogram-windows/src/lib.rs:8168`).

Нужен сквозной сценарий двух чистых установок: создать аккаунт, обменяться
проверяемым инвайтом, подтвердить контакт, автоматически установить допустимые
membership/prekeys/admission, написать. Acceptance обязательно расширить до
трёх аккаунтов, чтобы выявлять A4.

### A6 — P1: сетевое ожидание блокирует управление runtime

`wait_for_runtime_event` принимает IPC только когда `outbound_cycle.is_none()`
(`apps/kilogram-cli/src/main.rs:25500`). Connect может занять 30 секунд, затем
идут failover/mailbox; actor-response IPC timeout тоже 30 секунд
(`kilogram-runtime-ipc/src/lib.rs:54,2133`). Поэтому обычные send/status/history/
shutdown могут перестать отвечать именно при offline peer. WaitForChange
обрабатывается отдельно и этого не исправляет.

Входящая application session также исполняется inline и удерживает state lock
до device authorization (`main.rs:25785`). Границы/timeouts ограничивают одну
попытку, но не обеспечивают отзывчивость UI под повторными медленными peers.

Нужны короткие actor prepare/commit, отдельная bounded network phase,
отзывчивые reads/cancellation, тесты с blackhole endpoint и concurrent local
commands. Single writer полезен; удерживать его через сетевое ожидание не нужно.

### A7 — P1 для multi-device: expired pool secondary блокирует directory-dependent пути

Directory требует ровно pool каждого устройства полного roster, а `verify_at`
проверяет expiry всех pools (`kilogram-ratchet/src/lib.rs:447,473`). Default TTL
7 дней (`:60`). Свежесть всего directory проверяется при ticket verification
(`main.rs:3394`), подготовке runtime (`21969`) и fan-out (`27600`).
`encrypt_with_pool` требует свежий pool даже при существующей сессии (`ratchet:832`).

Если secondary device выключено дольше срока его pool, онлайн-устройство не
может обновить его подпись. Без получения свежего pool или изменения roster
подготовка полного directory/новых отправок блокируется. Это чрезмерная связь
доступности аккаунта с доступностью каждого его устройства.

Область вывода: startup, new-session и send paths, требующие fresh directory.
Невозможность локального чтения уже committed history этим разбором не доказана;
первоначальный заголовок «блокирует весь аккаунт» был слишком широким.

Нужна явная политика per-device delivery debt, пропущенного устройства и
восстановления, отделяющая создание новой сессии от продолжения существующей.
Не принимать просроченные новые prekeys без анализа безопасности. Для первой
alpha допустим честный single-device scope вместо обещания готового multi-device.

### A8 — P2: сообщения очереди могут изменить порядок

Queue ID случайный (`apps/kilogram-cli/src/runtime_queue.rs:86`), а
`select_due_runtime_queue` выбирает первый ключ BTreeMap (`main.rs:24720`).
Author sequence и causal parents назначаются лишь при материализации
(`24911`). Накопленные до обработки сообщения «A, затем B» могут навсегда
попасть в историю «B, затем A». Нужен durable enqueue sequence, проверяемый
после restart и при retry.

### A9 — P2: черновик переносится между получателями

`select_conversation` меняет адресата, но сохраняет единственное поле message
(`apps/kilogram-windows/src/lib.rs:1140`). `queue_draft` объединяет его с текущим
адресатом (`1109`), composer использует то же поле (`9053`). Набрать для Alice,
открыть Bob, нажать отправить — прежний текст адресуется Bob. Нужны drafts
по contact ID или другое явно безопасное поведение. Проверено по source,
вручную GUI-сценарий не исполнялся.

### A10 — P2: незавершённый lifecycle истёкшей mailbox доставки

Наличие любого mailbox dispatch исключает direct retry (`main.rs:24723`),
expired dispatch пропускается repair (`23336`). Статус MailboxExpired есть,
но renew/reset/resend этой записи в просмотренном пути не найден.
Обычный sync может перенести тело, однако reconciliation delivery marker
для этого пути не найден. Нужны явное завершение/повтор и согласование
статусов direct/mailbox/sync, тест на TTL и потерянный reverse ACK.

## Дополнительные security boundaries

OTK выбирается детерминированно из 16 entries по умолчанию; разные инициаторы
могут выбрать один ключ (`kilogram-ratchet/src/lib.rs:388`). После расходования
первым инициатором второму нужен безопасный retry/reset. Обновление pool само
по себе не заменяет уже сохранённую outbound session (`:878`), а неизвестная
новая PreKey после подтверждения отклоняется (`:957`). Отсутствие полноценного
authenticated reset признаётся и RFC-0009. Это дополнительный P2 для первого
контакта/утраты state; не ослаблять запрет подмены сессии ради быстрого обхода.

Legacy `SyncSessionBinding` — hash listener EndpointId, а не fresh challenge
или binding текущего QUIC connection (`wire.rs:34`, `main.rs:21256,27081`).
При утечке plaintext application transcript auth+inventory допускают повтор
на другом соединении к тому же endpoint. Проверки подписи и account остаются;
пассивный relay не видит plaintext TLS, самостоятельный remote exploit без
такой утечки не установлен. Это P2 hardening/неточная гарантия session binding,
а не доказательство взлома E2EE. Нужен connection-bound proof of possession.

Это ограниченный обзор зависимостей, не полный SCA. Аудит vodozemac не является
аудитом его интеграции в Kilogram — A1 показывает важность этого различия.

## Что сделано хорошо

- Используется готовый Olm/Double Ratchet из vodozemac; HPKE и подписи имеют
  определённые области применения. Это разумная база, хотя интеграцию надо проверять.
- Account Root отделён от device identity; имеются signed roster, revocation,
  anti-rollback и явные recovery authority boundaries.
- Event materialization, ratchet advancement, local projection и outbox marker
  объединены транзакцией; retry использует тот же event.
- ACK сохраняется до ответа, duplicate delivery возвращает существующий ACK.
  Mailbox storage receipt не выдаётся за доставку адресату.
- Есть реальные Rust/process/IPC тесты, physical field evidence, длительный
  fuzzing и воспроизводимость offline executable. Называть все тесты
  «проверкой строк» было бы неверно.
- Конкретная слепая зона A1: многопакетный sync test использует MemoryStore
  без decrypt, а helper создаёт отдельный ratchet для отдельных events
  (`kilogram-session/src/lib.rs:566,934`). Реальный CLI canary проверяет лишь
  три сообщения пакетами 2+1 (`main.rs:41485`). Каждый слой может проходить
  свои тесты при сломанной совместной работе.
- Документация M1.0.0 честно ограничивает обещания technical PoC и не заявляет
  production security или независимость всех storage providers.

## Архитектурные и процессные решения

1. Сохранить Rust core, существующие crypto primitives, явные trust boundaries
   и durable transactions. Основной долг — композиция и эксплуатация.
2. Выделить runtime application layer из CLI: текущий main.rs содержит 43 363
   строки с тестами; production orchestration занимает большую часть файла.
   Переносить по мере исправления конкретных сценариев, не начинать отдельную
   масштабную перепись ради чистоты.
3. Закончить/отложить dormant recovery work по конечному acceptance result.
   M1.2.60–63 последовательно скрывают transitional types; current next plan
   прямо запрещает IPC/UI/network integration. Это полезная инкапсуляция,
   но не причина откладывать A1–A7.
4. Разделить architecture lint, реальные behavior tests и product acceptance.
   Readiness проверяет markers/manifests/source shape, fmt и Clippy; число
   gates не является числом E2E-сценариев. Новые тесты должны ловить найденные
   failures, а не только сохранять ожидаемое строение кода.
5. CI сейчас `workflow_dispatch` only. Автоматизацию push/PR вводить отдельно
   с учётом принятой политики запусков/затрат; добавить короткий обязательный
   набор regression и плановую dependency-advisory проверку.
6. Сжать актуальный project index: кратко отделить active product, dormant code,
   исторические evidence и MVP blockers. Сотни RFC не должны заставлять следующий
   чат заново угадывать critical path.

## Что означает «без серверов»

Выбранная система не требует доверенного центрального сервера с plaintext
историей. При этом Iroh использует вспомогательные relays; `DirectOnly`
сохраняет relay coordination (`kilogram-transport-iroh/src/lib.rs:98`).
Volunteer devices временно хранят чужой ciphertext. Это совместимо с
децентрализованной моделью, но отличается от «никаких серверов/посредников».
Официальное [описание подключения Iroh](https://docs.rs/iroh/latest/iroh/)
подтверждает relay-assisted connection и direct-path upgrade.

Нужно зафиксировать продуктовое обещание: история и ключи у пользователей,
временная слепая доставка/relay могут обслуживаться заменяемыми узлами.
Доступность всех закрытых устройств, бесконечный TTL и полная metadata anonymity
из этой архитектуры автоматически не следуют.

## Готовность функций

| Область | Фактическое состояние |
| --- | --- |
| Подготовленная 1:1 текстовая переписка | Реализована, есть прежние field evidence |
| Crypto/authority/durable storage | Существенный фундамент, есть integration defects |
| Чистая установка → первое сообщение без оператора | Не завершено |
| Несколько внешних собеседников одновременно | Inbound runtime ограничен одним |
| Большая история и offline backlog | A1/A3 блокируют эксплуатационную готовность |
| Офлайн-доставка | Реализована, требуются порядок/TTL/retry/reconciliation tests |
| Multi-device | Много механизмов, чрезмерная связь freshness и сложный UX |
| Recovery | Много реализованного; новый guided service ещё dormant |
| Группы и каналы | Основной протокол/MLS delivery/membership ещё впереди |
| Файлы и изображения | Пользовательский путь не найден, нужен blob lifecycle |
| Android/iOS | Готовых клиентов нет |
| Установка/обновление/фон/уведомления | Требуют пользовательского завершения |

OpenMLS отсутствует в workspace dependencies. Pairwise fan-out по устройствам
не является готовым групповым протоколом. MLS решает group key agreement,
но приложению всё равно нужны delivery и membership/epoch ordering:
[RFC 9420](https://www.rfc-editor.org/info/rfc9420/).

## Предлагаемый путь до реально используемой версии

### Узкая закрытая Windows alpha

Это отдельная ранняя цель, не переименование исходного MVP с группами/каналами.
Scope: 1:1 text, несколько контактов, одно основное Windows-устройство на человека,
честная best-effort offline доставка и понятное сохранение истории.

1. Исправить A1/A2/A3 и добавить регрессии, включая смешанные беседы/transports.
2. Завершить invite/membership/admission для трёх чистых аккаунтов (A4/A5).
3. Разделить actor/network work; восстановить порядок outbox, drafts и
   завершение TTL/retry; не блокировать локальное управление плохой сетью.
4. Довести профиль/launch, unread и уведомления, per-message статусы,
   opt-in фон, restart/sleep/wake; проверить простой backup/recovery и upgrade.
5. Провести несколько недель dogfooding с несколькими людьми/сетями:
   100k событий; backlog 256+ сообщений; offline дольше TTL и 7 дней;
   crash после durable commit до ACK; смена IP/relay; повреждение/backup;
   UI responsiveness при blackhole, ограниченный рост диска.

Для первоначального планирования при одном ведущем разработчике с помощью ИИ
можно заложить **4–8 полных рабочих недель** на такую alpha после смены приоритетов.
Это предварительная инженерная оценка с низкой уверенностью, не срок,
вычисленный из количества milestone. После первого законченного vertical
slice её нужно пересмотреть. Dogfooding входит в эту вилку и идёт параллельно
доработкам; нижняя граница предполагает отсутствие большой переделки
sync/ratchet domains и хранения. Такая переделка увеличит срок.

### Исходный MVP с группами/каналами и файлами

Добавляются самостоятельные большие направления: group membership/removal,
MLS epoch/commit conflict delivery, channel permissions/feed, encrypted blobs,
offline retention и соответствующий UX. Для desktop-проекта такого scope
первоначальный ориентир — **3–6 месяцев или больше всего,
включая alpha**.
Это сценарная оценка при регулярной разработке и ограниченном scope, без обещания
mobile/звонков/Telegram feature parity. Мобильные платформы требуют отдельной
оценки background/push/battery и UX; до публичных сильных E2EE/security claims
необходим независимый профильный review.

Не выражать готовность процентом на основе RFC, тестов или номера M1.2.62.
Измерять её числом законченных пользовательских сценариев и отсутствием
потерь/ошибок при длительной реальной работе.

## Результаты дополнительных проверок

### Воспроизведение A1 и части A7

Отдельный offline binary `target/debug/kilogram-audit-probe.exe` использовал
реальные `SyncServer`, `EventStore`, `SignedEvent` и persistent `RatchetState`.
Продуктовый код не подменялся. Сравнивались одинаковые ciphertexts и копии
одних исходных recipient keys:

```text
ordered_control_decrypted=256
hash_order_first_batch_decrypted=64
hash_order_second_batch_failure_sequence=3
hash_order_second_batch_failure=Decryption(MissingMessageKey(3))
backlog_missing_key_reproduced=true
established_session_expired_pool_rejected=PrekeyPoolExpired { ... }
```

В первой hash-selected странице были sequence от 5 до 248. Все 64 расшифровались;
после reload persistent state сообщение 3 следующей страницы уже не читалось.
В хронологическом контроле успешно расшифровались все 256. Подтверждён именно
дефект композиции crypto/sync, а не неверный ciphertext. Это component-level
reproduction; весь CLI transaction/vault/GUI/network pipeline не запускался.
Проверка expiry показывает наличие persistent sender session, но не моделирует
полный двухустройственный account lifecycle A7.

Сохранены [исходник, lockfile и инструкция](repro-2026-10-22/README.md), а также
[точный вывод](repro-2026-10-22/observed-output.txt). SHA-256 исходника:
`65678ffdd99df29952e563b12e9278dc297058d376ba47a3fbc913798e62714c`.

### Существующие Rust suites

Все Windows harnesses запускались через `scripts/run-cargo-tests-stable.ps1`
с постоянными именами, `verification` profile, Cargo jobs=2 и offline dependencies.

| Library suite | Passed | Failed | Ignored | Время самого harness |
| --- | ---: | ---: | ---: | ---: |
| kilogram-crypto | 3 | 0 | 0 | 0.01 s |
| kilogram-identity | 24 | 0 | 0 | 48.43 s |
| kilogram-protocol | 156 | 0 | 15 | 981.85 s |
| kilogram-ratchet | 10 | 0 | 0 | 7.72 s |
| kilogram-session | 25 | 0 | 0 | 209.62 s |
| Всего завершённых library tests | 218 | 0 | 15 | |

15 ignored — явно помеченные генераторы golden vectors, не скрытые падающие тесты.
После трёх первых library suites первый общий запуск перешёл к полному
`canonical_decoder_mutations` integration corpus (31 851 mutations). Его
повтор намеренно остановлен через Ctrl+C: прежний документированный прогон
занимал 2647.50 s, а изменённого decoder code в этом аудите нет. Этот corpus
не считается заново пройденным, Ctrl+C не классифицируется как обнаруженный
продуктовый сбой. Последующие protocol mutation/fuzz-seed integration harnesses
того запуска не выполнены. Ratchet/session затем запущены отдельно и завершились
`status=passed`.

Локальные полные логи: `.tmp/astra-audit-readiness-20261022.log`,
`.tmp/astra-audit-core-tests-20261022.log`,
`.tmp/astra-audit-ratchet-session-tests-20261022.log`,
`.tmp/astra-audit-probe-build-20261022.log`,
`.tmp/astra-audit-probe-run-20261022.log`.
Это не полный workspace regression и не свежий field acceptance.
