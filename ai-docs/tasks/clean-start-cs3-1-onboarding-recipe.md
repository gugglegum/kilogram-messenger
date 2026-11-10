# CS3.1: Windows invite/join pilot

Новый явный путь в существующем Windows UI: **First-dialog pilot — create or join invitation**. Создаётся настоящий recoverable CS2 аккаунт, публичное приглашение/ответ и сохраняемое состояние ожидания. Чат пока не подключается: согласие CS1, Profile2, transport и messaging относятся к следующим slices.

## Запуск и действия пользователя

Используйте собранные согласованные `kilogram-windows.exe` и `kilogram-bootstrap.exe`. По умолчанию helper ищется рядом с UI; `--bootstrap-exe` позволяет явно указать доверенный локальный helper. При работе из репозитория проверенные fixed paths:

```powershell
$gui = Join-Path $PWD 'target/clean-start-runtime/kilogram-windows.exe'
$bootstrap = Join-Path $PWD 'target/clean-start-runtime/kilogram-bootstrap.exe'
& $gui --fresh-home (Join-Path $PWD 'my-first-dialog') --bootstrap-exe $bootstrap
```

`--fresh-home` открывает только pilot screen; без этого аргумента прежний desktop entry остаётся прежним, pilot доступен отдельной кнопкой при остановленном runtime. Выберите отдельную папку setup для своего первого аккаунта; internal filenames и context IDs назначает приложение. Не используйте этот путь как «новый аккаунт для каждого контакта».

1. Alice выбирает **Create invitation**. UI заранее сохраняет `setup.json` вне ещё не существующей папки аккаунта и запускает helper с двумя случайными context IDs. После создания открывается recovery screen; слова скрыты по умолчанию, доступны через **Reveal recovery words privately**. Запишите их отдельно. Обычного clipboard action нет.
2. После фактического сохранения слов установите **I actually saved the recovery words offline**, затем **Hide words and continue**. Фраза очищается из state/view. Только теперь доступен **Export public invitation**. Файл сохраняется в `SETUP/exchange/invite.kilogram`; UI показывает полный путь. Передайте только этот публичный файл будущему собеседнику.
3. Bob открывает pilot со своей отдельной setup folder. В поле public invitation file выбирает полученный путь и нажимает **Inspect public file**. UI проверяет credentials/authority и показывает действительные account/device IDs Alice. Сравните их с Alice через известный независимый канал; криптографическая подпись файла сама по себе не подтверждает личность человека.
4. После сравнения отметьте **I compared this peer's identity independently**, затем **Join invitation and create account**. У Bob создаётся новый CS2 аккаунт с точной ordered парой contexts из приглашения. Сохраните его слова, подтвердите и скройте их так же, как у Alice. **Export public reply** сохраняет `SETUP/exchange/reply.kilogram`, привязанный к invitation и account/device Alice.
5. Alice загружает public reply через **Inspect public file**, независимо сверяет Bob и нажимает **Keep this peer selection**. Выбранный peer сохраняется, последующие конфликтующие reply отвергаются. Оба экрана честно заканчиваются ожиданием mutual consent. **Connect and send messages (next step)** недоступна.

Для разных пользователей файлы переносятся обычным пользовательским способом. Для двух synthetic окон на одном ПК можно выбрать экспортированный публичный файл прямо по его пути. Не передавайте `setup.json`, IPC tokens, папку аккаунта, приватные ключи или recovery words.

## Закрытие, сбои и повторное открытие

Запустите UI с тем же `--fresh-home`: сохранённый setup откроется автоматически. Либо выберите setup folder и **Open saved setup**. Фраза никогда не загружается повторно из файлов.

- Нет проверенного ready receipt: **Creation incomplete or result not received**; файлы сохранены, повторный bootstrap в занятой папке не выполняется. **Reopen and check saved setup** только проверяет результат.
- Ready/public bindings проверены, но acknowledgement отсутствует: UI сообщает, что one-time words больше недоступны. Продолжайте только если действительно успели их сохранить до закрытия. Иначе оставьте создание заблокированным; автоматического reset/recreate нет.
- Acknowledgement сохранён: вновь открывается awaiting screen с теми же local/peer IDs и contexts. В этом slice runtime не запускается.

Cancel/выход из pilot/закрытие приложения очищают фразу и отменяют принадлежащий ему helper. Product watchdog60s, cleanup reap до5s. Поздний ответ не попадает в новый UI state. Same-byte public export можно повторить; чужой существующий файл не перезаписывается.

Слова восстанавливают ключ и идентификатор аккаунта. Для актуальной authority нужен отдельный recovery package и последняя независимо сохранённая witness; история сообщений имеет отдельные требования. Existing account recovery export остаётся отдельным действием; не считайте invite/reply резервной копией.

Пилот ограничен одной парой собеседников. Работа direct между двумя ПК, полноценный multi-contact onboarding и fresh volunteer storage пока не подключены; эти продуктовые требования не отменены.

## Воспроизведение focused checks разработчиком

Использовались Rust1.98, locked/offline, Jobs2/BelowNormal и existing bounded NativeToken owner. Сначала обычная сборка UI и compile-only harness, затем только stable runner; никакого прямого запуска network harness через `cargo test`.

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'target/s54-diagnostics-source-checks'
$env:CARGO_NET_OFFLINE = 'true'
cargo clippy --locked --offline --jobs 2 -p kilogram-windows --all-targets --profile verification -- -D warnings
cargo build --locked --offline --jobs 2 -p kilogram-windows --bin kilogram-windows --profile verification
& ./scripts/run-cargo-tests-stable.ps1 -Package kilogram-windows -Filter cs31_ `
  -Profile verification -CargoJobs 2 -SkipCacheStatus
# После staging fixed UI EXE и проверки содержимого helper:
& ./scripts/run-cargo-tests-stable.ps1 -Package kilogram-windows -Filter cs31_ `
  -Profile verification -CargoJobs 2 -SkipCacheStatus -Run -NoCapture
```

Запускайте команды внутри принятого bounded owner со свежим evidence, как `.tmp/clean-start-cs3-1/owner.ps1` и его named step scripts; не заменяйте работающий fixed EXE. Actual helper для tests — `target/clean-start-runtime/kilogram-bootstrap.exe`. Synthetic producers запускаются через тот же fixed-name stable library harness. Tests не логируют words, создают только fresh синтетические stores и отдельно проверяют реальную egui reveal/clear rendering без снимков секретного экрана. Проверенные hashes/results и все неудачные attempts перечислены в [результате](E:/AI-Projects/kilogram-p2p-messenger/ai-docs/tasks/clean-start-cs3-1-onboarding-result.md).
