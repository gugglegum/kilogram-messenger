# S5.3 r31 — матрица требований и фактических доказательств

2026-10-29. Documentation-only audit по [поручению r31][R31].
**S5.3 ещё не завершён: 12/12 control cuts приняты, но остаются отдельные обязательные проверки.**
Эта матрица не активирует тесты и не является приёмкой или publication candidate.

Нормативная база: [исходная задача S5.3][TASK], [принятый S5 contract][CONTRACT],
[handoff mapping][PLAN] и [принятая revision1][REV1]. Объём whole resumed-L1 P5
уже выделен в [S5.4][S54]; перечисленные ниже S5.3 gaps туда не переносятся.

## Как читать scope и повторное использование

`PROVED(scope)` означает только принятый фактический результат в названном scope.
`PARTIAL/GAP` означает незакрытое обязательство, а не установленный дефект продукта.
Каталог selector/лимитов в binding не доказывает выполнение.

Обозначения реализации (пути от корня репозитория):

- **ADM**: [runtime7/admission2.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/admission2.rs), `fresh_pair`, `current_existing`, `begin2/finish2`, `check_existing_lease`, Locator2; [owner.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/owner.rs), account pins.
- **CTL**: [runtime7/control.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/control.rs), `import_consent/request_upgrade/cancel`, `propose/prepare/select/commit_b/activate`, `converted/validate_saved`; [state existing.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/existing.rs), `ExistingOperation7/commit_existing`.
- **NET**: [runtime7/runtime.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/runtime.rs), actor/mailbox16, IPC3, global permits2, dual ALPN dispatch; [network2.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/network2.rs), fresh admission/worker/publication; [scheduler.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/control/scheduler.rs).
- **PUB**: [existing_admin.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/existing_admin.rs), offline ceremony; [ordered2_cli.rs](../../apps/kilogram-cli/src/ordered2_cli.rs) и [ordered3_cli.rs](../../apps/kilogram-cli/src/ordered3_cli.rs), явные CLI/IPC operations; [wire existing.rs](../../crates/kilogram-protocol/src/ordered_dialog_v2/existing.rs).
- **DATA**: [runtime7/data2.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/data2.rs), `receive2/materialize2/drain2/accept_receipt2`; [protocol2.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/protocol2.rs); [state live1/mixed.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/mixed.rs), closed mixed writer.
- **HIST/BUDGET**: [live1/history.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/history.rs), [read.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/read.rs), [data.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/data.rs), [credits.rs](../../crates/kilogram-state/src/vault/indexed_dialog/credits.rs), [records.rs](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs).
- **PROCESS**: [migration_s53_handoff.rs](../../apps/kilogram-cli/tests/support/migration_s53_handoff.rs), controls:516, discovery:6248, pre-Q:6502, two-C:7160, exhausted:7379; [capacity controller](../../apps/kilogram-cli/tests/support/migration_s53_capacity_review2.rs):4. Это source anchors, не свидетельства запуска.

Source-impact аргументы:

1. **R30-preserved.** Сейчас проверены raw SHA256 всех 114 dirty files под apps/crates/scripts по первым 841 canonical rows binding r30: unbound0/mismatch0. Девять current images совпадают по bytes/SHA256. Controller344341B/`68fea9a6b25acfcd4960609271343e858eb81a58fc45d79230bd06cb5a9d5564`; process owner9997824B/`86d813f98e2840c358915c314bff033cbedbba1eb739adfc1e53b2d7a5d5730d`. Это целевая проверка, не новый обход тысяч исторических артефактов.
2. **R14-reuse.** Сравнение записанных `build_inputs.source_inputs` (487 rows) r14 с r30 показывает единственное изменение — PROCESS controller. Production Rust, component controllers и пять из шести scale images неизменны. То же верно для просмотренных r15/r16(01,03)/r18(03)/r23/r29 manifests. Это обосновывает сохранение приёмок компонентов и production путей; процессные старые сценарии остаются запусками своих старых controller/image.
3. **Earlier-reuse.** r12→r30: изменены только integration-test source и PROCESS controller; r11→r30 дополнительно regression-test source. r8→r30 также изменён production `data2.rs` и добавлен Seen-test source. Поэтому admission r8 не объявляется полным текущим rerun: более поздние принятые FULL/MIX/DISC/PREQ реально используют admission и исправленный data path. Исторические r3/r4 bindings отличаются ещё `network2.rs`; их route/runner results сохраняются только в исходном scope. Ни один старый receipt не переписывается.
4. **Controls-reuse.** r19–r30 меняли контроллеры отдельных окон, диагностику и доказательства остановок. Root каждый раз принимал source-impact/preservation соответствующей ветки; последняя сводная приёмка [R31] принимает 12 cuts в их respective scopes. Общее production сохранено с r14, но двенадцать окон не запускались заново на process owner r30. **Current55/55 на r30 не заявляется.**

