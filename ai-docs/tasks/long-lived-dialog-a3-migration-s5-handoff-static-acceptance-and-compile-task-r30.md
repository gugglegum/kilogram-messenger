# S5.3 r30: static components приняты; compile-only подготовка

2026-10-29. Root 01a164ec-589c-7b41-bc34-3ba158d1f22b.
Прежний исполнитель 01a1653c-0154-7611-8e8c-044fbe1e4d7a, GPT-6.1 Sol Max, без Fast.

## Решение по фактическому static result

[Static readiness](long-lived-dialog-a3-migration-s5-handoff-static-readiness-r30.md),
46784 B / 2b2369520b7ed2688080dbe76d3a0860b18e05495ba37f32d9c43ee5cee802e9,
проверен независимо. Один S-s53-r30-static-01 остаётся **failed native preparation,
exit 1**. Причина — eagerly evaluated diagnostic label $r.name на old_arms,
где правильное поле policy. Первая такая запись имеет index 49; их 33.
Исходная command, error/receipts и отсутствие её verification.json сохранены.
Ни исправления старого command, ни повторного native job не требуется.

При этом root принимает фактические успешные scoped fmt recheck, workspace fmt,
один strict CLI/all-target Clippy и разрешённую formatting правку. Root отдельно
завершил все post-format read-only preservation checks, на которых остановился
скрипт. Это новая независимая проверка, не переписывание native failure в PASS.

[Root verification](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-static-01.root-independent-verification.json):
39685 B / ad61a3957c8c87e3322143773c7e89b0aa8790c7ae452ca2f39dc474a9a489d9.
[Closed failure](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-static-01.root-closed-failure.json):
1706 B / 04607251d31365f78c96eeb50db52be543a138157575917fc8d73dc46cb075f7.

Root 691158 exit 0 проверил 38 records, 1016 file-row comparisons, все 487 actual
build inputs, 109 protected spans, prefix/suffix/death, 838 других canonical rows,
95 prior-unit files, 9 current images, 9 old archives и обе before-copies.
Полный formatter diff совпал с actual Git diff; прочитаны все 21 hunk:
переносы/отступы, допустимая trailing comma и стандартное сокращение closure block.
Предикаты, strings, helpers, timers и протокол не изменены.
Root a8c007 подтвердил AST/6 exact utilities/literal argv, 0620c3 — HEAD/index
и отсутствие живых Cargo/Rust/Kilogram процессов.

Root получил настоящий commandExecution exec-610d7135-ef56-4bab-8d74-79a1729a7f2a:
native exit 1, duration 36.067 s, полный output совпал с сохранёнными exec+wait
и stdout. Final cleanup/restoration/terminal/Finish true, secondary errors empty.
До-Finish snapshot не использован как окончательный результат.

Observed whole **40.4093324 s** учтён один раз отдельно; script 35.5613738 s.
Receive gap 30.4036995 s нарушил лимит 20 s на 10.4036995 s. Это сохранённое
процедурное отклонение; native work/whole guard соблюдён. Успешные компонентные
receipts и независимый root preservation позволяют идти дальше без повторного
fmt/Clippy. Следующий job опрашивать каждые 10 s, без анализа между polls.

Текущий controller **344341 B /
68fea9a6b25acfcd4960609271343e858eb81a58fc45d79230bd06cb5a9d5564**.
Принятый до форматирования b79bcd… сохранён отдельно и не объявляется compiled source.
Current owner4927 и manifest63e2 пока не покрывают текущие bytes.

## Одна compile-only подготовка

[root window](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-compile-01.root-window.json):
S-s53-r30-compile-01, work 270 s / whole 300 s / owned cleanup 5 s,
Jobs12 / BelowNormal / одна lane / retry 0. Отдельный actual preparation time,
C/F/P reserve/charge 0. Не повторять fmt или Clippy.

Разрешены fresh .tmp/migration-s53-r30-compile-01.ps1, records prefix
S-s53-r30-compile-01, manifest S5.3-r30-compile-expected-artifacts-01.json
и новый отчёт long-lived-dialog-a3-migration-s5-handoff-compile-readiness-r30.md.
Production/controller/shared scripts/metadata producers/adapters не изменять.
Новых archive copies не нужно: все девять current image mappings уже проверены.

Основа command — .tmp/migration-s53-r29l-reader-preparation-03.ps1,
32185 B / 7fd254cb1c77a3022af502593a44d9019a01fa9c85710ad31010c9972e90c68c.
Перенести без изменения шесть utility declarations, existing deadline/cleanup,
independent finally/restoration/terminal/Finish и compile/provenance algorithms.
Меняются id/paths/pins и входные схемы baseline, перечисленные ниже.
Не исполнять прежний main и не создавать новый reusable runner/guard.

### Проверить предпосылки до actual invocation

- AST без errors, exact шесть utilities, один вызов existing stable runner
  без Run/Filter/--list/IsolatedTestsFile; literal runner args, immediate exit
  capture до assert, независимый finally, fresh output paths, отсутствие иной lane.
- Прочитать все input records как данные и проверить поля каждого элемента,
  которые использует command, включая diagnostic payload/labels. Не переносить
  reader с безусловным name. Для heterogeneous protected spans labels:
  old_arms.policy; helpers/old_blocks.name. В новом canonical baseline обычные
  tuples используют только path/bytes/sha256.
- [Post-static baseline](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-post-static-baseline-01.json),
  204904 B / e82ee80ebb4dbbed807f40eb22269ff0bf98dbf1da688ad92e42c03abb07f3e3:
  canonical 839 с уже actual controller68fea; protected 22 без трёх живых indexes;
  prior_unit_files 95, old_preserved_images 9, existing_before_copies 2,
  current_images 9. Это новый explicit schema, не прежний r29k payload.
