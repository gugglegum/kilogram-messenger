# A3-S4: production owner, два процесса и offline-доставка

2026-10-25, Asia/Yekaterinburg. **Контракт принят: [root review2](long-lived-dialog-a3-runtime-s4-contract-review-2.md). Реализация не начата.**
Baseline `7f82036ee5803edd64be98b73572604250f5961c`, branch `master`.
Итерация2: адресный repair по [contract review1](long-lived-dialog-a3-runtime-s4-contract-review-1.md),
SHA256 `f2631bb7a48de92a06a205f2c0d016c719f37acc978c944ce9aeccc9e298d9c1`.
Review1 и предыдущие принятые документы остаются frozen; решения ниже приняты review2.
Основание — [S4 task](long-lived-dialog-a3-runtime-s4-task.md), 9900 bytes,
SHA256 `7c99018a8bd4c5cb7a7aed121b77612917749ebef9d8f6e6662ede45aecf44bd`.

Нормативны [A3 contract](long-lived-dialog-a3-contract.md), особенно §2/3/6–9,
[его review2](long-lived-dialog-a3-contract-review-2.md), [S3 plan](long-lived-dialog-a3-protocol-s3-plan.md)
и [S3 implementation acceptance](long-lived-dialog-a3-protocol-s3-implementation-review-1.md).
Их wire layouts, counters, credits и historical evidence не переписываются.
Исключение, требующее явного решения root до product edit, — недостающий relay locator
из §8 ниже. Остальные предлагаемые новые форматы — device-local runtime metadata.

## 1. Рекомендуемая граница

S4 вводит **настоящий production runtime для fresh L2**: обычные CLI/enrollment,
authenticated IPC, durable FreshHandshake→Active2, current-policy owner, indexed
queue/materialization/ingress/receipts, direct/own relay и restart/recovery. Одни и те
же production функции вызываются CLI, сетью и process acceptance. Test driver не
создаёт за runtime native state, ModeProof, source rows или readable projections.

Результат S4 — два отдельных процесса с двумя C между двумя single-device участниками,
долгим encrypted dialogue, реальной остановкой peer и доставкой backlog после restart.
A остаётся sole dialer/coordinator, B — listener, locators передаёт оператор.
Это продолжение принятого A3, а не очередной компонентный slice.

**Реальный V4→V5 file upgrade, indexed L1 compatibility adapter и existing P/A/K/Committed
handoff предлагается реализовать следующим этапом S5.** Это отдельное решение для
root review, не объявление этих требований выполненными. S4 сохраняет обычный V4/L1
путь и его ограничения; existing profile нельзя выдать за fresh, усыновить в L2 или
сбросить. Конкретные отказы и successor описаны в §10. Поэтому whole A3, resumed-L1
часть A3-1 и A3-8 остаются открытыми после успешного S4.

GUI/onboarding, новые contacts/devices, groups/channels, mailbox, M1.2.63,
deferred A2 reproducer и внешний/public network readiness сюда не входят.

## 2. Проверенный baseline и production seams

