# CS3.2a: взаимное согласие из Windows UI

Этот этап продолжает существующий first-dialog pilot после сохранения recovery words и подтверждения первого peer. Он подписывает и устанавливает существующие CS1 contexts локально. Подключения к сети, Profile2 и сообщений здесь ещё нет.

## Действия двух пользователей

Запустите согласованные UI/CLI/helper по постоянным путям. Обычно executables лежат рядом; явные пути передаются обычными desktop options:

```powershell
$fixed = Join-Path $PWD 'target/clean-start-runtime'
& (Join-Path $fixed 'kilogram-windows.exe') `
  --fresh-home 'E:\path\to\your-existing-setup' `
  --bootstrap-exe (Join-Path $fixed 'kilogram-bootstrap.exe') `
  --runtime-exe (Join-Path $fixed 'kilogram-cli.exe')
```

Откройте тот же setup, который уже создали через CS3.1. Никакого нового аккаунта создавать не нужно. Должны быть сохранены recovery acknowledgement и подтверждённый peer. Старый `setup.json` без ceremony state автоматически означает «согласие ещё не начато».

1. UI сам определяет, кто создаёт предложение, по меньшему проверенному device ID. Это может быть как автор приглашения, так и присоединившийся пользователь; роли вручную не назначаются.
2. У соответствующего пользователя доступно **Create or resume consent proposal**. Действие означает разрешение первого диалога на фиксированный срок до30 дней. Время начала, TTL и exact public inputs сохраняются до первой подписи; оставшийся срок показывается в UI. Повтор не продлевает разрешение.
3. После успешной подготовки двух permissions нажмите **Export consent proposal**. UI покажет публичный файл `SETUP/exchange/proposal.kilogram`. Передайте только его peer.
4. Второй пользователь в разделе **Mutual consent** указывает полученный файл и нажимает **Inspect and keep this consent file**. Проверяются адресат, invitation, ordered contexts, frozen identities/authority и обе публичные proposals. Затем явно нажмите **Verify and sign both permissions**. Подпись выполняется только вашим собственным локальным Root через CS1 CLI.
5. Нажмите **Export signed consent**. Файл `SETUP/exchange/final.kilogram` содержит ровно два прежних CS1 final artifacts, а не новый wire format. Верните его proposer.
6. Signer может нажать **Install or recheck both local permissions** для собственной установки. Proposer сначала выбирает полученный final через **Inspect and keep this consent file**, затем выполняет ту же локальную установку.
7. Честный итог каждого пользователя: **Mutual consent installed locally; connection is the next step**. Это не подтверждает импорт на стороне peer и не означает Active2/сетевую сессию. Connect/send остаются недоступны.

Invite/reply остались прежней формы, без optional proposals. Root directory, local claim, IPC descriptors/tokens и recovery words не передаются; remote paths/commands из файлов не исполняются. Каждая сторона сверяет peer identity независимо, как в CS3.1. Нельзя подменить выбранного peer или «исправить» несовпадение созданием ещё одного аккаунта.

## Повтор, остановка и reopen

Два слота не являются общей атомарной транзакцией. UI показывает результат signature/import каждого permission отдельно. Если операция остановилась после одного успешного шага, он сохраняется. Не удаляйте папки и не переписывайте outputs.

- Ошибка signer может оставить пустой output: CLI резервирует имя до подписи. Наличие файла не означает успех. Повтор создаёт новое собственное имя и использует прежние inputs/time; неудачные файлы сохраняются.
- Если импорт мог завершиться, но ответ потерян, результат остаётся uncertain. **Install or recheck both local permissions** повторяет exact finals через существующий идемпотентный импорт. Он не заменяет другое установленное согласие.
- На reopen проверяются сохранённые references/digests, public bindings и signatures. Старые acknowledged import flags показываются как прошлый результат, а не как новая проверка DB. Явный recheck подтверждает установку заново, без нового commit для byte-identical installed contexts.
- При directory-lock conflict остановите использующий этот state runtime и повторите то же действие. UI не завершает посторонние процессы и не сбрасывает store.
- Expired/future-outside-skew или конфликтующий artifact отвергается до нового импорта/подписи. Старые файлы/установленные contexts сохраняются, но UI не называет их действующим разрешением. Автоматического renewal здесь нет.

Cancel/закрытие UI отменяет и дожидается только собственного helper. Per-command watchdog60s и bounded cleanup5s, stdout/stderr capped независимо64KiB, ошибки не цитируют helper output. После потерянного результата снова откройте тот же setup и используйте явный Resume/Recheck.

Изменение роли, authority/recovery, произвольные контакты, remote/volunteer storage и messaging требуют следующих отдельных этапов. После новых Root membership действий актуальный recovery package/witness по-прежнему сохраняется отдельным существующим recovery export; final consent не является backup переписки.

## Focused verification

По просьбе пользователя новые сборки используют **Idle / Jobs1**; системные настройки не меняются. Existing stable runner компилирует Rust harnesses без исполнения, затем запускает fixed-name EXE. Используйте finite owner, fresh evidence и актуальные fixed UI/CLI/bootstrap images; источник scoped scripts — `.tmp/clean-start-cs3-2a`.

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target/s54-diagnostics-source-checks'
$env:CARGO_NET_OFFLINE = 'true'
# Parent процесса компиляции должен иметь Idle priority.
cargo clippy --locked --offline --jobs 1 -p kilogram-windows -p kilogram-cli `
  -p kilogram-protocol --all-targets --profile verification -- -D warnings
& ./scripts/run-cargo-tests-stable.ps1 -Package kilogram-windows,kilogram-cli `
  -Filter fresh_onboarding -Profile verification -CargoJobs 1 -SkipCacheStatus -Run -NoCapture
& ./scripts/run-cargo-tests-stable.ps1 -Package kilogram-windows,kilogram-cli `
  -Filter cs1_fresh_ -Profile verification -CargoJobs 1 -SkipCacheStatus -Run -NoCapture
```

Сохраняйте одинаковый набор пакетов compile-only/run, чтобы не вызывать лишние пересборки из-за feature unification. Проверки создают fresh synthetic CS2 аккаунты; signer работает при недоступном peer Root path. Фактические проверки двух слотов/идемпотентности читают только собственные новые test repositories. Слова не выводятся и не снимаются на скриншоты. Полные crypto/endurance/migration matrices не повторяются.