## Матрица (16 требований)

| № / нормативное обязательство | Реализация | Принятый фактический результат и граница | Source/image и reuse | Статус |
|---|---|---|---|---|
| 01. Existing Consent2 по public export/offline Root-admin; per-C explicit intent, idempotency/cancel только до P; IPC3 tags7–10 (§8, TASK1) | PUB, CTL; NET IPC dispatch | [POL acceptance][POL-ACCEPT]: own-Root ceremony, Consent CAS/expiry, direct IPC malformed variants7 без writes. [DISC][DISC-ACCEPT], [PREQ][PREQ-ACCEPT] и control cuts используют реальные CLI children/IPC и original intent IDs. Direct handler negatives не названы OS IPC-server negative tests. | POL r14; DISC r15; PREQ r16/03; cuts ниже. R14-reuse/R30-preserved. Исторические [A/B/sim controls][CONTROLS] подтверждают их собственный control scope. | PROVED(scope); public положительный путь + scoped negatives. |
| 02. ALPN2 existing admission до Q, текущие cert/Consent/role/Locator/exporter pins; no fake selection; ALPN1 закрыт с durable P/Prepared (§7.1–7.1.1, M06) | ADM, NET, `known/shape/fresh_pair`, CTL | [AD][AD-ACCEPT]: genuine TLS2/exporter/family2 lease при L1 без selection. DISC оставляет L1 без intent. PREQ и cuts проверяют fresh reconnect и отсутствие ALPN1 fallback после fence. | AD r8 historical; поздние FULL/MIX/PREQ используют final production path. R8 data2 delta учтён, admission2 сохранён. | PROVED в указанных admission/reconnect scopes; cross-C pin invalidation относится к 10. |
| 03. Asymmetric pre-Q и Selected/Prepared; A Selected receive-only при B Active, exact K retry, hint не даёт Q/Native/debt release (§7.1.1, M06) | ADM `fresh_pair/live_view`, CTL `peer_hint/select`, NET, DATA | [MIX][MIX-ACCEPT]: opaque Prepared hint/early Selected receive. [PREQ][PREQ-ACCEPT]: реальные Selected/Prepared, затем B Active/A Selected, оригинальный request104 format2/n3 читается и delivered за1.7113285s; same current family2 generations до/после IO. | MIX r14 component; PREQ r16/03 process. R14-reuse; старый PREQ controller/image остаётся старым. | PROVED(scope). |
| 04. P/Prepare/K/CommitB/ActivateA durable exact bytes, freeze без rematerialization/downgrade; 4×3 loss boundaries (§7.2, M06) | CTL + state commit, NET before-publication; PROCESS | Двенадцать принятых process-stop/reopen cuts приведены ниже. Последний [r30 receipt][CUT30] и [independent verification][CUT30-VERIFY]:7 incarnations/26 lifecycle events/5 cold observations, isolated B Active5/A Selected4, null Committed у A, saved P/A/K/Q согласованы, conversion once, original400/401 once in both histories. | Свои r19–r30 bindings/owner pins в таблице; Controls-reuse. Ни power-loss всех типов, ни current-image sweep не доказаны. | PROVED(scope), 12/12 accepted controlled cuts. |
| 05. Фоновый discovery без неявного intent/upgrade и ordinary starvation; scheduler5/60/45/15/30/90 (§7) | NET coordinator, scheduler, CTL notifications | [DISC][DISC-ACCEPT]: ordinary #1 ACKed, first family2 opportunity возвращается в L1/NoLocalIntent без Q/cuts/freeze/OpenIntent/Propose, ordinary #2 delivered/ACKed; starts spaced60.075s. Второй start виден, второй completed opportunity не утверждается. | r15 controller; production scheduler/network2 сохранены r14→r30. Это single-C observation, не доказательство concurrent fairness. | PROVED(scope); multi-C ресурсы отдельно 10. |
| 06. Genuine old gap/pending/head/Prefix/Resume/Receipt1; no debt release от possession/hint; bounded pending64+1; Seen и cold continuation (M03/M06, TASK4) | DATA, HIST/BUDGET, CTL cut verification | [SEEN corrected receipt][SEEN] / [acceptance][SEEN-ACCEPT]: реальный Owner7, eleven subcases, write-fault retry/cold continuation/quarantine. [FULL][FULL-ACCEPT]:131 offline encryptions, concurrent append, R4 конфликт rejected без Native mutation, cold pending64+1/drain65/ACK65. PREQ: genuine held A2/A3/B2 gap, late old repair при Selected/Prepared. | Seen r11, FULL r12, PREQ r16/03; Earlier-reuse. FULL намеренно оставляет67/132 source debts: не «все sources доставлены». Receipt2(format1) в PREQ выведен из reviewed paths+history, не raw-header read. | PROVED перечисленных старых debt/gap/receipt cases. |
| 07. Atomic Mixed7 conversion once, same keys/D/E/SID, no account/session constructors/Packet calls на handoff; настоящий Native до/после (M01/M06, §7.3) | CTL `converted`, [native.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime7/native.rs), DATA/state mixed | MIX: same lineage, constructors0, conversion packet calls0, genuine4 encrypt/6 decrypt после; final Active обе стороны. FULL: реальная Native1/conversion/Native2 работа. Cuts показывают once-only conversion через stop/reopen. | r12/r14 components + individual process images; R14-reuse. Более поздние encryption/decryption законно меняют Native bytes; они не сравниваются как frozen conversion boundary. | PROVED(scope), exhausted lineage отдельно13. |
| 08. Current policy/final gates, renewal не переписывает historical selected proof; archive/cold reopen (M07/M11/M12) | ADM/CTL final gates, identity `account.rs`, closed state writer | POL: rollback/floor/wrong role/context refusal, expired/CAS refusal, current renewal3days при selected historical3600 и clock+7200; Q/controls/Native/escrow после reopen сохранены. Controls подтверждают own final-gate negatives. Реальный cross-context revoke-after-staging всё ещё требует10. | POL r14 без TLS/процессов; production сохранён. Component policy нельзя назвать живым revoke/relay/OS IPC test. | PROVED(component); PARTIAL относительно concurrent final gates. |
| 09. Per-C paid credits/escrow, exact old ACK retirement once, независимость holds, capacity CAS/publication recovery (§6.1–6.2, M08) | HIST/BUDGET, state mixed, [storage_migration.rs](../../apps/kilogram-cli/src/ordered_dialog/storage_migration.rs); capacity controller | FULL/MIX доказывают реальные списания/ACK в своих cases; MIX repair_free67108864→67103503. [META][META-ACCEPT] — Windows runner baseline/error/timeout/cleanup classification, **не capacity recovery**. `F-s53-capacity-recovery-r2-01` не имеет найденного actual receipt/acceptance. | Исторический component scope не покрывает два настоящих capacity/profile interruption windows с validated relay route. Для них нужны reviewed controller, current CLI+relay pins и новая activation. | PARTIAL/GAP G2 — live capacity/route recovery и cross-C holds. |
| 10. C1 pin change/revoke/blocked transition не останавливает разрешённый C2; independent leases, global permits2 до TLS, mailbox16/batch4, refs128=64/64 (TASK7/8, §6.1/7.1) | Owner7 pins/resources, NET permits/coordinator; PROCESS two-C | Изолированные POL/controls не доказывают работу другого C. `F-s53-two-c-routes-integrated-01` есть в source/catalogue, но actual receipt/acceptance не найдены. | Существующий трехузловой controller не запускался по найденным публичным доказательствам. Наличие compiled selector/current nine images не доказывает capacity/relay child applicability. | GAP G3 — concurrent C/resources/account-pin effects. |
| 11. Concurrent C/protocol routing: DirectOnly пока есть L1, RelayOnly лишь all Active, explicit own relay/Auto и restart persistence (TASK8, §7.1) | ADM `configure_route/publish_locator2`, NET, Profile3/capacity overlay | [ROUTE][ROUTE-ACCEPT] принят r3 как route/bind table; POL проверяет in-memory policy. Ни один из них не показывает живой relay или persistent routing. Two-C и capacity process scopes остаются без actual приёмки. | r3 binding/code/image существенно старше r30 (network2/data2 deltas). Сохраняется table result, не current runtime relay PASS. | PARTIAL/GAP G2/G3. |
| 12. Единый readable HVI1 old/new, original IDs/requests/history и frozen pre-Q cursor suffix после append/restart (§8, M10) | HIST/BUDGET, DATA, NET history IPC3 | MIX: old frozen suffix после cold reopen; PREQ: original request103 once format2/n4, original104, same history/cursor suffix до/после реального restart; cuts: exact original old/new histories. | Component r14/process r16 и cuts; R14-reuse. Малый реальный suffix не заменяет exhausted512 suffix или fresh escaped-byte regression. | PROVED(scope); длинная граница13 и shared fresh14 остаются. |
| 13. **Тот самый genuine stopped exhausted-L1 checkpoint: cutA512/B18 → первый Event2 A.n512/B.n18**, original lineage/source/request/receipt/history без replay/regeneration (M02, TASK minimum, PLAN§7) | PROCESS `s53_exhausted_exact_path_public`:7379–7534; public [old controller](../../apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs) | [Checkpoint-info][EX-INFO] явно `no_handoff_or_Event2=true`, next[512,18], receipts[511,17], queues/debt/pending0. [Preservation][EX-PRESERVE] — сохранение исходника, не handoff. [S5.2 acceptance][S52] прямо исключает first Event2. Нового S5.3 actual receipt и approved public prerequisite manifest не найдено. | Existing inert selector требует `KILOGRAM_S53_EXHAUSTED_MANIFEST`, exact paths, source/copy/root-decision hashes, current own Roots и single active Native branch. Их текущая доступность здесь не проверялась. Source loop проверяет conversion/next, но отдельный pre-Active UpgradeRequired из PLAN§7 явно не вызывается: нужен contract review, не вывод о product bug. | **GAP G1. S5.3, не S5.4.** |
| 14. Регрессии затронутого fresh пути: реальный fresh offline/restart/readable + IPC history escaping/byte budget/no cursor skip (TASK minimum, accepted plan) | [runtime2_process.rs](../../apps/kilogram-cli/tests/runtime2_process.rs), existing S4 helpers; changed shared runner/transport/history | `F-s53-fresh-regression-01` → `a3_s4_process_smoke_fresh_offline_restart_readable`; `F-s53-fresh-history-regression-01` → `a3_s4_component_ipc_history_escaped_byte_budget_no_cursor_skip`. Найдены только inactive catalogue/план, actual S5.3 results не найдены. | Исторический S4 acceptance сохраняется для исходного diff. FRESH fixed runtime CLI/relay/shell dependencies требуют отдельной applicability, а не подмены normal CLI из r30 graph. | GAP G4 — два focused regressions; whole S4/P не запрашивается. |
| 15. Required static/frozen/source correspondence, честная классификация runtime; stable Windows harness (§10/11, task rules) | [stable runner](../../scripts/run-cargo-tests-stable.ps1), frozen-artifacts, S53 metadata/adapters | [Static acceptance][STATIC-ACCEPT]: fmt/strict CLI all-target Clippy приняты отдельно при whole native static preparation **FAIL**. [Compile acceptance][COMPILE-ACCEPT]: actual501 units,386 runtime/115 host, sole rebuilt process owner, preserved8 images. [R31]/CUT30 подтверждают текущий runtime binding. | R30-preserved. Root принял successful static components после независимой preservation, не переименовывал native failure. Historical C route/metadata и r29 inventory repairs остаются scoped supporting proofs. В r31 ничего не запускалось. | PROVED(scope) static/preparation + принятый последний runtime; не whole S5.3. |
| 16. Whole resumed-L1 P5 / длинный совместный operational trace (§10.3) | Будущий S5.4 plan/runtime | [Отдельная задача S5.4][S54] уже определяет scope и gate: принятый и опубликованный S5.3 baseline. Сейчас такого publication SHA ещё нет. | Это существующее разделение этапов, не новое исключение exhausted/capacity/two-C/fresh из S5.3. | ASSIGNED ELSEWHERE — S5.4, не started. |

