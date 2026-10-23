# A1 durable ingress — repair iteration 2

2026-10-23. HEAD `3e82e6380c30d0f9443b6692ea34faccaacddef7` + рабочий diff.
Основание: [review итерации 1](lossless-sync-a1-integration-review-1.md),
первый согласованный slice из [F1/F2 review](lossless-sync-a1-spike-review.md).
Прямое одобрение реализации и repair/review cycles проверено ранее по
userMessage `01a1657e-b03f-7d01-aaf0-e5af979125fd` в чате оркестратора.

Статус: I1–I4 исправлены; быстрые regressions, финальные gates и единственный
256-message acceptance этой repair-итерации прошли. Последующее независимое
[review приняло этот узкий slice](lossless-sync-a1-integration-review-2.md).
A1 целиком остаётся открыт; ниже сохранены отчёт исполнителя и границы evidence.

После приёмки пользователь поручил commit/push. Принятая реализация сохранена как
`dcc1300b759d45b0dfbb77e08b93d2cac0957b80`; source hashes совпали с environment.json.
HEAD и dirty-tree statements в logs ниже относятся к моменту выполнения проверок.
Архивные export scripts рассчитаны на исходное pre-commit working tree; сохранённые
patches не нужно перезаписывать. Diff сохранённой реализации можно получить так:

```powershell
git diff --binary 3e82e6380c30d0f9443b6692ea34faccaacddef7 dcc1300b759d45b0dfbb77e08b93d2cac0957b80 -- apps/kilogram-cli/src/main.rs apps/kilogram-cli/src/ordered_ingress.rs apps/kilogram-cli/src/ordered_ingress_tests.rs crates/kilogram-ratchet/src/lib.rs
```

[Отчёт итерации 1](lossless-sync-a1-integration-iteration1.md) и её
[evidence](lossless-sync-a1-integration-evidence/) сохранены. Текущие evidence:
[`lossless-sync-a1-integration-iteration2-evidence/`](lossless-sync-a1-integration-iteration2-evidence/).
Один [полный reviewable.patch](lossless-sync-a1-integration-iteration2-evidence/reviewable.patch)
содержит все четыре исходных файла, включая untracked ingress/test files.
Полный diff от HEAD: 2341 insertions / 23 deletions.
Manifests/lock, wire/default, GUI, migration и другие milestones не менялись.
SHA-256 полного patch: `d4bccd100fba4d89f19ff44c159bf37b252627a05633e1ab8b9a4fad9cb6e359`.
Toolchain, исходники, manifests/lock и stable EXE записаны в
[environment.json](lossless-sync-a1-integration-iteration2-evidence/environment.json).

## I1 — typed failure classification и rollback

`RatchetError::is_invalid_ordered_input` — явный allowlist детерминированных
ошибок ciphertext/crypto input. Native I/O, pickle/record/encoding failures,
missing keys, maintenance и неизвестные варианты не считаются bad input.
Ошибка такого класса выходит из `run_state_transaction`, откатывая и более
ранний prefix этого же drain. Строки Display не участвуют в классификации.

Local trust load/decode (`current_peer_authority`) вынесен из pure input
validation и вызывается с `?` внутри транзакции. Чистая проверка авторизации
по успешно загруженному pinned snapshot возвращает типизированный IdentityError;
только её input rejection может быть quarantined. Bad-MAC S1 по-прежнему
коммитит корректный prefix вместе с durable quarantine.

Два новых Windows regressions используют настоящие session/vault paths:

- account становится read-only после загрузки ratchet workspace. Session save
  реально происходит; следующая atomic account replacement получает отказ.
  Fault снимается до rollback. После reopen — native digest/generation pre-state,
  точные pending bytes, нет quarantine/readable/ACK; retry читает один раз;
- на втором pending входе Windows handle запрещает share-read `state-vault.key`
  при настоящем открытии trust repository. Уже обработанный K0 и projection
  откатываются вместе с ratchet. После reopen оба pending остаются; retry даёт
  ровно два text и два ACK. Это не synthetic bad-author result.

На старой логике оба возвращали `Ok(...Quarantined...)` и ловились regression:
[red-cli-2.log](lossless-sync-a1-integration-iteration2-evidence/red-cli-2.log).
Unit classifier дополнительно проверяет, что текст `invalid MAC` в I/O не
меняет класс, а missing message/prekey и local record failures требуют rollback.

## I2 — явный известный формат account