| Текущий код | Наблюдение и изменение S4 |
|---|---|
| [main.rs](../../apps/kilogram-cli/src/main.rs#L4045), [ordered_cli.rs](../../apps/kilogram-cli/src/ordered_cli.rs#L147) | Explicit launch mode сейчас распознаёт только v1 и отвергает другой явный mode. Добавить typed dispatch V1/V2/ordinary до mutable repositories; неизвестный mode сохраняет отказ. `main.rs` остаётся dispatch. |
| [runtime.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime.rs#L1127) | Открывает `OrderedVault`, создаёт `Engine`, `Arc<Mutex<Engine>>`, отдельные QUIC tasks. Этот путь остаётся для V4/L1; schema5 открывает новый owner. |
| [engine.rs](../../apps/kilogram-cli/src/ordered_dialog/engine.rs#L69) | Engine владеет V4 store, Device, contexts, leases и history cache; `save` требует lease для native mutation. Нельзя подставить Guard2 и оставить эти assumptions для offline L2. |
| [outbox.rs](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L195) | `materialize_one` сканирует paths, ограничивает unacked128 и требует lease. Новый путь использует typed seek и disk-backed debts; прежний метод не вызывается L2. |
| [state.rs](../../apps/kilogram-cli/src/ordered_dialog/state.rs#L166) | PrivateState payload содержит Guard1 и phase-specific AAD. Нужен отдельный NativeState2 на том же public native adapter, без изменения старого encoding. |
| [IndexedDialogVault](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L367) | Уже держит lifetime lock, schema5 root/Device secrets, quotas, Immediate durability и common writer. Публичного owner bootstrap/metadata API и production activation ещё нет. |
| [HistoryProtocol2Operation](../../crates/kilogram-state/src/vault/indexed_dialog/history_protocol2.rs#L30) | Init/Genesis/TextBatch/Pending/Acknowledge дают closed correspondence, но не сохраняют runtime phase/current policy/Ready2 в той же операции. Init требует D/E/SID: его нельзя применить к OfferedA/GrantedB с fake SID. |
| [history.rs](../../crates/kilogram-state/src/vault/indexed_dialog/history.rs#L144) | Есть bounded history/point/request API. Request outcome не экспортирует body, нет typed runtime queue/source/receipt iterator. Добавить узкие readers, не paths/full snapshot. |
| [ordered transport](../../crates/kilogram-transport-iroh/src/ordered.rs#L14) | Сейчас loopback, ALPN1, relay отключён. Новый adapter2 использует Codec2, real exporter и explicit Minimal/operator builder §8.1. Общий N0 builder для S4 не вызывается. Старый adapter не меняет формат. |
| [Descriptor2](../../crates/kilogram-protocol/src/ordered_dialog_v2/authority.rs#L511) | S3 parser принимает девять полей body с IP addresses, без relay URL и public interval/revision accessors. Одних runtime флагов для signed relay locator недостаточно; §8 требует явной поправки. |

Предлагаемые production modules: `ordered_dialog/runtime2/{mod,owner,repository,
policy,private_state,handshake,outbox,ingress,repair}.rs`. Это конкретная граница
ответственности; небольшие файлы можно объединить без смены владельца состояния.
`ordered_dialog/mod.rs` экспортирует runtime2 обычным build, а не cfg(test).
`ordered_cli.rs` и `main.rs` добавляют dispatch/admin/IPC commands; IPC transport
[RuntimeIpcCommand::OrderedDialog](../../crates/kilogram-runtime-ipc/src/lib.rs#L722)
сохраняется, вложенный codec выбирается по authenticated runtime mode.

State changes локальны: `indexed_dialog/runtime2.rs`, private lowering в common
`Work/CommitInput`, typed readers и расширение namespace/fences. Ratchet primitives,
Native config1, Cargo dependency graph и Codec1 не меняются. Новых dependencies
не ожидается; если реальный seam требует их, сначала отдельный вопрос root.

## 3. Один владелец и ограниченная работа

```text
CLI admin (Root ceremony outside runtime) -> signed public artifacts
CLI ordered commands -> authenticated local IPC -> bounded owner mailbox
QUIC adapter2 -> verified admission / bounded frames -> owner mailbox
owner thread -> current-policy check -> trial -> closed atomic indexed writer
            -> committed exact records -> dispatch permit -> QUIC adapter2
history IPC -> тот же owner -> bounded indexed history reader
```

Один `Owner2` на profile, один `StateDirectoryLock`, один `IndexedDialogVault`.
Dedicated owner thread владеет DB, Device secrets, двумя NativeState2 и bounded
current metadata. Tokio tasks получают только handle/oneshot, connection и owned
encoded frames. Они не получают mutable vault/native references. Offline worker —
очередная команда тому же owner, не второй writer или второй Engine.

Lock приобретается до открытия DB/keys; `AlreadyLocked` означает Busy. Существующий
[schema-aware recovery fence](../../crates/kilogram-state/src/lib.rs#L950) сохраняется:
prepared legacy journal над schema5 запрещён. Admin с живым runtime использует IPC;
offline enrollment/profile-create использует тот же lock и никогда не идёт параллельно.

Owner сериализует send/receive/authority/receipt по всему profile, включая shared
Device author_sequence. Не держит native trial/DB read transaction через network
await. Native batch≤4 frames либо одна admin/control operation; yield после каждого
такого turn. Ready work C1/C2 обслуживается round-robin: при готовом C2 следующий
data turn не отдаётся C1 повторно. Reauth/ACK/head repair имеют bounded приоритет,
а не бесконечный цикл, вытесняющий другой C. Per-C fork/gap блокирует C1, shared DB
IO/reopen/own revocation могут остановить оба — это явно общий ресурс.

Profile-total scheduling window≤128 **различных queue/undelivered refs**, включая
prefetched/in-flight selections обоих C. При двух enabled C каждому выделено≤64,
**без заимствования** его slots другим C; при единственном enabled C доступны128.
Enabled set меняется только при restart launch profile, когда прежние RAM selections
уже уничтожены. Поэтому C1 с64 занятыми refs и ожидаемыми ACK/gap не забирает базовые
64 C2: ready C2 получает admission и следующий fair turn без ACK/progress C1.
Disk debt/pending не занимают scheduling slots сами по себе и не имеют лимита128.

Slot возвращается после materialization commit/discard, отказа/устаревания prefetch,
окончания ограниченного transport round либо его timeout/cancel/lease invalidation.
Если IDs нужны для Complete/correlation, они входят в cap до конца round; deadline15s,
никакого удержания до Delivered или до устранения peer gap. Release slot удаляет
только RAM selection, не durable queue/source/debt/receipt; повтор читает exact bytes
из store. После IO/close cancellation освобождает permit/refs/buffers. Не читать все
unacked ради count: counters из root/ledger, work — authenticated seek/range.
Pending не кэшируется целиком. P07 наблюдает отдельные occupancy и освобождение slots.

Mailbox≤16 full-frame commands, каждый≤1MiB; сверх него backpressure до чтения
следующего payload. **Два profile-total connection permits включают TLS/route/application
admission и established connections**, а не только уже admitted. Inbound accept loop
делает try-acquire до await TLS/spawn/read-frame allocation; если места нет, Incoming
refused/dropped без новой application task. Outbound A берёт permit до connect/spawn;
одна outstanding dial/reconnect attempt на C. Permit переносится в established link,
не дублируется. TLS/connect deadline15s, весь TLS+route+Hello/Proof/Admitted≤30s;
это один absolute deadline от получения permit, phase timeout ограничен remaining
time, а не отдельные складывающиеся30s для каждого шага.
Admission failure/close/timeout освобождает permit и временные buffers. После проверки
C действует также exclusive admitted-link gate этого C. Общие Iroh UDP/QUIC buffers
считаются в memory measurements; не обещается нулевая внутренняя allocation библиотеки.
Per connection≤4 bidi streams, включая admission stream, без uni; stream permit
получается до application spawn/read, idle/round timeout≤15s. IPC и listener не
создают unbounded pending tasks за этими gates. Все tasks/response buffers учитываются
вместе, в том числе ещё не admitted. IPC request≤32KiB,
history response≤48KiB, page limit≤32. Не выдавать slice счётчика за memory bound:
инструментировать также queued commands, native trials, frame buffers и plan.
Сохраняются redb cache64MiB, A3-owned buffers≤192MiB, profile PrivateBytes≤384MiB,
MAX_PLAN16MiB и physical policy A3 §6.1. Не повышать их под новый actor молча.

## 4. Durable runtime state и закрытый repository

### 4.1. Идентичность, bootstrap и local format

`RuntimeProfile2` — authenticated признак production profile, создаваемый с новым
runtime root encoding, account/claim/Device keys, никогда из JSON. S3 component DB без этого признака
возвращает `UnsupportedRuntimeProfile`; loader не конвертирует её в приложение.

<a id="r1-runtime-root"></a>

### 4.1.1. R1: authenticated root discriminator и predecessor fence

Namespaces/marker сами по себе не останавливают S3 reader: его
[Root::decode](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L255) принимает
все64 summaries. Выбирается **отдельный runtime root encoding6 внутри schema5 container**:

```text
ComponentRoot5_plain = 05 || Body5
RuntimeRoot6_plain   = 06 || 4b 47 4f 44 32 52 54 00 || 00 01 || Body5
                             ASCII "KGOD2RT\0"        layout_rev=1 (u16 BE)
```

`Body5` — точная последовательность полей существующего `Root::encode` после его
первого05: incarnation/account/claim/Device secrets/contexts, generation/commit_id/
request_digest,64 summaries,2 ledgers. Component encoding05 и его field codecs остаются
byte-exact. Runtime prefix обязателен; parser требует exact magic/layout_rev1,
строгий конец Body5 и прежний32KiB envelope cap. Unknown discriminator/layout revision,
лишний suffix или missing/mismatched marker — отказ до writable runtime handle.
Будущее несовместимое runtime record/purpose изменение обязано увеличить layout_rev;
одного нового marker/unused namespace без этого недостаточно. F10 rev2 — проверка
отказа текущего reader, не утверждение о поведении ещё не написанного future binary.
Это local root/layout version, **не** смена wire2 или outer schema sentinel на6.

Envelope `[incarnation32 || sealed_root]`, существующие root key/AAD и schema5 sentinel
с digest полного encrypted anchor сохраняются. Prefix06/magic/revision находится
**в AEAD plaintext**, значит runtime/component выбор аутентифицирован, а marker
дополнительно связан через roots[22]. Существующий S3 decoder после успешного AEAD
сразу отвергает06 как `Integrity("anchor version")` на проверке первого byte;
он не выдаёт writable API и не доходит до Queue/Protocol2/generic mutation. Это
source-backed отказ, не предположение о неизвестных ему slots. Поля/namespace names
в JSON не влияют на этот discriminator.

Первый durable runtime root уже06 и содержит marker/claim/saved keys в той же
transaction. Component-compatible runtime root05 никогда не коммитится даже временно.
Все следующие runtime commits сохраняют06/magic/rev1; CAS/reopen/last-commit path
проверяет тот же layout. Нет API flatten/export runtime root в writable component05.

В новом коде public component `IndexedDialogVault::open` принимает только05 и
отвергает06 до выдачи component handle; `RuntimeRepository::open` принимает только06
с valid marker. Private common core выбирается explicit typed loader, без decoder
fallback. Component05 с заполненными runtime-reserved slots21/22/45/46 тоже invalid;
легитимные S1–S3 profiles этих slots не использовали. Runtime slot46 всегда запрещён.
Все same-version public component/generic/history mutations над runtime handle
отказывают `RuntimeWriterRequired` по authenticated layout **до** retry fast path;
readonly history может использовать общий private core. Marker не единственный fence.

Это accidental old-version/API downgrade fence, не защита от злонамеренного local
кода, который имеет master/Device secrets и сам переписывает ciphertext. F10 проверит
отдельно same-version обходы, future/unknown layouts и **exact S3 predecessor7f82036**;
одного V4 d9b EXE для этого свойства недостаточно. Component fixtures/encoding frozen.

Для минимального schema5 расширения предлагаются namespaces `Runtime=21`
(context slots21/45) и `RuntimeProfile=22` (только context0/slot22; context1 запрещён).
Они помещаются в существующие ROOTS64 и не меняют discriminants0–20. Это **новый
local storage layout на review**, а не утверждение, что S1 уже содержит эти records.
RuntimeProfile marker обязателен и immutable. Generic mutations этих namespaces
запрещены и до marker; после marker generic/history component commits вообще не
могут менять production profile. Только `CommitInput::Runtime2` проходит runtime
lowering. Проверка выполняется до exact-retry fast path, не после него.

Новые records имеют собственные local version/domain tags; открытый тип не принимает
arbitrary key/value mutations. Все references разрешаются hash/type/scope-checked:

| Запись | Содержимое и bound |
|---|---|
| Profile marker | local version1, runtime mode2, Root incarnation/account/claim/Device cert ref, два immutable C/slot mappings, owned Device author slot. Ни Account Root secret, ни filesystem paths. |
| Current profile policy | latest own/peer signed authority, pinned Account Root public IDs, monotonic revisions, policy_generation, next own Device author_sequence, committed wall-clock high-water. Не более двух participants; shared между C, без per-event копии current pins. |
| ContextHead2 | C/role, context_generation/native_generation, native phase и activation stage отдельно; реальные optional Offer/Grant/D/E/SID/Q, proof/selected-pair/selected-consent/control refs, Ready refs, blocked reason. Source/readable frontiers проверяются против typed history, не выбираются hint. |
| Context current policy | current bilateral Consent2 и его ref, membership/cert refs; selected historical consent отдельно. Renewal не меняет selected proof/Q. |
| Locator current records | own/peer exact signed locator, highest revision, same-revision exact bytes, expires; новые endpoint/IP/relay не меняют C/D/E/SID. |
| Pre-session native | phase-tagged sealed OfferedA/GrantedB,≤128KiB complete blob; Q/D/E/SID optional по phase. При первом Genesis перемещается в единственный authoritative Native slot одним commit. |
| Runtime controls/intents | bounded Offer/Grant/selected initial pair/Ready refs, ACK scheduling metadata, monotonic peer source watermark. Large immutable Event/receipt/Auth bytes остаются в принятых namespaces; новых копий всей истории нет. |

До InstallContext peer pins действительно отсутствуют; это optional pre-context
state, не fake participant. Own next author_sequence начинается с0 и shared между C.

Значение≤512KiB, общие control/runtime metadata≤4MiB/C, полная retained стоимость
в ledger. Current records заменяются с корректным used-byte accounting; historical
captures O/A/F/R живут по прежним immutable links и оплачиваются frame credit.
Runtime metadata не добавляется в `Control`13: S3 cardinality
`2+source_count+receipt_marker_count` и mandatory correspondence сохраняются.
Pre-session native retirement не удаляет sole source/native без durable replacement.
Новые Head/profile/cursor mutations входят в прежние24 point mutations/frame и
16MiB total plan, не прибавляются сверх лимита. Worst-case encode/accounting gate
проверяет этот полный набор; если batch4 не помещается, уменьшить batch до publication,
если не помещается один frame — явный StorageBlocked, а не расширение bounds.

`runtime_owner_material(ExpectedOwner)` — новый узкий bootstrap reader: возвращает
Zeroizing Device signing/encryption material и immutable owner/context metadata из
RuntimeRoot6. Он доступен owner repository, не IPC. DeviceId/encryption public key должны
совпасть с certified identity и claim. Это Device secrets, **не Account Root**.
Не вызывать `DeviceState::load`/legacy files как fallback при missing root/keys.

### 4.2. Proposed API и атомарность

Новые API ниже — задание реализации, не существующие exports:

```text
RuntimeRepository::create(lock, claim, enrollment, config)
RuntimeRepository::open(lock, expected_owner, config)
snapshot(c) -> RuntimeSnapshot { root_gen, context_gen, policy_gen,
                               head, native_hash, bounded current records }
request(c, request16, exact_text) -> Absent | Same(Queued/EventId/Delivered) | Conflict
queue_page(c, after, limit<=128); source_page(c, lane, start, end<=start+32)
debt_page(c, after, limit<=32); receipt_page(c, lane, start, limit<=32)
pending_head(c); seen(c, role, key); history_first/page/point(...)
commit_runtime2(expected_root, commit_id, closed_operation, commit_gate) -> CommitReport
resolve_after_reopen(request/control/event identity) -> durable outcome
```

Readers декодируют records в state, проверяют generation/whole-source correspondence
и range completeness. Они не экспортируют generic path construction или mutable raw
vault. `history_request` сам по себе не доказывает same body: новый request reader
проверяет durable body binding/queued text либо materialized source+projection.
Hash mismatch/missing linked evidence — Integrity, не Absent.

Closed operation family:

| Operation | Что попадает в один common-writer commit |
|---|---|
| EnrollRuntime | Root/claim/saved Device keys + production marker + initial public enrollment/current pins. Fence до public artifacts. |
| InstallContext | exact C/role/certs/membership/current Consent2, initial runtime head; C уже входит в immutable two-slot claim. |
| PinAuthority / RenewConsent / ImportLocator / RotateLocator | проверенные public bytes, monotonic current pointer/policy_generation, affected context generation. Rotation связывает новый current locator с actual bound endpoint до export. Invalidate old leases/dispatch permits после соответствующего commit. |
| PrepareOffer / ReserveGrant | exact selected controls/initial pair/consent refs и pre-session native/account/one OTK reservation. Publication только после commit. |
| MakeInit / AcceptInitAndConfirm / AcceptConfirm | selection/head + при необходимости HistoryProtocol2 Init + Genesis row(s), global Author, Seen, authoritative native transition и retirement pre-session pointer. Один commit, не последовательность публичных commit API. |
| AcceptReady | exact own/peer Ready records и stage; Active2 устанавливается только если Work подтверждает оба readable genesis и оба Ready с exact Q. |
| QueueAccept | authenticated request/body/FIFO queue и credit. Duplicate читает прежний outcome; новая очередь оплачивается до ответа Queued. |
| MaterializeBatch | FIFO queue→Event2/source/projection/Native/Seen/request mapping/debt/credit + Head/native generation + own shared author_sequence;≤4 frames. |
| AcceptPending / DrainReadable | exact source/admission captures/temporary+permanent funding либо readable prefix/native/Seen/F/receipt/ACK intent в соответствующем одном commit. |
| Acknowledge | verified exact receipt/R/Auth, debt release/Delivered + bounded intent bookkeeping; duplicate сохраняет прежний receipt и historical refs. |
| AckDispatchProgress | exact contiguous≤32 receipt IDs от сохранённого ACK cursor; продвинуть только scheduling cursor после допущенного write. Source/debt/Native этим не меняются. |
| RepairGrant/Cancel/Refill / BlockContext | только разрешённые ledger/fork changes, bounded evidence; не generic credit action от peer. |

Private lowering вызывает existing `history2_apply`/history plans внутри **того же**
Work/root transaction, добавляет runtime-purpose checks и затем H1/H2/Author/resource
checks. Вызов `commit_history_protocol2` и отдельный commit Head/Ready недопустим:
такой разрыв не даёт production guarantee. Component API сохраняется для component
profiles, но runtime marker закрывает обход через него и через old Queue/Repair facade.
Layout discriminator из §4.1.1 обязателен дополнительно: проверка marker только
новым writer не была бы достаточным old-version fence.

Каждая mutation несёт captured root/context/policy generations и Native-before hash.
Для изменяемого shared author counter проверяется exact next; current policy changes
между capture и commit дают stale/recheck. State доказывает структурные bindings,
не MAC/current clock. Owner доказывает их до построения operation и повторно у commit.
DTO/permit не называются независимым доказательством криптографии.

`commit_gate` — синхронный reject-only owner callback непосредственно перед
`redb transaction.commit()`, после построения/проверки bounded plan. State передаёт
ему immutable purpose/fence/current-policy digest, не mutable vault/keys/операции.
Callback не может писать records, re-enter repository или делать network await;
он повторно проверяет fallible time и actual connection/lease atomics для network
purpose. Это не generic mutation escape hatch. Одной проверки до долгого plan/staging
недостаточно. После начала physical commit нельзя обещать отмену уже принятой
transaction при последующем expiry: логическая авторизация относится к последнему
pre-commit check, а новая публикация/ACK всё равно проходит свой свежий dispatch gate.

Gate refusal **после staging, но до вызова tx.commit** — отдельный результат
`PolicyRejected(reason)`: explicit `tx.abort()` с проверкой Result и discard всего volatile
Work/native trial, без accepted RAM swap, нового root generation/commit_id/source/
queue/debt/Ready/receipt или dispatch. При успешном abort и отсутствии backend error
repository остаётся usable для следующей команды; expiry/closed connection не
становятся искусственной IO ambiguity. Staging мог выполнить физические writes/grow:
не утверждать zero backend bytes или byte-identical redb file, измерять их отдельно.
Если abort/staging/backend дал IO error, либо tx.commit уже вызывался и исход неясен,
это ReopenRequired/ambiguous, а не PolicyRejected. Нельзя восстановить успех по одному
gate status. Нынешний S3 common writer при write error poison-ит workspace; новый
reject-only branch ещё предстоит реализовать и проверить, это не существующая API.

Root generation возрастает на один committed transaction; context_generation — на
изменяющую этот C операцию; native_generation — на committed native batch, а не на
каждый frame внутри batch. Packet n/j/g/T обновляются на каждый native step. Все
переходы checked; промежуточные trial checkpoints не становятся отдельными roots.

Own author_sequence shared между C; выдача и increment атомарны с event row. Не
резервировать sequence отдельным commit. Remote global Author collisions по прежним
slots отвергаются, даже если событие имеет другой C. All arithmetic checked.
RuntimeProfile фиксирует injective cert→author-slot mapping: own0, тот же peer1 в
обоих C; `HistoryProtocol2Binding.author_slots` выводится из него с учётом roles.
Нельзя дать тому же cert второй slot на C2 и обойти shared Author collision check.

## 5. Current policy, admission и commit purposes

`Policy2` хранит pinned Root public IDs, exact participant Device certificates,
membership и последние locally known signed authority/Consent2. Root signatures
проверяются существующими identity/S3 verification APIs. Certificate/Root замена не
маскируется под renewal. Revision ниже prior отвергается, equal revision требует
exact bytes, signed newer revision сохраняется даже если она отзывает доступ у
приславшего её Hello; затем admission/message processing прекращается.

Такое сохранение revocation knowledge — отдельный Policy operation; отказ peer не
позволяет продолжить native/receipt commit. Shared own/peer pin update инвалидирует
leases обоих затронутых C. Valid higher revision renewal меняет current refs, не
исторические selected proof/consent/initial pair, Q или captures старых events.

Consent2 не содержит authority revision: её нельзя выдумывать из expires. Current
consent меняется только authenticated owner import с expected-current-ref/policy_gen
CAS; incoming Hello лишь требует равенства current refs. Historical support-artifact
import не переставляет current pointer. Renewal не удаляет known revocation и не
возобновляет старые leases; новая current permission проверяется независимо от Q.

| Purpose | Допуск |
|---|---|
| LocalRead / exact request query | authenticated local IPC + owner/scope/integrity. Истечение network consent не скрывает уже readable history/accepted outcome. |
| QueueAccept нового request | current local bilateral Root2 policy и storage funding; history scope уже установлен. До SID/history Init — NeedHandshake без принятия request. Очередь может ждать Ready; это не Text encryption. |
| LocalEncrypt | durable Active2, verified selected proof/Q, current local Root2 consent/authority обеих сторон, time, generations, FIFO/funding/Native-before. **Live lease не требуется.** |
| Fresh control/activation, PeerReceive включая drain сохранённого pending, receipt creation/debt ACK, DispatchStart | всё перечисленное по scope плюс fresh admitted real TLS connection/exporter, active lease generation/deadline и current policy; phase-specific разрешённые controls. |
| ImportPolicy/Locator | authenticated owner IPC и проверенные signed pins/revisions; до network action обязателен новый admission при инвалидированной lease. Root secret runtime не нужен. |

S4 не создаёт revocation oracle. Offline permission использует последние локально
известные подписи; неизвестный offline revoke не может быть обнаружен заранее.
Known revoke, expired consent, clock/read failure запрещают новые encrypt/receive/
dispatch/ACK, сохраняя accepted queue/source/debt/history. Нет fake lease для local
encryption и нет permission из одной historical Root signature.

Production clock: fallible wall-clock UTC seconds плюс monotonic Instant для lease.
Для S4 предлагается строгая проверка `not_before <= wall_now < expires` без раннего
разрешения; wall_now ниже committed high-water даёт ClockBlocked. High-water обновляется
с изменяющими state operations; автоматического продвижения или reset времени нет.
Это явное fail-closed runtime policy decision на review. Восстановление clock —
исправить часы/дождаться high-water, затем повторить current checks; данные не удалять.
Targeted clock hook в tests не является доказательством реального истечения времени.

Lease создаётся только после actual ALPN2/TLS remote endpoint и route-policy checks,
сопоставления signed locators и полного Hello2/Proof/Admitted round. Body pins,
nonces, exact certs/membership/current consent, origin, suite1/limits2, exporter и
correlation проверяются; current consent refs двух сторон должны совпасть. Deadline
не позже min(now+300s, current consent expires, own/peer locator expires); runtime
использует также monotonic deadline. Connection close/reconnect/authority update
делают старую generation недействительной. Lease после restart всегда отсутствует.

На commit boundary owner заново читает time, проверяет actual link handle/generation,
current policy_generation/digests и funding; state сравнивает captured fences.
Предварительная crypto cache допустима только по exact immutable bytes; срок/lease
проверяются заново. При смене policy/gen trial не публикуется и accepted RAM не меняется.

Dispatch получает одноразовый permit на exact committed IDs/bytes и current lease.
Network task непосредственно перед началом write проверяет connection alive,
generation и monotonic deadline; после invalidation новые permits не выдаются,
старые unsent отменяются, connection закрывается. Linearization — начало допущенного
write; уже начатый до revocation write нельзя «отозвать из сети». После IO failure
повтор только по новым admission/current checks, с прежними durable Event bytes.
Никакой статус Sent/Complete не снимает debt и не означает Delivered.

## 6. Fresh2: от enrollment до Active2

Admin enrollment `device-enroll --profile-mode ordered-dialog-v2` отдельно от V1.
Два C IDs задаются явно для immutable slots; profile может enable один или оба.
Один Account Root на participant применяется только admin ceremony для Device cert,
membership и bilateral Consent2 (`origin=fresh`, offline_encrypt=1). Artifact signing
происходит вне runtime; owner импортирует public artifacts через authenticated IPC.
Два C имеют отдельные native accounts/sessions; внутри каждого C один Account,
одна B OTK reservation и одна selected Session на весь trace.

Create-new path: account fence, затем первый atomic RuntimeRoot6 с claim/Device keys и
runtime marker, затем public cert/profile publication. Fence-only/missing committed
claim — CreationConflict; не усыновлять и не генерировать replacement Device.
После durable claim, но до public profile можно явно повторить profile-create с
exact expect-account/claim/Device; используются saved keys. Не создавать второй root.
Root secrets не кладутся в state/runtime profile/IPC/logs.

| Durable stage | Правило и restart |
|---|---|
| Enrolled/ContextInstalled | Native Account отсутствует; D/E/SID/Q optional отсутствуют. Новый Text/materialization запрещён. |
| OfferedA | Account A и exact Offer/selected initial pair/current-at-selection Consent2 записаны до send. Нет Session/SID/Guard2. Retry/reconnect возвращает exact Offer. |
| GrantedB | Account B, единственный reserved OTK, Offer/Grant, proof/Q/D/E сохранены до Grant send. Session/SID ещё отсутствуют. Retry не создаёт OTK. |
| FreshHandshake with A0 | A создаёт outbound Session и signed Init/Core2(Q); peer B создаёт inbound только для exact selected Init. First Native/header/MAC/inner/guard/IDs проверяются; writer устанавливает history binding и первый Genesis атомарно с runtime state. |
| FreshHandshake with A0/B0 | B Confirm может быть в одном ≤4-frame native commit с A0 receive и own B0 send; A принимает B0. Constructor/SessionKeys/SID/Guard provenance проверяются, оба genesis readable. Own Ready2 создаётся и durably сохраняется до send. |
| Active2(Q) | Одним AcceptReady commit доказаны оба readable genesis и оба durable Ready2 по A3 §3. Q/cuts=[0,0] immutable. Только теперь LocalEncrypt без lease. |

Нельзя заменить commit обоих Ready одним bool из RAM, наличием proof/Q или peer
Resume hint. Active peer после reconnect повторяет exact own Ready, если другой
FreshHandshake. До его activation Text получает NeedHandshake без possession/ACK.

Fresh E/D вычисляются принятой формулой, Q известен до Init и не зависит от SID.
Генезис request16 = выбранный OfferID/GrantID prefix и пустые user parents проверяются
до native; private HPB1 sentinel normalization остаётся только storage detail S3.
Подпись, Q, cert/role/C/D/E/SID и expected native bindings проверяются до вызова;
после trial сверяются public packet header, MAC, exact Inner2/Core2 и Guard2.

`NativeState2` phase-tagged и sealed≤128KiB вместе с public account/session snapshots
и bounded Guard2. AAD включает incarnation/profile claim, C/cert/role, phase,
native_generation, реальные optional selected IDs/D/E/SID/Q. Blob hash исключён из
своего AAD. Complete serialized output проверяется до commit; Guard2 не получает
растущий seen-set. Authenticated Seen читает indexed repo и записывается с event.
Restart берёт единственный authoritative slot; missing/hash/AAD/provenance failure
блокирует C, без old Native shadow/private pickle parsing/fresh constructor fallback.

Selected historical initial pair проверяется полностью: обе Hello signatures,
scope/roles/certs/membership, selected consent ref, suite/limits, transcript и оба
Device proofs. Current admission отдельно. Runtime сохраняет выбранный raw Consent2
с Offer/Grant. Если B впервые видит старый Offer после renewal и нужный historical
consent уже неизвестен локально, отказ `NeedHistoricalConsent` до native; operator
может импортировать exact bilateral artifact в bounded selection-support slot,
который не меняет current pointer. Hash/старой Device signature для bypass недостаточно.
После этого exact Offer retry продолжает прежний выбор, без reselect/reset. Проверка
этого окна входит в targeted fresh/reconnect matrix.

## 7. Offline очередь, ordered ingress и receipts

### 7.1. Local send

`ordered send` вызывает owner, а не вызывает native из IPC handler. После проверки
UTF8/≤4096B выполняется authenticated `(C,request16)` lookup **до новой capacity и
policy admission**. Same exact body возвращает прежние Queued/EventId/Delivered без
writes/native calls; другой body — RequestIdConflict. Оба исхода требуют integrity
linked records, не доверия cached status. Нет Unicode normalization.

Новый request получает funding `P+sealed body` и FIFO position одним commit. До
materialization он не имеет EventId, не становится readable/Delivered. Once accepted
не теряется при revoke/quota/lease loss. Before Active2 очередь может ждать, но не
превращается в ciphertext. После Active2 scheduler materializes и при dead peer:

1. Capture snapshot + Native-before + global own sequence и current local policy.
2. Fork public native account/session и Guard2, preflight checked n/j+1/g/T/root/
   native generation и Seen. Одновременно≤4 FIFO items, storage plan≤16MiB.
3. Build/sign Event2; exact header/inner/Core/text/caps. Проверить complete sealed
   checkpoint cap и current policy/funding/generation повторно.
4. Один Materialize commit переводит queue credit в retained source/projection/
   request mapping/Seen/native/debt и runtime counters. Accepted RAM swap после успеха.
5. Пока peer offline, никаких dispatch permits. При reconnect публикуются сохранённые
   bytes. Лимит128 не останавливает disk materialization256; остановить её могут
   реальные quota/cap/counter/IO/policy gates.

При CounterExhausted нет wrap/epoch/OTK refill; уже accepted queue/debt остаются.
При нехватке quota/physical capacity — StorageBlocked с причиной; увеличение owner
capacity и reopen позволяют продолжить exact request в том же C/D/E/SID.

### 7.2. Один ingress и funded head repair

EpochCipher2, Deliver2, PrefixResp2 и reconnect/retry используют один owner ingress.
Single decoder выбирается по outer tag/mode; fresh accepts только Event2/cuts0.
Frame/auth catalog/scope/device signature/current admission проверяются до pending
admission, Core/Guard/native/inner — до readable commit. AuthRecords sorted/unique,
unused запрещены; wire fit учитывает весь frame с4-byte prefix, count≤32.

Source cursor, durable possession, native readable cursor, causal history frontier
и Delivered различны. PrefixReq/Resp2 используют sequential lane slots, exact prev,
actual_end/more, source head watermark и IDs; remote source_next не выделяет RAM или
credits пропорционально всей истории. Каждый commit заново проверяет grant/credit.

Ahead ordinal в окне≤1024 может получить ordinary pending:64 slots×64KiB temporary
и P на каждый future readable. Exact duplicate не получает второй P и не переписывает
O/A. Missing head обслуживается отдельным head-only slot64KiB и durable grant token,
даже при64 ordinary slots. Head-only max_events1/end=start+1/max_wire68KiB.
Один RepairGrant window≤256/repair pool64MiB; consumed retained P не считается снова
свободным. Cancel отдаёт только unissued tokens. Без реального refill новый grant
запрещён, accepted raw/debt не удаляются.

Drain потребляет только exact next/prev и проверяет ClosedTail/new-DH causes. Входящая
новая chain может ждать старый tail; native skipped-gap decrypt не используется.
Readability и новый Native/Seen/projection/F/receipt фиксируются одним closed commit.
Bounded receipt/ACK intent сохраняется с readable, не после сетевого write.

После boot owner планирует drain из durable pending без ожидания нового data packet.
Это всё ещё PeerReceive purpose: если fresh admission отсутствует, native drain
ждёт reauth; один Resume/admission wake достаточен. Saved pending не даёт fake lease.
Offline LocalEncrypt собственного accepted queue при Active2 — отдельное право.

OperationalFailure откатывает весь ещё не committed trial/batch. Invalid следующий
event не отменяет ранее durable prefix; можно commit уже проверенный prefix≤4,
затем сохранить bounded quarantine/fork evidence для проблемного C. Invalid event
не становится readable и не получает ACK. Partial response перечисляет exact durably
saved IDs; Complete запрещён для subset. C2 не зависит от устранения gap/fork C1.

### 7.3. Receipt и lost ACK

ACK отправляется только после readable commit и с fresh lease/current policy.
На source стороне подпись exact opposite Device, Q/C/D/E/SID/format/lane/n/EventId/
cert/AuthRef сопоставляются immutable source; только после этого debt→Delivered.
Possession/Complete/Sent/peer cursor сами по себе debt не освобождают.

O = source author, A = original admission, F = readable-current capture, R = receipt
signer capture сохраняются как S3; duplicate/reconnect renewal не подменяет ни один
historical snapshot current bytes. Missing R/F/proof/mandatory source evidence
отвергает также exact last-commit retry. AckQuery и paged receipt lookup возвращают
прежний signed receipt; read-only retry не encrypt/decrypt повторно. ACK intent
Attempted/Sent — transport bookkeeping, не отдельное подтверждение доставки.

Durable ACK intent не является растущим Control list: это per-C cursor плюс диапазон
до committed receive frontier и сами обязательные receipt records. Readable commit
атомарно расширяет этот диапазон. На restart посылается следующая bounded receipt
page; после write AckDispatchProgress проверяет exact saved IDs/contiguous cursor.
Lost ACK после продвижения cursor восстанавливается sender AckQuery/debt paging из
permanent receipts. Старые receipts не удаляются; runtime metadata остаётся bounded,
per-event receipt bytes оплачены P, а не бессрочно накапливаются в control pool4MiB.

## 8. Route2 и явный relay-locator gap

ALPN2/exporter label/context/domains, Frame2/Event2/Core2/Receipt2/ModeProof2 layouts
берутся из S3 без изменений. Adapter `kilogram-transport-iroh/src/ordered_v2.rs`
использует accepted `exporter_context/exporter_hash`, actual TLS `remote_id`, explicit
DirectOnly/RelayOnly/Auto и existing [await_route_policy](../../crates/kilogram-transport-iroh/src/lib.rs#L257).
Application Hello/data не посылаются до route gate. Переключение/reconnect меняет
binding/lease, не source bytes или ratchet. Старый `ordered.rs` остаётся direct L1,
общие transport helpers вне S4 не перестраиваются.

<a id="r3-operator-routes"></a>

### 8.1. R3: production operator route config без public defaults

Нельзя просто вызвать общий `endpoint_builder`: в baseline он использует N0,
а `endpoint_builder_with_relay(None)` сохраняет default relay/discovery. Pinned
iroh1.1.0 `endpoint/presets.rs` подтверждает N0 DNS/Pkarr и public relay; Minimal
выбирает только crypto provider. `endpoint.rs:190–219,503–585` даёт нужные explicit
clear/config APIs. Production S4 builder, общий для обычного CLI и P/F, начинает с:

```text
Endpoint::builder(presets::Minimal)
    .clear_address_lookup()
    .clear_ip_transports()
    .clear_relay_transports()
    .alpns([ALPN2])
    .transport_config(the bounded QUIC config from §3)
```

Затем **только** RouteConfig добавляет необходимые transports. Нет N0 preset,
PkarrPublisher/Resolver/DNS lookup, default relay list, proxy/address lookup plugin
или fallback к прежнему builder. S4 IP bind задан оператором и loopback, с port0
либо заданным port. Direct/Auto вызывают `bind_addr` после clear; RelayOnly его
не вызывает вообще, чтобы случайно не вернуть IP transport. Relay URL берётся
только из explicit local launch config `permitted_relay: Option<RelayUrl>`; signed
peer locator подтверждает тот же URL, но не добавляет новый разрешённый relay.
Проверки config выполняются до bind; required relay readiness/admission имеют
bounded deadlines, не бессрочный `online().await`.

| Local policy / permitted_relay | Production transports и допустимая local advertisement |
|---|---|
| DirectOnly / None | Только explicit loopback IP bind; relay transports/URL отсутствуют. Own IP-list nonempty, relay_opt=[]. |
| DirectOnly / Some | Config error `UnexpectedRelayForDirectOnly`; не игнорировать конфигурацию молча. |
| Auto / None | Только explicit IP, без relay/discovery fallback. Own IP-list nonempty, relay_opt=[]. Effective route gate DirectOnly. |
| Auto / Some(U) | Explicit IP + `RelayMode::custom([U])`, lookup пуст. Own addresses наблюдаются у bound endpoint; URL только U после подтверждённой relay registration. Boot registration deadline15s, отказ TransportUnavailable при неуспехе, не implicit default relay. |
| RelayOnly / None | `RequiredRelayMissing` до bind/connect; не создавать endpoint без routes. |
| RelayOnly / Some(U) | Только `RelayMode::custom([U])`, **ни одного IP transport**. Own locator IP-list empty, relay_opt=[U]; bounded registration необходима до publication. |

Для connect `EndpointAddr` строится из actual pinned endpoint ID и разрешённых
маршрутов verified current locator, не из endpoint ID с implicit lookup. DirectOnly/
Auto-None требуют хотя бы один IP и не добавляют peer URL в dial addresses. Auto-Some
разрешает relay только если peer URL=U; если peer URL отсутствует, нужны IP и effective
gate DirectOnly, без relay fallback. Отличающийся peer URL при relay-enabled policy
отвергается RelayNotPermitted. RelayOnly требует peer URL=U даже при наличии IP и
не добавляет эти IP в dial addresses. Общий `await_route_policy` остаётся настоящей
проверкой выбранного path; effective policy может быть строже launch Auto.

Снятие/смена policy требует нового endpoint lifecycle/locators по §8.3, а не замены
несогласованного route hint внутри старой lease. P/F сохраняют secret-free snapshot
конфигурации: preset Minimal, lookup builders0, relay set empty/[U], actual IP bindings,
ALPN2 и QUIC limits. Это production конфигурация, не test-only clear_address_lookup;
её проверка не требует обращения к public services. В P URL U — собственный literal
loopback relay; explicit external URL не является основанием для public-readiness claim.

### 8.2. Exact RuntimeLocator2 layout и route predicates

**Предлагаемая поправка D2 для root:** добавить production runtime locator с точной
формой, содержащей предусмотренный A3 relay URL:

```text
RuntimeLocator2 = [body, device_signature64]
body = [2,C,own_cert_ref,peer_cert_ref,endpoint32,
        sorted_unique_ip_addresses<=16, relay_opt,
        revision_u64>0, not_before, expires]
relay_opt = [] | [canonical_url_utf8_bstr<=2048B]
```

Полный locator≤8KiB, прежний descriptor signature/ID domain2. IP-list допускает
**0..16** элементов; tuples `[family4|6,address_bstr4|16,port1..65535]` и strict
sorted/unique по canonical encoded bytes как S3. Predicate нового parser:
`IP_count>0 OR relay_opt=Some(U)`; empty IP + empty relay отвергается. URL-only
locator с empty IP допустим и обязателен для actual local RelayOnly advertisement.
Старый S3 nonempty-IP predicate не переносится в новый parser. Дополнительно policy
проверяет eligibility по таблице §8.1: синтаксически valid URL-only artifact может
быть unusable для DirectOnly/Auto-None и не даёт ни dial fallback, ни lease.
URL — ровно один explicit relay, без credentials/fragment, canonical public RelayUrl
string; https или http для literal loopback собственной проверки. Не нормализовать
подписанные bytes после проверки. Intervals/revision проверяет runtime policy.

S3 nine-field `ParsedDescriptor2` и его frozen vectors остаются доступны как прежний
component layout. Новый `ParsedRuntimeLocator2` требует **ровно ten-field body**;
production fresh runtime не принимает nine-field descriptor и не пробует оба decoder
до успеха. Новый producer всегда выдаёт ten-field body, с `relay_opt=[]` для direct.
Оба именованных layout различимы на явном entrypoint, а не silent compatibility.
Это обоснованное расширение/исправление S3 seam, требующее root review до реализации;
оно не выдаётся за уже принятые bytes. Frame Hello locator ref по-прежнему hash32.

Locator import: Device signature, scope/pins, own/peer certs, interval, monotonic
revision; same revision требует exact bytes. Peer endpoint на real TLS обязан совпасть.

<a id="r2-locator-lifecycle"></a>

### 8.3. R2: ephemeral endpoint и per-restart locator rotation

Выбран **новый transport endpoint на каждый process restart/rebind**, без persistent
Iroh secret/port custody. Pinned `endpoint.rs:226` генерирует SecretKey, если его не
задали; port0 также может измениться. Поэтому durable locator прошлого endpoint
нельзя просто переэкспортировать как адрес нового процесса. Native Account/Device/
D/E/SID/Q при transport rotation остаются прежними.

Lifecycle конкретен:

1. На boot old locator/revision читаются только как authenticated high-water/history.
   Пока новый Endpoint не bound/registered, current export возвращает LocatorUnavailable;
   старый artifact на диске не объявляется текущим адресом процесса. IPC status отличает
   publication pending от Ready. Bind/registration подчинены §8.1 и deadlines.
2. Network adapter создаёт новый Endpoint и volatile instance token, передаёт owner
   actual EndpointId/IP/relay snapshot. Owner сверяет snapshot с explicit RouteConfig,
   подписывает locator каждого enabled C с `revision=checked(highest_durable+1)`,
   свежим interval, реальными адресами. До commit эти bytes не публикуются.
3. Closed RotateLocator operation commit-ит новые raw locator/highest revision и
   current endpoint binding до export. Owner/adapter помечают активный instance;
   export permit включает этот instance token и current locator ID. Public file
   заменяется атомарно после commit. IPC/file export повторяет exact raw только
   пока жив **этот же** endpoint instance, его addresses/route соответствуют locator,
   locator current и interval valid. Ошибка экспорта при живом endpoint — повтор,
   не лишняя revision и не re-enrollment.
4. Process kill/endpoint loss отменяет export/lease permits. После restart снова
   bind/новый endpoint и checked newer revision **даже если предыдущий commit ещё
   не успели экспортировать**. Повторный kill в том же окне снова расходует revision;
   пропуски revisions допустимы, rollback/reuse старой revision нет. При overflow
   явный LocatorBlocked, не wrap/reset. Изменение actual адресов/route или expiry
   при живом endpoint тоже требует нового signed/current commit до нового export.
5. Peer import lower revision отказывает; same revision/другие bytes отказывает;
   exact same **current valid** raw идемпотентен без writes. Higher valid eligible
   locator заменяет current pointer, инвалидирует старую lease, будит A reconnect.
   Stale/expired/не соответствующий actual TLS endpoint locator даёт паузу/отказ,
   а не implicit lookup или B dial. Import актуального artifact возобновляет B-only
   backlog без нового send/open пользователя у A.

F11 делает real kill после locator commit до file export/peer import, затем новый
direct bind с другим EndpointId/port и второй kill в этом окне. Для смены port
controller удерживает прежний UDP port либо задаёт другой explicit loopback bind;
не полагается на случайное отличие port0. Проверяются actual address
sets и EndpointId последнего artifact, strictly increasing durable revisions и
ordinary TLS admission. Same/lower/conflicting imports и stale actual mismatch
отделены от exact-current retry. Не обещать восстановление ephemeral старого endpoint.

Acceptance использует собственный loopback relay по
[принятому normal transport примеру](../../crates/kilogram-transport-iroh/tests/local_relay_transport.rs#L15).
Два профиля — два actual application процесса; relay service — третья служебная
transport задача, не участник и не storage server. Direct сначала, RelayOnly после
реального reconnect/restart с новым binding. External relay/service не требуется.

## 9. Recovery, outcomes и публикация

Profile2 JSON secret-free, create-new: version/mode2, state_dir, expected account/
claim/Device, enabled C, IPC/locator paths outside state, bind/route/explicit relay.
Quotas изменяются только owner admin policy, не произвольным network hint. JSON
выбирает launch branch, но authenticated marker/C mode подтверждает DB. Edited
JSON не переводит schema5 в generic/legacy и не создаёт fresh-native fallback.

CLI: mode2 enrollment/profile-create; `ordered consent2-create/import`, context
create/import, locator import/export, status, send/request-status, sync/wake,
history(first/cursor/limit), shutdown. Root-admin создаёт signed artifacts вне state;
живой owner принимает public import через IPC. Исторический offset-v1 history
сохраняется для V4, L2 использует frozen-snapshot cursor, а не offset/full-history cache.
Новый IPC payload имеет explicit version2/opcode, unknown→UnsupportedProfile/Operation
до writer. В V2 ordinary mutating IPC commands запрещены, в ordinary runtime V2 ops
также запрещены. Ключи/pickles/plaintext history не попадают в публичные locators.

| Outcome/window | Обязательное действие |
|---|---|
| Rejected before commit | current accepted state/credits/queue/native неизменны; discarded trial не published encryption. |
| Commit gate PolicyRejected после staging | До tx.commit вызван reject-only gate; successful abort без backend error оставляет прежние logical root/records/Native/Ready/credits, repository usable, publication0. Physical staging/growth не приравнивается к0 bytes. Trial calls могут уже быть выполнены и считаются отдельно. |
| Successful commit до IPC reply/socket send | retry lookup возвращает прежние request/control/EventId/receipt bytes; native повторно не вызывается. |
| IO/sync/ambiguous writer error | poison owner repository, остановить все writes/dispatch/ACK; закрыть DB, reopen штатным redb recovery под тем же exclusive ownership. Не trim suffix/не fallback в V4. |
| Reopen old root | exact request остаётся queued/absent; unpublished trial отброшен. После новых policy/lease checks допустим новый trial на прежнем accepted native. Не утверждать, что неудачный volatile encrypt вообще не вызывался. |
| Reopen new root | request/source/Native/Ready/receipt связаны coherent root; вернуть durable outcome, resend exact bytes. Не re-encrypt из-за потерянного ответа. |
| Torn/corrupt/missing binding | Integrity/blocked; отсутствие доказательства commit не считается Absent/permission повторить encrypt. |

Common root хранит last commit id/digest; это **не** unlimited operation journal.
После ReopenRequired owner не обслуживает другие mutations до resolution, поэтому
in-flight operation проверяется по этому root и её semantic request/control/event
identity. После process kill исход определяется typed records: queue mapping/source/
receipt/Ready/Head, а не утерянным RAM nonce. Durable receipt intent реконструируется
bounded reader. Никакого per-message filesystem shadow/v2 journal для V5.

Reopen восстанавливает identity/current phase/Native/queue heads и bounded work,
а не загружает все events/requests/receipts. Cursor view и source correspondence
сохраняются при параллельных append. Исчерпание physical capacity не лечится batch1
или silent eviction; owner увеличивает место/quota и повторяет после reopen.

## 10. Existing L1, version fences и точный successor S5

| Вход/действие в S4 | Исход |
|---|---|
| V4/new binary + profile1 | Прежний Engine/V4/ALPN1, R3 lifetime bound/lease сохраняются. Auto conversion отсутствует. |
| V4 + profile2/enroll2/runtime2 | ExistingStateRequiresMigration/CreationConflict до нового fence/root/Native mutation. Старые bytes/queue/pending/receipts остаются. |
| `ordered storage-upgrade` / `ordered upgrade` в S4 | Explicit UnsupportedMigration/UnsupportedHandoff, статус указывает successor S5; команда не pretend-success и не записывает Offering/Fence5. |
| S3 ComponentRoot5/schema5 | Supported прежним component loader, UnsupportedRuntimeProfile для runtime2; не усыновлять sample DB и не создать Native. Existing component encodings/fixtures сохраняются. |
| RuntimeRoot6/schema5 + same-version component/generic/history facade | Отказ по layout до writable component handle/commit/retry fast path, включая старые Queue/Repair facade. Только runtime-purpose lowering меняет runtime profile. |
| RuntimeRoot6/schema5 + exact S3 schema5 predecessor7f82036 | При свободном lock, correct ExpectedOwner/config AEAD проходит, plaintext byte06 отвергается старым Root::decode как anchor version до writable API. Native/Head/queue/debt/root logical records неизменны. F10 — отдельный actual predecessor helper. |
| runtime2 + V4 d9b EXE/ordinary/V1 profile | Busy при живом owner; после shutdown schema/account fence запрещает mutable old/generic path. Это отдельный V4 gate, не доказательство предыдущей schema5 несовместимости. |
| future/unknown root layout/magic/revision/suffix | Отказ до writable API у соответствующего loader, без try-decode/fallback или marker adoption. |
| Fresh Active2 ↔ wire1-only peer | ALPN/version refusal/UpgradeRequired; нет new native/possession/ACK/downgrade. Fresh cuts0 не принимают Event1/Receipt1. |
| Existing ModeProof/Selected/Active2 из будущего S5 | S4 loader явно UnsupportedExistingRuntime; не принимает existing как fresh и не меняет Q/cuts. |
| Unknown/corrupt schema/mode/profile marker | fail closed до mutable repository/native; JSON не восстанавливает отсутствующий authoritative mode. |

В реализации S4 нельзя добавить «временный import» старых pickles/JSON/state rows.
S3 pure L1 facts conversion не является file migration. Причина разделения: V4
OrderedVault/index/shadows и growing Guard1 требуют собственных streamed copy,
reservation reconciliation и recovery/fence proofs; публичный S3 bound2 reader пока
знает fresh proof, а не полный mixed imported source writer. Подмена этого native
constructor или snapshot-copy оставила бы незакрытыми preservation/dual-writer gates.

S5 остаётся конкретным continuation принятого A3 §7→§3:

1. Под lifetime lock recovery/preflight V4; atomic Fence5+MigrationIntent и mode5
   replacement в той же DB; до fence cancel безопасен, после него forward-only resume.
2. Streamed authenticated index/V4Refs/copy checkpoints, same Device/account/native
   SessionKeys/D/E/SID, queue/pending/receipts/debt/legacy reservation. Atomic Ready5
   и retirement old current pointers. Indexed L1 adapter реально interoperates с old remote.
3. Per-C existing Consent2 + P/A/K/Committed; freeze только send materialization,
   cuts/watermarks/old prefix/ACK, Selected receive-only, restart lost Committed trace.
   Bound writer расширяется на exact Event1/Receipt1 ниже cuts, Event2 выше cuts,
   прежние IDs/history не переписываются. После activation очередь продолжается один раз.
4. Real baseline L1 active/exhausted511/queue/pending64+head/late-chain/old receipts
   fixtures и все deferred A3-1/A3-7/A3-8 gates. Новые временные данные; frozen DB не открывать.

S4 не закрывает эти четыре шага и не переименовывает fresh trace в resumed L1.
Root принимает либо эту границу, либо расширяет S4 до S5 до implementation dispatch.

## 11. Acceptance matrix будущей реализации

Все строки ниже — **обязательные будущие проверки, сейчас не запущены**. PASS в S3
не переносится на runtime activation/current-policy/IO. Для каждого нового свойства
фиксируются actual entrypoint, raw command/source/EXE hashes, fixture, measured
boundary, records/calls/outcome. CLI helper управляет процессами/IPC, не подменяет store.

### 11.1. Один final two-process dialogue/offline/2C run — suite P, ceiling2400s

Новый isolated pair профилей, два C; normal production entrypoints §2/§9.
После Fresh2 setup A/B имеют durable Active2 с одинаковым Q на C1 и отдельным Q на C2.

| Шаг | Fixture/schedule и наблюдаемый результат |
|---|---|
| P01 enrollment/activation | Root-admin artifacts → ordinary runtime2/IPC, actual TLS2/Hello/Ready. Evidence собственных Device IDs, Account curve keys, one B OTK/C, D/E/SID/Q/Ready IDs, RuntimeRoot6 discriminator и marker. Profile/locator публичны только после их durable roots и actual endpoint binding. |
| P02 one-way | C1 A sends640 Text, B не отправляет Text. Same native chain проходит512 без reset/refill. После256 actual parent kill/restart обоих; после512 graceful restart обоих. Exact next/j/Native/cursor/request/UTF8 сохранены, retry не encrypt. Reconnect не создаёт Session. |
| P03 duplex/DH | Ещё20 rounds:2 A Text и32 B Text/round. Итого до offline C1 A680/B640 Text. Receiving DH changes≥16 на сторону подтверждаются actual headers/guard transitions, не номером цикла. Все n contiguous, D/E/SID прежние. |
| P04 late chains | В rounds7/8 delay tail B7, получить его first, materialize A8, затем вернуть late B7 tail и следующий DH через реальный transport. Наблюдать own Active/T не меняется от late tail; future first ждёт exact prefix. Только normal committed packets, без изменения crypto/header bytes. |
| P05 genuine offline | B process остановлен и PID действительно завершён; A lease invalidated. A queues/materializes256 дополнительных Text C1 при current local Root2, без live lease; sealed source/debt count>128. Restart A пока B всё ещё отсутствует. Проверить сохранённые IDs/bytes и RAM scheduling refs≤128, отсутствие dispatch. |
| P06 full64+head/pages | После возврата B delayed lowest missing C1,64 ordinary ahead pending оплачены и сохранены. Получить funded head через отдельный slot, drain без eviction. Для измеримого paging receiver запрашивает обычные интервалы≤16 в PrefixReq (hard cap32 прежний),≥8 nonempty catch-up pages. Restart B между pages2/5, restart A при потерянном ACK. Page records/source cursor/grant/Native/receipts после reopen точны. |
| P07 C2 progress | Пока C1 gap/full pending/catch-up после возвращения B, занять его64 scheduling refs задержанным bounded round/prefetch и не давать C1 ACK/progress до наблюдаемого C2 advance. C2 admission использует собственные64, duplex≥32 Text/сторону в тех же двух процессах; profile refs≤128. Проверить release refs после timeout/cancel, отсутствие debt eviction и повтор exact source из disk. Между owner turns нет starvation; отдельные native/Q/credits/IDs. Пока весь B process отсутствует network C2 тоже недоступен — обратного не заявлять. |
| P08 routes/lost ACK | Начальный direct/IP-only locator, затем свой loopback RelayOnly с actual empty IP-list/URL-only locator и reconnect. Production builder из §8.1, lookup0/relay empty либо [U], без external defaults. At least3 real TLS bindings, симметричные на A/B и разные после reconnect; прежний binding rejected. Artifact actual EndpointId/routes совпадают с живым endpoint, revisions растут после restart. Drop созданный ACK до socket publication, exact source/query возвращает прежний receipt. B-only queued work после relaunch/import актуального locator A прогрессирует без нового send/open у A. |
| P09 convergence/history | После final restart обоих C1 A936/B640 Text, C2 A32/B32: всего1640 Text и4 genesis source identities; exact IDs/request16/UTF8/old cursor suffix, без duplicates/false Delivered. История читается paged production IPC/reader, не test DTO. |

Для успешных новых packet transitions ожидается3288 calls =2×(1640 Text+4 genesis),
считая encrypt и inbound/decrypt на обеих сторонах. Retries/duplicates без native;
failed/aborted attempts считать отдельно и не скрывать. Counters ведутся по process/C/
operation, совпадают с durable source/receipt/native outcomes. Fixture expected IDs
может хранить controller; это не разрешение runtime whole-history RAM cache.

Schedule hook расширяется новым debug-only runtime2 helper по образцу
[test_support.rs](../../apps/kilogram-cli/src/ordered_dialog/test_support.rs#L1):
delay/drop normal committed records, bounded barrier/trace и scoped clock/IO injection.
Он не создаёт Ready/keys/claims или DB rows и не меняет signed data в suite P.
Release отвергает test env. Новый ordinal selector — checked u64/count≤128,
не унаследованный old test `<512`. Hook activation/source hashes показаны явно.

### 11.2. Selected policy/fresh/fault/version — suite F, aggregate≤1200s

| Gate / production entrypoint | Cases и evidence |
|---|---|
| F01 policy owner |14 cases: исходные12 (own/peer revoke; lower revision; equal-revision changed bytes; consent expiry/not-before; clock failure/backward; authority read failure; stale captured policy generation; expired lease; old connection generation) плюс2 barriers **после plan/staging перед tx.commit**: targeted clock crosses expiry и actual connection closes. Gate даёт PolicyRejected с successful abort, accepted Root/Native/queue/source/debt/Ready unchanged, no dispatch/ACK, следующая valid операция возможна без mandatory reopen. Уже выполненные trial calls отдельно; stale generation injection не объявлять actual concurrent owner race. DB/abort IO error проверяется отдельно в F07. |
| F02 renewal/selection | Active renewal с теми же certs: old Q/proof/captures byte-exact, fresh current Hello refs required. Reconnect before first Grant с old Offer: missing historical consent fail-closed, explicit support-artifact import, exact continuation. Wrong Q в genesis/Text/Ready/receipt — до native/debt/activation boundary соответственно. |
| F03 Ready kills | Parent kills after own Ready before peer Ready commit → FreshHandshake/offline encrypt denied; after Active2 commit before first Text publication → Active2/exact first send; Active peer resends own Ready to non-Active peer. No hints/JSON activation. |
| F04 bootstrap kills | Fence-only before durable Root→CreationConflict; committed claim/keys before profile export→same-key explicit resume. After Offer-before-send и Grant-before-send→exact controls/Account/OTK, no new constructor. |
| F05 native/outbox kills | После A0 commit до send; B A0/Confirm commit до reply; queue commit до IPC reply; materialization commit до socket. Reopen lookup возвращает прежние bytes/requests/Native; не создаёт ordinal holes/duplicate encryption. |
| F06 ingress/receipt kills | Pending commit до Complete; readable commit до ACK send; receipt/debt commit до IPC Delivered reply. Durable prefix/receipt/queue/credit coherent после реального parent kill, exact retries и авто-drain после fresh reauth. |
| F07 IO/capacity | Inject до write, root commit/sync ambiguity и ACK-intent update; реальные small configured quota/capacity refusal через existing backend, затем increase/reopen/retry. Precommit/committed/ambiguous outcome различимы; no ACK/publication до доказанного commit. Новые hardware full-volume claims отсутствуют. |
| F08 bypass/version | Восемь исходных categories: V4-as-V2; runtime2-as-V1/ordinary; edited unknown JSON; missing/corrupt marker; component schema5 adoption; V4 old/live writer; old wire peer; existing-origin handoff input. Rejection до protected logical writes/native, old history доступна V1. Baseline d9b EXE создаёт новые small L1 fixtures; это не F10 predecessor gate и не migration success. |
| F09 R1–R4/I1–I4 | Historical pair/current permission split, fence-only creation, exact queue retry-before-new-capacity, R4 pre-native genesis; OperationalFailure batch rollback, invalid-after-durable-prefix, format/resource fences, RuntimeFromProfile admission before mutable repositories. C1-only block leaves C2 operational unless причина общая. |
| F10 root/layout fence | Current public component open/queue/Protocol2/generic paths и same-version internal bypass/retry probes отвергают RuntimeRoot6 до mutation; неизвестный root byte, wrong magic, layout_rev2, trailing suffix, missing/mismatched marker и ComponentRoot5 с runtime slots тоже отвергаются. **Exact S3 predecessor** public API helper отдельно cold-open-ит runtime profile с correct owner/config после shutdown и пытается queue/component/generic action лишь если open ошибочно удался: ожидается anchor-version refusal уже на open. Old/new legitimate ComponentRoot5 positive controls остаются supported. Root/Head/Native/queue/credits/counts/IDs неизменны; не обещать byte-identical redb maintenance headers. |
| F11 locator lifecycle | B имеет заранее materialized B-only debt; normal dispatch задержан, plaintext queue уже пуста. Real kill B после newer locator commit до export/import; новый EndpointId и контролируемо другой direct port, снова commit/kill; третий bind/commit/export. Проверить revisions r,r+1,r+2 и actual EndpointId/IP/relay artifact, Native/identities во время rotation не пересоздаются; старые bytes не advertised как current. Same-process export-error retry при живом том же endpoint exact/idempotent. Lower revision, equal revision changed bytes, expired/stale actual endpoint отвергаются; exact-current import idempotent. Import latest locator на живом A возобновляет B-only backlog без нового send/open, D/E/SID/Q/source bytes прежние. |
| F12 route/admission bounds | Actual direct-only IP locator и real RelayOnly URL-only/empty-IP locator; empty/empty reject; RelayOnly None config/missing peer URL/different permitted URL refusals; signed locator vs actual TLS endpoint mismatch; Auto None не даёт default relay/lookup. Snapshot production config доказывает explicit transports/lookup0. Нормальными собственными connections удержать2 TLS/admission permits и предложить третий: отказ до application spawn/frame allocation; deadline/close освобождает slots/tasks/buffers, затем новый valid admission. Проверить streams≤4/connection, mailbox≤16 и accounting ещё до admitted. |

Kill означает принудительное завершение actual child по durable barrier с PID/exit
evidence; graceful shutdown отдельно. IO injection — явно injection, component
reopen — component; одно не выдаётся за другое. Existing14 crash windows полностью
не повторяются: F03–F07/F11 покрывают конкретные новые owners/commits/recovery seams.

F10 helper собирается **в будущем** из изолированного exact S3 source snapshot
`7f82036ee5803edd64be98b73572604250f5961c`; меняется только внешний probe/integration
entrypoint, все pinned library sources/Cargo.lock/features сверяются с этим commit.
Он не линкует текущий S4 kilogram-state вместо predecessor. Fixed executable path
`target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe`,
source/helper/EXE hashes отдельно. Correct ExpectedOwner/capacity и позитивный
ComponentRoot5 control исключают ложный PASS из-за неверных параметров/нерабочего
helper. Запускаются только новые temporary profiles, не frozen S1/S2 fixtures.

### 11.3. Targeted components/regressions, aggregate≤600s; static отдельно

P/F/components и текущий process CLI используют **Cargo profile `verification`**
из baseline Cargo.toml, locked/offline, Jobs12/BelowNormal. Rust harnesses компилируются
`--no-run --profile verification`; field CLI/helper build — `--profile verification`.
Predecessor helper также использует S3 `verification`. Ни release, ни S2-only
`verification-scale`, ни неявный debug/profile override к этим gates не применяются.

Bounded new writer tests проверяют runtime marker/fences/local layouts/record caps,
atomic Head+Ready+history/native, exact retry и missing mandatory evidence, namespace
injections и max encode plan. Дополнительно targeted Codec1/Guard1/byte adapter/S1
ledger/H1-H2/S2 cursor/history и S3 bounded tests по затронутым seams. Список selectors
фиксируется перед final run; successful heavy N01/N02/root32 schedules и S1/S2 scale
не повторяются лишь ради свежего EXE hash. Reuse требует byte-exact production
dependencies/driver scope; изменившийся serializer/Guard/storage path получает свой
адресный regression, а не необоснованный reuse.

Strict Clippy all-targets затронутых packages, locked/offline build, rustfmt и diff
checks отдельно. S4 не наследует S3 resource exception1200s. Suite P≤2400s,
F≤1200s и components≤600s — суммы final runner command elapsed соответствующих
наборов, с попавшими в них compile/copy/setup overhead; static/compile-only отдельно.
Это не per-test budgets и не время параллельного wall run. При превышении — stop,
сохранить FAIL/timings и вынести bottleneck root, не сокращать counts/не менять caps.

Windows: Rust harness compile `--no-run`, execution исключительно
[stable runner](../../scripts/run-cargo-tests-stable.ps1), Jobs12/BelowNormal,
fixed EXE paths. Process CLI после compile копируется один раз в постоянный
`target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe`; имя не содержит hash.
Baseline L1 EXE остаётся в отдельном принятом постоянном пути. Source/EXE hashes
фиксируются по содержимому; runners, перезаписывающие один EXE, не запускаются вместе.
Все fixtures новые изолированные workspace temp; frozen S1/S2 DB не открывать.

### 11.4. Статус общих A3 gates после будущего S4

| A3 gate | Completed/reused/new/deferred |
|---|---|
| A3-1 | NEW P01–P04/P09 — real fresh two-process dialogue. DEFERRED resumed L1 до S5; весь gate не закрыт. |
| A3-2 | NEW P05–P08 — actual offline256, disk>128/RAM≤128, pages/restarts/full64+head/C2; component PASS не заменяет. |
| A3-3 | REUSED S3 checked arithmetic/native probes и root schedule evidence при exact source scope; NEW P02 production one-way640, checkpoint/restart owner. |
| A3-4 | COMPLETED library persistent100k в [S2 review4](long-lived-dialog-a3-history-s2-implementation-review-4.md), REUSED без открытия frozen fixture. Не100k E2EE/runtime. Новый runtime не получает full-history scan. |
| A3-5 | REUSED S1/S2 storage measurements в их явно принятых profiles; NEW wrapper counters/buffers/PrivateBytes в P/F. S2 p95 figures не выдаются за bound2/IPC/runtime latency. Если изменён сам reader/index/cache path — адресный scale gate до claim. |
| A3-6/6a | REUSED unchanged [S1 backend/ledger/growth acceptance](long-lived-dialog-a3-storage-s1-review-3.md); NEW F07 small quota/IO/recovery/funding с production owner. Изменение backend/allocator требует соответствующего нового actual growth gate. |
| A3-7 | NEW F03–F07 Fresh2/runtime windows; DEFERRED Fence5/copy/Ready5 и existing P/Prepared/Selected/Committed kills до S5. |
| A3-8 | NEW F08 safe version/old-binary refusal и fresh trace; DEFERRED real L1 storage upgrade/mixed compat/cut/handoff preservation. Safe refusal не является successful migration. |
| A3-9 | NEW P08 real normal direct/own relay/exporter/reconnect плюс targeted ordinary IPC/legacy smoke/static checks. Supporting A2 adapter evidence лишь reuse foundation. |

S2 verification-scale/его time amendments остаются только S2; их profile/limits не
переносятся на новый runtime. Не повторять100k или старые long route suites без
изменившегося production path/конкретного сомнения. Сохранять соответствующие старые
evidence hashes, scope reuse и отдельно ограничения новых измерений.

## 12. Доказательства, стоимость и порядок реализации после review

Implementation report/evidence будут отдельными, без переписывания принятых A3/S1–S3.
Для каждой final suite: exact source snapshot, executable hashes, commands/environment/
profile/PID, raw stdout/stderr/exit/timing, counters по C/role и actual storage stats.
Failed/intermediate attempts сохраняются отдельно; final aggregate не скрывает их.

Отдельно измеряются enrollment/setup/admission/Ready; queue admission/materialize;
offline/catch-up/reauth; native packet calls/restores; page32/history_first/resume
после reopen; backend write/grow/sync/plan; memory и scheduler refs/tasks/buffers.
Полная длительность dialogue не является page latency или профилем crypto/IO.
Обычный history call использует неизменный S3 bounded reader с bound2
`40*(limit+2)+3`, unbound `8*(limit+2)+2`; дополнительные owner/status/transport reads
и время показываются отдельно, не вычитаются из общей IPC latency. Никакой O(history)
валидации на каждый append/page/status. Cold open/redb recovery и maintenance отдельно.

Implementation dependency order, один S4 production результат:

1. Typed runtime records/owner bootstrap/readers/generic fences/common atomic lowering;
   bounded component/fault checks до network activation.
2. Production NativeState2/Policy2/Fresh2/Ready2 owner и ordinary CLI/IPC dispatch.
3. Offline scheduler/ingress/repair/receipt + actual route2/locator seam, bounded
   admission/tasks и recovery. До этой части small component PASS не сдаётся как S4.
4. Final P/F/regressions/static на frozen diff, implementation report → root review/
   repair → explicit acceptance → exact manifest commit/push → root remote verification.

Нужные решения root при review этого контракта:

1. Принять S4 как complete fresh **production runtime** и S5 как обязательный successor
   реального L1 upgrade/handoff с перечисленными deferred A3 gates.
2. Принять RuntimeRoot6 discriminator/strict component-runtime loaders и namespaces21/22,
   закрытый runtime-purpose writer/bootstrap; запретить two-commit activation/native,
   component API bypass и normal old S3 writer downgrade.
3. Явно принять ten-field RuntimeLocator2 из §8 вместо попытки выдать nine-field S3
   descriptor за relay locator; старые parser/bytes/vectors не переписывать.
4. Принять strict current-clock high-water policy, before-SID NeedHandshake для новой
   очереди, исторический consent support-artifact отказ/continuation и budgets/matrix.
5. Зафиксировать per-restart locator rotation и Minimal/operator-only route config,
   profile-total pre-admission bounds, non-borrowed64/64 refs и clean gate rejection.

## 13. Текущая сдача

На contract-only этапе выполнены чтение task/accepted docs/actual production seams,
проверка baseline и последующая local-link/diff проверка документа. Ни будущая matrix,
ни benchmarks/real processes сейчас не запускались. Изменён только этот контракт;
root-owned S4 task/README/coordination/review1 и unrelated state сохраняются.
Clean development-environment SHA256
`53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f`, L1 baseline EXE
`d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1` сохраняются.

### 13.1. Disposition review1 — только contract repair

| Finding | Исправленный контракт / будущая проверка |
|---|---|
| R1 | [§4.1.1 authenticated RuntimeRoot6](#r1-runtime-root): exact06/magic/rev1 внутри AEAD, ComponentRoot5 byte-exact, strict loaders/первый root/runtime-purpose guards. §10/F10 отдельно проверяют same-version обходы, unsupported/future layouts и actual S3 predecessor7f82036 с correct owner/config/positive control. Смена namespace сама по себе больше не называется old-writer fence. |
| R2 | [§8.3 endpoint lifecycle](#r2-locator-lifecycle): per-restart/rebind новый endpoint+checked newer revision committed до export; exact retry только при живом том же current instance. P08/F11 включают repeated real kill до export/import, контролируемо другой port, actual artifact/endpoint match, revision refusals и B-only continuation. |
| R3 | [§8.1 production operator routes](#r3-operator-routes), §8.2 и F12: Minimal/lookup0/explicit transport set, полная None/Some policy table, URL-only/empty-IP RelayOnly и empty/empty reject, required/approved relay и actual path gate. Public helper N0 defaults и test-only overrides не подменяют production rule. |

Уточнения review также внесены: §3/F12 ограничивают TLS/application admission до
admitted, tasks/streams/deadlines; §3/P07 определяют non-borrowed64/64 и slot release
без debt eviction; §11.3 явно задаёт `verification`; §4.2/§9/F01/F07 разделяют clean
post-staging gate refusal и IO/ambiguous outcomes. Это specification и запланированные
gates, не воспроизведённые runtime PASS. Frozen review1/принятые A3/S1–S3 не изменены.

После проверки — один содержательный callback с baseline/path/bytes/SHA256,
решениями §12 и фактическими read-only checks. Затем stop до root contract review.
Rust/helpers/dependencies/fixtures, tests/benchmarks и commit/push этим этапом не запускаются.
