# CS1: независимая Fresh Consent2 через CLI

Рецепт для двух новых CLI-профилей. Каждый пользователь хранит **только свой Root**; обмен — credentials, authority snapshots, ожидаемые публичные account/device pins, proposal/final context и Locator2. Account/device pins подтверждаются отдельно от недоверенного proposal. Desktop/defaults не изменены. Текущий Runtime6 поддерживает два immutable context slots и один peer pair.

Ниже `$cli` — собранный текущий CLI по постоянному пути, например `E:\AI-Projects\kilogram-p2p-messenger\target\clean-start-runtime\kilogram-cli.exe`. Вызывайте каждую команду с проверкой `$LASTEXITCODE`; продолжать после ошибки нельзя. Все parent directories публичных outputs должны существовать. Не используйте реальные старые state directories.

## 1. Согласовать context IDs до enrollment

Один участник генерирует два новых случайных ID и передаёт их второму публичным файлом:

```powershell
$contexts = 0..1 | ForEach-Object {
    [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
}
$contexts | ConvertTo-Json | Set-Content ./contexts.json -Encoding utf8
```

Каждый участник на своём устройстве задаёт отдельные пути `$root`, `$state`, `$public` и `$profile` (Root вне state; public/profile/IPC/locators также вне state). Создаёт только родительские/public directories, затем:

```powershell
$contexts = Get-Content ./contexts.json -Raw | ConvertFrom-Json
& $cli account-create --account-dir $root
if ($LASTEXITCODE) { throw 'account-create failed' }
$lines = & $cli device-enroll --profile-mode ordered-dialog-v2 `
    --account-dir $root --state-dir $state `
    --ordered-context $contexts[0] --ordered-context $contexts[1] `
    --certificate-file "$public/own.credential"
if ($LASTEXITCODE) { throw 'device-enroll failed' }
$own = $lines[-1] | ConvertFrom-Json
$own | ConvertTo-Json | Set-Content "$public/own.pins.json" -Encoding utf8
& $cli account-snapshot --account-dir $root --snapshot-file "$public/own.authority"
if ($LASTEXITCODE) { throw 'account-snapshot failed' }
```

`account-create` — существующий development CLI, а не новый recoverable desktop onboarding. CS1 его не меняет. Claim/account/device из последней enrollment JSON-строки нужны своему profile/import; это публичные pins, не Root secret.

Оба обмениваются `own.credential`, `own.authority`, `own.pins.json`, сохраняя полученное как `peer.*`. Root directory, recovery phrase и device secrets не передаются. `$peer` загружается из проверенного `peer.pins.json`. Authority floor — сохранённый доверенный публичный snapshot; абсолютную «последнюю в мире» ревизию offline-команда установить не может. Credentials должны включать current authority своего подписанта и не быть ниже отдельно сохранённых floors. Stale/revoked credentials отклоняются; подписи и проверки не обходить.

## 2. Role0 предлагает, role1 проверяет и подписывает

Canonical role0 — меньший device ID (32 bytes; lowercase fixed-width hex сравнивается ordinal). Это правило прежнего membership/Consent2, а не выбор инициатора по имени пользователя. Role1 при попытке propose получает ошибку.

```powershell
$peer = Get-Content "$public/peer.pins.json" -Raw | ConvertFrom-Json
$role0 = [StringComparer]::Ordinal.Compare($own.device, $peer.device) -lt 0
$notBefore = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
```

Для каждого `$c` из `$contexts` **только role0** выполняет:

```powershell
& $cli ordered context2-propose --account-dir $root `
    --expect-account $own.account --expect-peer-account $peer.account `
    --expect-device $own.device --expect-peer-device $peer.device --context $c `
    --credentials-file "$public/own.credential" "$public/peer.credential" `
    --minimum-authority-file "$public/own.authority" "$public/peer.authority" `
    --not-before $notBefore --valid-for-seconds 2592000 `
    --output-file "$public/$c.proposal"
if ($LASTEXITCODE) { throw 'context2-propose failed' }
```

Передать `$c.proposal` role1. **Только role1** выполняет со своими `$root/$own/$peer`:

```powershell
& $cli ordered context2-cosign --account-dir $root `
    --expect-account $own.account --expect-peer-account $peer.account `
    --expect-device $own.device --expect-peer-device $peer.device --context $c `
    --credentials-file "$public/own.credential" "$public/peer.credential" `
    --minimum-authority-file "$public/own.authority" "$public/peer.authority" `
    --proposal-file "$public/$c.proposal" --output-file "$public/$c.context"
if ($LASTEXITCODE) { throw 'context2-cosign failed' }
```

Публичные credentials/floors могут идти в любом порядке; команды сами устанавливают canonical роли. Proposal запечатывает точные credentials, membership, Fresh body, validity и первую подпись. Final context имеет **старый неизменённый** wire-format с обеими Consent2-подписями. Сам proposal не импортируется.

Передать final `$c.context` обратно role0. Оба импортируют в собственный свежий store:

```powershell
& $cli ordered context2-import --state-dir $state `
    --expect-account $own.account --expect-claim $own.claim `
    --invitation-file "$public/$c.context"
if ($LASTEXITCODE) { throw 'context2-import failed' }
```

Output всегда no-clobber. Имя резервируется **до** подписания/membership mutation. Ошибка может оставить пустой/неполный публичный output: сохраните его как evidence и повторите с новым именем. Для byte-identical propose retry сохраняйте все аргументы, включая `--not-before`/TTL и публичные bytes. Уже созданный membership той же пары проверяется и переиспользуется; иной membership не заменяется. Если публикация сорвалась после создания корректного membership, retry не требует его удаления. Нельзя менять часы/сроки ради pass.

## 3. Запуск проверенного local two-process сценария

Проверенный smoke использует один компьютер, два независимых Root/state и loopback DirectOnly. На каждом профиле:

```powershell
& $cli runtime-profile-create --profile-mode ordered-dialog-v2 `
    --state-dir $state --profile-file $profile `
    --expect-account $own.account --expect-claim $own.claim --expect-device $own.device `
    --ordered-context $contexts[0] --ordered-context $contexts[1] `
    --ipc-file "$public/runtime.ipc.json" --locator-directory "$public/locators" `
    --route-policy direct-only --ordered-bind 127.0.0.1:0
if ($LASTEXITCODE) { throw 'runtime-profile-create failed' }
& $cli runtime-from-profile --profile-file $profile
```

Последняя команда держит runtime; команды ниже — из второго terminal для этого пользователя. После публикации Locator2 обменять `locators/<context>.locator` и для каждого slot 0/1:

```powershell
& $cli ordered locator2-import --ipc-file "$public/runtime.ipc.json" `
    --slot 0 --locator-file "$public/peer-slot0.locator"
& $cli ordered status2 --ipc-file "$public/runtime.ipc.json"
# После Active2 у обоих:
& $cli ordered send --ipc-file "$public/runtime.ipc.json" --slot 0 `
    --request-id 00000000000000000000000000000001 --text 'Привет от первого пользователя'
& $cli ordered history2 --ipc-file "$public/runtime.ipc.json" --slot 0
```

Второй участник отправляет собственное сообщение со своим устойчивым request ID; повтор логического send использует тот же ID. Пример показывает slot0; для slot1 выполняются те же команды с другим slot/ID. Для разных компьютеров нужны выбранные actual bind/route/relay и публичные Locator2; loopback-рецепт не доказывает внешний connectivity. CS1 не меняет transport policy. Завершить через `ordered shutdown --ipc-file ...`.
