# T1.3c — внутренний допуск пары локаторов в Owner2

База для review — `7e5ed15be4a633b238eecc804f1df97d9a364c7e`. Обычный запуск CLI по-прежнему создаёт Owner2 в ручном режиме. Внутренний Auto начинает сетевое поколение после проверки обеих установленных пар credentials и consent, точного Direct LAN маршрута и обоих локальных подписанных локаторов на наблюдаемом endpoint.

При приёме Owner2 заново открывает исходную HPKE-датаграмму под текущими ключами и контекстами. Токен наблюдения связывает операцию с сетевым поколением, маршрутом, policy generation, экземпляром локального endpoint и обоими локальными локаторами. Обе входящие записи и их high-water проверяются до закрытия действующей пары. Owner2 затем закрывает допуск и отзывает lease и незавершённые попытки. Он записывает изменившиеся слоты через два обычных `RuntimeOperation::Locator` и открывает допуск после контрольного чтения точных байтов и повторной проверки текущей политики. Точный повтор записывает только недостающий слот. Другая обёртка той же действующей пары сохраняет идентификатор пары и действующие разрешения.

Проверка действует для цели dial, входного сокета до TLS, TLS endpoint и ранней записи authority из Hello, всех раундов admission, lease, peer commit и IPC статуса. Задержавшаяся попытка хранит ссылку на ту пару, с которой началась. Окно объявления ограничивает импорт. Срок действующей пары определяют локаторы и consent обеих сторон, а lease сохраняет собственный предел. Локальные очередь и история используют текущую policy. IPC импорт одного локатора в Auto отклоняется.

Привязка Auto остаётся внутренней. Пользовательские Profile2, Setup и команды CLI используют прежний путь. Для T1.3d остаётся подключить наблюдение сети и вызовы Owner2 к Windows runtime. Проверка двух физических хостов относится к T1.3e.

## Воспроизведение

Команды ниже выполняются из корня репозитория с Rust 1.98.0 MSVC, в Windows NativeToken. `.tmp/terminal-t1-3c/owner.ps1` устанавливает Idle, запускает дочерний шаг в ограниченном Job, сохраняет stdout/stderr и JSON-квитанцию. Рабочий/общий лимит и имена шагов приведены ниже. Для повторения создайте новые имена шагов, чтобы сохранить исходные результаты.

```powershell
cargo clippy --locked --offline --profile verification --jobs 1 --package kilogram-cli --lib -- --no-deps -D warnings
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification -CargoJobs 1 -SkipCacheStatus
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-cli -Profile verification -CargoJobs 1 -SkipCacheStatus -Filter 'ordered_dialog::runtime2::runtime::tests::' -NoCapture -Run
& scripts/run-cargo-tests-stable.ps1 -Package kilogram-protocol -Profile verification -CargoJobs 1 -SkipCacheStatus -Filter 'lan_discovery_' -NoCapture -Run
```

Для всех семи Rust-файлов в allowlist выполняется `rustfmt --check --edition 2024 --config skip_children=true`. Стабильный runner собирает Windows test harnesses без исполнения и запускает фиксированные пути только на шаге с `-Run`.

| Шаг | Лимиты, с | Результат |
| --- | ---: | --- |
| [lint04](../../.tmp/terminal-t1-3c/lint04.receipt.json) | 300 / 340 | Scoped rustfmt и строгий Clippy библиотеки `kilogram-cli`. |
| [build02](../../.tmp/terminal-t1-3c/build02.receipt.json) | 300 / 340 | Чистый compile-only, четыре стабильных CLI harnesses. |
| [test02](../../.tmp/terminal-t1-3c/test02.receipt.json) | 420 / 460 | 8 прошли, включая 2 новых Owner2 теста и 6 ручных регрессий. |
| [codec01](../../.tmp/terminal-t1-3c/codec01.receipt.json) | 300 / 340 | 5 прошли, включая неверные подписи, несовпадающие endpoint/socket и scope, пределы датаграммы и интервалы. |

Все четыре шага завершились с кодом 0, EOF обоих потоков и пустыми Jobs без принудительного завершения. SHA-256 образов до и после запусков совпали.

- `target/verification/deps/kilogram-cli-lib-kilogram_cli-tests.exe` — `bd2c4ecb2eddf2db197a764434b706c545667caa901444a9c8a64eab8cb1245f`.
- `target/verification/deps/kilogram-protocol-test-lan_discovery-tests.exe` — `ef6199f1bc822bea61b18a775707fc4bf2f5d6885d9fbad9b97e992b512594e0`.

В `test01` и `build01` дочерние процессы вернули 0, но после линковки пришлось завершить принадлежащий Job `vctip.exe`. Эти шаги сохранены как попытки с нештатным завершением. Широкий Clippy остановился на существующем `chunks_exact_to_as_chunks` в `kilogram-runtime-ipc` (`lint01`) и на предупреждениях в неизменённых целях бинарного CLI (`lint02` и `lint03`). Связанные файлы не входят в allowlist. [Квитанции и логи](../../.tmp/terminal-t1-3c/)

## Покрытие

- Текущее хранилище с кэшированными локаторами при входе в Auto имеет закрытый допуск. При смене действующей пары допуск закрывается до первого commit. Отказ второго реального staged commit сохраняет ревизии `[3,2]`, в том числе после reopen. Повтор даёт `[3,3]` и ровно одну новую запись.
- Между слотами проверены dial и listener, выбор TLS endpoint, подписанный Hello на реальном loopback TLS с более новой authority, начало admission и раунд Proof, peer commit и IPC статус обеих context. Одиночный IPC импорт отклоняется. Локальная запись queue сохраняется при открытом и закрытом допуске. Обычная DB generation не меняет ссылку на пару. Старый сетевой токен не может открыть допуск после смены поколения.
- Повреждённый wire, неверный consent, иной маршрут, нижняя и конфликтующая ревизии оставляют здоровую пару. Повторный HPKE envelope сохраняет пару, DB generation и валидность разрешения. После истечения окна объявления старый wire отклоняется, а проверка срока продолжающейся lease проходит. Срок locator, изменение наблюдаемого локального endpoint и новая authority проверяются отдельно. Мутация authority отзывает прежнее разрешение и требует нового наблюдения.
- Существующие ручные проверки используют настоящие Iroh/TLS loopback, реальное хранилище и native send/receive после admission. В двух новых тестах точный LAN bind представлен подставленным наблюдением адреса. Тест lease проверяет привязку к паре и сроки через действующий loopback TLS. Production selected-path проверка LAN сохранена. Положительный Auto TLS/selected-path на физическом LAN остаётся для T1.3e.

## Proposed publication allowlist

- `ai-docs/tasks/terminal-t1-3c-result.md`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/admission.rs`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/discovery.rs`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/discovery_tests.rs`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/mod.rs`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/owner.rs`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/runtime.rs`
- `apps/kilogram-cli/src/ordered_dialog/runtime2/runtime_tests.rs`

Точные нормализованные Git blobs и SHA-256 рабочих файлов закреплены в [review.json](../../.tmp/terminal-t1-3c/review.json). Commit и push выполняются после явной приёмки.
