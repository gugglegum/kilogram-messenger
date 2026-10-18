# Текущая точка: M1.1.88 завершён локально

Статус: `canonical-sync-v2-guided-restart-verified`; публичный default
остаётся `legacy-only`.

Acceptance: desktop `28/28`, dedicated positive/negative restart state-machine
tests и fail-closed boundary self-test зелёные; accepted-tag readiness
`115/115`, rustfmt и locked/offline strict Clippy прошли.

Windows desktop теперь проводит bounded lifecycle для собственного runtime:
authenticated shutdown, фактический `Child` exit, offline authority status,
exact plan/apply, spawn нового child и authenticated connect. До обычной
загрузки conversations новый runtime обязан вернуть exact applied Device,
record/epoch/state/disposition/evidence/rights, equal loaded/configured policy,
`configured_matches_loaded=true` и `restart_required=false`.

Mismatch немедленно завершает новый desktop-owned process и оставляет клиент
disconnected. Startup exit/timeout также fail closed. Уже применённый durable
record не откатывается молча: exact ожидание сохраняется в памяти для retry,
который не добавляет новый authority record. Load/save другого launch profile
сбрасывает workflow. IPC остаётся v30/domain v11; hot apply и unchecked toggle
не появились.

Следующий отдельный пакет — M1.1.89: decision-readiness для публичного
dual-reader rollout. Он должен собрать явные критерии допуска, cohort/rollback
границы и доказательства, но не обязан автоматически менять публичный default.
Canonical-native history materialization и legacy retirement остаются
отдельными последующими решениями.

## Исторический план M1.1.81–M1.1.85

Статус: завершён; итоговый статус
`canonical-sync-v2-opt-in-dark-launch-verified`, публичный default остаётся
`legacy-only`.

Оценка: 5.5–7.5 часа чистого времени при отсутствии внешних сетевых сбоев.

Цель: выполнить первый обратимо управляемый dark launch canonical sync v2 в
штатном runtime. Обычный запуск и все существующие пользователи остаются в
`legacy-only`: Postcard v6 / `kilogram/m0/sync/8`. `/cbor/2` может быть включён
только явной локальной политикой на специально выбранных тестовых Devices.

Этап не меняет публичный default, не удаляет legacy reader, не создаёт release,
ZIP или tag и не включает автоматический GitHub build. Он должен превратить
проверенный evidence-only transport в контролируемый opt-in путь того же
runtime, не ослабив запрет fallback после authenticated canonical selection.

## Исходная точка

- M1.1.80 закрыт commit-ом
  `478bd71049585fe0dc19901858b5c895b843fbff`;
- exact implementation revision
  `b1f23c3719a0842979f047d874141953ae93ccac` подтверждена independent hosted
  run `39116855502` и двухмашинным Windows field run
  `m1180-field-20261018T093610Z`;
- все 18 targeted canonical surfaces и все activation requirements verified;
- `changes_active_wire=false`, `/cbor/2` не зарегистрирован и не dial-ится
  обычным runtime;
- default и единственный публично активный transport — Postcard v6;
- текущее имя состояния `advertise-cbor-v2` семантически неоднозначно: manifest
  называет его Postcard-only reader, хотя capability нельзя публиковать как
  доступную до готовности безопасно принять соответствующий ALPN;
- `dual-reader-prefer-legacy` и `dual-reader-prefer-cbor` также требуют точного
  определения того, кто публикует capability, кто принимает `/cbor/2` и кто
  первым его dial-ит. Название состояния не должно подменять эти три независимые
  разрешения.

## Ожидаемый итог

1. Есть один machine-readable activation policy и RFC с точной матрицей
   `publish / accept / initiate / resume` для каждого migration state.
2. Policy хранится атомарно, привязана к Device и monotonic migration epoch,
   а tamper, fork и rollback блокируют activation.
3. Новый и существующий профиль без явной policy остаётся `legacy-only`.
4. Штатный runtime умеет opt-in `/cbor/2` без отдельной evidence-only логики.
5. Два явно выбранных тестовых Devices проходят direct v2 sync, restart/resume,
   mixed-version control и selected-v2 fail-closed test.
6. Аварийное containment запрещает новые v2 sessions, но сохраняет reader и
   возможность resume уже выбранной canonical session; оно никогда не
   превращает canonical failure в legacy retry.
