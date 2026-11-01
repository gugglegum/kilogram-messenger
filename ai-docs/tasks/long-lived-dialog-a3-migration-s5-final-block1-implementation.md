# S5.4 block1: foundation и отключённые по умолчанию observations/hooks

2026-11-01. Сдача по [block1 task][TASK] на baseline `20418e41e4c486158fdc30b0c9f857750432e25e`. Принятый [plan][PLAN] сохранён byte-for-byte; уточнения реализации находятся здесь. Whole P5 body, process/F/P activation и commit/push не выполнялись.

## Изменения и владение ресурсами

Изменены15 Rust paths: девять ранее tracked и шесть новых. Dependency manifests, wire/storage formats, криптография, writers, production timers, frozen OD1 schema и Runtime2 hooks не менялись.

- Новые `runtime7/verification.rs`, `verification/control.rs`, `verification/tests.rs`: bounded default-off S54 control, observations, Source64 pressure selection, page/ACK pauses и pure tests.
- `runtime7/mod.rs/owner.rs/runtime.rs`: module/observer wiring; наличие control фиксируется при создании Owner7. `Handle7` хранит этот boolean, поэтому без control не добавляются owner callbacks на каждом RPC. Guard g/index/first/rotate_due добавляется в status только при opt-in. Публикуются public facts, не Native/key bytes.
- `runtime7/network2.rs/protocol2.rs`: новые hooks стоят вокруг действительных closed production operations. Cap16, reserve32, RPC15, signatures/current gates и debt retirement остаются прежними. `accept_prefix2` возвращает bounded observation notice вместо неиспользовавшегося снаружи списка ID; прежние callers игнорировали этот список. Изменение private return type учтено компиляцией всех targets.
- `runtime7/data2.rs/data.rs/bootstrap.rs`: существующие Stats7 переиспользованы; запись debug counters следует после успешного commit и увеличения настоящих committed counters. Дополнительные Legacy/bootstrap call sites существенны: current CLI выполняет часть old interop и C2 L1; наблюдение только Mixed7 пропустило бы эти операции. При отсутствующем control это только быстрый false branch, без новой записи состояния/IO.
- Новые `migration_s54_p5.rs`, `migration_s54_oracle.rs`, `migration_s54_oracle_tests.rs`, подключённые child-module из handoff: typed phases/requests/history/counter/page oracles, genuine two-C old-origin helper без setup Text и stopped in-place migration mapping. Все существующие S53 controller bodies после первого helper сохранены в Git-normalized bytes; rustfmt изменил только порядок module declarations в header. Helper не является новым whole-P5 тестом и не вызывался этим блоком.

### Настоящие64 refs

Owner callback проверяет exact context/role, current family2 lease, Active и два enabled C. `RefWindow2::reserve(c,64)` выполняется **до первого чтения**. Затем ровно две `debt_page(...,32)` с actual monotonic `after`, проверка64 distinct IDs и каждого immutable source через существующий `verify_source2` (context/selected proof/signature) плюс exact ID/ordinal/role/Text-kind. Сохраняются только bounded `(ordinal,Id)` metadata. Нет padding, fabricated rows или расширения wire window.

Один RAII permit переходит с selection через owner reply в async worker; ожидание находится **вне Owner7 и вне нового pending RPC**, до формирования следующего Head/dispatch. При busy нет inner wait/spin/read. Ошибка source/read, dropped receiver callback, cancellation, connection loss и release уничтожают тот же permit. После release refs освобождаются и вызывается действующий current-lease gate, затем строятся новые production frames. У pressure probe нет права публиковать старый frame или менять queue/debt/Native. Pure проверки второго64 share не названы реальным C2 liveness.

Для будущего process measurement есть конкретная предпосылка: пока C1 реально занимает все64 refs, peer C1 не должен посылать другие RPC, которым необходима та же share. Suspend prefix и существующий context-scoped `disable_ack` для pressure interval предотвращают, например, query удержанного B640; C2 остаётся независимо включённым. Иначе обычный SchedulingWindowBusy вправе закрыть C1 opportunity и probe будет отменён. Эти pressure flags снимаются **до** ACK-loss phase; это не основание скрыть ACK-query reply или расширить quotas/deadlines.

### Page/ACK/counter boundaries

Prefix inputs сначала проходят существующую полную проверку и durable acceptance/CancelGrant. Новизна определяется по отсутствию immutable/pending source до acceptance, а не по длине произвольного ответа или росту readable frontier. Notice содержит реальные range/root generation/last operation/grant. Head, replayed head и replayed normal pages имеют отдельные classifications; только forward normal pages увеличивают local counter и могут остановить page2/5. Empty/rejected response не превращается в progress. Публикация notice/marker и ожидание выполняются после owner reply. Controller `Pages` ведёт bounded cross-incarnation operation set; после global2 следующий stop global5 означает local3 нового PID, не сброс к пяти новым страницам.

