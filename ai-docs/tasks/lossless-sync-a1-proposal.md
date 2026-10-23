# A1: lossless sync — baseline и решение для review

2026-10-23, Asia/Yekaterinburg. **Проект, не разрешение на реализацию.**
HEAD совпадает с аудитом: `3e82e6380c30d0f9443b6692ea34faccaacddef7`.
Review 2026-10-23: направление принято для ограниченного feasibility spike;
production change-set пока не готов. Замечания R1–R4 и следующая задача:
[независимое review](lossless-sync-a1-review.md). Исходное предложение ниже
сохранено; его численные лимиты и compatibility guarantees ещё не утверждены.
F1/F2 завершены: [исполняемый spike и фактические результаты](lossless-sync-a1-spike/README.md).
Уточнения §5 имеют приоритет над предварительными гарантиями и квотами §2–4.
Результат [review F1/F2](lossless-sync-a1-spike-review.md): spike принят в узких
границах; ограничения S1–S3 и условия первого runtime/vault slice имеют приоритет
над обобщениями transition table §5. Пользователь затем одобрил и отправил
ограниченный runtime/vault slice на реализацию; полный change-set §4 по-прежнему
не утверждён. [Статус](../chat-coordination.md).
Исходные незакоммиченные документы сохранены; product code, wire default и
recovery facade не изменены. Основание: [задача](lossless-sync-a1-design.md),
[передача](../project-handoff-2026-10-23.md),
[identity/history](../../docs/RFC-0193-user-sovereignty-and-contextual-identity.md).

## 1. Baseline и установленная причина

Повторён **неизменённый** [offline repro](../reviews/repro-2026-10-22/README.md):
locked/offline build, jobs=2, затем постоянный
`target/debug/kilogram-audit-probe.exe`; никаких sockets/test harnesses.
SHA-256 source `65678ffdd99df29952e563b12e9278dc297058d376ba47a3fbc913798e62714c`.
Факты: [среда/HEAD/хэши](lossless-sync-a1-evidence/baseline-environment.txt),
[build](lossless-sync-a1-evidence/baseline-build.log),
[точный stdout/stderr и exit](lossless-sync-a1-evidence/baseline-run.log),
[проверка scope/ссылок/хэшей](lossless-sync-a1-evidence/verification.log).

Результат: `build_exit=0`, `ordered_control_decrypted=256`,
`hash_order_first_batch_decrypted=64` (minimum sequence 4, maximum 255),
после reload на второй странице — `Decryption(MissingMessageKey(0))`,
`backlog_missing_key_reproduced=true`, `probe_exit=0`.

Успешный exit этого probe означает **воспроизведение дефекта**. EventId случайны;
этот прогон не является детерминированным regression. Дополнительный A7 check
expired prekey pool — побочный результат существующего probe, не scope A1.

Проверяемая цепочка production-кода на этом HEAD:

