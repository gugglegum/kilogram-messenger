# CS2: direct recoverable Runtime6 bootstrap

Новый entrypoint: `kilogram-bootstrap create-ordered2 --workspace-dir <new-path> --context <C0-hex64> <C1-hex64>`. IDs должны различаться; оба согласуются до создания устройств, как в CS1. Внутри создаются separate sibling directories `account-root` и `device`, настоящий Runtime6 store и публичные credential/certificate/authority/device-list exports. Peer/session не выдумываются до CS1 ceremony.

**Stdout этого entrypoint содержит recovery phrase.** Запускайте его из защищённого one-shot потребителя; не направляйте результат в обычный terminal, transcript, `tee`, log или evidence-файл. GUI-потребитель ещё не добавлен в CS2. Прежний `create` и его output type остаются отдельными.

## Воспроизводимая проверка без вывода secret stdout

На Windows нужны текущий bootstrap EXE по постоянному пути `target/clean-start-runtime/kilogram-bootstrap.exe` и принятый CS1 CLI `target/clean-start-runtime/kilogram-cli.exe`. Сборка — обычный locked/offline `cargo build -p kilogram-bootstrap --bin kilogram-bootstrap --profile verification --jobs 2`, process-local `CARGO_TARGET_DIR=target/s54-diagnostics-source-checks`; полученный EXE копируется в постоянный путь только после проверки отсутствия его запущенных экземпляров. В задаче применялись NativeToken, BelowNormal и existing bounded owner.

После compile-only запускайте Rust harnesses только через существующий stable runner, с конечным watchdog и свежими evidence:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target/s54-diagnostics-source-checks'
$env:CARGO_NET_OFFLINE = 'true'
$env:KILOGRAM_CS1_CHECKS = '1'
& ./scripts/run-cargo-tests-stable.ps1 `
  -Package kilogram-bootstrap,kilogram-bootstrap-contract,kilogram-cli `
  -Profile verification -CargoJobs 2 -Filter cs2_ -Run -NoCapture -SkipCacheStatus
```

Runner сначала компилирует без исполнения, затем запускает постоянные имена EXE. Здесь выполняются четыре focused tests, включая реальный CLI child и два новых аккаунта; их secret output не попадает в harness stdout. Проверка создаёт только синтетические аккаунты. Не считать её пользовательским UI для сохранения phrase. Конкретные конечные bounds/receipts текущего запуска находятся в `.tmp/clean-start-cs2`.

## Защищённый CLI consumer / API

Работающий потребитель находится в [cs2_bootstrap.rs](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/runtime2_process/cs2_bootstrap.rs:1). Его существенный pattern:

```rust
let mut child_output = command.output()?; // fixed, verified bootstrap EXE
let secret_stdout = Zeroizing::new(std::mem::take(&mut child_output.stdout));
ensure!(child_output.status.success(), "bootstrap failed; secret output omitted");
let result = Ordered2BootstrapOutput::decode(&secret_stdout)
    .map_err(|_| anyhow::anyhow!("invalid bootstrap result; secret output omitted"))?;
let public = result.public().clone();
// Hand result.recovery_phrase() only to the trusted first-run recovery UI.
// Log/export public metadata only; result and secret_stdout zeroize on drop.
```

Input cap —64KiB; result имеет отдельные version1/mode `ordered-dialog-v2`, `public`, `recovery_phrase` и `recovery_scope`. `Debug` редактирует phrase. Decode проверяет форму/версии, canonical IDs, distinct contexts, path roles и связь phrase→Account ID; parse errors не цитируют secret input. Если phrase забрана через `take_recovery_phrase`, она возвращается как `Zeroizing<String>`. GUI hand-off/backup UX — следующий этап, не реализация CS2.

Без subprocess доступен `kilogram_bootstrap::ordered2::create(new_path, [context0, context1]) -> Ordered2BootstrapOutput`. Bootstrap не зависит от CLI application; здесь используются существующие identity/state/protocol primitives.

## Совместимость с CS1

Из `public` используйте `account_id`, `device_id`, `claim`, `contexts`, `account_root_dir`, `state_dir`. Для публичных CS1 pins соответствие такое:

```text
account = public.account_id
device  = public.device_id
claim   = public.claim
contexts = public.contexts
```

CS1 `--credentials-file` получает **`public.credential_file` (`public/credential.bin`)**, содержащий настоящий RootVerifiedCredential bundle. `public.certificate_file` — отдельный bare certificate, его нельзя подставлять вместо bundle. `public.authority_file` — сохранённый signed authority floor; `public.device_list_file` — действительный Root-signed list для recovery/administration.

После bootstrap не выполняйте повторные `account-create`/`device-enroll` в этих directories. Подтвердите expected account/device/context pins, обменяйтесь только public bundle/snapshot/pins и выполните propose/cosign/import из [CS1 recipe](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs1-cli-recipe.md). Каждый signer использует свой `public.account_root_dir`, import — свой `public.state_dir` и owner/claim pins. Затем создайте Profile2 существующим `runtime-profile-create --profile-mode ordered-dialog-v2`. Этот путь реально проверен на обоих новых stores; оба runtime открылись и штатно завершились. Прежний CS1 transport smoke повторно целиком не запускался.

## Commit, failures и recovery scope

Workspace атомарно резервируется `create_dir` до secret creation; появившийся существующий target не заменяется. Затем `bootstrap-pending.json` помечает незавершённое создание. После проверки данных появляется public `bootstrap-candidate.json`; protected Root/repository закрываются и открываются заново по конечным путям. Только последняя no-clobber запись `bootstrap-ready.json` с `status=ready-runtime6` завершает bootstrap; после неё возвращается secret result. Нет перемещения live Root/repository или directory-replacing rename.

Обычная ошибка или остановка после reservation сохраняет incomplete workspace, который следующий `create-ordered2` отвергает. Если места/IO не хватило даже на pending marker, существующая папка всё равно не переиспользуется. Нет auto-resume, auto-reset, удаления или скрытой миграции. Сохраняйте неудачный путь для ручного разбора; новый пробный аккаунт создавайте по другому свежему пути. Наличие произвольного файла с именем ready не является разрешением переписать/использовать directory; creation API всегда отказывает на любом existing workspace.

Файлы/DB записываются существующими sync/transaction primitives, ready receipt — существующим `persist_noclobber`. Hardware power-loss durability и атомарная видимость всей папки как единого rename здесь не заявляются; commit point — корректный ready receipt плюс успешный API/CLI result. При потере stdout после commit папка может быть завершена, а потребитель — не получить phrase: автоматического повторного вывода phrase нет.

Phrase восстанавливает Root key/Account identity. Актуальная authority history требует отдельного recovery package и независимо сохранённого witness; история сообщений/секреты устройств имеют отдельные требования. Тест использует настоящий `AccountRootState::recover` с phrase+package+witness и проверяет тот же Account ID. Phrase сама по себе не обещает восстановление всей переписки.
