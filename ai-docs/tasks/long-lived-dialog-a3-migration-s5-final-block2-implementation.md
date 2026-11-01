# S5.4 block2 — continuous P0–P6 controller (source/static/pure)

2026-11-01. Работа по [block2 task](long-lived-dialog-a3-migration-s5-final-block2-task-01.md), baseline `8bac8da7984ab12026e9b2aeab4bcae9a90c02cc`. Принятый [plan](long-lived-dialog-a3-migration-s5-final-plan.md) не изменён. Это реализация ранней половины будущего **одного** continuous run, не executed P0–P6 и не whole P5 PASS. Нового executable test/partial selector нет; enrollment, account/state fixtures, runtime/process/F/P не запускались.

## Файлы и границы

Новые child modules под опубликованным `migration_s54_p5.rs`:

- `migration_s54_control.rs`: per-node OD1/S54 controls, child ownership, fresh locator lifecycle, deadline propagation, actual status/counter samples и stopped-source closure.
- `migration_s54_observations.rs`: bounded incremental parser настоящего stderr `s54_observation`, PID/source/incarnation/sequence/context validation, existing Operations/Pages ledger.
- `migration_s54_source.rs`: exclusive bounded witnesses **только новых S54 origins**; original encrypted Native/Head/Spec/queue/source binding, in-memory secret equality, actual old/current overlap и R1/B640 associations. Нет копирования state/Native branch.
- `migration_s54_early.rs`: P0–P2, actual HVI1 cursor, P5/P6, owned continuation.
- `migration_s54_transition.rs`: P3/P4, pre-Q R1, exact saved controls, lost Committed/both restart, current family2 brackets, pending→readable B25 при Selected A.
- `migration_s54_block2_tests.rs`: новые несетевые decoder/control/cursor/ledger/lease negatives.

Существующие S53 controller bodies не изменены. В новом block1 oracle открыта видимость типов/методов только ближайшему parent S54 module; арифметика/предикаты не заменены. `tests/migration_s5_process.rs::command` дополнительно удаляет inherited `KILOGRAM_TEST_S54_SCHEDULE`: все старые CLI/runtime helper paths продолжают default-off поведение, S54 attachment выполняется явно только новым controller. Другие test env удаляются прежним helper. Глобальные environment variables контроллер не меняет.

Три узких Runtime7 дополнения к block1: `verification.rs` формирует безопасные `validated_head` и `saved_pending` envelopes; `protocol2::accept_head2` наблюдает только уже проверенный Head/current lease; `data2::receive2` наблюдает только успешный actual Pending commit после production validation. Это нужно, поскольку normal CLI не содержит прежнего cfg(test) worker Head counter, а один pending-count не связывает B25 с его actual EventId. Без S54 control это быстрый false branch, без IO/новых owner callbacks; wire/storage/writer/crypto/permission/timer не менялись. Native sequence не увеличивается для этих non-Native events.

## Phases и действительные preconditions

| Phase | Source entry | Preconditions и сохраняемое свидетельство |
| --- | --- | --- |
| P0 | `Controller::p0` | `OldOrigin::create` выполняет frozen enrollment и обе context ceremonies на исходных двух state dirs. OD1 controls прикреплены до frozen start; S54 fields туда не передаются. Оба C достигают настоящих Ready/genesis без Text. После kill/wait берутся original V4 witnesses, actual signed Spec/device roles обеих C. Затем те же owners рестартуют;8 causal A2/B2 rounds, B16 committed/held, A readable15, B knows A16. |
| P1 | `Controller::p1/migrate` | B stopped+waited, у A наблюдён настоящий ConnectionBackoff. Exact requests A17/18 приняты при unchanged send_next17. A stopped; sealed queue bodies/IDs и bytes читаются с собственного V4 source, `--check` source_id сопоставлен, тот же state dir проходит `--resume`/Ready/zero_migration_native_calls. Spec/Head/opaque Native/secrets/queue/all readable sources сравниваются в памяти до первого current start. Materialization suspended до первого indexed history: ровно прежние31 Text. |
| P2 | `Controller::p2`, `Frozen` | Тот же frozen B restart; actual ALPN1 locators обоих C, original A17/18 materialize once, затем A19–24/B17–24. B16 остаётся held, prefix A suspended, A readable15/pending8. До A24 current A ACK-query disabled и frozen B ACK-push disabled, поэтому actual debt A24 остаётся1. History39 читается через API: first16 дают настоящий cursor, им реально читаются оставшиеся23, сохранены F/view/порядок/IDs/body. |
| P3 | `Controller::p3` | Оба владельца waited. B migration со своим исходным V4 witness. Saved R1(A24) у B проверяется по действительному подписанному receipt/source и тому же ненулевому A debt. Оба current, real Consent2, fresh Locator2/import и explicit intents. Actual P held outside Owner позволяет независимому B worker выполнить pre-Q Head/ACK. До Q наблюдаются Head обоих владельцев и debt A24→0; затем P released/K held, Selected/Prepared, cuts25/25. В этом свежем incarnation bracket observed Native counters0. Original C/D/E/SID/Native keys сравнены с enrollment witnesses. |
| P4 | `Controller::p4` | Release K даёт real B Committed-before-publication, A Selected/B Active. Conversion-only counters0 проверяются отдельно после B conversion. Оба killed/waited; принятый R1 byte hash проверяется против исходного saved R1. Committed rearmed **до** reopen, fresh authenticated locators, тот же Q/K/Committed signed fingerprint. B25 отправлен original request/body, observed actual pending EventId позади old16–24. Release B16/prefix разрешает настоящий repair; B25 становится readable при A Selected, неизменных current family2 generations, Q/K и A own_next25. Только после этого release Committed и оба Active. |
| P5 | `Controller::p5` |16 causal A2/B32 rounds; каждое противоположное чтение дожидается фактического frontier. Первый B range25–56 содержит retry original B25, его EventId сравнивается после rounds. Конец A56/B536 и actual receiving guard peer_g>=16 у обоих. Fixed sleep используется только между bounded polls, не как свидетельство readiness. |
| P6 | `Controller::p6` | A57–384/B537–640, batching в существующих queue/window bounds. До B640 включены S54 direct ordinal640 hold и A prefix suspension. B640 real source/request/debt1 подтверждены после B kill/wait; сохранены source/ciphertext digest и точная ожидаемая receipt association, без fabricated receipt. Тот же B restarts, fresh Locator2 imported. C2 по-прежнему L1, next1/1, queued/pending/debt0. Real frozen cursor снова читается через API. |

