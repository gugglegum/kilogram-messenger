# Текущая точка: M1.2.41 завершён, следующий M1.2.42

Дата: 2026-10-21.

M1.2.41 собрал M1.2.38 read, M1.2.39 authorization и M1.2.40 mutation в один
dormant dispatcher. Plan остаётся no-create, Commit требует opaque exact Plan
stage и тот же descriptor, authorization durable до mutation, completed receipt
durable до Device-signed response. Exact replay не вызывает loader/consume.
Проверки: stable-path 2/2, boundary 10/10, readiness 167/167.

## Следующий этап: M1.2.42 — authenticated runtime registration

1. Добавить отдельный authenticated active runtime command для consumption
   ceremony, не меняя transport wire и public `legacy-only` default.
2. Runtime должен владеть bounded ephemeral Plan-stage cache: exact descriptor,
   request digest, Device и TTL; restart/expiry очищают authority, Commit без
   exact live stage fail closed.
3. Plan остаётся read-only. Commit требует explicit phrase/approval и вызывает
   только M1.2.41; response остаётся current-Device-signed.
4. Не добавлять CLI/desktop control в этом этапе и не выполнять product network.
5. Добавить authenticated loopback fixtures, boundary/RFC/manifest, следующий
   readiness gate, rustfmt и locked/offline strict Clippy, затем commit/push.

## Stop conditions

- runtime cache переживает restart или TTL;
- Commit принимается без exact same-runtime Plan stage;
- Plan создаёт durable authorization/mutation state;
- unsigned outcome либо identity secret выходит в IPC;
- CLI/desktop получает mutation authority преждевременно;
- transport wire/public default меняются ради прохождения теста.

# Завершённый расширенный пакет: M1.1.92–M1.2.5

Статус на 2026-10-19: M1.1.92–M1.2.5 реализованы и проверены; M1.1.95
зафиксирован commit-ом `14f7db3`, основной dormant control-plane пакет —
`2f005c1`. M1.1.98 и полевая часть M1.2.5 закрыты exact-binary two-host run
`m1205-rollout-20261019T071251Z` на основном ПК и физическом Windows-ноутбуке.
На обоих хостах прошли 56/56 rollout tests, named rehearsal и оба no-create
observer checks; SHA-256 бинарников совпали. Public rollout остаётся `hold`,
default — `legacy-only`.

Пакет расширен по просьбе пользователя до полноценного ночного окна. Версия
здесь означает завершённый проверяемый контракт, а не искусственный временной
интервал; работа не замедляется ради часов. После каждого устойчивого результата
разрешены commit и push.

## M1.1.98 — exact-binary two-host portability evidence

- собрать не-сетевой rollout test harness и обычный CLI в debug с постоянными
  именами файлов;
- выполнить один и тот же SHA-256 на основном ПК и тестовом Windows-ноутбуке;
- на обоих хостах доказать полный rollout regression и absent-store/no-create
  read-only observer;
- хранить в tracked evidence только revision/hash/counts/host labels, без state,
  Device/Account identifiers, raw logs и секретов.

Статус: выполнено тем же exact revision `2f005c1`, который проверен M1.2.5;
compact evidence сохранён в
`CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-TWO-HOST-EVIDENCE.json`.

## M1.1.99 — offline release trust root и bounded keyring

- отделить долгоживущий offline trust root от операционных policy-signing keys;
- ввести root-signed monotonic keyring с bounded набором key certificates,
  exact key IDs, назначениями и явным отзывом;
- reject unknown signer, key substitution, fork, rollback, skipped epoch,
  unauthorized purpose и oversized keyring;
- не встраивать production private/public key и не добавлять сеть.

## M1.2.0 — rotation-aware signed policy envelope

- exact-связать policy с keyring epoch, signer certificate и release artifact;
- разрешить безопасную ротацию операционного ключа без перепривязки локального
  trust root;
- containment permit сделать уже normal rollout permit;
- сохранить старый single-key format только как неподключённый исторический
  primitive, не как permissive fallback.

## M1.2.1 — atomic policy inbox and quarantine

- принимать policy/keyring только из явно указанного локального файла;
- сначала bounded read + canonical decode + full verification, затем atomic
  staging/commit;
