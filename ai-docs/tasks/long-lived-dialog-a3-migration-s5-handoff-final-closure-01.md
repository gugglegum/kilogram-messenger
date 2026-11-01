# S5.3: итоговая сверка перед whole-stage acceptance

2026-11-01. **Функциональные обязательства S5.3 закрыты в принятых scopes; документ готов к финальному review root.** Whole-stage acceptance ещё не объявлена: root должен независимо подтвердить G4 publication, принять эту сверку и определить точный завершающий documentation commit. Новых product gaps по прежней 16-row matrix не найдено. Whole resumed-L1/P5 остаётся S5.4 и не запускался.

Нормативная база: [исходная задача][TASK], [принятый contract][CONTRACT], [mapping][PLAN], [revision1][REV1] и [её приёмка][PLAN-ACCEPT]. Сверка продолжает [историческую r31 matrix][MATRIX], не меняет её прошлый verdict, receipts, timestamps или failure classifications. Это source/document-only работа после разрешённой публикации G4; новых компиляций, тестов, fixture/private-state reads не было.

## Опубликованные закрытия G1–G4 и B-only

| Принятый пакет | Фактический commit | Что закрывает |
| --- | --- | --- |
| [G1 acceptance и источники][G1] | `2f7e44f756ea2ae13cf270be33889bada356f8f6` | Genuine exhausted lineage, первый Event2, old history/requests/receipts и restart; coherent S5.3 source cohort. |
| [G2 acceptance][G2] | `9aa47d938af990c2cb61ea6c2ffe8dc1db02a9fa` | Два capacity/profile interruption windows, validated RelayOnly и recovery. |
| [G3 acceptance][G3] | `5f31239ecc3be0eace190db8362c310a99d7e53f` | Два C, account-pin isolation, capacity, DirectOnly/RelayOnly/Auto и persistence. |
| [B-start acceptance][BSTART] | `8b3237a593c529d55af9a556fa44ad3f7a704a7c` | B-only request до текущего C1 ALPN2/Locator2 readiness; без local intent у A; subsequent real delivery. |
| [G4 acceptance][G4] | `1f404b54ebce47ae3b52eaeb6855e30f3b8f3309` | Fresh offline/restart/readable и escaped IPC history byte budget/cursor. |

G4 parent `8b3237a593c529d55af9a556fa44ad3f7a704a7c`, tree `2e6e90a62bd1c21dfe53a2f8d15c8ccb953e8c4c`: ровно28 разрешённых paths (7 scripts,21 packet files), raw bytes и normalized Git blobs совпали с root allowlist. Ordinary non-force push и отдельный `ls-remote` подтвердили тот же SHA; индекс пуст, три чужих navigation files и4061 untracked paths вне allowlist сохранены. [Executor publication receipt][G4-PUB] содержит реальные команды и actual tool returns; независимое root publication verification ещё ожидается. Root verification G1/G2/G3/B-start остаются отдельными локальными records из списка публикации ниже.

## Source preservation и предел повторного использования

Для строк таблицы ниже применяются следующие аргументы, уже принятые в прежних review; текущая сверка не выдаёт их за новые execution results:

- **H:** r31 `R14-reuse/R30-preserved/Earlier-reuse/Controls-reuse`: production r14→r30 сохранён; r12 менял integration/process controllers, r11 также regression-test source, r8→r30 включал исправление data2 и Seen. Поэтому r8 admission сохраняет исторический scope, а поздние FULL/MIX/PREQ реально используют исправленный data path. Каждый из12 cuts остаётся запуском своего controller/image. Current55/55 или повтор всех cuts на последнем owner не заявляются.
- **P:** опубликованный [G1 source-provenance][G1-SOURCE] закрепляет64 source paths:57 Rust и7 runtime/guard scripts. Восемь G1 Rust paths — reviewed changes;48 иных Rust совпадают с r14/r30 snapshots; оставшийся live1 delta — ранее опубликованный Cold-B verification-close hook. Original controls/discovery/PREQ/two-C bodies сохранены по этому provenance. Это accepted source-impact argument, не весь текущий compiler graph и не blanket rerun.
- **L:** последующие G2/G3 product additions — bounded read-only diagnostics/accounting; storage writers, wire и crypto не менялись. B-start менял verifier/setup/strict decoders; G4 — только семь preparation/runner scripts. Поэтому старые production assertions и control scopes сохраняются. Каждый новый процессный результат привязан к собственному packet source/evidence mapping. G4 не менял production Rust и existing runtime2 test assertions.

