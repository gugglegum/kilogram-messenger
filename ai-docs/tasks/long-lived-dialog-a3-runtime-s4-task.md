# A3-S4: два процесса, перезапуски и offline-доставка — сначала контракт

2026-10-25, Asia/Yekaterinburg. Первое поручение — только integration contract
на root review. Реализация и публикация выполняются после соответствующей приёмки.

## Основание и baseline

Пользователь в root `01a164ec-589c-7b41-bc34-3ba158d1f22b` прямо поручил:
«Да, запускай следующий этап — диалог двух процессов с перезапусками и offline-доставкой.
Сначала review контракта, затем реализации; после приёмки — commit и push».
Действуют AGENTS.md и разрешение на review/repair cycles в рамках A3. Повторное
разрешение человека для обычной публикации принятой работы не требуется.

Исполнитель — прежний OD1, «Подготовить baseline lossless sync»,
`01a1653c-0154-7611-8e8c-044fbe1e4d7a`, GPT-6.1 Sol / Max, без Fast.
Новые чаты/субагенты для этой сдачи не нужны.

Baseline **7f82036ee5803edd64be98b73572604250f5961c**, опубликованный S3.
Root независимо сверил sole parent e6f9b8ff…, все194 changed paths/blobs,
raw hashes/manifest и GitHub master (compare SHA→master: identical).
Перед новым заданием index/worktree чистые. Новые task/README/coordination — S4.
S3 принят как library/components, не production Active2 или offline runtime.

Минимальный dossier: [A3 contract](long-lived-dialog-a3-contract.md), особенно
§2/3/7/8/9, [review2](long-lived-dialog-a3-contract-review-2.md),
[S3 plan](long-lived-dialog-a3-protocol-s3-plan.md),
[S3 acceptance](long-lived-dialog-a3-protocol-s3-implementation-review-1.md),
[S3 implementation report](long-lived-dialog-a3-protocol-s3-implementation.md),
[A3 task](long-lived-dialog-a3-task.md) и actual production OD1 engine/runtime/IPC.
Остальное читать адресно. Основная архитектура уже принята; нужны конкретные
production seams, а не повторное проектирование wire или новый test-only driver.

## Первая сдача

Создать `long-lived-dialog-a3-runtime-s4-contract.md`: одна рекомендуемая граница
реализации, точные API/state transitions, code references и regression matrix.
Ссылаться на принятые layouts вместо их переписывания; существенные изменения
принятых правил отдельно вынести на root review до реализации.

1. **Production owner.** Call graph CLI/IPC→single owner→atomic writer→transport;
   current policy, activation, native checkpoint, queue/pending/reconnect и fair
   scheduling. Сопоставить старый Engine/V4 и indexed writer. Исключить двух writer
   одного profile, whole-history RAM cache и обход closed writer через generic API.
   Назвать точные modules/exports и минимальные необходимые новые seams.
2. **Authority/activation.** Fresh2 до durable Active2, единый Q, Root pins,
   monotonic authority/revocation/expiry/clock failures и renewal. Historical proof
   не заменяет current permission; Root secrets только в admin ceremony. Network
   dispatch/receive/ACK требуют actual admission/exporter/fresh lease и policy/
   generation/funding recheck перед commit. Offline materialization после Active2
   использует принятую local Root2 policy без фиктивного lease. Restart не создаёт
   session/OTK/genesis заново и не выдаёт static verification за runtime permission.
3. **Durability/offline.** Request lookup до новой capacity, atomic queue→source/
   projection/native/request/debt; publication после commit, receipt после readable
   receive commit. Disk backlog может превышать128, RAM scheduling bounded128.
   Единый ordered ingress для live/catch-up/retry/reconnect, full64+head, late chain,
   lost ACK и прогресс C2. Для новых crash/IO windows — durable state, exact retry,
   precommit/committed/ambiguous outcomes; исключить holes/duplicates/false Delivered.