## Двенадцать control cuts: фактические accepted receipts

Все ссылки — root actual-return proofs, а не только assertion envelopes.
В каждой строке `whole_exit=outer_exit=0`. Whole seconds включают учтённые root
receipt intervals; они не суммируются и не начисляются повторно этим аудитом.
Имя manifest ниже в том же evidence directory. Owner SHA приведён сокращённо;
полный SHA и controller/source inventory закреплены соответствующим manifest/binding.

| Cut | Accepted attempt / actual proof | Whole s | Manifest | Process owner SHA prefix |
|---|---|---:|---|---|
| P before commit | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-p-before-commit-01-attempt-01.root-actual-return.json) |115.707635|r19-01|053d035da725|
| P after commit | [03](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-p-after-commit-01-attempt-03.root-actual-return.json) |85.9976411|r19-01|053d035da725|
| P before publication | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-p-before-publication-01-attempt-01.root-actual-return.json) |86.0878404|r20-01|ceea70edf2e8|
| Prepare before commit | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-a-before-commit-01-attempt-01.root-actual-return.json) |102.8475394|r24-02|48f0b6dc8e87|
| Prepare after commit | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-a-after-commit-01-attempt-01.root-actual-return.json) |93.2874201|r21-01|2ffa5544cf32|
| Prepare before publication | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-a-before-publication-01-attempt-01.root-actual-return.json) |86.1504473|r25-01|70c8d479b493|
| K before commit | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-k-before-commit-01-attempt-01.root-actual-return.json) |89.464141|r26-01|225cc0bc1098|
| K after commit | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-k-after-commit-01-attempt-01.root-actual-return.json) |85.1199479|r22-02|0ce40d46d4ef|
| K before publication | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-k-before-publication-01-attempt-01.root-actual-return.json) |91.6958231|r27-01|7a87a362349f|
| Committed before commit | [02](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-committed-before-commit-01-attempt-02.root-actual-return.json) |100.780668|r29-01|4927fa0de333|
| Committed after commit | [01](long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-committed-after-commit-01-attempt-01.root-actual-return.json) |84.0107088|r23-01|7045a39088b7|
| Committed before publication | [01][CUT30] |101.6072904|r30-01|86d813f98e28|