## Все16 требований r31

`Закрыто(scope)` означает принятую проверку конкретных перечисленных сценариев. В последнем столбце приведены реальные пределы, а не новые обязательства вне исходной задачи. Часть приёмок до G1 пока local-only; FULL, MIX, последний r30 cut и cumulative r31 root decision уже tracked. Точный минимальный public documentation set указан ниже.

| № / обязательство | Принятый scope и конкретные доказательства | Сохранение и remaining |
| --- | --- | --- |
| 01. Existing Consent2, offline own-Root ceremony, explicit per-C intent/cancel, IPC3 tags7–10 | [POL][POL]: ceremony, CAS/expiry, семь malformed direct-handler variants без writes; [DISC][DISC]/[PREQ][PREQ] и cuts дают реальные CLI/IPC positive paths. [B-start][BSTART] закрывает revision1 initiation до readiness, local intent только у B. | H/P/L. Закрыто(scope). Direct negatives не названы OS IPC-server negative tests; локальная cancellation не обещает remote rollback. |
| 02. Pre-Q ALPN2 existing admission, actual TLS/exporter/Consent/roles/Locator2, no fake Q, ALPN1 fence с P/Prepared | [AD][AD]: настоящий family2 lease при L1 без selection; FULL/MIX/PREQ и12 cuts используют admission/reconnect; [B-start][BSTART] подтверждает notification на новом genuine startup path. | H учитывает r8 data2 delta; P/L. Закрыто(scope), historical r8 не становится current-image rerun. |
| 03. Asymmetric Selected/Prepared, A receive-only при B Active, exact K и opaque hints | [MIX][MIX] и [PREQ][PREQ]: late old repair, original request104 format2/n3 readable/delivered за1.7113285s при A Selected/B Active, те же family2 generations до/после IO; request103 materialized once после activation. | H/P/L. Закрыто(scope); hints не дают Q/Native/debt release. |
| 04. Durable P/Prepare/K/Committed, exact retry, freeze/no downgrade,4×3 cuts | [r31 root acceptance][R31] принимает все12 process-stop/reopen cuts; их exact actual-return proofs перечислены ниже. Последний r30:7 incarnations,26 lifecycle events,5 cold observations; B Active/A Selected без Committed у A, exact saved controls/Q и conversion once; original400/401 once. | H/P/L, каждый cut в своих source/image pins. Закрыто12/12; физический power-loss и current-image sweep не заявляются. |
| 05. No-intent background discovery, ordinary L1 progress, scheduler5/60/45/15/30/90 | [DISC][DISC]: ordinary #1 ACK, family2 opportunity возвращается L1/NoLocalIntent без P/Q/cuts/freeze/OpenIntent, ordinary #2 delivered/ACK; starts spaced60.075s. [B-start][BSTART]: current startup достигает Active после B-only trigger, A не создаёт local intent. | H/P/L. Закрыто(scope); second start не равен second completed opportunity. Single-host timings не universal SLA/fairness. |
| 06. Genuine old gap/pending/head/Prefix/Resume/Receipt1, no possession release, pending64+1, Seen/cold continuation | [corrected Seen receipt][SEEN]:11 Owner7 subcases, write-fault retry/cold/quarantine. [FULL][FULL]:131 offline encryptions, concurrent append, R4 conflict/no Native mutation, cold64+1/drain65/ACK65. PREQ сохраняет genuine A2/A3/B2 gap и late repair. | H/P/L. Закрыто(scope). FULL оставляет67/132 source debts намеренно; не «все доставлены». PREQ Receipt2(format1) — reviewed paths+history, не независимый raw-header read. |
| 07. Atomic Mixed7 conversion once, same keys/D/E/SID, zero handoff constructors/packet calls | [MIX][MIX]: same lineage, conversion calls0, затем genuine4 encrypt/6 decrypt; FULL и cuts подтверждают conversion/reopen. [G1][G1] переносит исходный exhausted Native domain и продолжает его. | H/P/L. Закрыто(scope). Поздняя легальная crypto работа меняет Native bytes; byte equality относится к соответствующим frozen boundaries. |
| 08. Current/final gates, renewal vs historical selected proof, archive/cold, cross-C pin effects | [POL][POL]: rollback/floor/role/context/CAS/expiry refusals, renewal3days при selected3600 и clock+7200 без изменения Q/Native/escrow. [G3][G3]: реальный pin change/revocation Y у X блокирует C1, отвергает новый send без его data advance, разрешённый C2 продолжает. | H/P/L. Закрыто прежнего G3 дополнения к component scope. Не заявляется каждый возможный revoke race/OS IPC negative. |
| 09. Per-C credits/escrow, exact ACK once, independent holds, capacity CAS/recovery | FULL/MIX: реальные списания/ACK, repair_free67108864→67103503. [G2][G2]: stops capacity-committed/profile-staged, Ready resume, retained route/messages/accounting. [G3][G3]: independent C2, live capacity increase и stopped source/ACK equations. | H/P/L. Закрыто G2/G3; META runner classification не используется как capacity proof. Solo G3 pre-CAS bridge source-derived, не independently observed raw pre-CAS state. |
| 10. Independent C leases, pin isolation, global permits2/mailbox16/batch4/64–64 shares | [G3][G3]: три accounts, два migrated C через один owner, C1 revoked/paused, C2 доставляет три distinct messages, ordinary resource occupancy и final zero checked processing counters; source preserves existing resource bounds. | P/L. Закрыто исходного G3 controlled scope; arbitrary saturation/stress fairness не доказаны и не требуются вместо row16 whole trace. |
| 11. DirectOnly пока L1, RelayOnly после all Active, explicit own relay/Auto/restart | [G2][G2] даёт validated RelayOnly через оба interruption windows; [G3][G3] — actual route/public observations, C1/C2 eligibility, RelayOnly, Auto/direct и final profile/locator/restart persistence. | P/L. Закрыто G2/G3. Старый route-table test не подменяет live relay; public relay default/fallback не добавлен. |
| 12. Unified HVI1 old/new, original IDs/requests, frozen pre-Q suffix через append/restart | MIX/PREQ/cuts: original history/cursor и exact once103/104; [G1][G1]:530 original signed sources/528 Text per role,17 pages/527-row frozen suffix, затем A600/B601 и restart. [G4 history][G4] проверяет escaped JSON fit/cursor. | H/P/L. Закрыто(scope), длинная граница покрыта G1, affected fresh — G4. Private histories повторно не экспортировались. |
| 13. Тот же stopped exhausted L1, cuts512/18, first Event2 A.n512/B.n18, no regeneration | [G1 root composite acceptance][G1] соединяет accepted first-opportunity/refusal/current-permission/sole-conversion prefix attempt01 с completed preservation/A600/B601/restart attempt02; final next[513,19],532 sources,receipts[512,18],queues/pending/debt0, Native generations520/91. Original lineage, old retries/requests/receipts/history сохранены. | P/L. Закрыто G1 composite; attempt01 остаётся whole FAIL, его lost PID/Instant/first_m не восстановлены. Не новая генерация511, не fresh-session substitute, не whole P5. |
| 14. Fresh offline/restart/readable и IPC history escaping/byte cap/no cursor skip | [G4][G4]: exact native markers и named1PASS каждый; actual final857d5b/d5e502 exit0. Smoke24 Text/4genesis/8offline, оба restart; history6 NUL texts×4096B, cap49152,6 pages/peer, exact request set. Case/outer0, six known Node absent/job cleanup0. | L. Закрыто G4; unchanged test assertions. Native-marker contract допускает assertions_sha256=null. Memory samples не доказанный unsampled peak. |
| 15. Static/frozen/source correspondence, honest failure classifications, stable Windows paths | [r30 static acceptance][STATIC] сохраняет failed native preparation отдельно от принятых fmt/strict Clippy+root preservation; [compile acceptance][COMPILE] —501 units,386 runtime/115host. Новые G1/G2/G3/B-start packets сохраняют свои focused checks/strict lint/compile; G4:25 policy+12 legacy checks, memory-shell probe, frozen captures/source/image/dependency checks. | H/P/L. Закрыто(scope). Публикация не меняла принятые bytes; никакого повторного начисления или whole current suite. |
| 16. Whole resumed-L1/P5 operational trace | По исходному task и r31 root split это отдельный S5.4 scope после whole-S5.3 acceptance/publication. Ни один G1–G4 packet не назван whole P5. | ASSIGNED ELSEWHERE: S5.4 не started, не выполнена в этом задании и не является остаточным функциональным gap S5.3. |

