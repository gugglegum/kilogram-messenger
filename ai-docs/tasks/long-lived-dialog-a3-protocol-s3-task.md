# A3-S3: протокол2 и ограниченный native guard — сначала контракт

2026-10-24, Asia/Yekaterinburg. Разрешена только подготовка конкретного integration
plan на root review; product implementation и публикация этой части пока не поручены.
Это следующая логическая часть уже разрешённого A3, не новый product milestone.

## Основание, baseline и исполнитель

Пользователь разрешил весь A3, последовательные contract/implementation review,
repair и commit/push принятых частей: human message
`01a16bf9-89cd-7cb3-8661-36eef244e0fc` в root
`01a164ec-589c-7b41-bc34-3ba158d1f22b`; scope описан в [A3 task](long-lived-dialog-a3-task.md).
Callback authorization — прежние `01a1657e-b03f-7d01-aaf0-e5af979125fd` и
`01a16568-4515-7420-b3af-32d6fc0bfeb4`. Прямые разрешения действуют между итерациями.

Исполнитель прежний OD1: «Подготовить baseline lossless sync»,
`01a1653c-0154-7611-8e8c-044fbe1e4d7a`, GPT-6.1 Sol / Max / default без Fast.
Новые чаты/субагенты не создавать.

Baseline **e136c59b829c6ecc5eb501273f25091e9b9e4789**, parent61524df… . Root независимо
подтвердил HEAD/origin/master/actual remote tip, exact268 changed paths/blobs,
267 accepted files bytes/SHA и manifest blob3fb94629f17643b941cb12ea52a6c9317b1a680b.
После публикации дерево было чистым; сейчас новые root task/README/coordination
относятся к S3. S2 принят по [review4](long-lived-dialog-a3-history-s2-implementation-review-4.md).
Его contract/evidence/manifests, старые failures и fixtures остаются frozen.

## Задача первой сдачи

Подготовить `long-lived-dialog-a3-protocol-s3-plan.md`: одну рекомендованную,
реализуемую схему следующего component slice и точную regression matrix.
Основной [A3 contract](long-lived-dialog-a3-contract.md), особенно §2/2.1/3/8,
и [его review2](long-lived-dialog-a3-contract-review-2.md) уже приняты. Уточнять
implementation seams; не переизобретать архитектуру и не расширять scope молча.

1. **Codec2 и trust boundaries.** Точные canonical layouts, domains/IDs/signatures,
   limits и exports для Core2/Event2/Inner2/Frame2, mixed-format DataItems и необходимых
   authority/mode/control values. Разделить parse/bounds, cryptographic validation и
   future live-policy checks: parsed DTO не объявлять authenticated. Одинаковый
   ModeProof Q в Core/Ready/Resume/Receipt; новые domains не меняют сохранённый AuthRef.
   Old Event1 bytes/IDs/decoders/512 limits не ослаблять, fallback-by-trial запрещён.

2. **Guard2 и сохранение session.** Bounded checkpoint вместо растущего RAM seen-set,
   exact own/peer cursors/first/previous/ClosedTail/rotation phase. По actual pinned
   vodozemac0.10.0 и существующему byte-oriented adapter расписать pre-native checks,
   native trial/header/inner checks и discard при неуспешном результате. Сохраняются
   account/session/epoch/SID; новых epochs/OTKs/reset каждые512 нет. Native+guard
   serialized cap128KiB. Не получать RatchetCount через private pickle/debug parsing.

3. **Checked arithmetic и provenance.** Конкретные pure preflight функции/ошибки для
   j+1, n+1, sequence/generation, T=2*g+role и T_pre→T_post; A0/B0/A1/B1,
   continuation, same-key late tails, new peer DH в Inactive. Near-limit vectors
   остаются чистой bounded арифметикой; реальный >512 native run — отдельный тест.
   Поддерживаемый L1 import принимает только проверенное прежнее checkpoint origin;
   actual file migration и live handoff в этот component slice не включаются.

4. **Atomic storage seam.** Показать, как Guard2 пользуется authenticated seen-key
   index и как native checkpoint/seen-key/source/HistoryOperation сохраняются в одной
   допустимой транзакции. Сопоставить это с реальными S1/S2 namespaces/closed writer,
   accounting, stale generation, immutable conflict и failure/retry outcomes.
   Нельзя открывать generic writer для bound history, обходить H1/H2 или принимать
   caller claim вместо source/receipt correspondence. Если нужен узкий новый typed
   seam, предложить exact API/invariants на review, пока не менять код.

5. **Состав реализации и доказательств.** Предложить размещение файлов между
   kilogram-protocol, kilogram-ratchet и существующим CLI guard, минимальные exports
   и таблицу «новое / reused / остаётся future». Matrix должна включать canonical
   round trips/size bounds, version separation, scope/role/header/inner disagreement,
   replay/seen lookup failures, counter/phase checks, trial rollback и atomicity.
   Для реального native component — обычная длинная chain>512 и обе DH directions,
   reload accepted checkpoint и exact plaintext/IDs. Указать точные counts и
   фиксированное test setup; не подменять этим future two-process runtime/offline gate.

## Граница этого component slice

План должен отделять codec/guard/component validation от будущих actor/transport
activation, bilateral live policy и deployment. В draft определить необходимые
control types, но не включать сюда UI/onboarding/multi-device/relay rollout, storage
migration command, live L1→L2 handoff или полную runtime activation.
Если надёжная граница требует ещё более узкой части, обосновать её без реализации
и без нового набора произвольных milestone-версий.

S2 measured storage throughput/latency и trusted synthetic DTO не являются доказательством
native/protocol readiness. Whole A3 сохраняет640 Text в каждом направлении, offline256,
upgrade/fault/route gates. Absolute UX latency и I/O amplification остаются отдельными
вопросами. Не повторять100k fixtures/benchmarks для подготовки этого документа.

## Режим работы и сдача

Сначала читать текущие modules/tests и pinned dependency sources адресно. Никаких
Rust/product/helper edits или новых dependency/lock updates до root review плана.
Тесты при одной документации не запускать; достаточно проверки local links и diff.
При последующей реализации — stable Windows runners, compile-only Cargo перед ними,
Jobs12/BelowNormal и принятый проверочный профиль где применимо; сейчас только план.

development-environment.md clean SHA53074ff… сохраняется; удалённую330ad3… правку
не восстанавливать. L1 EXE и S1/S2 fixtures/evidence сохраняются. Ограничения A3 task
на соседние работы и deferred A2 scope продолжают действовать.

Сдача: baseline, путь/bytes/SHA плана, точные code/source ссылки, рекомендуемая
граница следующей реализации, matrix и нерешённые вопросы. Один содержательный
callback root без смены его model/thinking, затем stop до contract review.
Commit/push текущего draft отдельно не выполнять: root сначала примет его exact diff.
