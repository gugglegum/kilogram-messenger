# A3-S2: bounded history и устойчивый cursor

2026-10-24. Вторая логическая часть уже разрешённого A3.
**Первое поручение — короткий integration contract на review, без product edits.**
Не переоткрывать общую архитектуру и не создавать новую общую RFC-серию.

## Основание и baseline

Human `01a16bf9-89cd-7cb3-8661-36eef244e0fc` в root
`01a164ec-589c-7b41-bc34-3ba158d1f22b` разрешил весь A3, contract/implementation
review, repair cycles и commit/push принятых частей. Callback permissions из
`01a1657e-b03f-7d01-aaf0-e5af979125fd` и `01a16568-4515-7420-b3af-32d6fc0bfeb4`
ранее проверены. Исполнитель прежний OD1, чат «Подготовить baseline lossless sync»,
`01a1653c-0154-7611-8e8c-044fbe1e4d7a`, host local, GPT-6.1 Sol Max/default без Fast.
Новые чаты/субагенты не нужны.

S1 опубликован `4d1d8b191f6e84c812c10ed3a2fa17a8dfa03629`, единственный parent
`f59d285d5704dc2c6969b6f8e416f204223e27cc`. Root независимо сверил actual remote
refs/heads/master,76 committed paths/blobs, все raw working hashes и manifest self
`6517dbfda97a33e12cb1a2954145ded16b9a1b7c01b9dddd8951554415fb8c4d` /
blob`f8fc060a8f10cc507c481c67a2b47992fe42124d`. Index пуст; до S2 docs единственная
чужая правка development-environment.md. Non-log whitespace checks PASS; замороженные
raw logs с исходным whitespace намеренно не переписывались.

Нормативная база: [контракт A3](long-lived-dialog-a3-contract.md), особенно §4–5/9,
[memory amendment1](long-lived-dialog-a3-memory-budget-amendment-1.md),
[S1 final review](long-lived-dialog-a3-storage-s1-review-3.md). Frozen accepted
contract/report/manifest/evidence не редактировать задним числом.

## Почему следующий именно этот слой

S1 доказал persistent point/range/atomic ledger и100k raw Event records. Полная
пользовательская история ещё не реализована: два causal prefixes, readable projection,
restartable snapshot cursor и ограничения настоящей page. S2 закрывает этот отдельный
контракт поверх готового store; последующая native/protocol2 интеграция получит точный
интерфейс записи и чтения. Transport/crypto engine и пользовательский UI здесь не нужны.

## Первая сдача: точный integration contract

Создать `long-lived-dialog-a3-history-s2-integration-plan.md`: ориентир2–4 страницы
основного текста плюс компактная matrix/необходимые layouts. Одна схема по §5 A3,
не новый выбор между unrelated architectures. Обязательные конкретные решения:

1. Typed authenticated metadata: связь Event/Projection/Lane, role/ordinal и parent
   role/ordinal/EventId, C/D/E/SID, committed exclusive frontiers. Сейчас
   `ProjectionValue.binding` — adapter-owned opaque bytes: определить canonical
   layout/version и проверку связи с source/Core, не объявлять generic bytes уже
   verified metadata. Где хранится постоянная view identity/heads, кто меняет их
   атомарно вместе с source/projection/native/credit, какие проверки выполняет
   storage и какие остаются будущему verified native/protocol adapter.
2. Bounded writer interface: не допустить silent lane holes/conflicting IDs/heads
   вперёд unreadable projection. Читать родителей точечно; не загружать весь DAG.
   Обновление heads/metadata сохраняет S1 per-C quotas, positive-charge coverage,
   R1 final-baseline conservation и R2 matching-receipt rule. Не добавлять обходной
   importer/write path для runtime. Fixture trust boundary явно отделить от product.
3. Atomic capture F и exact cursor codec/MAC, persistent vault-derived `cursor`
   domain из A3. MAC key остаётся внутри store; raw master/native/device secrets не
   экспортируются. Cursor включает incarnation/C/D/E/SID/F/next_A/next_B,≤384B
   binary/≤512 Base64 chars; обычный restart/append не инвалидирует snapshot.
   Нельзя принимать caller-supplied scope вместо authenticated persisted identity.
