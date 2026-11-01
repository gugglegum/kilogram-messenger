# S5.3 r30: исходник принят; одна подготовка fmt и strict Clippy

2026-10-29. Root 01a164ec-589c-7b41-bc34-3ba158d1f22b.
Прежний исполнитель 01a1653c-0154-7611-8e8c-044fbe1e4d7a: GPT-6.1 Sol Max, без Fast.

## Приёмка исходника

[Readiness r30](long-lived-dialog-a3-migration-s5-handoff-integration-readiness-r30.md),
96268 B / b918dd2d9281950cfeaadca748dc2976fda1cc5c440f4dbd94d6d5270c25c213,
принят в пределах [контракта](long-lived-dialog-a3-migration-s5-handoff-committed-publication-contract-acceptance-r30.md).
Принятый controller: 343074 B /
b79bcd8501ff7ac4fdf8c3c9793f16f9ab08723e3efb4522160fdde7c7928029.
Содержательных исправлений не запрошено. Синтаксис, типы, lint, новые compiled
artifacts и фактический r30 runtime пока не проверены.

Root прочитал все 41 hunk (+696/−7 строк), новые callers/evidence и необходимые
неизменённые helpers. Проверены actual three-field marker, происхождение cold B
anchor и его сверка с C0 до повторного использования, B→A kill/wait до clear,
actual cold headers/partials, isolated B Active и A Selected, собственные Native
и старые views, последовательное обновление per-peer trackers, recovery и
доставка 400/401. Новые count assertions сверены с реальными started/note/stop,
структурой committed_cut_witness и trace_files: A3/B4, пять cold rows,
26 phase events, 14 разных полных logs. Runtime outcome этим не объявляется.

Root d676de exit 0 независимо проверил 838 остальных canonical rows, 9 EXEs,
3 current indexes, все 49 helpers / 33 old arms / 27 old blocks, 15 guard
contexts и exact prefix/suffix/death projection. Root b5c577 exit 0 подтвердил
совпадение полного actual Git diff с embedded diff: 59357 B /
78a9781eb95f8ade3f99fe63d41d901149df6616d8265a6db83c8b456357fb13.
Native git --no-index exit 1 означает наличие diff; stderr пуст.
Пять ссылок readiness и whitespace проверены.
[Независимый root proof](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-source-review-01.json)
фиксирует scope и pins. HEAD 64c2359788ead360314793e47741adacb83b8d4f, index пуст.

Отчёт исполнителя о полной обратной текстовой проекции сохранён как его
дополнительная проверка. Root самостоятельно проверил весь diff, перечисленные
защищённые участки и их контексты; это не заявление о выполненном Rust AST,
compiler или runtime proof.

## Следующая узкая задача

Разрешена одна static preparation S-s53-r30-static-01 по
[root window](long-lived-dialog-a3-migration-s5-handoff-evidence/S-s53-r30-static-01.root-window.json).
Work 150 s / whole 180 s / owned cleanup 5 s, Jobs12 / BelowNormal / одна lane,
retry 0. Это отдельное actual static preparation time; C/F/P reserve и charge = 0.
Пределы ресурсов взяты из принятого проектного режима параллелизма.

Сдать новый отчёт:
ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-static-readiness-r30.md.

Разрешены один command file .tmp/migration-s53-r30-static-01.ps1, свежие records
с prefix S-s53-r30-static-01 в handoff-evidence, две before-copies под
.tmp/migration-s53-r30-pre-static-01 и только механическое форматирование
принятого controller. Production и scripts/ не менять, не создавать пока
prepare/freeze-r30 или adapters. Readiness/proposal/contract/proof и прежние
records immutable. Три индекса обновляет root до dispatch; executor их не меняет.

### Подготовка invocation без выполнения прежних main

Использовать уже принятые шесть utility declarations, deadline/owned cleanup
и независимый finally/restoration/terminal/Finish из
.tmp/migration-s53-r29k-static-01.ps1,
29920 B / 37d38c945813cdb875995ca228f1b67022e5fb37c3cd1921ff26d07085b1466d.
Для ветки однократного formatter apply использовать существующий flow
.tmp/migration-s53-r29j-static-01.ps1,
31926 B / 921305b8ea74164a7f0145fdcbfc4d8524c63a411d2ddd9c044effb812fbff68.
Не исполнять старые main, не переносить r29k assertion repair и прежние
устаревшие source/adapter/index/manifest pins как текущие.

Адаптировать только id/paths/pins/current baseline и разрешённую formatter branch.
Шесть utility extents остаются точными. До запуска проверить AST без errors,
literal argv, calls/flags/exit gates/finally и отсутствие test/application calls.
Сохранить полный command source, diff к template и SHA.
Не создавать новый reusable runner/deadline guard.

Текущие pins берутся из root window / source proof: canonical 839,
controller b79bcd…, остальные 838 из r29n baseline, 9 current images из proof.
Case adapter теперь 39146 B / 45c98cace0875434368efd8fc0b3056add9a15659d5a3bc02727f05c2d50b0af;
старый r29k adapter 99ce… больше не current. Исторические hashes не переписывать.

