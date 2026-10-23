# A1: первый реальный durable ingress — итерация 1

2026-10-23. Основание: [review F1/F2, следующий конечный шаг](lossless-sync-a1-spike-review.md#следующий-конечный-шаг-реальный-durable-ingress-узкий-внутренний-scope)
и последующее одобрение пользователя, указанное в [координации](../chat-coordination.md).
Статус: реализация и конечные проверки завершены; остановка на независимом code review.
Implementation HEAD: `3e82e6380c30d0f9443b6692ea34faccaacddef7`; изменения пока в рабочем дереве.
Это узкий disposable single-chain slice для code review; A1 целиком не закрыт.
Единый [reviewable.patch](lossless-sync-a1-integration-evidence/reviewable.patch)
содержит все четыре файла, включая untracked sources: 1808 insertions / 16 deletions.
Reverse check проверяет соответствие patch текущим исходникам, ничего не применяет.
SHA-256 patch: `a8e635ef671c623f6d4d6ce4d73eccc90ee81d0ac4c482a716c1ed4faf3e2a1b`.

## Что включено

Один diff затрагивает четыре исходных файла:

- `apps/kilogram-cli/src/ordered_ingress.rs` — ingress, signed checkpoint,
  budget, quarantine, lease и существующие CLI Sync/History на защищённом профиле;
- `apps/kilogram-cli/src/ordered_ingress_tests.rs` — синтетические реальные
  session/vault fixtures и проверки;
- `apps/kilogram-cli/src/main.rs` — подключение существующего
  `DecryptingSessionStore`, транзакционного ratchet workspace и CLI dispatch;
- `crates/kilogram-ratchet/src/lib.rs` — чтение публичного Olm header,
  single-chain guard и запрет обычного доступа к отмеченному account.

Профиль создаётся только тестовой фабрикой из пустого нового synthetic state.
В обычном CLI нет команды включения или миграции. В release-коде при наличии
такого профиля разрешены существующие Sync/History только для выбранной беседы
и peer device; остальные команды, direct/mailbox/runtime writers запрещены.
Весь профиль удерживается настоящим `StateDirectoryLock`. Обычный ratchet
loader/decrypt/encrypt отказывает для отмеченного account; снятия этой защиты
или обхода legacy decrypt в разрешённом receive path нет. Повторная проверка
после vault recovery не позволяет выполнить generic writer по восстановленному
защищённому account.

Selection фиксирует recipient account/device, peer account/device,
conversation, известный genesis EventId, один Olm SessionId и одну receiving
DH chain. Header index, author sequence и EventId имеют разные значения:
fixture намеренно использует `author_sequence = index * 2 + 7`.
Новая DH chain получает `UnresolvedLegacyTail`, другой контекст/peer — отказ.
Принудительного skip/reset или автоматического выбора новой session нет.

## Commit и подтверждения

Pending содержит точные `AuthorizedEvent::encode()` bytes (event вместе с
certificate/authority), точный EventId и состояние. Он хранится в encrypted
vault как подписанный checkpoint существующего mutable Ratchet namespace,
отдельно от readable EventStore/LocalTextProjection/causal state. Retained
filesystem shadow также содержит этот checkpoint; это ciphertext и публичная
авторизация, plaintext сообщений в него не записывается. Acceptance читает
checkpoint из vault DB-primary, а не из самостоятельного файла.

После durable admission `run_state_transaction` пробует только contiguous
голову. Native ratchet workspace, encrypted LocalTextProjection, события,
удаление pending, frontier/receipt и подписанный ACK intent входят в одну
существующую vault-primary транзакцию. ACK попадает в доступный inventory и
`events_by_id` только после readable commit. Доставка ACK по сети здесь не
проверяется. До commit native trial находится в rollback workspace.

Некорректный ready pending получает durable quarantine с причиной. Корректный
prefix, его projections/ACKs и quarantine коммитятся вместе; байты bad entry
сохраняются. Повтор видит quarantine до decrypt и не создаёт busy loop.
При storage/transaction failure действует существующий rollback/recovery;
это не доказательство всех отказов файловой системы.

Результат события и drain разделены. Exact duplicate требует и того же EventId,
и тех же authorization bytes. Другой EventId в том же ordinal — Conflict.
Partial admission сохраняет принятый subset, но реальный `SessionStore::put_events`
возвращает ошибку; `SyncClient` не получает успешный batch для subset SyncComplete.
Успешный pending-only round подтверждает possession ciphertext, не readable
доставку пользователю. CLI не объявляет readable convergence, пока есть pending.

## Budget и границы гарантии

Этот slice имеет lifetime `<512` native indices, максимум 512 обычных pending
slots плюс голова, 2 MiB pending bytes, 8 KiB authorized event, 2 KiB ciphertext
и 4 MiB signed checkpoint. Собственная ACK authorization также ограничена 8 KiB.
По умолчанию `vault_charge=128 MiB`, `transient_charge=512 MiB`.

Проверяются реальный текущий plaintext volume vault и длины всех файлов
профиля, включая database/shadows; symlinks отвергаются. Для обоих budgets
используется `3 * base + (pending_entries + 1) * 256 KiB + 4 MiB`.
Это консервативный admission budget для полного commit и staging, а не
физическое резервирование свободного места/страниц redb. Generic I/O failure
по-прежнему возможен и не выдаёт ACK/успешный round.

Гарантия постоянно доступного moving head reserve **не заявляется**. Реальный
trace `[3,4,5] → 0,1 → 2` проверяет явный StorageBlocked, сохранность future
bytes и readable prefix. После явного увеличения fixture quota trace завершается;
измеренный final vault charge сравнивается с прежним blocked limit. Таким
образом final history fit не выдаётся за гарантию admission/convergence.
Исходные числа S3 `138+30>150`, final `144<150` отдельно воспроизводятся
арифметически; реальная encrypted database не объявляется 150-байтовой.
Для unknown-tail test отдельная synthetic native копия Bob, сделанная до
защиты профиля, принимает K0 и отправляет B0; Alice затем создаёт L0 с тем же
SessionId и новым DH key. Защищённый receiver не переводится в sending mode
и не снимает account fence; L0 явно блокируется.

## Evidence по критериям

| Критерий задания | Проверка и граница |
|---|---|
| 1. Изоляция | Настоящий exclusive lock, tagged native account, unleased SessionStore отказ, выбранная session/chain; никакие настоящие пользовательские профили не изменялись. |
| 2. Реальная транзакция | EncryptedStateVault DB-primary, существующий StateTransaction/PendingVaultPrimaryWrite, реальные signed events, projections и ACK intents. |
| 3. 256 exact plaintext | Реальные SyncServer/SyncClient и DecryptingSessionStore, 4 hash pages по 64, отдельный процесс каждой страницы и отдельный history process; точное EventId→body равенство всех 256. |
| 4. Negative checks | S1 future bad MAC → reopen → valid prefix/quarantine/no ACK; S2 exact duplicate/conflict/partial durable inventory; S3 real vault budget trace и same SessionId/new DH tail. |
| 5. Reopen | После pending commit, native trial до readable commit, readable commit до возврата ACK. Error/drop/reopen/recovery; проверены native hashes, exact history и ровно один text/ACK. |
| 6. Scope/ресурсы | Locked/offline, jobs=2, stable Windows harnesses. Нет новых ratchet domains, wire activation/default rollout, GUI или M1.2.63. |

Положительный сценарий использует два реальных session paths/профиля, но
вызывает компоненты SyncServer/SyncClient внутри worker: это **не два запущенных
сетевых runtime actors**. Session binding — trusted fixture constant; Iroh,
handshake/ALPN, IPC, relay, mailbox и физическая сеть не участвовали. Source
использует реальный EventStore; receiver — настоящий encrypted vault. History
читается через существующий `collect_runtime_history_page` и encrypted projections.
Reopen положительного сценария — реальные новые процессы; fault seams —
контролируемые ошибки/drop/reopen, не аварийное выключение ОС между каждым fsync.

## Проверки и воспроизведение

Все команды выполняются из корня проекта. Файлы evidence находятся в
[`lossless-sync-a1-integration-evidence`](lossless-sync-a1-integration-evidence/).

```powershell
$env:CARGO_NET_OFFLINE='true'
$env:TEMP=(Resolve-Path -LiteralPath '.tmp').Path
$env:TMP=$env:TEMP
& .\scripts\run-cargo-tests-stable.ps1 -Package kilogram-cli -Filter ordered_ingress -CargoJobs 2 -Run -NoCapture
& .\scripts\run-cargo-tests-stable.ps1 -Package kilogram-ratchet -CargoJobs 2 -Run -NoCapture
& .\scripts\verify-kilogram-m1-readiness.ps1 -AcceptedTag M1.0.0
git diff --check
```

Runner только компилирует через `cargo test --no-run`, затем копирует harness
в постоянное имя. Worker запускает `current_exe()` этого же stable пути.
Хеш ревизии не используется в имени EXE.

| Финальная проверка | Результат / log |
|---|---|
| 256 signed ciphertext / hash pages / process reopen / exact history на финальном diff | 1 parent passed, 5 child phases passed, 0 failed; 1105.00 s; [log](lossless-sync-a1-integration-evidence/targeted-final-256.log). |
| CLI negative/reopen, filter `ordered_ingress`, skip только долгого 256 test | 4 passed, 0 failed, 1 helper ignored; 48.96 s; [log](lossless-sync-a1-integration-evidence/targeted-final-negative-2.log). |
| Обычный ratchet | 10 passed, 0 failed; 8.38 s; [log](lossless-sync-a1-integration-evidence/ratchet-final.log). |
| M1 readiness `-AcceptedTag M1.0.0` | exit 0; 188/188 boundaries, rustfmt, workspace/all-targets/all-features locked/offline strict Clippy; [log](lossless-sync-a1-integration-evidence/readiness-final.log). Mutation harnesses здесь только проверяются компилятором, не запускаются. |
| Единый diff | `git apply --reverse --check --whitespace=error` exit 0; [log](lossless-sync-a1-integration-evidence/patch-check.log), [export script](lossless-sync-a1-integration-evidence/export-reviewable-patch.ps1). |
| Финальная сверка | Global `git diff --check`, patch reverse check и повторные SHA-256 всех sources/manifests/stable EXE прошли; [log](lossless-sync-a1-integration-evidence/final-checks.log). |

В финальном положительном прогоне frontiers четырёх страниц: `0 → 0 → 0 → 256`.
Genesis оказался на последней hash page; все 192 ранее принятых ciphertext
пережили process reopen в pending. Пятый child process прочитал все 256 exact
plaintext через history API; pending=0. Child phases заняли 120.11 / 121.69 /
123.16 / 359.61 / 206.60 s. Отличие порядка от первого прогона ожидаемо:
fixture генерирует новые device keys/EventId, assertion не зависит от конкретной
hash permutation и не выдаёт случайные ordinals за детерминированный trace.

В финальном S3 trace `final_charge=4,525,793`, прежний
`blocked_limit=5,354,124` bytes. Future ciphertext `[3,4,5]` сохранены побайтно;
нехватка admission lease не превращается в ACK или ложную convergence.
Toolchain, HEAD, SHA-256 четырёх sources, manifests/lock и stable executables
записаны в [environment.json](lossless-sync-a1-integration-evidence/environment.json).
Cargo.toml/Cargo.lock не изменены. Вложенный cache-status helper выводит своё
default jobs=12; компиляция и readiness явно используют jobs=2. Cache не очищался.
Clippy завершился с техническими Cargo warnings о fallback с hard links на
копирование incremental cache; ошибок исходников/Clippy нет. Это функциональная
проверка корректности, не performance acceptance обычного мессенджера.

Первый полный прогон: [targeted-1.log](lossless-sync-a1-integration-evidence/targeted-1.log),
5 passed, 1 helper ignored в aggregate (явно выполнен 5 раз дочерними процессами),
0 failed; 1221.16 s. Frontiers после страниц: `0 → 2 → 4 → 256`, pending=0,
history=256 exact. После него добавлены более строгие negative assertions,
catch авторизации ready pending, bound ACK envelope и recovery dispatch recheck;
поэтому этот первый лог не объявляется проверкой побайтно финального diff.

Промежуточные compile/clippy logs сохранены: ошибки компиляции исправлены;
Clippy замечание к одинаковому префиксу fault variants исправлено. В первом
повторе negatives проверялся outer Display `StoreError`, скрывающий текст
вложенного `io::Error`; assertion исправлен на типизированную причину. Эти
промежуточные failures не замаскированы как успешные проверки.

## Точка остановки

Нужен независимый code review этой итерации. Полная двусторонняя переписка,
DH transitions, миграция legacy history, распределённый head discovery и
универсальный storage/liveness contract остаются за границей. Полный
workspace test/mutation corpus, GUI, real-profile migration и все power-loss
windows здесь не проверяются. Коммитов, push, PR и продуктового включения нет.