1. [`plan_sync`](../../crates/kilogram-session/src/lib.rs#L412) сортирует missing
   EventId по hash и обрезает страницу до
   [64](../../crates/kilogram-protocol/src/wire.rs#L14).
   [`accept_diff`](../../crates/kilogram-session/src/lib.rs#L132) проверяет session
   binding/membership, затем вызывает `put_events`; server делает это же перед
   [`SyncComplete`](../../crates/kilogram-session/src/lib.rs#L291).
2. [`DecryptingSessionStore`](../../apps/kilogram-cli/src/main.rs#L35260)
   сортирует только текущий batch по `(author_device_id, author_sequence)`;
   [`put_events`](../../apps/kilogram-cli/src/main.rs#L35383) выполняет decrypt,
   sealed projection и event write в store transaction, затем обновляет overlay.
3. [`decrypt`](../../crates/kilogram-ratchet/src/lib.rs#L924) выбирает session set
   по DeviceId; [путь pickle](../../crates/kilogram-ratchet/src/lib.rs#L1314)
   не содержит conversation. [Fan-out](../../apps/kilogram-cli/src/main.rs#L27594)
   тоже не принимает context. Поэтому разные беседы расходуют одни receive keys.
4. В закреплённом `vodozemac 0.10.0`,
   `src/olm/session/receiver_chain.rs:50,64–69,186–221`: 40 skipped keys на chain,
   старые вытесняются; `MAX_MESSAGE_GAP=2000` — другой лимит, не retention.
   `session/mod.rs:53,103–118` ограничивает также число receiving chains пятью.
   Нормальное продвижение и [сохранение pickle](../../crates/kilogram-ratchet/src/lib.rs#L1035)
   делают потерю ключа устойчивой к reload.
5. [Direct/relay receive](../../apps/kilogram-cli/src/main.rs#L27337) и
   [mailbox commit](../../apps/kilogram-cli/src/main.rs#L23970) используют тот же
   ratchet. Mailbox [list](../../crates/kilogram-mailbox/src/store.rs#L509)
   идёт по opaque item ID; runtime берёт
   [первый item](../../apps/kilogram-cli/src/main.rs#L24510), а не crypto predecessor.
   `prepare_inbound` [открывает внешний envelope](../../crates/kilogram-mailbox-client/src/ledger.rs#L964),
   но само по себе не сохраняет inner event. Application commit предшествует
   [replica receipt/delete](../../apps/kilogram-cli/src/main.rs#L24523);
   между ними возможен безопасный повтор, требующий projection-based dedup.

`author_sequence` выделяется [на устройство](../../apps/kilogram-cli/src/main.rs#L5383),
включает ACK и другие беседы. Olm index относится к одной sending chain и
меняется при DH transition; EventId — content hash; parents — causal DAG.
Ни одно из этих четырёх понятий не заменяет остальные. Causal sort одной беседы,
sort одного batch и включение canonical v2 не задают общий receive order.
[V2 materialization](../../apps/kilogram-cli/src/main.rs#L28245) передаёт все
сохранённые carriers в тот же adapter после завершения sync; это иной batching,
но общая device-pair session остаётся. Сам v2 этим component repro не проверен.

**Уровень evidence:** контроль 256 и persisted page failure воспроизведены
компонентно. Production composition/transactions/direct/mailbox установлены
чтением кода. Runtime/process, GUI и physical two-host A1 в этом этапе не запускались.

## 2. Выбор

| Вариант | Оценка |
| --- | --- |
| Общая device-pair session + единый durable ordered ingress | Может предотвратить вытеснение; необходим полный приватный порядок всех encryptions обоим устройствам, включая другие беседы. Пропуск A блокирует B; поиск predecessor связывает контексты и плохо совместим с будущей identity. Минимальный diff не компенсирует эту связь. |
| Только отдельная session на conversation | Изолирует A от B, но direct `0..64` всё ещё вытесняет ключ позднего mailbox сообщения той же беседы. Недостаточно. |
| **Context domains + общий для delivery paths durable ordered ingress** | **Рекомендуется.** Контексты независимы, пропуски не разрушают ключи; цена — protocol/state migration и ожидание при gap. |

Контрпримеры: A содержит Olm `0..40`, B — следующий `41`; получение B первым
оставляет A0 без ключа в старой общей session. При разделении domains этот случай
исчезает, но поздний A0 из mailbox после A64 по direct всё ещё требует ingress.

### Domain и аутентифицированный порядок

Domain: `(protocol suite, conversation/context, пара endpoint credentials,
согласованный session epoch)`. У него две направленные recipient lanes.
Epoch — свежий transcript с ролями initiator/responder, context, ключами/prekeys,
credential/membership bindings и подписанным согласием устройств; конфликтующие
transcripts отклоняются. Root остаётся административным ключом.

Новый namespace получает независимый Olm account/prekeys на local
`(context, epoch)` и session на peer внутри него; старые pickles не клонируются
в несколько domains. Olm KDF/cipher suite остаются существующими: separation
обеспечивают свежие key agreement inputs и authenticated context transcript.
Точное encoding/domain tags требуют crypto review, это не доказанная новая схема.

В новой версии signed event каждый recipient имеет `(domain, epoch, n,
previous_event_id)`, genesis `n=0, previous=null`. Это **context-local** ordinal
всех Olm encryptions данному recipient, через DH transitions; обычный ACK его
не расходует. Он не заменяет существующие EventId/author_sequence/parents.
Подпись охватывает metadata и ciphertext; внутренний encrypted plaintext frame
повторяет binding/version/order (у текущего Olm wrapper нет application AAD).
Перед commit сверяются оба binding и ожидаемый Olm chain/index; ложный signed
`n` с большим crypto gap не разрешает продвижение. Один slot имеет один EventId;
fork/conflicting successor изолирует lane, не переписывая committed history.

В namespace нет нового публичного account-wide ID/counter. Сегодня credentials
мапятся на существующие Device/certificate; будущие context credentials заменят
этот mapping без общего Root-derived domain ID. Нынешний публичный AccountId и
author_sequence этим изменением анонимными не становятся; contextual identity
остаётся отдельной работой.

### Приём, commit и границы ресурсов

Все paths вызывают один ingress. После size/admission/membership/certificate/
authority/revocation/replay checks он durable сохраняет **PendingCiphertext**
в encrypted vault. Перед drain authority проверяется снова по действующей
политике. Pending не является readable projection или message ACK.
Новый direct protocol сразу возвращает storage-only `Pending`, завершая network
attempt; не удерживает writer/соединение до закрытия gap. Sender сохраняет outbox
и обслуживает/приоритизирует prefix repair; legacy attempt завершается без ACK.

Drain допускает только `n=next` с точным predecessor и безопасным Olm transition.
Decrypt выполняется в rollback-able workspace. Одна transaction фиксирует
ratchet, immutable event, sealed projection, receive frontier, удаление pending
и durable ACK intent. ACK передаётся после commit. Sender одной transaction
фиксирует ordinal/ratchet/event/projection/materialized outbox; retry повторяет
те же байты. Ошибка до commit не создаёт дырку в lane. Основа — текущие
[transaction/vault boundaries](../../apps/kilogram-cli/src/main.rs#L5448),
[outbox materialization](../../apps/kilogram-cli/src/main.rs#L24904) и
[ACK/outbox commit](../../apps/kilogram-cli/src/main.rs#L25108).

Для sync нужны separate `have_ciphertext` и `readable` frontiers: иначе pending
страница снова приходит бесконечно либо ложно объявляется доставленной.
В исправленном legacy v6 adapter inventory означает durable possession: объединяет pending
и committed IDs, отдаёт запрошенные подписанные ciphertext bytes без decrypt;
это не message ACK и не статус readable convergence.
Дополнительно — bounded authenticated запрос lane range от `next`, count≤64,
с source index `(domain,lane,epoch,n)`, чтобы gap не требовал полного inventory.
Это узкая зависимость A1, не реализация lifetime paging A3. Mailbox остаётся
opaque: scan cursor проходит **все** list pages, независимо от readable frontier;
не останавливается на первом pending item. Delete receipt разрешён только после
readable commit. Резерв для predecessor предотвращает deadlock заполненного inbox.

Предлагаемые review limits: pending 8 MiB/512 items на domain,
16 MiB/1024 items на authenticated peer, 64 MiB/4096 items на profile;
внутри них резерв одной максимальной допустимой записи для `next`.
Первый pending domain допускается только после резервирования head и в общей квоте.
Размер записи `E≤512 KiB` согласуется с envelope overhead и fan-out при negotiation;
слишком большой event отклоняется до crypto. Общий RAM budget ingress — 64 MiB
включая buffers/workspaces/indexes; число items в page уменьшается до доступной квоты;
≤128 загруженных domains, ≤2 retained epochs на пару/context. Превышение любого
лимита даёт backpressure **до acceptance**, без eviction принятого ciphertext.
Ratchet/pending/receipt metadata тоже учитываются квотой; обычная user-owned
committed history имеет отдельный disk admission budget, не TTL-очистку.
CPU: ≤8 drain events за turn, ≤2 concurrent fetches/profile; verification token
bucket на peer 8/s, burst 16; bounded invalid-input cache 1024 entries; gap retry
1→60 s с jitter, без циклического trial-decrypt всех pending. Числа — предложения
для проверки, не уже измеренные продуктовые настройки.

Если predecessor навсегда отсутствует, lane остаётся `waiting-for-predecessor`:
ключ не продвигается, ACK нет, диск ограничен, остальные domains работают
в пределах своих зарезервированных квот.
Нельзя одновременно гарантировать бесконечное переупорядочивание, конечную память,
удаление старых keys и независимое продвижение через любые gaps. Этот вариант
выбирает сохранность ценой availability. Ожидание удлиняет жизнь chain secret и
откладывает post-compromise healing; endless skipped-key retention отвергнуто
([Double Ratchet §8.4](https://signal.org/docs/specifications/doubleratchet/#deletion-of-skipped-message-keys)).
User-approved новый epoch может возобновить будущую связь, но старый state/pending
остаётся в квоте; auto-reset/rewrap/import/downgrade нет. Потерянные у всех copies
или уже удалённые keys автоматически не восстанавливаются.

## 3. Совместимость — обязательный migration review

| Объект | Переход и прежние данные | Evidence / неизвестное |
| --- | --- | --- |
| Persistent ratchet/load | Новый tagged namespace + frontier; legacy v1/v2 loader и один прежний device-pair state сохраняются отдельно. Нельзя переименовать его в per-context session. | [Текущий loader](../../crates/kilogram-ratchet/src/lib.rs#L1224) проверен; новый transactional manifest ещё не спроектирован побайтно. |
| Signed wire/auth/KDF/negotiation | Новая явно выбранная версия event/order/transcript; свои signature/hash tags, свежие Olm inputs. Mutual device-signed selection и durable monotonic floor до writer activation; никаких parse-error fallback. | [Действующая downgrade policy](../../docs/RFC-0196-m1169-mixed-version-downgrade-matrix.md); encoding, capability и crypto binding требуют review. Legacy Postcard v6 остаётся public default этого этапа. |
| Readable history / unread legacy ciphertext | Старые EventId/подписи/projections/parents неизменны. Legacy unread остаётся с исходным state в bounded inbox. Guarded drain разрешён только при доказанном contiguous crypto prefix; неизвестные old DH tails блокируются. Автоматического lossless upgrade всей старой истории нет. | Initial-state single-chain fixture можно дочитать от index 0. Для существующих pickles нужен проверенный экспорт receive frontiers; public wrapper его сейчас не даёт. Уже вытесненные keys не вернуть; recovery только отдельным consent flow. |
| Outbox/mailbox/checkpoints | Материализованные legacy events повторяются побайтно в legacy domain; ещё не материализованные queued тексты переходят только по согласованному cutover. Mailbox items сохраняются до readable commit/TTL; checkpoints типизированы по версии и phase, не переносятся как readable. | Текущий [delete guard](../../crates/kilogram-mailbox-client/src/replication.rs#L978) требует app receipt; новый inbox/range index и restart reconciliation ещё не проверены. |
| Mixed peers/restart/rollback/consent | Старый peer не получает новый event; legacy lane защищается guard/backpressure и честно может ждать. Explicit signed migration consent с точным context/peer/epoch; pinned старые state и floor переживают restart. Старый binary после activation отказывает в write; rollback к legacy только отдельной согласованной церемонией без стирания данных. | Нужны schema-version writer gate и crash tests. Whole-vault rollback без внешнего witness не обнаруживается абсолютно. Удаление profile и скрытый fallback запрещены. |

Writer gate требует новой версии **обязательного vault manifest**, которую
[старый loader отклоняет](../../crates/kilogram-state/src/vault.rs#L2574),
а не дополнительного файла, который он проигнорирует. Activation без vault
отклоняется до согласованной миграции. R3 должен проверить реальный исходный
binary на upgraded profile; одной новой проверки номера версии недостаточно.

Crash contract: до pending commit повтор запрашивается у источника; после него
restart восстанавливает ciphertext и frontier. Между decrypt и readable commit
откатываются все изменения; после vault-primary commit/shadow interruption
выполняется существующее recovery, затем dedup по projection. Между readable
commit, mailbox-ledger commit, ACK send и delete любой повтор возвращает тот же
ACK, без второго decrypt/ordinal. Lost delete response даёт idempotent retry.
ACK у sender фиксируется с delivered marker; storage receipt не завершает outbox.
TTL expiry при отсутствии других источников — availability loss, не «delivered».

## 4. Узкий будущий change-set и regression gate

Production: `kilogram-ratchet` (namespace/frontier guard), `kilogram-protocol`
(`lib.rs`, `wire.rs`, typed carrier adapters), `kilogram-session` (ingress/range
planning и canonical adapter), `kilogram-store` + `kilogram-state` (inbox/index,
typed vault transaction/load), `kilogram-sync-capability` +
`kilogram-peer-capability-store` (selection/floor), `apps/kilogram-cli/src/main.rs`
и `runtime_mailbox.rs` (все receive/send/ACK paths),
`kilogram-mailbox-client/{ledger,replication}` (cursor/reconciliation).
Transport/IPC/desktop — только version admission и честный pending status,
если текущий контракт его не выражает. Volunteer store не получает порядок/ключи.

До реализации зафиксировать synthetic **v5 golden fixture**: initial recipient
ratchet/account/pickle secret, 256 authorized events, membership, время, hashes
и точные страницы реального `SyncServer`; выбрать один failing run, проверить
first-page membership и expected missing key. Использовать fixture без keygen
в каждом regression. Тот же ciphertext/initial state ordered control обязан
дать 256 plaintexts. В отличие от audit probe, assertion regression — **вся
readable history**, failure должен давать nonzero exit. На исходном HEAD это
`MissingMessageKey`, а не unsupported-feature/decoder error. Новый legacy ingress
должен пройти тот же single-chain fixture, начиная с проверенного index 0;
new-format domains проверяются отдельным fixture после mutual selection.
Несовместимость формата не засчитывается как устранение A1.

| Уровень / сценарий | Проверяемый результат |
| --- | --- |
| Component C1: golden v5, 256, реальные hash pages≤64; reload **после каждой** страницы | Все 256 `LocalTextProjection` открываются и равны manifest text+author+conversation; EventId/parents неизменны. Baseline red → guarded legacy green, без изменения ciphertext. |
| Component C2: ≥256, два contexts, обе стороны, несколько DH chains; сначала B, поздние A pages | Независимые domains; ACK не считается Olm index; нет eviction ещё не обработанных keys. |
| Component C3: ≥256 одного context, direct/relay новые + старый mailbox; pages/reverse order | Durable pending, prefix fetch/полный mailbox scan, затем exact readable convergence; проверять поздний item, не только set EventId. |
| Component C4: duplicate/retry из всех paths и двух volunteer replicas | Один event/projection/ACK; повтор не вызывает decrypt и не меняет frontier. |
| Component C5: missing genesis/middle, quota full, unsigned/outsider/revoked/wrong recipient, forged n/index/epoch, fork | Неверные inputs не меняют keys/frontier/history/ACK; после лимита admission закрыт. Head reserve даёт прогресс после появления gap; при permanent gap другие contexts работают, pending bytes/CPU ограничены через restart. |
| Runtime/process R1: два реальных runtime, ≥256, restart между pages, legacy и negotiated new mode | Те же readable assertions через vault/history API; реальные outbox/checkpoints/status. Включение v2 само по себе не считается результатом. |
| Runtime/process R2: kill до/после pending; после decrypt до commit; после vault-primary до shadow; после readable до ledger/ACK; после ACK до sender marker; после delete до receipt | После restart либо точный pre-state, либо полный post-state; ни ACK без projection, ни double decrypt, ни потеря outbox. Replay reconciliation завершает доставку. |
| Runtime/process R3: mixed-version, migration interrupted, downgrade/old writer, partial backup | Без silent activation/erase/reset; readable history сохранена, unread состояние и ограничения явно видны; bad floor отклонён. |
| Physical H1: два хоста/разные сети + volunteer replicas, ≥256 в нескольких bounded upload waves | Direct/forced-relay/mailbox, sender offline при receive, restart/sleep/lost ACK; exported readable manifests совпадают, TTL exhaustion отражён как unavailable. Это отдельный будущий acceptance, не локальный process test. |

Запуск будущих Windows harnesses: `scripts/run-cargo-tests-stable.ps1 -Package …
-Filter … -CargoJobs 2 -Run` (сам компилирует `--no-run` и запускает стабильные EXE).
Process/field binaries тоже имеют постоянные пути; content hashes пишутся в logs.
Полный workspace/mutation corpus для этого design этапа не запускался.

**Открыто для review:** цена strict gap blocking; точный epoch/transcript и
receive-frontier API для legacy pickles; byte/schema/capability migration и
cutover незавершённых outbox; negotiated event cap с fan-out/mailbox overhead;
выбор численных квот и отображение pending. A3 требует отдельного lifetime paging,
A2 остаётся security priority до внешнего использования; A7 freshness может мешать
setup checks. Onboarding, multi-contact, actor redesign, contextual identity,
группы/media и dormant recovery integration здесь не реализуются.

**Точка остановки:** baseline + этот проект передаются на независимый
архитектурный review. Продуктовую реализацию/миграцию до его решения не начинать.

## 5. F1/F2 feasibility spike — уточнения после review

2026-10-23, тот же implementation HEAD. [Harness/команды/границы](lossless-sync-a1-spike/README.md),
[фактический run](lossless-sync-a1-spike/evidence/run.log),
[повтор](lossless-sync-a1-spike/evidence/run-repeat.log).
Оба exit 0. F1: 512 exact plaintexts в двух contexts, по 8 DH transitions,
14 held successors, 48 guard pickle/reloads, 14 exact rejection/replay checks.
F2 — fake ciphertext/численные бюджеты, не реальные vault/wire transactions.

| Review | Подтверждено / уточнено | Осталось открытым |
| --- | --- | --- |
| R1 | Real Olm guard с собственным ordinal от agreed genesis не продвигается через gap; inner-binding failure отбрасывает trial Session. Legacy K0/K1/B0/L0 даёт **UnresolvedLegacyTail**, а unsafe native control после переходов вытесняет K chain. Выбран version-pinned serde adapter для native **local** cursors/skipped keys. | Native next=6/skipped `[1,2,3,4]` не дают sender tail length или application ordinal. Автоматический upgrade произвольного profile не доказан и может остановиться уже на первом unseen DH key; authenticated closure evidence не разработано. |
| R2 | Три ordinary slots + head slot/full lease не устраняют hash-page deadlock: legacy повторяет `[2,1]`, не достигая genesis 0. Выбран **CompatibilityBlocked/UpgradeRequired**. Prefix-capable модель и mailbox cursor/wrap доходят до всех 6. | Нельзя приписывать old peer byte/range credit. Этот ограниченный контракт не гарантирует legacy convergence; расширенный repair algorithm не реализован. |
| R3 | Admission резервирует **полный commit**, включая event/auth, projection, ratchet/frontier, ACK intent и staging/shadow. Full history и oversized legacy дают StorageBlocked без ACK/переписывания. Response construction ограничивается encoded bytes целого frame, а не только count≤64. | Квоты §2 отозваны как гарантия progress: реальный общий vault cap 512 MiB, frame 8 MiB; учёт всех дельт/FS overhead и production negotiation ещё требуют проверки. |
| R4 | Durable subset после разрыва/квоты входит в следующий possession inventory, но partial round получает **error/no SyncComplete**. Четыре frontiers разделены. Test epoch выбирает одну session/roles/base key/OTK/SID до data writers; оба handshake занимают n=0 своих lanes. | Реальны Device signatures и Olm MAC; credential admission, role election и OTK reservation заданы trusted fixture. Production races, authorization, atomic vault/ACK transitions и fencing ещё не проверены. |

Исполняемые переходы F2 (model page width=2 масштабирует production 64):

| Переход | Durable state / ответ / retry |
| --- | --- |
| AcceptedPending | Event/auth ciphertext и full commit lease сохранены; possession растёт, crypto/readable/causal/ACK не растут. Full batch может подтвердить exact storage IDs; message ACK отсутствует. |
| AcceptedReadable | Contiguous prefix и полный commit; readable/crypto/causal и ACK intent меняются вместе в модели. Повтор не добавляет второй ACK. |
| Backpressure | Нет ordinary slot; existing ciphertext не удаляется. Если часть batch уже committed — round error, **не subset SyncComplete**. Legacy retry только после изменения inventory/capacity/peer capability. |
| StorageBlocked | Не помещаются event/full lease/history или недоступна physical write; до admission нет новой possession. При сбое после pending commit accepted head сохраняется, ACK не создаётся. Retry после capacity/IO change. |
| InterruptedPartial | Сохранён checkpoint `[5,4]`; после restart inventory `[4,5]`, остальные frontiers пусты. Полного completion нет. Новый authenticated round использует новую possession, не выдуманную полноту прежнего batch. |
| Invalid | Никаких pending/frontier/ACK изменений; ограниченный negative cache/backoff. В F2 это mock classification, crypto rejection проверяет F1. |

Legacy `SyncComplete` не является message ACK и отдельно не подписан; сохраняются
authenticated transport/session, signed inventory/diff и exact equality полного
набора IDs. Possession не включается автоматически в parents исходящего события.
Mailbox cursor/deadline сохраняются после **scan**, включая отказ admission;
на EOF курсор обходит retained items заново с bounded work/backoff, delete только
после readable commit. Full history не лечится обходом страниц.

Нужен reservable upper bound **всего** readable commit до Pending acceptance,
общий со всеми vault writers и лимитом snapshot, плюс физический staging budget.
F2 демонстрирует lease=30 условных units (10 event/auth + 8 projection + 4 ratchet/
frontier + 2 ACK + 6 staging), не измеряет production overhead. Storage receipt
не гарантирует future physical write. E≤512 KiB применим лишь к явно negotiated
новому формату; валидный legacy fan-out до 32×64 KiB может быть больше E.
Он сохраняется byte-exact, а отсутствие capacity выражается StorageBlocked,
не Invalid и не «delivered». Новый sender должен включать encoding/framing/auth
overhead в byte budget≤8 MiB: 64×512 KiB уже больше frame cap. Старый sender
такого credit не понимает; гарантировать приём его следующего batch нельзя.

При двух retained blocked epochs **третья activation отклоняется даже с consent**.
Owner может ждать predecessor, явно разрешить конкретное bounded увеличение
storage/epoch quota при наличии capacity либо отдельно решить судьбу своих копий.
Эта owner ceremony не реализована; автоматических erase/reset/rewrap нет.
Поэтому обещание безусловно возобновить связь новой эпохой из §2 снято.

Writer fencing остаётся обязательным **process gate**: запустить исходный binary
до cutover, задержать send/receive/ACK/queued mutation, затем попытаться активировать
новый state и возобновить старый процесс. Activation должна отказать при живом
незафенсенном writer либо сначала доказанно quiesce его под version-independent
exclusive ownership. Проверять mutable entrypoints после network await, stale
workspace и crash до/после manifest commit. Старый процесс нельзя заставить
уважать новый необязательный token; если существующий lock это не обеспечивает,
activation блокируется. Одного schema gate при новом запуске недостаточно.
В этом spike такого process evidence нет.

**Предлагаемый первый production slice после review:** внутренний opt-in
vault-backed ingress для одной legacy conversation и recipient с известным
genesis, только single-chain prefix: pending possession, точный round-error
контракт, full-commit admission и readable projection/ACK transaction. Реальный
runtime должен дочитать 256 небольших ciphertext через несколько hash pages
с restart, **когда они помещаются в admission budget**; при quota starvation,
unknown legacy DH tail и full history — проверяемая честная блокировка без
потери принятого ciphertext/false ACK. Arbitrary existing profiles, новые domains,
protocol activation/migration и default rollout этим slice не объявляются готовыми.
Следующие решения о production epoch/closure/prefix protocol остаются на review.

**Остановка:** F1/F2 feasibility сданы для согласованного архитектурного review.
Production crates, wire default, workspace dependencies и vault loader не изменены;
runtime/process/network/GUI/physical acceptance не выполнялся, A1 остаётся открыт.