ACK pause применяется только к действительному nonempty saved AckBatch перед write, как на push, так и на incoming ACK-query reply. Маркер различает оба пути и сохраняет только receipt count/digest, binding/request. Retirement/ACK progress не продвигаются hook. Для incoming reply сохраняется исходный RPC15 bound; outgoing pause предшествует raw RPC. После паузы остаются current-lease и dispatch gates. Длительный Source64 probe не прячется внутри этого RPC.

Native observations содержат actual PID/random incarnation/source owner, bounded sequence, реальный commit operation/generation и cumulative Stats7; нет нового unbounded in-memory journal или full private status на каждый packet. Логи возникают после real committed counters, до возможной новой page pause. На graceful actor end публикуется final counters/resolved flag; kill не выдумывает final outcome. Pure ledger отклоняет пересекающиеся samples, regression, nonce/PID confusion, повтор operation, новый current constructor и unresolved/missing final counts. Source reconciliation требует согласованных observed source/readable deltas и actual resolved counters; будущий driver обязан предоставить эти observations, проверить process/repository outcome и whole native return. Сам pure oracle не создаёт доказательство чтением expected cardinalities.

71/95/104 сохранены как old/genesis subtotals; ранний B25 даёт106 chronological effects, а не104. Есть отдельная проверка пересечения: current CLI выполняет ещё8 old encrypt (A17..24) и9 late old decrypt (B16..24). При нормативной workload сумма всех current counters2609, source-proved overlap17, frozen-only remainder87: **104−17+2609=2696**. Нельзя прибавлять104 к current ledger, уже включающему эти17 operations. Expected request set/format/body/parents и frozen cursor39−16=23 проверяются отдельно; B25 range содержит idempotent retry один раз.

## Изоляция debug control

`KILOGRAM_TEST_S54_SCHEDULE` отсутствует — нет file reads/waits/markers/Native observations и новых per-RPC callbacks. При configured control нет fallback на default после malformed/missing file. Release admission отказывает до чтения; path должен быть local absolute drive, outside-state, schema deny_unknown_fields, bytes<=16KiB, canonical context/role/phase, sorted unique ordinals1..4096<=128, max pause<=120s. Deadline фиксируется при создании pause и не продлевается dynamic update. Mixed B640 не меняет frozen OD1<512.

На Windows canonical parents удерживаются **GENERIC_READ**, sharing READ|WRITE, backup/no-reparse flags; final control открыт no-reparse. Requested/canonical namespace и actual handle attributes проверяются при каждом bounded read; child file можно atomically обновить, parent нельзя переименовать с активными read handles. Marker<=4096B публикуется atomically в том же checked outside-state namespace. Unsafe code, новые dependencies/features и системные настройки не добавлены.

Первый pure run выявил неверную первоначальную предпосылку: metadata-only access0 directory handle не запретил rename. Падающий тест и binaries/source сохранены. Малые собственные Windows FS probes подтвердили: access0/READ_ATTRIBUTES rename допускают; GENERIC_READ+share READ запрещает также нужный child atomic replace; **GENERIC_READ+share READ|WRITE сохраняет child atomic replace и блокирует parent rename**. Исправлен opener, критерий теста не ослаблен. Эти probes касались новых пустых test directories, без аккаунтов/сети/старых stores.

## Фактические проверки

Финальные sources прошли compile-only, strict Clippy и **22/22** точечных несетевых tests (12 Runtime7 hooks +10 controller/oracle), без failed/ignored. Frozen artifacts guard PASS; все501 compiler units при проверочном запуске были fresh, пересборки между compile-05 и pure-02 не было. Normal CLI unit действительно test=false, opt-level3, debug assertions/overflow checks enabled. Фактические outcomes сохранены в [.tmp/s54-block1-01][EVIDENCE]:

| Check | Actual native result / elapsed | Область |
| --- | --- | --- |
| compile-01 | FAIL1 /0.7106678s | Frozen wrapper потребовал принятую пару CLI+transport; Cargo/tests не стартовали. Исправлены параметры helper. |
| compile-02 /compile-03 | PASS0 /560.2669899s и568.1007585s | Nonexecuting scale compile, normal CLI/test=false также собран; промежуточные source/image revisions. |
| compile-04 | PASS0 /77.7580706s | Пересобран уточнённый oracle, normal image сохранён; до исправления Windows pin. |
| compile-05 | PASS0 /513.0032093s | Финальные15 Rust paths, CLI+transport/all-target compile-only и normal current CLI. |
| clippy-01 | FAIL101 /23.5635461s | Единственный needless_range_loop в новом helper; заменён проходом по views. |
| clippy-02 /03 /04 | PASS0 /30.0691199s /29.4921814s /7.9656826s | Scoped intermediate strict checks. |
| clippy-05 | PASS0 /33.1726664s | Финальные sources, CLI/all-targets, literal `-- -D warnings`. |
| pure-01 | FAIL1 /13.419494s | Frozen guards PASS; hook kernel11PASS/1FAIL, harness101. Windows parent-rename test выявил access0 проблему; oracle harness ещё не запускался. |
| pure-02 | PASS0 /15.5244059s | Финальные22 exact s54_ tests через fixed stable harnesses; frozen guard PASS; cleanup verified. |
| fmt-final | PASS0 /4.406763s | `cargo fmt --all -- --check`, без вывода/изменения sources. |

