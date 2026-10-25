# A3-S3: реализация Codec2, bounded Guard2 и closed storage seam

2026-10-25, Asia/Yekaterinburg. **Реализация принятого S3 component slice разрешена.**
Сначала код и проверки на root review; commit/push — после отдельной явной приёмки.

## Основание и baseline

Это часть уже разрешённого пользователем A3: human message
`01a16bf9-89cd-7cb3-8661-36eef244e0fc` в root `01a164ec-589c-7b41-bc34-3ba158d1f22b`.
Работа через прежний чат «Подготовить baseline lossless sync»,
`01a1653c-0154-7611-8e8c-044fbe1e4d7a`, GPT-6.1 Sol Max/default без Fast.
Новые чаты или субагенты для этой реализации не нужны.

Baseline **e6f9b8ff6c7e9305d7216a94b142f0220b0c4a25**, parent
`e136c59b829c6ecc5eb501273f25091e9b9e4789`. Root независимо проверил7 exact changed
paths/blobs,6 raw документов и manifest, единственного parent, HEAD/master/origin/master,
actual remote через `git ls-remote`, чистое дерево/index после публикации.

Нормативны [принятый S3 plan](long-lived-dialog-a3-protocol-s3-plan.md), SHA256
`b0e90087f43142d844f7de25963f62efe0bfb43f9c23743dc816847ab4c13a49`, и
[review2](long-lived-dialog-a3-protocol-s3-plan-review-2.md), SHA256
`172cfaefd378841513bcfb44ab1298bd85d73752ed0b7e614ce02d67ac513830`.
Они уточняют [общий A3 contract](long-lived-dialog-a3-contract.md), не заменяют его.
Все опубликованные S3 plan/reviews/manifests теперь frozen; результаты реализации
оформлять отдельным report/evidence. Текущие root task/README/coordination относятся
к новой реализации и должны сохраниться.

## Реализуемая граница

Выполнить §1–§6 плана, в предложенном dependency graph и порядке:

1. Typed Codec2/static verification: canonical bytes/domains/limits, exact selected
   proof/Q, authority/receipt/controls, явные parsed/verified wrappers и single-decoder
   mixed-format dispatch. Сохранить bytes/IDs/limits старого Codec1.
2. Pure Guard2/preflight/checkpoint: fixed two-lane state, authenticated Seen lookup,
   checked successors/native T до calls, trial rollback и cap общего sealed checkpoint.
   Сохранить account/session/D/E/SID; не добавлять resets/OTK refill через512 сообщений,
   private pickle/debug parsing или patch pinned native dependency.
3. Closed state operations и read correspondence: source-derived metadata,
   normalization только HPB1 genesis request при exact wire/R4, resolver O/A/F/R,
   mandatory pending capture и immutable F/receipt Control links. Всё вместе с
   Native/Seen/source/projection/credit в managed atomic writer, с H1/H2 и shared Author
   fences. Старые trusted/generic операции не обходят bound2 проверки.
4. Test-only CLI component driver, соединяющий реальные Codec2/native/guard/indexed
   store; cfg(test) wiring без production actor/transport activation.

Минимальные изменения в kilogram-protocol, kilogram-ratchet, kilogram-state и CLI
test modules по §5. Existing byte adapter остаётся прежним, dependency/lock changes
не предполагаются. Если реальный compile/API seam требует отклонения от контракта,
сначала описать конкретное препятствие root; не ослаблять проверку и не добавлять
generic escape hatch. Продолжать независимые части, не зависящие от этого вопроса.

Не входят production bootstrap, current-policy owner, file migration, live L1→L2
handoff, ALPN2 runtime/relay rollout, offline actor, UI, multi-contact/multi-device,
groups/channels, M1.2.63 и отложенная A2 работа. Guard/import component не выдавать
за готовность живой миграции; trusted DTO/static signature не выдавать за native MAC
или действующее разрешение пользователя на network action.

## Проверки и доказательства

Matrix §6 обязательна, включая S07/A01–A04/D01–D09 из исправлений. Использовать
перечисленные bounded fixtures и различать actual tests, vectors, source rows и native
calls. Counts в плане — ожидаемые, не заранее успешные результаты.

- N01:640 A Text без B Text, реальные genesis, >512 chain, reload после256/512.
- N02:320/320 Text,20 DH cycles и точный late-tail schedule с reload.
- N03:generated L1 8+8 Text, проверенное происхождение и pure conversion, затем1+1
  Event2; без profile migration/новой session.
- Pure near-limit/phase/provenance vectors без huge native states; preflight errors
  до native, post-trial errors с discard, source/receipt/authority negatives,
  atomicity/failure/retry/bypass и cross-C isolation.

Проверить конкретные root/Native/Seen/source/projection/receipt/Auth/credits outcomes,
exact durable bytes после reopen, caps и отсутствие растущего seen-set. Отдельно
сверить storage structural reader с Codec2 по37 подходящим comparisons; Frame codec
в state ради счётчика не добавлять.

Windows: Cargo только compile `--no-run`; запуск harnesses исключительно через
`scripts/run-cargo-tests-stable.ps1` с постоянными EXE paths. Jobs12/BelowNormal,
без Fast, без одновременных stable runners, перезаписывающих одинаковые EXE/targets.
Clippy — через invoke-cargo-friendly.ps1, locked/offline где применимо. Fmt/whitespace
и relevant existing Codec1/Guard1/S1/S2 targeted regressions по изменённым seams.
Не повторять завершённые heavy100k/legacy route suites без нового конкретного основания.
Component ceiling10min и профиль по принятому плану: при проблеме показать измерение
и bottleneck root, не сокращать vectors/counts и не менять limits молча.

S2 performance figures не переносятся автоматически на новый bound2 reader. Его actual
component timing показать отдельно; в этом slice не заявлять UI latency или whole-A3
640 Text каждой стороны/offline256/two-process acceptance.

Все новые DB/fixtures — только изолированные временные данные. Сохранить L1 baseline
EXE и frozen S1/S2 fixtures/evidence, не открывать их для новых component runs.
development-environment.md остаётся clean SHA256
`53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f`; старую чужую правку
не восстанавливать. Сохранять новые unrelated changes, если они появятся.

## Сдача

Подготовить `long-lived-dialog-a3-protocol-s3-implementation.md` и отдельную evidence
директорию: baseline, фактический file diff, source/EXE hashes, exact commands/counts/
timings/outcomes, source-backed решения и оставшиеся ограничения. Failed runs сохранять,
не заменять поздним PASS. Сдавать законченную реализацию component scope на root review.

Один содержательный callback в root разрешён прямым userMessage
`01a16faf-e3a0-76a3-912a-1249ed4ca1e5` в самом чате исполнителя: результаты, проблемы
и уточнения по Kilogram до явного отзыва. Пользователь также разрешил межчатовую работу
в root message `01a16fb2-f93a-7ba3-8d21-bb74deacec7c`. Последняя публикация уже успешно
передала callback. Root model/thinking не менять. Если новое решение auto-review
блокирует конкретное действие, сообщить точную причину и сохранить готовый результат;
не скрывать отказ и не обходить его.

До явной root acceptance **commit/push не выполнять**. После сдачи остановиться на
review boundary; root проверит implementation/evidence и при необходимости даст repair.
