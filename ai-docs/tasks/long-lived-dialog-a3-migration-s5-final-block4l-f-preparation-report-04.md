# Block4L — successful actual F04 preparation, pending root runtime dispatch

2026-11-03. Задание `long-lived-dialog-a3-migration-s5-final-block4l-f-preparation-task-04.md`. На опубликованном HEAD `9013f9cfb2e1803d279eaa0377b2865c835618ec` выполнен один production Prepare F04. Prepare вернулся0; отдельный свежий pwsh завершил actual `Assert-S54Inventory -State Fresh` с кодом0. Полная inventory validation заняла **31.378297s**, весь bounded validation command — **65.0742002s**. Activation/Run/P, новые продуктовые тесты, source changes и commit/push не выполнялись.

## Exact binding

- Purpose `FeasibilityF`, case `F-s54-mixed-hooks-feasibility-01`.
- Attempt `F-s54-mixed-hooks-feasibility-01-attempt-04`.
- Run ID `27f67783ab7bbcfd5b306178bc010a11317b213a8aba317d7230915ccccb95d2`.
- Selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`.
- Inventory: `E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-04/inputs.json`.
- Inventory116874 bytes, SHA256 **`b548cf47be85fdae660bf7ed183438b5810f376bc2c3275b78f38e7ac824b85d`**.
- Public evidence: `.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-04`.
- Fresh administration: `.tmp/s54-block4l-f-preparation-04`.

Installed executable: `C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe`.

Фактический production argv свежего дочернего процесса:

```text
-NoProfile -NonInteractive -File E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s54-whole.ps1 -Action Prepare -Purpose FeasibilityF -PreparationRunId 27f67783ab7bbcfd5b306178bc010a11317b213a8aba317d7230915ccccb95d2
```

## Actual bounded returns

Каждый шаг вызван pinned pwsh с `-NoProfile -NonInteractive -File .tmp/s54-block4l-f-preparation-04/owner.ps1 -Step <step>`. NativeToken; original owner tick/stopwatch охватывает startup и pre-Prepare gate. Исторические owner/preflight/validation не менялись: адаптация выполнена только в свежей L administration. Рутинных повторов и неуспешных L шагов не было.

| Step | Actual tool return | Exit / full owner elapsed | Budget | Child / owner PID |
| --- | --- | --- | --- | --- |
| preflight-01 | ab3ed3 (launchfc8389/session91956) | 0 /12.6440275s | 270/300/30 | 46872 /59868 |
| prepare-01 | 3b9ebf (launch770482/session31313) | 0 /535.2939613s | 1170/1200/30 | 27536 /53580 |
| validate-01 | ad3280 (launch62d68a/session4326) | 0 /65.0742002s | 570/600/30 | 68216 /31096 |

Actual child start UTC: preflight `2026-11-02T23:56:12.8625109Z`; Prepare `2026-11-02T23:56:59.2766841Z`; validation `2026-11-03T00:06:12.8821764Z`. Original owner ticks:513866000/513905093/514465828 соответственно. Все actual handles/Job membership verified, child waited=true/native exit0, bounded stdout/stderr EOF=true, reader_failed=false, work_expired=false. Full raw outer stdout/stderr сохранены.

Prepare cleanup был **forced** для одного оставшегося owned Job member PID63404, executable identity не установлена. Cleanup verified=true/failed=false/after=[]; это административная очистка подготовки. Preflight и validation cleanup clean/nonforced/before=[]/after=[]. Runtime verdict не выводится из этого результата.

Prepare stdout1478B SHA256 `4a34ab66a5da9b23d8cb7863d2dfed22e5c8f6d765dbdc4703e1e0531af93ed8`; stderr596B SHA256 `31dfa3d17dc9f2174da75e14b289d7220c184a4962646832be4a2bdd0614fe8c`. Stdout содержит один фактический `S54_PREPARED` с точным inventory pin выше. Compiler stderr сообщает test build4m32s и normal build2m59s. Остальные121B preflight/validation stderr — известное Git CRLF warning в root navigation, без ошибки проверки.

## Preparation and validation evidence

Read-only preflight проверил current source closure519, shared HEAD resolver=Git=published baseline, current F04 identity/selector/deadlines/schema/interfaces/порядок compiler stages, отсутствие обеих F04 directories и P, доступность workspace lock, actual tool resolutions и pins, installed Rust target libraries/MSVC linker/Windows SDK, Cargo build inputs/graph/config/environment, native exact-image inventory0. Неудачная process metadata query не трактовалась как0. Код и `.git` metadata не изменялись.

Единственный Prepare использовал production seven-image guard. До первой замены сохранены семь фактических preimages текущих mutable destinations с write leases; каждый saved pin совпал с исходным preflight image pin. Actual test-first/normal-last последовательность:

| Stage | Role | Final stage write UTC |
| --- | --- | --- |
| 01 | stable-cli-bin | 2026-11-03T00:01:43.2395304Z |
| 04 | stable-runtime2 | 2026-11-03T00:01:44.1422435Z |
| 05 | stable-iroh-lib | 2026-11-03T00:01:45.0175831Z |
| 02 | stable-cli-lib | 2026-11-03T00:01:45.9823628Z |
| 03 | stable-migration | 2026-11-03T00:01:46.8392182Z |
| 06 | stable-iroh-relay | 2026-11-03T00:01:47.7670155Z |
| 00 | current-cli, normal-last | 2026-11-03T00:05:07.6661226Z |

Семь stage records уникальны, guard owner27536 соответствует actual preparation child. Outcome `stage_completed=true`, `preparation_succeeded=true`, staged7, automatic_rollback=false. Outcome2018B SHA256 `17fa61e6944690efd52c2c93c17597870142eb274eacca3568d7c82aa1f0abe5`.

Actual Cargo test capture/normal capture, embedded before/after/dependency closure, frozen manifest, isolated selector и stable harness `--list` созданы этой подготовкой. Stable compile-only lane использовал Jobs2/BelowNormal/offline/locked/verification-scale/x86_64-pc-windows-msvc; test harnesses не исполнялись как tests. Единственный listing native harness подтвердил exact selector один раз. Historical compiler captures не переименовывались и не перепривязывались.

После завершения actual Prepare свежий installed pwsh прочитал ровно `inputs.json`, выполнил полный published `Assert-S54Inventory $inventory $policy -State Fresh` и измерил весь вызов31.378297s. Затем проверены source519/current images9, seven actual stages/preimages/outcome, after-final-normal origins1 normal+6 test и dep-info1 normal+6 test, embedded/test dep-info equality, source/index/navigation, lock release и successful native exact-image process inventory0. Проверки не создают runtime claim/capability.

Legacy CLI61531648B SHA256 `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1` и relay10460160B SHA256 `cf9d029cebf1d04745c90a9712401d70e823ecf4af5a082fe7269649df358e5e` неизменны. Live Cargo origins и семь mutable runtime destinations обновлены только через разрешённую production preparation; их старые F03 observations не объявляются текущими pins.

168 deduplicated immutable historical pins проверены до и после подготовки: F01/F02 admin/public/report snapshots, F03 preparation/runtime admin/public/reports/manifests/archived preimages и H/K source-check images. F01/F02 run directories пусты; F03 сохраняет прежние восемь top-level failure artifacts. F03 остаётся consumed/failed, причина UNKNOWN. Root navigation3/HEAD/index сохранены; relevant source files clean, index пуст. F04 run содержит только inputs; activation/launch/capture/claims/assertions/profiles отсутствуют. P directories отсутствуют. Private profiles/seeds не читались и не создавались. Закрытые matrices/tests не повторялись.

## Concrete future Run proposal — not executed

После независимой проверки root должен создать fresh external activation с полями:

```text
schema=1
classification=root_s54_feasibility_activation
inventory=<exact path/116874 bytes/b548cf47... pin above>
case=F-s54-mixed-hooks-feasibility-01
attempt=F-s54-mixed-hooks-feasibility-01-attempt-04
run_id=27f67783ab7bbcfd5b306178bc010a11317b213a8aba317d7230915ccccb95d2
run=E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-04
selector=live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public
implementation=9013f9cfb2e1803d279eaa0377b2865c835618ec
execution_mode=NativeToken
deadline={controller:450,work:480,outer:510,origin:owned_launch,clock:windows_tick_count64}
retry=root_reallocation_only
not_before_utc_ms=<fresh root-issued interval>
not_after_utc_ms=<fresh root-issued interval>
```

Точная машиночитаемая форма inventory pin, всех materializable activation fields, required relevant environment и future argv дана в `.tmp/s54-block4l-f-preparation-04/run-proposal.json`. Валидность и SHA256 activation пока не определены, activation file не записан.

Предлагаемая будущая команда свежего installed pwsh:

```text
-NoProfile -NonInteractive -File E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s54-whole.ps1 -Action Run -Purpose FeasibilityF -InventorySha256 b548cf47be85fdae660bf7ed183438b5810f376bc2c3275b78f38e7ac824b85d -ActivationSha256 ROOT_ISSUED_ACTIVATION_SHA256
```

Обязателен тот же parent→fresh-child recipe: process-local `C:\Users\Paul\.rustup\toolchains\1.98.0-x86_64-pc-windows-msvc\bin` prepend PATH ровно один раз к исходному tail; `CARGO_NET_OFFLINE=true`; BelowNormal, Jobs2, x64; TOKIO_WORKER_THREADS отсутствует, available/effective24. PATH SHA256: original owner `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`; configured parent `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`; actual fresh child/inventory `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. Перед возможным runtime нужно сверить полное relevant environment с новым inventory; persistent settings не меняются. F450/480/510 и P2350/2375/2400 не расширялись.

Этап остановлен для root review. Результат — воспроизводимая prepared-state evidence, не FeasibilityF runtime pass и не приёмка whole S5.4. Commit/push этого отчёта пока не выполняется. Соседний pin manifest ссылается на оригинальные public/admin/source/current-image/origin/history artifacts без дополнительных копий больших captures/binaries.