### Единственный native static job

1. Использовать pinned pwsh из window, login=false, native -File с настоящим
   exit propagation. Clock стартует первым действием job; при arm 150/180
   вычитать уже elapsed. Проверить свежесть путей и отсутствие другого Cargo/test
   lane. Установить Jobs12/BelowNormal; не менять настройки компьютера/toolchain.
2. До форматирования CreateNew/readback сохранить две точные before-copies:
   принятый Rust b79bcd… и current stable owner 9863680 B /
   4927fa0de333c08732b20b414df2106d9923601c87a8a81e296ce485a6695d1d.
   Relative paths сохраняются под указанным новым root. Проверить containment
   и reparse ancestry; без overwrite, move или delete. Старые архивы не копировать
   заново и не перечислять private fixture directories.
3. Process-only CARGO_NET_OFFLINE=true с exact presence/value restoration.
   Получить actual Get-KilogramFrozenBuildInputs для verification-scale /
   x86_64-pc-windows-msvc, packages kilogram-cli и kilogram-transport-iroh,
   features empty. Сравнить целиком с deep copy build_inputs из r29 manifest:
   S5.3-r29-expected-artifacts-01.json, 1279146 B /
   63e2f05db985c2f6db210ae040af1196850ec0200b87c8be6f4e1b4351c85fdc.
   Единственное допустимое отличие — уникальная controller source_inputs row:
   old 417c4840… → accepted b79bcd850… . Остальные 486 source rows и все
   tool/environment/config/package/graph поля должны совпасть.
4. Зафиксировать actual selected rustfmt path/hash/version из pinned toolchain.
   Выполнить scoped --check. Если нужна только обычная правка форматирования,
   разрешён один вызов того же formatter без --check, затем один повторный check.
   Иная ошибка означает stop. После apply сохранить actual source hash и полный
   mechanical diff. Никаких ручных изменений predicates/helpers/timers или
   исправлений compiler/lint на этом шаге.
5. Выполнить workspace fmt --check, затем один strict CLI/all-targets Clippy.
   Full fresh logs и actual native exit каждой команды сохранять немедленно
   после её возврата, до assertions/throw. При failure последующие шаги проверки
   не запускать; finally/cleanup и truthful records обязательны.
6. Получить after build_inputs. Допустимо изменение hash только той же controller
   row при фактически выполненном rustfmt. Остальные поля точны. Старый manifest
   не редактировать и не объявлять сертификатом для новых bytes. Все 838 иных
   canonical rows, 9 current images, переданные root актуальные indexes и прежние
   evidence/archives остаются неизменными. Сохранить 49 helpers / 33 old arms /
   27 old blocks и prefix/suffix exact; механическое форматирование новых guards
   фиксируется diff и отдельно проверяется root.
7. Finally независимо восстанавливает environment (для отсутствовавших значений
   использовать [NullString]::Value), проверяет owned cleanup и пишет terminal.
   Guard остаётся armed через records; Finish последний. До-Finish snapshot
   не самостоятельный PASS. Зафиксировать final native stdout/exit и whole actual.
   При ошибке/timeout — stop, без retry, semantic repair или увеличения лимита.

Разрешены только compiler/check/build-script children самого Clippy. Не выполнять
cargo test, test EXEs, --list, CLI/fixtures, stable-runner artifact capture/staging,
новый expected-artifacts manifest, metadata producers, bind/freeze/admission или
runtime. Compile-only через stable runner будет отдельным следующим поручением.

### Literal argv

Scoped rustfmt:
--check --edition 2024 --config skip_children=true
apps/kilogram-cli/tests/support/migration_s53_handoff.rs

Workspace Cargo: ['fmt','--all','--','--check'].

Strict Cargo:
['clippy','--locked','--offline','--jobs','12','--profile','verification-scale',
'--target','x86_64-pc-windows-msvc','--package','kilogram-cli','--all-targets',
'--','-D','warnings'].

Передавать literal arrays; разделитель -- перед -D warnings не терять.
Ни workspace formatter apply, ни lint suppression не разрешены.

## Сдача и остановка

В readiness: full command source/diff/pins, before-copy mappings, actual
formatter/tool/Clippy results, полный formatting diff, inputs before/after,
preservation, environment/cleanup/Finish, native receipts и actual elapsed.
Сохранить exec/wait returns и receive timestamps до анализа. Во время job
polls не реже 20 s; whole считать как first exec wall плюс first-return →
terminal-return, сопоставив с native app duration и script timer, не суммой polls.

После callback в root 01a164ec-589c-7b41-bc34-3ba158d1f22b остановиться для
static/diff review. Commit/push и S5.4 пока не начинать.
Private exhausted/root-gen511 данные не читать и не перечислять.

S5.3 actual C1433.7664909/F1441.9749663/P0; combined C2021.2151018/F6158.2951152/P0.
Ceilings 1550/1600/0; remaining C116.2335091/F158.0250337/P0.
Активных C/F/P резервов нет, ceilings не меняются. 11/12 cuts приняты в прежних
scopes; последний runtime, полная S5.3 и её публикация ещё впереди.
