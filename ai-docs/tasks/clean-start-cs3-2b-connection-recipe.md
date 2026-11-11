# CS3.2b: повтор проверки локального подключения на Windows

[Контракт](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-2b-connection-contract.md) и [принятые уточнения](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-2b-contract-review-01.md). Два синтетических аккаунта, один ПК, DirectOnly `127.0.0.1:0`. Для приёмки нужны две текущие Lease2 у **каждого** процесса, штатное завершение и повторное подключение после reopen. Переписка в этом сценарии не используется.

Для проверки исправлений из [implementation review 01](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-2b-implementation-review-01.md) запустить целевые фильтры `cs32b_owned_start_failure_and_cancel_preserve_setup` для `kilogram-windows` и `cs32b_cli_runtime_refusal_preserves_message_and_strict_reply` для `kilogram-cli` через stable runner. Первый тест включает фактический сбой дочернего CLI и управляемую задержку наблюдения его выхода. Второй проверяет исходный текст отказа и отклонение неправильного вида, версии, opcode и формы ответа. Выполнить шаги 1–4 ниже на новом образе, затем штатно остановить один runtime и закрыть оба окна. При подтверждённом соединении после импорта отображается **Both locator imports were acknowledged.**

## Подготовка и focused checks

Запускать из `E:/AI-Projects/kilogram-p2p-messenger`. Брать проверяемый исходный состав из exact candidate. Нужен локальный locked Cargo cache; рабочий профиль `verification`. На Windows: компиляция **Idle / Jobs1**, test harnesses компилируются без запуска, выполняются только под постоянными именами через stable runner. Для loopback/DPAPI использовать разрешённый NativeToken и ограниченный owner. В этом исполнении owner900/930s для check/build/test, UI600/630s. Product IPC/start/graceful-exit/cleanup — 5/30/10/5s.

```powershell
. .\scripts\cargo-resource-policy.ps1
$null = Set-KilogramCargoResourcePolicy -RequestedJobs 1
$env:CARGO_NET_OFFLINE = 'true'
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target/s54-diagnostics-source-checks'
cargo clippy --locked --offline --jobs 1 -p kilogram-windows -p kilogram-cli -p kilogram-runtime-ipc --all-targets --profile verification -- -D warnings
cargo build --locked --offline --jobs 1 -p kilogram-windows -p kilogram-cli --bin kilogram-windows --bin kilogram-cli --profile verification
```

Перед заменой фиксированных тестовых `target/clean-start-runtime/kilogram-windows.exe` и `kilogram-cli.exe` проверить отсутствие экземпляров по этим точным путям. Сохранить прежние образы в свежий каталог evidence и записать SHA256 до и после копирования. `kilogram-bootstrap.exe` — ранее принятый CS2 helper, SHA256 `baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4`. Путь EXE не содержит SHA или номера попытки. Actual build/copy: `.tmp/clean-start-cs3-2b/build02.ps1` и receipt рядом.

```powershell
$runner = Join-Path $PWD 'scripts/run-cargo-tests-stable.ps1'
$pkg = @('kilogram-windows','kilogram-cli','kilogram-runtime-ipc')
& $runner -Package $pkg -Filter cs32b_ -Profile verification -CargoJobs 1 -SkipCacheStatus
& $runner -Package $pkg -Filter cs32b_ -Profile verification -CargoJobs 1 -Run -NoCapture -SkipCacheStatus
```

`cs32b_actual_independent_connect_restart_and_failures` сам создаёт новые pair directories, проводит реальную независимую ceremony и восстанавливает отсутствующие slots через тот же Start, что UI. Сверить `CS32B_LIVE_PASS`: actual4 current Active2, частичный locator commit и повтор с единственным недостающим commit, те же profile/consents, обновлённые revisions, stop/descriptor conflict/forced outcome. Диагностика `CS32B_ACTIVE_PROGRESS` печатает age от начала запросов. `active=true, lease=false` после restart — ожидаемая промежуточная стадия. Для проверки compatibility отдельно применить фильтры `cs31_`, `cs32a_partial_output_conflict_ambiguous_import_and_expiry` и `old_instance_does_not_remove_replacement_descriptor`; для старых fixtures установить process-local `KILOGRAM_CS_TEST_FIXTURE_ROOT` в свежий подкаталог `.tmp/clean-start-cs3-2b`. Старые данные из CS3.1/CS3.2a не использовать.