[Review manifest][REVIEW],30413B/SHA256 `f6dab6042f6395c39c33829023752e35633394413d4f423792cc5d6d35f9b270`, содержит exact15 source paths с file hashes/Git blobs, exact22 passing full test names, immutable12 step receipts и evidence hashes. Автоматическая сверка проверила unchanged plan bytes, неизменённые normalized S53 controller bodies, empty index/исходный HEAD, отсутствие изменений crates/Cargo/scripts, scoped diff whitespace, четыре неизменённых fixed field images и семь прежних scale/compiler image archives. Финальные sources совпадают с compile-05/pure-02 build inputs.

| Финальный static artifact | Bytes | SHA256 |
| --- | --- | --- |
| normal CLI `target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe` |50087936|`f5444996cee23322f2ca9784da046af5a6688ff056c4280cdbc23d70a391a6d0`|
| stable `kilogram-cli-lib-kilogram_cli-tests.exe` |32519168|`d162a7f37a6ac4297045b7a1229be2eeb21b5858708e4f4b11ab1a4b86de69e0`|
| stable `kilogram-cli-test-migration_s5_process-tests.exe` |16046080|`5c46e9e2e926b65f3929a11e025a3c37e14b1670640c3677d61fcb1a67807222`|

Минимальное воспроизведение из того же workspace/native Windows окружения: `check.ps1 -Step compile-06`, затем `check.ps1 -Step clippy-06`; перед новым `pure-03` указать новый compile-06.capture.json в локальном pure helper вместо compile-05. Числовые step names новые, старые evidence не перезаписывать. Helpers [compile][COMPILE]/[pure][PURE]/[check][CHECK] фиксируют реальные аргументы, profile/target, source provenance и finite watchdog. Для проверки уже готовых финальных bytes pure helper по-прежнему привязан к compile-05, поэтому без изменения sources возможен новый immutable pure step. Это воспроизведение только разрешённых static/pure проверок, не активация P5.

Все native check steps использовали pinned pwsh, Jobs2/BelowNormal/locked/offline, existing non-breakaway Windows job с конечным watchdog и verified cleanup0. Test execution только через stable names и `-Filter s54_`; cargo test использовался wrapper исключительно как --no-run compilation. Non-network tests создают маленькие temp files/dirs и memory-only channels, не account/state fixtures.

Первые failed outputs не переписаны: `compile-01.*`, `clippy-01.*`, `pure-01.*`; failed pure source15 paths и два соответствующих binaries дополнительно сохранены в `.tmp/s54-block1-01/pure-01-preserved/`. Post-compile02 images сохранены перед последующими пересборками. Прежние S53/G4 image tuples доступны в проверенных G4 preimage archives; fixed d9b/current migration/fresh CLI/relay не перезаписывались. Compiler output и stable harnesses — новые static artifacts, не новые field EXEs.

## Граница сдачи

Не выполнены и не объявляются PASS: real two-C zero-Text origin/start/migration, actual64+C2 concurrent liveness, live TLS cancellation/revalidation, реальный непустой ACK loss/query race, page2/5 stops/reopen и counter/source reconciliation на живом P5. Они требуют reviewed per-node control attachment, фазового driver и отдельной F/P activation после следующих blocks. Синтетические pure metadata/rows доказывают predicates/ownership, не genuine Native lineage.

Whole P5 body/selector и новый runner framework не добавлены. Нужны blocks2/3 и согласованный короткий measurement из block4; ни F510, ни P2400 не активированы. Исторические S5.3 F2845.3231047/combined F7561.6432536/P0 не переобозначаются. Static elapsed — наблюдения отдельных steps; новый guessed grand C total или административный stop не вводились.

Root navigation и остальные historical/untracked files не редактировались исполнителем. Index пуст, HEAD исходный; commit/push не делались. Блок готов к source review; сдача ограничена этим block1, затем остановка до решения root.

[TASK]: long-lived-dialog-a3-migration-s5-final-block1-task-01.md
[PLAN]: long-lived-dialog-a3-migration-s5-final-plan.md
[EVIDENCE]: ../../.tmp/s54-block1-01/
[REVIEW]: ../../.tmp/s54-block1-01/review.json
[COMPILE]: ../../.tmp/s54-block1-01/compile.ps1
[PURE]: ../../.tmp/s54-block1-01/pure.ps1
[CHECK]: ../../.tmp/s54-block1-01/check.ps1