7. Public default, release artifacts и пользовательская история не меняются.

## Рабочие пакеты

### M1.1.81 — activation semantics и decision record — 45–60 минут

1. Разделить transport policy на шесть независимых прав:
   - сохранять либо отключать legacy reader;
   - публиковать authenticated `/cbor/2` capability;
   - регистрировать `/cbor/2` ALPN;
   - принимать новую mutually selected `/cbor/2` session;
   - инициировать новый `/cbor/2` dial;
   - продолжать уже выбранную durable canonical session.
2. Сверить имена состояний в Rust, JSON ledgers и RFC. Если прежнее имя
   `advertise-cbor-v2` допускает unsafe interpretation, уточнить его семантику
   без переиспользования wire identifiers.
3. Зафиксировать exact forward transitions, explicit one-step rollback до
   canonical evidence и отдельное emergency containment после selection.
4. Создать RFC и `CANONICAL-SYNC-V2-ACTIVATION-POLICY.json` с negative
   fail-closed matrix.
5. Добавить verifier, который запрещает capability publication без matching
   listener/read capability и запрещает legacy fallback после selection.

Контрольная точка: state names больше не скрывают поведение; из одного manifest
однозначно видно, что endpoint публикует, принимает, инициирует и возобновляет.
Active transport пока не меняется.

### M1.1.82 — durable local activation authority — 60–75 минут

1. Реализовать versioned Device-bound activation record с monotonic epoch,
   exact previous-record binding и bounded reason/actor metadata.
2. Использовать atomic no-clobber/replace discipline и существующие
   anti-rollback primitives; не вводить environment variable или скрытый
   permissive flag как источник authority.
3. Добавить команды `status`, `plan` и `apply` для явной локальной операции.
   `plan` не меняет состояние, `apply` обязан exact-match предыдущий record.
4. Missing, malformed, stale, forked или относящаяся к другому Device policy
   должна давать `legacy-only` либо fail closed, но никогда не активировать v2.
5. Проверить restart, interrupted write, replay, epoch skip и wrong-Device
   cases.

Контрольная точка: обычный state directory без activation record ведёт себя как
до этапа; включение нельзя получить случайным CLI argument, повреждённым файлом
или откатом каталога.

### M1.1.83 — production runtime dark-launch integration — 90–120 минут

1. Вынести проверенный Iroh `/cbor/2` adapter из evidence binary в общий
   runtime module, не копируя state machine и typed store.
2. Строить endpoint ALPN list строго из verified activation policy:
   - legacy ALPN остаётся зарегистрированным во всех opt-in состояниях этого
     этапа;
   - `/cbor/2` регистрируется до его capability publication;
   - dial разрешается только после mutual authenticated selection;
   - resume exact persisted canonical session не зависит от текущего желания
     начать новые sessions.
3. Связать client/responder dispatch с уже проверенными durable adapters и
   checkpoint-bound frame journal.
4. После выбранного `/cbor/2` любая decode, network или state error завершает
   attempt; Postcard fallback отсутствует.
5. Добавить stable telemetry fields для policy state, migration epoch,
   selected ALPN и containment, не записывая message contents или secrets.

Контрольная точка: один обычный `kilogram-cli` debug binary поддерживает оба
reader-а только при точной opt-in policy; без неё его endpoint остаётся
legacy-only. Отдельный evidence executable больше не является единственным
местом реального v2 transport.

### M1.1.84 — local и двухмашинный activation rehearsal — 60–90 минут

Сначала на одном хосте, затем на основном ПК и ноутбуке через уже настроенный
OpenSSH выполнить на свежих test-only state directories:

1. default/default control: только Postcard v6;
2. opt-in/default mixed-version control: согласованный legacy path без ложной
   canonical selection;
3. opt-in/opt-in: exact `/cbor/2`, bidirectional convergence;
4. process restart после durable progress и exact resume без duplicate/loss;
5. deliberate selected-v2 corruption/connection loss: fail closed, zero legacy
   retry;
6. pre-selection explicit one-step rollback;
7. post-selection containment: новые v2 attempts выключены, но persisted
   canonical evidence не стирается и не трактуется как legacy;
8. повторный обычный запуск доказывает, что public default не изменён.