- invalid, stale, forked, partially written и symlink input помещать в
  fail-closed diagnostic outcome, не меняя durable head;
- auto-download, remote admin и background polling отсутствуют.

## M1.2.2 — Device-signed rollout approval bridge

- превратить controller proposal в exact Device-bound activation plan;
- требовать отдельную явную подпись текущего Device и optimistic exact-head
  match перед apply;
- запретить release key, CLI observer, desktop toggle и network input напрямую
  подписывать либо применять activation;
- exact replay сделать идемпотентным, stale proposal — отклоняемым.

## M1.2.3 — explicit containment recovery ceremony

- добавить отдельный Device-signed recovery record, который ссылается на exact
  containment head, healthy policy/health evidence и свежий recovery nonce;
- запрещать implicit clear, remote clear, epoch skip и восстановление без
  уже существующей authenticated canonical evidence;
- проверить crash/restart/replay/fork и сохранение reader/resume rights.

## M1.2.4 — operator UX без скрытой authority

- вывести trust/keyring/policy/controller/approval/recovery состояние в
  privacy-safe CLI и desktop read-only view;
- plan/apply остаются отдельной осознанной церемонией с точным summary;
- никакого одного toggle, auto-enrollment, cohort bucket/seed или secret в UI;
- обычный запуск без policy остаётся прежним `legacy-only`.

## M1.2.5 — end-to-end rehearsal и расширенная readiness

- двуххостовый debug-only прогон key rotation, policy import, proposal,
  explicit approval, containment и explicit recovery на test-only state;
- negative matrix для foreign/stale/truncated/symlink/fork/replay;
- обновить aggregate readiness, strict Clippy, rustfmt и stable full regression;
- зафиксировать честный итоговый статус. Accept-canary и изменение public
  default запрещены без отдельного решения и независимого security review.

## Предыдущая точка: M1.1.91 завершён

Статус: `canonical-sync-v2-n-minus-one-production-interop-verified`; public
rollout по-прежнему `hold`, default остаётся `legacy-only`.

Exact accepted M1.0.0 binary и successor с revision `6d95cb7` прошли на двух
физических Windows-хостах четыре N/N-1 направления: current -> previous и
previous -> current по direct и strict relay. Каждый сценарий включал delivery
ACK, restart listener, sync и exact equality history inventory. Отдельная
forward-only цепочка на одном durable state прошла N-1/N-1 -> N-1/N -> N/N с
ростом истории 6 -> 12 -> 18 и финальным relay.

Полевой прогон обнаружил и исправил три совместимости:

1. outer connection-ticket envelope/signature возвращён к exact M1.0.0 v11;
   новый sync capability стал independently signed optional extension;
   это один v11 reader двух JSON shapes, а не version-level dual reader;
   superseded experimental v13 не принимается;
2. intermediate SyncDiff завершает stream без взаимной блокировки на
   `send.stopped`; receipt доказывает обязательный follow-up request;
3. старому debug binary требовалось около 53.8 s на durable import четырёх
   events, поэтому follow-up accept получил отдельный bounded timeout 120 s.

Tracked evidence не содержит raw logs/state, Account/Device IDs или message
bodies. Полевой harness использует постоянный `kilogram-cli.exe` path на обоих
хостах. На Windows сетевые Rust tests нельзя запускать прямым `cargo test`:
только `scripts/run-cargo-tests-stable.ps1`, иначе hash-named test EXE снова
создаёт новый Windows Firewall prompt.

Открытое ограничение: до первого durable canonical selection удаление optional
capability из ticket неотличимо от genuine M1.0.0 peer. Уже selected session не
может откатиться в legacy, но accept-canary нельзя открывать до отдельного
решения по этой first-contact ambiguity и independent review.

Следующий пакет — M1.1.92: dormant rollout controller + operational containment
rehearsal. Нужно выбрать границу production release key/policy delivery,
подключить cohort evaluation к существующей Device-signed activation ceremony
без automatic enrollment, затем доказать canary stop/containment/restart и
отсутствие legacy fallback для уже selected sessions. Independent security
review остаётся внешним обязательным gate до accept-canary.

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
