# S5.4 block2 revision02 — принятое исправление P4 scheduling R1/R2

2026-11-01. Baseline `8bac8da7984ab12026e9b2aeab4bcae9a90c02cc`. Реализация [принятого addendum01](long-lived-dialog-a3-migration-s5-final-block2-p4-addendum-01.md) и [обязательных деталей root review](long-lived-dialog-a3-migration-s5-final-block2-p4-addendum-01-root-review.md). Этот отчёт заменяет scheduling assumption P4 и final-source checks [первой сдачи block2](long-lived-dialog-a3-migration-s5-final-block2-implementation.md); прежний отчёт и35PASS остаются историческими доказательствами своих прежних bytes. [Root review01](long-lived-dialog-a3-migration-s5-final-block2-root-review-01.md) верно установил отсутствие пути repair при старом blocked-worker порядке. Самостоятельный repair неизменённым worker здесь не утверждается.

## Реальный путь и one-shot lifetime

Добавлены `runtime7/verification/retry.rs` и `retry_tests.rs`. Existing S54 control получил optional nested `retry_live { incarnation, lease_generation, from }`, strict deny_unknown_fields. Admission остаётся debug-only, outside-state pinned/atomic file. Поле разрешено только Mixed/lane1, canonical nonzero nonce, generation>0, ordinal1..511; suspend_live/hold того же ordinal несовместимы. Frozen OD1 schema и обычные protocol/storage/crypto/permission/quota/timer правила не менялись.

`network2::worker` хранит один bounded `Once` на lease. После настоящего Head RPC/accept_head2 и перед outgoing2 он передаёт состояние в **тот же** обычный Owner callback. Если S54 отсутствует, вызывается прежний outgoing2, без дополнительного callback, file read или копирования ID vector. Если request отсутствует, validation reader не вызывается.

Для fresh request `retry_outgoing2` проверяет current family2 binding/generation, actual B Active/context, observer nonce, `from == actual Head received == first actual undelivered debt`, `from < normal_first`, снятые OD1/S54 holds. Это только metadata selection; payload не удерживается до quota reservation. Затем вызывается обычный outgoing2 с retry first16. Он сохраняет свои credit/byte cap16, reserve32 и immutable source verification. Под **уже принадлежащим Dispatch обычным permit32** дополнительно проверяется первый действительно закодированный TaggedItem: format1, тот же verified source ID, own role1, original ordinal16, Text kind. Empty/skipped source отказывает. Остальной packet строится и подписывается тем же production кодом; ciphertext/ID/queues/frontiers не создаются hook.

Ticket содержит captured request, context, binding, original lowest-debt ID и normal cursor. Он остаётся InFlight через awaited RPC. Только после настоящего matching `check_complete2`, повторного current lease/scope/nonce gate и bounded actual-ID проверки публикуется `live_retry_complete`; затем worker помечает Ticket Completed. Это **possession**, а не receipt retirement/readability/новая Native операция. Existing committed Native/drain observations остаются единственным источником counter effects.

Для Completed повтор того же token возвращает no-op **до** closure с mutable debt/frontier checks: B16 мог уже исчезнуть из debt и Head мог продвинуться. Отсутствующий field тоже no-op, но сохраняет consumed state. Другая request на том же lease отказывает; очищение файла не re-arm. InFlight re-entry/другой binding отказывает. Новый worker на другой connection не принимает старые generation/nonce; ошибка не превращается в незаметный retry на новом lease. Состояние не сериализуется и не меняет Root7.

Путь P4 теперь явный: B outgoing worker, который не занят held Committed incoming task, получает реальный Head A16 и повторно отправляет original16 через normal Deliver. Incoming task A независим от его outgoing K/Committed wait. `receive2` принимает настоящий16, уже pending17..25 проходят обычную duplicate проверку, Owner background `drain2` делает16..24 и25 readable. A prefix остаётся suspended в течение доказательства; новый путь от него не зависит. Исходный B16 ID берётся из сохранённого B migration/source witness, а не конструируется.