4. Page алгоритм буквально §5: максимум два next candidates, implicit same-lane
   edge, сохранение signed parent edges, eligibility по emitted ordinal frontier,
   минимальный eligible EventId. Missing/corrupt/blocked remaining rows — ошибка,
   не пустой успешный конец. Read transaction короткая, только на текущую page;
   cursor не удерживает MVCC root или server-side session/cache.
5. API first/resume/point, finished/invalid cursor/integrity outcomes, limits1..32,
   exact encoded CBOR≤256KiB и JSON≤1MiB с escaping. Определить узкий reusable
   serialization boundary; CLI/IPC/network wiring пока не делать. Локальная уже
   readable history не требует присутствия peer. Delivery state остаётся отдельно.
6. Конкретные modules и verification plan: small causal cases, fault/reopen и
  100k настоящих indexed history records со scope/lanes/projections. Объяснить
   reuse существующих fixtures либо необходимость одного отдельного build.

Если точное S2 отображение выявляет конфликт принятого A3/S1, привести небольшой
counterexample и узкое предлагаемое уточнение до code. Не менять bounds/schema
молча. Заново мерить принятые unchanged S1 loops на contract phase не нужно.

## Ожидаемая реализация после root review

Небольшой history module рядом с `vault/indexed_dialog` и нужные typed codecs/APIs.
Предпочесть существующие зависимости/примитивы; изменение dependency/schema только
с конкретным обоснованием в integration plan. Не строить general query platform.
Native/protocol signatures/current-policy проверяет будущий adapter; storage test
не выдаётся за реальный E2E dialogue. L1 migration и runtime переключение пока вне S2.

Минимальная acceptance matrix будущей реализации:

- Empty/one lane/two lanes, cross-lane parent blocking, EventId tie-break и общий
  prefix/порядок с bounded complete reference в малых fixtures.
- Разные page sizes, все boundary cases, exact suffix без duplicates/gaps после
  append и reopen; прежний cursor не включает новые события, новый first включает.
- Cross-C/profile/incarnation/domain mismatch и изменённый cursor отвергаются;
  malformed/cyclic/missing metadata даёт явный отказ без изменения source/history.
- Atomic write отказ/ambiguous outcome не оставляет frontier впереди durable rows;
  targeted fault windows по новому path, без повторения всего старого kill corpus.
- Max encoded Unicode/control-character text и metadata; реальные response encodings
  укладываются в bounds, cursor не содержит секретов. Typed generic store остаётся
  scoped; known unreadable/non-history fixture не становится готовой историей молча.
- 1k/10k/100k настоящих history pages, ordinary append после bulk, repeatable resume
  после restart/concurrent append; node/value lookups и candidates bounded, без
  full-DAG массивов в обычном пути. Synthetic import явно0 native E2E messages.
- A3 memory amendment1, ratios≤4, absolute384MiB, app buffers192MiB и общий1200s
  scale ceiling; фазовые реальные memory/cache/I/O метрики. Старые S1 fixtures
  сохранять; не регенерировать100k многократно без затронувшего результата дефекта.

Windows harnesses compile --no-run и scripts/run-cargo-tests-stable.ps1, fixed EXE
paths, CargoJobs2/BelowNormal/locked/offline. Relevant fmt/strict Clippy и meaningful
checks; не broad rerun ради counters. Baseline L1 EXE d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1
в target/compatibility/long-lived-dialog-baseline сохраняется для будущей migration.

## Границы и остановка

Не менять wire2/native Olm/Root2/handoff/enrollment/live runtime, не начинать GUI,
onboarding/multi-contact/groups/mailbox/M1.2.63. A2 OPEN:7 warnings/strict gate FAIL,
deferred security reproducer не разрабатывать/не запускать; прежний extra secret
getter не нужен cursor и не добавляется под новым именем.

Foreign `ai-docs/development-environment.md` сохранить и исключить из публикации,
SHA-256 `330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Root S2 task/README/coordination относятся к следующему логическому изменению;
исторический S1 manifest отражает уже проверенный опубликованный snapshot.

После integration plan — один содержательный callback root с path/hash, решениями,
границами доверия и конкретными открытыми вопросами; остановка до review. Не менять
product code/tests и не commit/push до отдельной приёмки. Root затем разрешает
реализацию, ведёт repair cycles и поручает commit/push принятой части точным allowlist.