Для этого пакета достаточно одной LAN; повторять мобильную/relay топологию не
обязательно, потому что реальный relay path exact candidate уже подтверждён в
M1.1.80. Если adapter или route selection существенно изменятся, relay control
снова становится обязательным.

Контрольная точка: exact commit и binary SHA совпадают на обоих устройствах,
manifest содержит реальные Iroh telemetry и закрытые immutable log copies, а
истории совпадают. Старые state directories и Yandex Disk как coordination
transport не используются.

Результат: принят exact-revision run `m1184-exact-20261018T165652Z` на
`7ee5ef808cb56c3c48d7a0b6dc07c0984b81992c`; один debug binary SHA-256
`be8ca16754c2ad1291d4ea56c0a339b6e6a0f5994e6816c1c3d966871f42c532`
использован на `PAUL_HP_OMEN` и `DESKTOP-PAUL-RY`. Все шесть сценариев
прошли, включая exact resume, containment и selected-loss без legacy retry.
Field probe отдельно подтвердил ответы 1–8123 bytes и позволил исправить
смешение 20-секундного stream-I/O deadline с подготовкой responder: первое
ожидание ответа теперь ограничено двумя минутами, отдельные I/O — 20 секундами.

### M1.1.85 — regression, formalization и commit — 75–105 минут

1. Прогнать все изменённые boundary self-tests и negative mutation tests.
2. Выполнить readiness, rustfmt, locked/offline strict Clippy и полный stable
   Rust regression.
3. Legacy и v2 decoder mutation corpora повторять только если менялись decoder,
   frame codec или их strict validation surface; восьмичасовой fuzz campaign
   без такого изменения не повторять.
4. Обновить canonical migration, transport и coverage ledgers. Новый статус
   должен честно отличать `opt-in-dark-launch-verified` от public activation.
5. Зафиксировать результаты в RFC, roadmap и отдельном commit; push разрешён
   после чистого финального verification.

Итоговый статус: `canonical-sync-v2-opt-in-dark-launch-verified`, но default
остаётся `legacy-only`. Переход public default в dual-reader mode, rollout через
desktop UI и тем более legacy retirement — отдельные последующие решения.

Результат: все изменённые boundary self-tests и negative mutations прошли;
accepted-tag readiness закрыл `112/112`, rustfmt и locked/offline strict Clippy
зелёные. Stable runner с profile `test`, двумя Cargo jobs и isolated test до
aggregate harnesses завершился `status=passed`. Не запускались только два
неизменённых уже принятых mutation-corpus теста, как и было предусмотрено
планом. Release, ZIP, tag и автоматический GitHub build не создавались.

Следующий отдельный пакет — M1.1.86: безопасная desktop UI для просмотра
текущей activation policy и явной plan/apply церемонии. Она не должна менять
public default, скрывать последствия включения или превращать UI-toggle в
самостоятельный источник authority.

## Stop conditions

- capability публикуется раньше matching listener/read readiness;
- два состояния с разными именами дают неразличимое поведение без явно
  документированной причины;
- policy можно включить простым tampered file, environment variable или stale
  epoch;
- обычный профиль регистрирует либо dial-ит `/cbor/2`;
- selected canonical error вызывает Postcard retry;
- containment удаляет reader или durable canonical evidence;
- после field run меняется protected runtime/protocol surface без повторной
  сборки и затронутых прогонов;
- обнаружена потеря, дублирование или изменение ранее сохранённой истории;
- для прохождения теста требуется ослабить authority, membership или checkpoint
  validation.

При любом stop condition этап не получает новый milestone status. Default
остаётся Postcard v6, а найденная граница документируется как blocker.

## Ресурсная и операционная политика

- debug builds, без release/ZIP/tag;
- Cargo jobs 2 по умолчанию, existing target cache сохраняется;
- GitHub Actions только manual и лишь если локальное/двухмашинное evidence
  выявит host-specific uncertainty; push сам по себе build не запускает;
- ноутбук требовался только для M1.1.84; exact двуххостовый run уже завершён,
  M1.1.85 выполняется локально;
- никаких фоновых OS services, Task Scheduler jobs или постоянных firewall
  exceptions сверх уже используемого стабильного executable path;
- bounded evidence содержит hashes, counts и transport metadata, но не state,
  secrets, plaintext history или release binaries.