Saved controls publication hooks остаются существующими S53 hooks вне Owner; контроллер использует новые request ranges и original witnesses вместо вызова старого PREQ fixture. P4 early interval ограничен10s существующей absolute Phase и не продлевает RPC15. Исходный запрет нового A packet сохраняется через actual Selected/own_next25 brackets и штатные production gates.

## Ownership, observations и continuation

`Controller.running` содержит ровно два Option с реальными OwnedChild/PID/log/Decoder/start-source point. `Pair.children` не используется для второго writer. Перед migration/repository reads вызывается `stopped(owner)`; stop берёт actual last status, kills/waits known child, сразу сохраняет actual exit outcome, дочитывает bounded log и сравнивает actual counters со stopped source/readable deltas. До нового start прежний incarnation ledger закрыт. Старые public locator outputs удаляются только у stopped owner, затем ожидаются новые публикации; endpoint старого процесса не используется как доказательство fresh rebind.

До enrollment проверяется effective `TOKIO_WORKER_THREADS`/available parallelism>=2: существующий synchronous publication checkpoint не должен занимать единственный Tokio worker. Невалидная/недостаточная настройка отказывает до создания origin и child; настройка пользователя не переписывается.

Failure/cancellation Drop завершает только известные собственные handles, ждёт их и сохраняет bounded cleanup outcomes без PASS. Неразрешённый counter outcome при error остаётся unresolved; файл `owner-cleanup.json` не является checkpoint и не позволяет продолжить на другой ветке. Это обычный failure evidence, не новый runner framework. Используются существующие absolute `s53_deadline::Phase/check/output`; вызывающий будущий whole controller обязан заранее владеть managed Guard, отдельный budget не создаётся.

Decoder читает log с monotonic byte offset, bounded fragment/line/total/event count. Начальный zero-counter start обязателен, owner — actual repository source_id, PID — actual child handle, nonce уникален через Operations. Native commits несут actual operation/generation/8-field counters. Start/final/resolved обработаны явно. Non-Native Head/page/pending не создают Native effects; page notices могут прийти после более нового Native callback, поэтому не заполняют sequence gap. Killed хвост закрывается только actual observed resolved counter sample и stopped-source reconciliation. При неполной строке, counters/source mismatch или отсутствии real outcome контроллер отказывает; успехи/attempts из expected workload не достраиваются.

71/95/104 остаются отдельными old/genesis subtotals,106 учитывает отдельные2 операции B25. В P0/P2 сначала записаны expected annotations; actual subtotals вычисляются по sources при следующих exclusive cuts:71 из обоих original source witnesses до materialization17/18,95 из actual A readable frontiers/genesis и stopped B sources,104 из actual A format1 history/genesis и B original sources. Эти observed значения сравниваются с нормативом, не подставляются вместо current counters. Overlap17 отдельно привязан к sources: actual current-generated old A17–24 читаются из stopped A repository с signature/request/projection проверкой; actual late B16–24 у A сравниваются с original verified B V4 source witnesses. Эти данные и ledger сохраняются для финального whole reconciliation block3, а не суммируются с expected2696 сейчас.

`p0_to_p6(parent, until)` возвращает **owned `Continuation { controller, held_b640 }`** при next phase7/12 и explicitly incomplete Phases. Передаются те же два state dirs/administration roots, process handles/PIDs/start counts,2 contexts и actual Ready slot mappings, original enrollment/migration witnesses, request map/body function/original B25, safe phase/control facts, настоящий frozen cursor/view/F/suffix, Operations/Pages/incarnations/outcomes/source overlap17 и absolute until. Ничего не сериализуется для подмены original lineage. Block3 должен продолжить этим объектом с Offline, а не заново вызвать origin или склеить два successful prefix/suffix.