## Точные12 принятых control receipts

Все ниже — root actual-return proofs, whole/outer exit0 в исходном scope. Последний Committed-before-publication уже tracked; остальные11 пока local-only. Они не исполнялись заново. Во всех путях далее префикс `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/`.

| Control | Before commit | After commit | Before publication |
| --- | --- | --- | --- |
| P | `F-s53-p-before-commit-01-attempt-01.root-actual-return.json` (r19) | `F-s53-p-after-commit-01-attempt-03.root-actual-return.json` (r19) | `F-s53-p-before-publication-01-attempt-01.root-actual-return.json` (r20) |
| Prepare | `F-s53-a-before-commit-01-attempt-01.root-actual-return.json` (r24) | `F-s53-a-after-commit-01-attempt-01.root-actual-return.json` (r21) | `F-s53-a-before-publication-01-attempt-01.root-actual-return.json` (r25) |
| K | `F-s53-k-before-commit-01-attempt-01.root-actual-return.json` (r26) | `F-s53-k-after-commit-01-attempt-01.root-actual-return.json` (r22) | `F-s53-k-before-publication-01-attempt-01.root-actual-return.json` (r27) |
| Committed | `F-s53-committed-before-commit-01-attempt-02.root-actual-return.json` (r29) | `F-s53-committed-after-commit-01-attempt-01.root-actual-return.json` (r23) | `F-s53-committed-before-publication-01-attempt-01.root-actual-return.json` (r30) |