Все `persist_account` получают `ordered_access` из успешно загруженного
RatchetState; создание обычного account явно выбирает false. Повторного
best-effort чтения disk prefix при save нет. Разрешённый save защищённого
account всегда пишет prefix, включая обновление account/prekeys после genesis.
Обычный loader и cached legacy encrypt/decrypt продолжают отказывать.

Regression наблюдает настоящий Windows sharing violation 32 при чтении account.
Замена открытого target через MoveFileEx на этой машине сама отказывает; этот
вариант не выдаётся за воспроизведение успешного downgrade. Для разрешённой
replacement проверяется явно выбранный формат: уже загруженный protected account
сохраняется при unreadable UTF-8 disk tag (I/O InvalidData). На старом коде save
проходил, но `fence_retained=false`:
[red-ratchet-2.log](lossless-sync-a1-integration-iteration2-evidence/red-ratchet-2.log).
После исправления prefix сохранён, ordered reopen работает, legacy
load/encrypt/decrypt отвергаются; десять ordinary ratchet tests также проходят.

## I3 — head расходует byte reserve

Admission головы требует только hard pending byte cap. Future admission
по-прежнему оставляет MAX_EVENT reserve; сумма проверяется с checked arithmetic.
Slot reserve и общий full-commit/staging budget не изменены.

Regression: cap 9323, future 1131, head 1099 bytes. Future сохранён; лишний
ordinary admission заблокирован. При cap на байт меньше future+head голова
отклоняется с сохранением accepted bytes; после возврата cap head помещается
в reserve и даёт readable prefix 2. Следующие futures снова учитывают reserve.
Старое поведение отклоняло помещающуюся голову с Backpressure:
[red-cli-2.log](lossless-sync-a1-integration-iteration2-evidence/red-cli-2.log).
Это отдельная проверка от S3 transient/full-history exhaustion.

## I4 — runtime-from-profile отказывает до generic preparation

Проверка account fence находится внутри существующего `with_locked_state`:
после StateDirectoryLock и vault/shadow recovery, до directory preparation,
`prepare_runtime_listener` и authority writes. Порядок ordinary directory/listener
preparation сохранён внутри того же lock. Endpoint до отказа не создаётся.

Regression вызывает настоящий dispatch `Command::RuntimeFromProfile` с валидным
более новым own device-list. Проверяет generation/snapshot/authority и отсутствие
runtime artifacts. Два варианта: tag уже существует; tag отсутствует в shadow,
но восстановлен настоящим committed-primary/unconfirmed-shadow recovery.
Generation остаётся 2→2 / 3→3, authority не меняется, discovery directory не
создаётся. Старый путь менял generation 2→3 и authority:
[red-i4.log](lossless-sync-a1-integration-iteration2-evidence/red-i4.log).

Существующий ordinary runtime profile startup/shutdown test прошёл через
stable EXE, 1/1, 4.99 s:
[runtime-unfenced-unsandboxed.log](lossless-sync-a1-integration-iteration2-evidence/runtime-unfenced-unsandboxed.log).
Первая sandbox-попытка дошла до runtime IPC и получила os error 10013;
повтор со снятым sandbox прошёл. Это только synthetic loopback IPC, не two-host
network acceptance. Protected regression endpoint не запускает.

## Проверки и воспроизведение

Все Windows harnesses компилируются `--no-run`, выполняются только через
`scripts/run-cargo-tests-stable.ps1` и постоянные EXE. Child workers используют
тот же `current_exe()`. Builds: jobs=2, locked/offline, BelowNormal.

```powershell
$env:CARGO_NET_OFFLINE='true'
$env:KILOGRAM_CARGO_JOBS='2'
$env:TEMP=(Resolve-Path -LiteralPath '.tmp').Path
$env:TMP=$env:TEMP
& .\scripts\run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter ordered_ingress -SkipTest ordered_ingress_256_signed_hash_pages_process_restart_history -CargoJobs 2 -Run -NoCapture
& .\scripts\run-cargo-tests-stable.ps1 -Package kilogram-ratchet -CargoJobs 2 -Run -NoCapture
& .\scripts\run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter runtime_launch_profile_starts_and_stops_through_authenticated_ipc -CargoJobs 2 -Run -NoCapture
& .\scripts\verify-kilogram-m1-readiness.ps1 -AcceptedTag M1.0.0
& .\scripts\run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter ordered_ingress_256_signed_hash_pages_process_restart_history -CargoJobs 2 -Run -NoCapture
```