4. **Existing/version boundaries.** Fresh L2 отдельно от реального L1 storage
   upgrade и live handoff по §7/§3 A3; сохранить Account/D/E/SID, old bytes/IDs,
   receipts/accepted queue/pending. Если переход L1 требует следующей реализации,
   явно обосновать границу для root: действия S4 с existing profiles, безопасный
   отказ неподдерживаемых действий, конкретный successor и открытые gates.
   Fresh-only результат не закрывает A3. Profile JSON/ordinary startup/downgrade
   не обходят durable mode/fence и не переключают старые данные на generic writer.
5. **Evidence и стоимость.** Для каждого свойства указать actual production
   entrypoint, fixture, наблюдаемый результат/rejection boundary и регрессии.
   Различать kill, graceful restart, injection, component и реальный network run.
   Расписать ordinary direct/own loopback relay/reconnect и exporter freshness.
   Измерять setup/materialize/catch-up/history отдельно; общая длительность
   сценария не является временем чтения истории или профилем crypto/IO bottleneck.

## Измеримый runtime результат и граница A3

Для fresh runtime: два actual процесса, ≥640 Text каждой стороны, receiving DH≥16
на сторону, неизменные D/E/SID за511; exact IDs/requests/UTF8/cursor после restart
обоих. Offline: peer действительно остановлен после setup, ≥256 durable encrypted
Text без live lease, disk backlog>128/RAM≤128, ≥8 catch-up pages, restart между
выбранными pages, full64+head/reorder/late chain/lost ACK/exact retry. C2 одновременно
duplex≥32 Text/сторону. Counts подтверждают actual packet calls/records, не mocks.

Таблица A3-1…A3-9 должна показывать completed/reused/new/deferred. A3-1 также
требует resumed L1: fresh trace его полностью не закрывает. S3 native probes и
S1/S2 storage100k reuse с честными source/profile границами; не повторять тяжёлые
fixtures без затронувшего изменения. Новые current-policy/activation/restart/fault/
version paths требуют проверки. R1–R4/I1–I4 сохраняются или получают точный эквивалент.

Исходные budgets §9 A3: final dialogue+offline/2C≤40min, selected upgrade/fault/
version≤20min. S3 aggregate1200s — исключение только S3. В контракте определить
наборы/verification profiles/budgets; изменение правил заранее вынести на review,
не сокращать counts после FAIL. Targeted clock явно отличается от real expiry.
Сохранить A как sole dialer/coordinator, B listener, operator locators, два
single-device участника и два C. GUI/onboarding/multi-contact/groups/multi-device/
mailbox/M1.2.63/deferred A2 reproducer не включать; whole A3/MVP не объявлять готовыми.

## Сдача и остановка

Сейчас только чтение и документ: product/Rust/helper/dependencies/fixtures не
менять, tests/benchmarks не запускать. Проверить local links и diff. Frozen
contracts/reviews/evidence/manifests не переписывать. Clean development-environment
53074ff… сохранить, удалённую чужую правку не восстанавливать. L1 EXE d9b7ff… и
S1/S2 fixture DB не модифицировать. Unrelated worktree changes не включать.

Будущие Windows network harnesses — только stable runner после compile `--no-run`,
fixed EXE paths/source/content hashes, Jobs12/BelowNormal. Независимые проверки
можно распараллеливать без конкурирующей перезаписи stable EXEs/fixture roots.

Сдать baseline, path/bytes/SHA256 контракта, scope, решения для root и matrix.
Один содержательный callback root без model/thinking override, затем stop до
contract review. Прямое human разрешение callback в твоём чате:
`01a16faf-e3a0-76a3-912a-1249ed4ca1e5`, результаты/проблемы/уточнения и последующие
этапы до отзыва; эта запись лишь указывает на исходное разрешение пользователя.
Draft commit/push не делать. Root сначала принимает exact contract diff, затем
поручает публикацию и реализацию; после implementation acceptance — exact commit/push.