Manifest spelling: `S5.3-<rNN>-expected-artifacts-<01|02>.json`.
P-after attempt02 остаётся отдельно принятой исторической проверкой r18;
attempt03 — повторная focused regression на r19, прямо принятая
[r19 decision](long-lived-dialog-a3-migration-s5-handoff-r19-acceptance-and-p-publication-task-r20.md).

## Historical failures и предел вывода

Сохраняются все исходные failed/partial attempts, charges, source/image scopes и
append-only corrections. В частности: metadata attempt01 failed до probes;
[combined stack failure r4](long-lived-dialog-a3-migration-s5-handoff-regression-stack-failure-r4.md),
[Seen r9 failure](long-lived-dialog-a3-migration-s5-handoff-seen-batch-failure-r9.md),
[r12 prefix/cursor correction](long-lived-dialog-a3-migration-s5-handoff-prefix-cursor-test-correction-r12.md),
[mixed r13 failure](long-lived-dialog-a3-migration-s5-handoff-mixed-failure-r13.md),
[P-after r17 failure](long-lived-dialog-a3-migration-s5-handoff-p-after-commit-failure-r17-and-diagnosis-r18.md),
[Committed-before r28 inventory failure](long-lived-dialog-a3-migration-s5-handoff-inventory-failure-r28-and-repair-task-r29.md)
и static native failure r30 не становятся PASS от наличия успешного successor.
Corrected Seen receipt `root-actual-return-r2` используется вместо superseded
standard-path proof; original records не удалены.