## Минимальное дополнение public documentation:29 exact paths

Предлагается **отдельный review/publication этих29 файлов**, включая этот новый документ. Сейчас они не staged/committed; это не ещё одно разрешение на push. Accepted contract, FULL/MIX/последний r30 cut, cumulative r31 root acceptance и G1/G2/G3/B-start/G4 packets уже tracked, повторять их не надо. Root может принять меньший summary-only scope явно, но для переносимой прямой проверки всех строк ниже нужен этот небольшой набор original root proofs/decisions. Никакой recursive import historical pool не предлагается.

В `ai-docs/tasks/` —8 paths:

```text
long-lived-dialog-a3-migration-s5-handoff-final-closure-01.md
long-lived-dialog-a3-migration-s5-handoff-task.md
long-lived-dialog-a3-migration-s5-handoff-plan.md
long-lived-dialog-a3-migration-s5-handoff-plan-revision-1.md
long-lived-dialog-a3-migration-s5-handoff-plan-review-2.md
long-lived-dialog-a3-migration-s5-handoff-completion-matrix-r31.md
long-lived-dialog-a3-migration-s5-handoff-static-acceptance-and-compile-task-r30.md
long-lived-dialog-a3-migration-s5-handoff-compile-acceptance-and-finalization-source-task-r30.md
```

В `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/` —21 paths:11 exact control receipts из таблицы выше, кроме уже tracked последнего Committed-before-publication, и следующие10:

```text
C-s53-admission-r6-01-attempt-03.root-actual-return.json
C-s53-seen-batch-r1-01-attempt-01.root-actual-return-r2.json
C-s53-policy-integrated-01-attempt-01.root-actual-return.json
F-s53-discovery-integrated-01-attempt-01.root-actual-return.json
F-s53-preq-integrated-01-attempt-01.root-actual-return.json
G1-publication-root-verified-01.json
G2-publication-root-verified-01.json
G3-publication-03.root-verified.json
B-start-publication-02.root-verified.json
G4-publication-01.executor-publication.json
```

Это portable acceptance trail, не self-contained rerun исторических activations. Ссылки внутри original receipts/r31 docs на их raw returns, старые accepted reviews, giant bindings/compiler captures/source archives/assertions/logs и private fixtures намеренно остаются local provenance dependencies. Для прямой проверки всей низкоуровневой proof graph потребуется отдельно согласованный точный public-only export; он не нужен для этой accepted-scope map и сейчас не выполняется. Historical bytes/path/hash не переписываются, superseded Seen proof не включается вместо corrected-r2. Source code/EXEs/cache/private DB/accounts/Root keys, `.tmp` contents и три navigation files в предложенные29 paths не входят.