- Root обновляет три coordination indexes до dispatch. Добавить их новые tuples,
  текущее поручение/window/command и root verification/closure в protection.
  Исторические index pins в прежних records не переписывать.
- expected_build_inputs — уже фактический inputs-after r30, 781120 B /
  584f2e03153ec09e6849986418f73c684bf205dfc41275381d32bda0dd711dda.
  Здесь нет source comparison exception: все поля и 487 rows обязаны совпасть.
- Historical manifest для 501 unit identities — actual final r29:
  1279146 B / 63e2f05db985c2f6db210ae040af1196850ec0200b87c8be6f4e1b4351c85fdc.
  Его old source417c остаётся historical, не current.
- Все nine archive mappings из window проверять readback по bytes/SHA. Current
  owner4927 соответствует r30 pre-static archive, не owner160deb из r29l template.
  Старые A02/r29 archives, две r30 before-copies и 95 unit files также сохраняются.
  Mappings имеют reused=true; новых копий, overwrite/delete/move нет.
- Все относительные paths разрешать относительно workspace, абсолютные брать
  как есть. Не передавать абсолютный child в Join-Path с workspace.
  BuildEvidencePath для runner должен быть абсолютным и ещё не существовать.

Сохранить complete native command, его diff к template и SHA до запуска.
Не выполнять metadata/compiler при source preflight; только parse/schema/hash
и read-only checks. Исполнитель вправе выполнить один job после этого preflight
в рамках данного разрешения; отдельный contract-only callback не требуется.

### Единственный native job и доказательство сборки

Clock первым действием; arm 270/300 с вычитанием elapsed. Pinned pwsh из window,
login=false/native -File/exit propagation. Process-only offline=true и exact
presence/value restoration, NullString для ранее отсутствовавшей переменной.

Проверить before current source/evidence/images/archives и Get-KilogramFrozenBuildInputs
целиком против принятого actual inputs-after r30. Использовать unchanged
scripts/run-cargo-tests-stable.ps1 с этим common:

~~~powershell
$common=@{Package=@('kilogram-cli','kilogram-transport-iroh');CargoFeatures=@();CargoJobs=12;Profile='verification-scale';CargoTarget='x86_64-pc-windows-msvc';StableArtifactsDirectory=(Join-Path $w 'target/verification-scale/deps');SkipCacheStatus=$true}
& (Join-Path $w 'scripts/run-cargo-tests-stable.ps1') @common -BuildEvidencePath $capturePath *> (Join-Path $e "$prefix-capture.log")
$compileExit=$LASTEXITCODE
~~~

$compileExit и returned flag сохранить сразу, до assert/throw.
Runner компилирует с --no-run, сам переносит test outputs в прежние постоянные
stable EXE paths. Compiler/build-script children разрешены внутри owned job;
test/application/list execution не разрешены.

Из реальных raw compiler records восстановить Get-KilogramCompilerUnits,
сверить с capture.units, затем нормализовать Get-KilogramFrozenUnitIdentity.
Требуются те же 501 identities, 386 runtime units, profile opt3/debug_assertions/
overflow_checks и прежний resolved graph из 524 packages. Не угадывать freshness
или число rebuilt units: сохранить то, что действительно вернул Cargo.

Сверить шесть exact test roles, их compiler src_path/package/features/profile
и source/stable bytes/SHA. Отдельно найти настоящий normal CLI bin/test=false
artifact. Пять других stable roles, normal/fixed CLI и pinned old CLI должны
остаться byte-identical. Только process owner source/stable tuple может измениться.
Не требовать заранее новый hash и не объявлять reuse доказанным без actual capture.
При drift/диагностике — сохранить partials и stop без source repair или retry.

Создать один fresh expected-artifacts manifest <=4MiB со schema existing runner:
current build_inputs_after, actual units_identity, шесть actual stable_artifacts,
SHA capture. CreateNew/readback/existing assertion helpers.
Это compile manifest, ещё не final r30 binding/seal/admission.

Повторно проверить source/evidence/archives и actual after inputs, HEAD/index.
Finally независимо восстанавливает environment, завершает только owned children,
пишет terminal и вызывает Finish последним. Final native exit и stdout обязательны.
Прошедший pre-Finish snapshot не заменяет actual terminal outcome.

## Сдача

Readiness long-lived-dialog-a3-migration-s5-handoff-compile-readiness-r30.md:
полные invocation/exec+wait/UTC receipts, actual whole/compile times, source pins,
capture/log/raw units, six source/stable roles и normal CLI, freshness,
manifest и все preservation/restoration/cleanup/Finish results.

Во время job сохранять returns сразу и переходить к следующему poll без чтения
других файлов и анализа. Poll wait 10 s, максимальный receive gap 20 s.
Whole = first exec wall + first-return → terminal-return, а не сумма poll walls.
Actual native app duration root дополнительно проверит через read_thread.

Сохранять ошибки честно; не повторять job, не чинить Rust или invocation после
его выполнения, не расширять caps. Callback root и stop на artifact review.
Не запускать tests/--list/CLI/fixtures, новые prepare/freeze/adapters,
bind/admission/runtime, commit/push или S5.4. Private exhausted/root-gen511
данные и каталоги не читать и не перечислять.

Ledger unchanged: S5.3 C1433.7664909/F1441.9749663/P0;
combined C2021.2151018/F6158.2951152/P0; ceilings 1550/1600/0;
remaining C116.2335091/F158.0250337/P0; C/F/P неактивны.
11/12 control-loss cuts приняты в своих scopes. Последний runtime,
полная S5.3 и её публикация ещё открыты.