[Boundary03][EX-BOUNDARY] остаётся whole FAIL649.592232s с отдельно принятыми
functional boundary observations. [Preservation review][EX-PRESERVE-ACCEPT]
принимает preservation-only PASS67.5439277s; она не восстанавливает потерянный
parent result и не доказывает first Event2 A.n512. Закрытые S5.1/S5.2 obligations
не открываются заново, но их приёмки не подменяют отдельный S5.3 handoff.

Итог audit: G1 exhausted; G2 capacity/validated route recovery; G3 concurrent
C/account-pin/resources/routes; G4 два affected fresh regressions. Эти gaps
основаны на исходных требованиях, явно оставленной remaining matrix и отсутствии
найденных фактических приёмок, а не на гипотезе о неисправности реализации.
Состав публикации и local-proof dependencies рассмотрены в
[publication proposal](long-lived-dialog-a3-migration-s5-handoff-publication-proposal-r31.md).

[R31]: long-lived-dialog-a3-migration-s5-handoff-r30-acceptance-and-completion-audit-r31.md
[TASK]: long-lived-dialog-a3-migration-s5-handoff-task.md
[CONTRACT]: long-lived-dialog-a3-migration-s5-contract.md
[PLAN]: long-lived-dialog-a3-migration-s5-handoff-plan.md
[REV1]: long-lived-dialog-a3-migration-s5-handoff-plan-revision-1.md
[S54]: long-lived-dialog-a3-migration-s5-final-task.md
[CONTROLS]: long-lived-dialog-a3-migration-s5-handoff-controls-acceptance-and-integration.md
[AD]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-admission-r6-01-attempt-03.root-actual-return.json
[AD-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-initial-admission-acceptance-r8.md
[FULL]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-regressions-r2-01-attempt-05.root-actual-return.json
[FULL-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-combined-acceptance-r12-and-next-components.md
[SEEN]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-seen-batch-r1-01-attempt-01.root-actual-return-r2.json
[SEEN-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-seen-batch-acceptance-and-combined-r11.md
[MIX]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-mixed-integrated-01-attempt-02.root-actual-return.json
[MIX-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-mixed-acceptance-r14-and-policy.md
[POL]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-policy-integrated-01-attempt-01.root-actual-return.json
[POL-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-policy-acceptance-r14-and-discovery-task.md
[DISC]: long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-discovery-integrated-01-attempt-01.root-actual-return.json
[DISC-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-discovery-acceptance-r15-and-preq-task.md
[PREQ]: long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-preq-integrated-01-attempt-01.root-actual-return.json
[PREQ-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-preq-acceptance-r16-and-p-after-commit-task.md
[ROUTE]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-route-regression-01-attempt-01.root-actual-return.json
[ROUTE-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-integration-small-gate-r3-result.md
[META]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-metadata-r2-01-attempt-02.root-actual-return.json
[META-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-integration-r4-metadata-and-regression-gate.md
[CUT30]: long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-committed-before-publication-01-attempt-01.root-actual-return.json
[CUT30-VERIFY]: long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-committed-before-publication-01-attempt-01.root-success-verification.json
[STATIC-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-static-acceptance-and-compile-task-r30.md
[COMPILE-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-compile-acceptance-and-finalization-source-task-r30.md
[EX-INFO]: long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-boundary-03-checkpoint-info.json
[EX-PRESERVE]: long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-preservation-01-result.json
[EX-PRESERVE-ACCEPT]: long-lived-dialog-a3-migration-s5-l1-preservation-review.md
[EX-BOUNDARY]: long-lived-dialog-a3-migration-s5-l1-boundary03-completion-review.md
[S52]: long-lived-dialog-a3-migration-s5-l1-implementation-acceptance.md