После callback root добавит **свой новый G4 publication-verification record и final whole-S5.3 acceptance decision**, если проверка пройдёт. Это ещё не существующие root-owned результаты, поэтому они не включены в список29 и не сфабрикованы исполнителем.

## Оставшиеся действия и accounting

1. Root независимо подтверждает новый G4 SHA/path/blob/remote и принимает или корректирует эту16-row сверку. Функциональных runtime gaps к повторению здесь не найдено.
2. Root принимает exact public documentation scope выше, даёт отдельное поручение commit/push и проверяет его публикацию вместе со своим whole-S5.3 acceptance decision. До этого стадия только **ready for whole-stage acceptance review**, не уже принята целиком.
3. Только после этого root решает переход к S5.4. Никакой S5.4 implementation/run этим документом не начинается.

Принятые totals без новых charges: **S5.3 F2845.3231047s; combined F7561.6432536s; P0**. G4 F78.5914879 уже начислен root один раз; test/compile/nested durations и прежний B-start повторно не прибавляются. Исторические C6402.2470595 (S5.3) / C6989.6956704 (combined) — именно snapshot при G1 publication, а не актуальный grand total: последующие preparation/build/pure/static observations остаются в своих records. Эта documentation работа не выдумывает точный новый aggregate C и не использует прежний административный C ceiling как stop rule.

Boundary03 остаётся whole FAIL649.592232 с separately accepted observations, preservation67.5439277 не заменяет lost parent result. Failed Seen/r13/старые controls/inventory/static preparations и failed G1/G2/G3/B-start attempts остаются failed даже при accepted successor/composite. Portable packets явно сохраняют эти различия. Нет claims stress fairness, arbitrary-network availability, physical power-loss, universal latency или полного current-image sweep.

[TASK]: long-lived-dialog-a3-migration-s5-handoff-task.md
[CONTRACT]: long-lived-dialog-a3-migration-s5-contract.md
[PLAN]: long-lived-dialog-a3-migration-s5-handoff-plan.md
[REV1]: long-lived-dialog-a3-migration-s5-handoff-plan-revision-1.md
[PLAN-ACCEPT]: long-lived-dialog-a3-migration-s5-handoff-plan-review-2.md
[MATRIX]: long-lived-dialog-a3-migration-s5-handoff-completion-matrix-r31.md
[R31]: long-lived-dialog-a3-migration-s5-handoff-r30-acceptance-and-completion-audit-r31.md
[STATIC]: long-lived-dialog-a3-migration-s5-handoff-static-acceptance-and-compile-task-r30.md
[COMPILE]: long-lived-dialog-a3-migration-s5-handoff-compile-acceptance-and-finalization-source-task-r30.md
[AD]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-admission-r6-01-attempt-03.root-actual-return.json
[FULL]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-regressions-r2-01-attempt-05.root-actual-return.json
[SEEN]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-seen-batch-r1-01-attempt-01.root-actual-return-r2.json
[MIX]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-mixed-integrated-01-attempt-02.root-actual-return.json
[POL]: long-lived-dialog-a3-migration-s5-handoff-evidence/C-s53-policy-integrated-01-attempt-01.root-actual-return.json
[DISC]: long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-discovery-integrated-01-attempt-01.root-actual-return.json
[PREQ]: long-lived-dialog-a3-migration-s5-handoff-evidence/F-s53-preq-integrated-01-attempt-01.root-actual-return.json
[G1]: long-lived-dialog-a3-migration-s5-handoff-evidence/G1-publication-01/ACCEPTANCE.md
[G1-SOURCE]: long-lived-dialog-a3-migration-s5-handoff-evidence/G1-publication-01/source-provenance.json
[G2]: long-lived-dialog-a3-migration-s5-handoff-evidence/G2-publication-02/ACCEPTANCE.md
[G3]: long-lived-dialog-a3-migration-s5-handoff-evidence/G3-publication-03/ACCEPTANCE.md
[BSTART]: long-lived-dialog-a3-migration-s5-handoff-evidence/B-start-publication-02/ACCEPTANCE.md
[G4]: long-lived-dialog-a3-migration-s5-handoff-evidence/G4-publication-01/README.md
[G4-PUB]: long-lived-dialog-a3-migration-s5-handoff-evidence/G4-publication-01.executor-publication.json