| Финальная проверка | Результат / evidence |
|---|---|
| 256 exact plaintext / hash pages / process restart | 1 parent + 5 child phases passed, 0 failed; 1095.55 s; [acceptance-256.log](lossless-sync-a1-integration-iteration2-evidence/acceptance-256.log). |
| Новые и прежние CLI negative/reopen | 8 passed, 0 failed, 1 worker ignored; 78.88 s; [fast-cli-verified.log](lossless-sync-a1-integration-iteration2-evidence/fast-cli-verified.log). |
| Ratchet | 12 passed, 0 failed; 8.50 s; [ratchet-verified.log](lossless-sync-a1-integration-iteration2-evidence/ratchet-verified.log). |
| Ordinary runtime profile | 1 passed; 4.99 s; [log](lossless-sync-a1-integration-iteration2-evidence/runtime-unfenced-unsandboxed.log). |
| M1 readiness `-AcceptedTag M1.0.0` | exit 0; 188/188 boundaries, rustfmt, workspace/all-targets/all-features locked/offline strict Clippy; [readiness-final.log](lossless-sync-a1-integration-iteration2-evidence/readiness-final.log). Mutation harnesses только проверены компилятором. |
| Единый diff | Reverse check exit 0; [patch-check.log](lossless-sync-a1-integration-iteration2-evidence/patch-check.log), [export script](lossless-sync-a1-integration-iteration2-evidence/export-reviewable-patch.ps1). |
| Финальная сверка | Global git diff --check, reverse/whitespace check patch и SHA-256 всех sources/manifests/stable EXE прошли; [final-checks.log](lossless-sync-a1-integration-iteration2-evidence/final-checks.log). |

Это единственный полный 256-test после окончания code changes в итерации 2.
Frontiers четырёх страниц: `0 → 0 → 0 → 256`; пятый child process прочитал
256 exact plaintext через `collect_runtime_history_page`, pending=0. Child
phases: 119.22 / 120.98 / 120.73 / 355.91 / 204.86 s. Total 1095.55 s
против 1105.00 s финального прогона итерации 1 — только функциональные измерения
на этой машине, без заявления product performance acceptance.

Промежуточные logs сохранены. `red-cli.log` — исправленная ошибка выбора
одноимённого run_command в самом новом тесте. Первый `red-ratchet.log` —
replacement открытого account запрещена Windows; не evidence downgrade.
В первом I4 probe mutable canary требовал mirror lease; probe исправлен на
сравнение всего vault snapshot без обхода ownership. Проверяемый I4 red trace —
отдельный `red-i4.log`. Эти setup failures не объявляются подтверждением bugs.
Первый readiness прошёл 188 boundaries, затем Clippy отклонил два `expect_err`
в новом unit test. Они заменены возвратом ошибки; strict Clippy затронутых
пакетов повторно прошёл, и быстрые tests повторены на финальных исходниках.
Промежуточный [readiness.log](lossless-sync-a1-integration-iteration2-evidence/readiness.log)
с failure сохранён, как и [clippy-repair.log](lossless-sync-a1-integration-iteration2-evidence/clippy-repair.log).

## Сохранённые границы

Disposable профиль, известный genesis, один выбранный SessionId/recipient,
одна receiving chain; другие context/direct/mailbox/writers запрещены.
Pending — точные подписанные event/auth bytes в encrypted vault, отдельно от
readable/causal history. Ratchet, projection, pending removal/frontier и ACK
intent по-прежнему коммитятся вместе. S1/S2/S3 из первой итерации перепроверены.

Budgets остаются 512 lifetime indices, 8 KiB event, 2 KiB ciphertext,
2 MiB pending, 4 MiB checkpoint, 128 MiB vault / 512 MiB transient charge.
Логический/staging charge не является физическим OS/redb reservation и не
обещает безусловный moving-head reserve или convergence при нехватке ёмкости.
DH tail явно блокируется. Новых domains, wire activation, rollout, migrations,
GUI, M1.2.63 и общего performance redesign нет.

Положительный 256 test — реальные SyncServer/SyncClient/session paths, source
EventStore, receiver encrypted vault и history API с отдельными процессами;
transport binding trusted fixture. Он не проверяет сетевые runtime actors,
Iroh/relay/mailbox/two-host или все power-loss windows. ACK network delivery
не проверяется. Full mutation/workspace test corpus не запускается.
После одного reviewable diff и финального evidence — остановка до review.
На момент сдачи на review коммитов/push не было; сохранение после приёмки описано
выше. PR и продуктового включения нет.