## Фактические static/pure проверки

Все финальные sources проверены:

| Check | Actual exit / elapsed | Result |
| --- | --- | --- |
| clippy-06 |0 /8.9840821s| CLI/all-targets, verification-scale/x64, literal `-- -D warnings`, locked/offline, Jobs2/BelowNormal. |
| compile-02 |0 /83.7203329s| Stable runner compile-only CLI+transport;501 units; normal current CLI test=false, opt3, debug assertions и overflow checks enabled. Before/after build inputs совпали. |
| pure-01 |0 /15.2565431s| Exact35 `s54_` non-network tests:13 новых block2 +12 Runtime7 hook +10 oracle regression. 0 failed/ignored, frozen guard PASS,501/501 units fresh, verified child cleanup. |
| fmt-final |0 /4.5074294s| `cargo fmt --all -- --check`, без вывода и source changes. |
| source/evidence review |0| Exact12 Rust paths/raw SHA/Git blobs, unchanged S53 controller/plan, empty index/исходный HEAD, scoped whitespace,4 fixed field images и7 сохранённых block1 preimages. |

[Review manifest](../../.tmp/s54-block2-01/review.json),28299B/SHA256 `76978d655bb00ad6972b28bc5c7f3516ab7f8af4f7670eae11cee194eb4d3662`, содержит exact35 passing full test names, source tuples, step receipts и hashes локальных logs/helpers. Exact13 новых names находятся под `live_l1::handoff::final_p5::block2_tests::` и проверяют complete start/commit/pending/Head/final decoding; PID/owner/nonce/C/kind/sequence/regression failures; overlap/unresolved final; stopped-source kill reconciliation; incremental fragments/truncation; per-child env/schema isolation; реальное повторное чтение cursor и changed order/view/F/source; current lease/mode/role brackets; actual Stats fields/missing attempts; request alias/duplicate source; worker prerequisite. Ни один из этих тестов не вызывает enrollment/runtime/`p0_to_p6`/V4 fixture reader.

| Final static artifact | Bytes | SHA256 |
| --- | --- | --- |
| `target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe` |50038272|`a42d8c56d900be3d23840d422b6606fbfaa0b57ed3918c8dd6efd4ff7bb35349`|
| stable `kilogram-cli-test-migration_s5_process-tests.exe` |16194560|`4ac5f37dfcbfdca9ce2dbb195f26b75488b6aff100716d53d0f2f846b4ff5ac1`|
| stable `kilogram-cli-lib-kilogram_cli-tests.exe` |32473600|`29950b2e254ffb54c8e4a4290ae05e7e5e16bdb57105a24f7235c89aadd5778d`|

Все исходы сохранены в [.tmp/s54-block2-01](../../.tmp/s54-block2-01/). Промежуточные failures: clippy-01 exit101/33.3689465s (shadowing `Context`, Option/JSON comparison, unused import), clippy-02 exit101/8.5404128s (unwrap/expect/needless borrow); исправлены без ослабления lint. Промежуточные clippy03/04/05 прошли0 за9.021531/8.7989616/8.6936867s. Compile-01 Cargo закончил compilation, но wrapper вернул1 за552.7986942s: исполнитель изменил controller helpers до завершения сборки, поэтому **Frozen build provenance mismatch before executable use** корректно отклонил результат. Этот результат не использовался как final evidence и tests им не запускались; immutable outputs сохранены. После завершения compile-01 источники были окончательно исправлены/отформатированы; clippy-06, compile-02 и pure-01 относятся к одинаковым final bytes. Все native helper steps имели finite watchdogs и cleanup_verified=true, build/staging lane одна; product/process/F/P не исполнялись.

Для повторения static/pure проверок использовать локальные [compile](../../.tmp/s54-block2-01/compile.ps1), [check](../../.tmp/s54-block2-01/check.ps1), [pure](../../.tmp/s54-block2-01/pure.ps1) helpers с новыми immutable numbered step names. Pure helper привязан к compile-02.capture.json и дополнительно проверяет текущие inputs; при новой сборке нужно привязать его к её exact capture. Это воспроизводит только compile/pure proof, не активирует continuous process run.

Не выполнялись и не объявляются PASS: любые реальные P0–P6 phases, old enrollment/Native open на fixture, simultaneous owners/TLS/ACK race,10s early timing/receiving DH slope и feasibility fit. Имеющиеся helper bodies и новая wiring проверяются source/compile/pure; их runtime reachability/порядок требует последующей разрешённой F measurement после block3 review. Fixed field EXEs не обновлялись; actual old compiler/stable bytes перед подготовкой сохранены в `.tmp/s54-block2-01/preimages/`.

Block3 P7–P11/F510/P2400 не активированы. Commit/push этого блока не выполнялись. Три unrelated navigation файла и historical evidence сохранены; сдача заканчивается одним root callback и остановкой.