## Два absolute intervals и kill/wait ownership

Первое lost-response окно10 начинается **до первоначального release K**. Marker/facts/status и оба real kill/wait входят в этот bound. `Controller.stop` теперь является явной композицией `kill_wait` и `reconcile_stop`; P4 намеренно вызывает два `kill_wait`, затем проверяет deadline и закрывает lost interval, и только потом `reconcile_both`/source::end/R1 equality. Таким образом, exclusive repository read действительно не стоит между двумя waits.

`kill_wait` сохраняет Running handle в controller до успешного OS wait, записывает actual exit outcome, затем переносит reaped handle/decoder/start-source point/actual last counter sample в bounded `pending_stops[2]`. `reconcile_both` требует оба running=None и оба pending stop=Some до первого source read. Reconciliation использует прежний actual source/outcome ledger; при parsing/source error pending outcome остаётся unresolved. Reopen запрещён до закрытия предыдущего stop. Error/cancellation Drop завершает только оставшиеся собственные handles и сохраняет реальные pending outcomes; deadline expiry не выключает cleanup.

Перед post-restart reopen rearm existing A K-before-publication и B Committed-before-publication. При K hold подготавливаются fresh locators/import, real admission, saved Q/K identity и обе local generations; K RPC ещё не начат, поскольку before_publication предшествует raw_rpc. Это bounded setup под существующим whole deadline/current lease.

Единственный early interval10 фиксируется непосредственно **до release этой K pause**. Он включает release, actual Committed retry marker/facts, B25 send и pending9, снятие B hold16, arming one-shot из actual B start nonce/current Head generation, normal retry Complete и отдельную early-readable B25 history проверку при A Selected/own_next25/тех же двух generations/Q/K. Затем в том же interval очищается token, освобождается exact Committed и наблюдается actual activation обоих без нового A encrypt. Никакого нового окна на marker/pending/Complete нет; nested waits ограничиваются тем же существующим Phase. RPC15 не менялся. A prefix освобождается после early proof/activation.

## Observation wiring и затронутые ветви

S54 decoder хранит последний actual C1 Head binding/generation и один ожидаемый retry. Запрос создаётся из декодированного start nonce и matching Head, а не expected identity. Completion сверяет PID/source owner/incarnation/sequence, expected C1/B/generation/binding/lowest source, фактический bounded unique ID list/next. Событие не меняет Operations counters или Native sequence. Actual B25 source/history и same-generation brackets проверяются отдельно. Continuation по-прежнему владеет теми же directories/handles/requests/original witnesses/cursor/ledgers; whole Phases остаётся incomplete7/12.

Относительно первой сдачи изменены runtime `network2.rs`, `protocol2.rs`, `verification.rs`, `verification/control.rs`; добавлены `verification/retry.rs`, `retry_tests.rs`; в S54 support изменены control, observations, source (только bounded original Text ID getter), transition и focused tests. Остальные P0–P3/P5–P6 algorithms и accepted oracle predicates сохранены. В общем block2 diff относительно baseline16 Rust paths. Старые S53 bodies, Runtime2 hooks, Cargo/crates/public scripts и historical plan bytes не менялись. Existing explicit S54 env removal из первой сдачи сохраняется.

## Фактические проверки revision02

Все проверки — новые immutable outputs в `.tmp/s54-block2-02/`:

| Check | Actual native exit / elapsed | Область |
| --- | --- | --- |
| clippy-01 |0 /29.3499958s| Первая реализация R1/R2, strict CLI/all-targets. |
| clippy-02 |0 /28.0995513s| Финальные sources после переноса first-payload проверки под ordinary reserve32; literal `-- -D warnings`, verification-scale/x64, locked/offline, Jobs2/BelowNormal. |
| compile-01 |0 /551.0336732s| Stable runner compile-only CLI+transport/all targets. Before/after build inputs совпали. Normal CLI test=false, opt3, debug assertions/overflow checks enabled. |
| pure-01 |0 /15.1277216s| Ровно11 focused `s54_r2_` non-network tests, 8 runtime kernel +3 controller decoder/ordering. 0 failed/ignored; frozen guard PASS,501/501 units fresh, cleanup_verified=true. |
| fmt-final |0 /4.9769126s| `cargo fmt --all -- --check`, без source changes. |
| scoped evidence review |0|16 Rust file SHA/Git blobs, unchanged baseline/index/old report/plan/addendum/S53 bodies/Cargo/crates/scripts,19 prior source/image preimages и4 fixed field EXE. |