Подготовить пару через stable runner с `-Filter cs32b_prepare_ui_fixture -Run` и `$env:KILOGRAM_CS32B_UI_FIXTURE='1'` в дочернем запуске. Helper выполняет независимую церемонию и устанавливает два согласия у каждого участника. Для свежей пары задать `KILOGRAM_CS32B_UI_FIXTURE_TAG` с уникальным именем из латинских букв, цифр и дефиса длиной до 32 символов. Индекс `ui-fixture-<tag>.json` создаётся без перезаписи. Тот же tag передать наблюдателю из шага 4. Без tag используется исходный `ui-fixture.json`. Прежние индексы, пары и результаты сохраняются.

## Два окна

1. Запустить фиксированный Windows EXE по одному разу для каждого setup из fixture, с `--fresh-home <home> --bootstrap-exe <fixed helper> --runtime-exe <fixed CLI>`. Фактическая проверка использовала `ui06.ps1`/`ui07.ps1` для первого запуска и `ui08.ps1`/`ui09.ps1` для повторного открытия. Для воспроизведения взять новые имена шагов. `--runtime-profile`/`--ipc-file` указывают на отдельные заведомо неиспользуемые legacy paths для проверки, что свежий режим их не создаёт. Фраза уже подтверждена в synthetic fixture. Не раскрывать и не снимать её.
2. В обоих окнах нажать **Start local connection**. Worker повторно проверяет и импортирует exact retained finals, создаёт либо валидирует выбранный Profile2 и наблюдает owned child. До обмена файлами нет connected badge. При profile conflict исходный файл остаётся; **Retry with a new local profile file** создаёт другой локальный artifact с теми же pins. При занятом store остановить его реального владельца; приложение не присоединяет чужой процесс.
3. В каждом окне нажать **Export current public locators**. Передать **только** показанный public JSON другому окну, указать его путь и нажать **Inspect and import both peer locators**. Export свежий для каждого runtime run. При частичном результате видно два исхода; **Retry retained peer locators** повторяет те же bytes. Отдельной сетевой команды Connect/Sync2 нет: canonical role0 сам dial, role1 принимает.
4. Наблюдать **Connected locally - authenticated Active2 on both slots** в обоих окнах. При наличии подготовленного индекса дополнительная read-only проверка: `-Filter cs32b_observe_current_ui_runtimes -Run` с уникальным `KILOGRAM_CS32B_UI_OBSERVATION`, например `first-current`. Она пишет две актуальные typed status проекции в `observations/first-current.json`, проверяя signed descriptor и context pins; сопоставить runtime PID с родительским PID соответствующего GUI из native process snapshot. Не сохранять descriptor/token в отчёт.
5. Нажать **Stop local connection** в одном окне. Увидеть подтверждение graceful stop и снятие отметки подключения во втором окне. Закрыть окна через Alt+F4 и сверить сохранённый `graceful-stop` и удаление IPC descriptor у обоих. Открыть те же setup в новых GUI процессах и явно нажать Start. Сверить прежние Profile/claim/account/device/contexts/finals. До обмена новыми локаторами окна показывают **Connection not currently confirmed**. Повторить обмен, подтвердить новые runtime instances, рост ревизий и четыре текущих Lease2 в read-only observation под новым именем. После закрытия обоих окон сверить exit0/EOF, сохранённый graceful outcome и отсутствие собственных процессов.

Каждой попытке — собственные invocation/receipt/stdout/stderr и свежие пути. `child=0, owner=1` при forced residual cleanup не является чистой попыткой; записать оба кода и повторять только обоснованно. Не менять IPC/start/stop deadlines ради pass, не перезапускать исторические migration/endurance/legacy кампании. Сверить actual source/image pins и не включать evidence directories в публикацию.