[Review manifest](../../.tmp/s54-block2-02/review.json),22659B/SHA256 `a524f7b1c1362ffd51374a7f4f53ceb63c1806d58453bb7cc13f18851a61749d`, содержит exact16 source paths/raw bytes/SHA/Git blobs, все11 full passing test names, compiler/runtime artifact hashes, step receipts и evidence pins. Полные выводы сохранены в соседних `clippy-*.stderr.log`, `compile-01.*`, `pure-01.*`. Checks использовали одну build/staging lane и существующий finite watchdog1190/1200 compile,590/600 lint,230/240 pure; каждый native step завершён с verified cleanup. Failed steps этого цикла нет.

8 kernel tests проверяют strict nested schema/debug/Mixed/B/old bounds, absent hook без reader, completed-repeat **до** mutable reader, rearm/новый binding после completion, stale nonce/generation/owner, missing/held/nonlowest debt/Head mismatch, in-flight replacement/retry и actual first-ID/count/next bounds.3 controller tests проверяют actual decoded start+Head→captured request→non-Native Complete без counter effects, отказ unarmed/stale/wrong-source/binding/extra-counter observation и gate двух waits/causally fixed absolute interval. Это memory-only metadata; source verifier/normal data path/liveness обоснованы исходниками, не синтетической арифметикой.

Прежние35 тестов без нового основания повторно не запускались; их35PASS не объединяются с11 новыми в claimed whole-suite/runtime PASS. Исходники не изменялись между финальным lint, compile и pure run. Последняя сборка также дала normal current CLI с hook в **не test** unit; field image не обновлялся.

| Final static artifact | Bytes | SHA256 |
| --- | --- | --- |
| `target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe` |50400256|`db66bf93923d4666eea7cc968b9ccd24116e57ebc01c788d866707a004cb6cd0`|
| stable `kilogram-cli-lib-kilogram_cli-tests.exe` |32656896|`e94f2b5fc99112b6f8c03e24aee039aa41fbc7dd19be35cb8c921a78c60b6d7e`|
| stable `kilogram-cli-test-migration_s5_process-tests.exe` |16224256|`5054b3008a3495e07b224735f785183f9e54169189badb91140b6ae48da4f0e7`|

Локальные [compile](../../.tmp/s54-block2-02/compile.ps1)/[check](../../.tmp/s54-block2-02/check.ps1)/[pure](../../.tmp/s54-block2-02/pure.ps1) helpers фиксируют actual invocation. Для повторения нужны новые immutable numbered names; pure helper привязан к exact compile-01.capture.json и сверяет current inputs перед использованием. После новой сборки его нужно явно привязать к её capture, не перезаписывая старые outputs. Это воспроизводит static/pure checks, не запускает product P4.

Предыдущие12 source files и7 compiler/stable EXE bytes сохранены **до изменений** в `.tmp/s54-block2-02/preimages/`, pinned [before.json](../../.tmp/s54-block2-02/before.json). Старые35PASS, их logs/captures, root reviews, accepted addendum и первоначальная provenance failure не переписывались. Fixed field EXEs не staging/overwrite.

Runtime границы сохраняются: не запускались enrollment/product/P0–P6/F/P и old private fixtures. Source-supported path и pure token/decode/ordering tests не доказывают TLS/Owner liveness либо fit в10s. Это остаётся последующим явно разрешаемым measurement после source review, не административным поводом расширить RPC timers. Commit/push и block3 не выполнялись. После одной сдачи root — остановка без polling.
