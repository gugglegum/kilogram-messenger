# T1.3d — LAN Auto в терминальном runtime

База для review — `2ad31ca01ad7eca64c2e24cb455ebb3f382ca796`.

## Доработка после review 01

Разрешение на публикацию теперь передаётся по очереди вместе с датаграммой. Owner2 выдаёт его для текущих локаторов и policy до срока, ограниченного объявлением, локаторами и consent. Изменение локаторов или policy отзывает ранее выданные разрешения в финальной проверке записи. Отзыв поколения или закрытие также делают разрешение непригодным. Поток сокета непосредственно перед отправкой проверяет отметку отзыва, срок и свежесть сети. Переиздание объявления сохраняет допущенную пару.

Для каждой попытки Auto admission создаётся разрешение, связанное с TLS-соединением, точной парой, сетевым поколением и сроком попытки. Проверка повторяется после ожидания открытия потока, перед записью частей кадра и перед закрытием потока для отправки. Ответы B проходят ту же проверку. Финальная проверка сетевого `PinAuthority` использует разрешение того Hello, который принёс подписанное обновление. Локальный импорт Root остаётся доступен без сети. Новый Root snapshot по-прежнему может быть принят из проверенного Hello до готовности контекста к admission, пока разрешение сетевого источника действует.

[Замечания review 01](terminal-t1-3d-root-review-01.md)

### Проверки доработки

Windows NativeToken, профиль `verification`, Rust 1.98.0 MSVC, Idle и один worker. Использован тот же ограниченный Windows Job. В каталоге `.tmp/terminal-t1-3d-review-01` находятся команды, квитанции и логи.

```powershell
cargo check --locked --offline --jobs 1 --profile verification --package kilogram-cli --package kilogram-first-dialog --tests
cargo clippy --locked --offline --jobs 1 --profile verification --package kilogram-cli --package kilogram-transport-iroh --lib -- --no-deps -D warnings
$packages = @('kilogram-cli', 'kilogram-first-dialog')
& scripts/run-cargo-tests-stable.ps1 -Package $packages -Profile verification -CargoJobs 1 -SkipCacheStatus
& scripts/run-cargo-tests-stable.ps1 -Package $packages -Profile verification -CargoJobs 1 -SkipCacheStatus -Filter 't13d_review_' -NoCapture -Run
& scripts/run-cargo-tests-stable.ps1 -Package $packages -Profile verification -CargoJobs 1 -SkipCacheStatus -Filter 'ordered_dialog::runtime2::runtime::tests::' -NoCapture -Run
```

| Шаг | Лимиты, с | Проверка |
| --- | ---: | --- |
| [lint01](../../.tmp/terminal-t1-3d-review-01/lint01.receipt.json) | 350 / 390 | Scoped rustfmt и строгий Clippy обеих библиотек. |
| [check01](../../.tmp/terminal-t1-3d-review-01/check01.receipt.json) | 350 / 390 | Типы тестовых целей. |
| [build02](../../.tmp/terminal-t1-3d-review-01/build02.receipt.json) | 460 / 500 | Чистая сборка без запуска harnesses. |
| [focused01](../../.tmp/terminal-t1-3d-review-01/focused01.receipt.json) | 460 / 500 | 2/2 целевых сценария R1 и R2. |
| [regression01](../../.tmp/terminal-t1-3d-review-01/regression01.receipt.json) | 460 / 500 | 13/13 Runtime2, включая 2 целевых сценария и прежние ручные допуск и отправку. |

Проверка R1 подставляет управляемые часы и счётчик отправок в тот же метод, который вызывает native socket worker. Вызов отправки пропущен при истечении объявления или consent, изменении authority или consent, замене собственного локатора и смене сетевого поколения. Действующее разрешение вызывает отправку один раз.

Проверка R2 использует реальный loopback TLS, задержку перед возвратом потока и синтетическую привязку Auto. С действующим разрешением кадр дошёл, после отзыва ожидание потока и повторная запись ответа завершились отказом. Для staged `PinAuthority` проверен путь записи от сетевого origin с реально подписанным Root snapshot. Отзыв сохранил прежнюю ревизию, локальный импорт того же snapshot без сети и сетевой импорт с новым допуском прошли. Проверка точного физического LAN-path остаётся для T1.3e.

Холодная сборка `build01` сохранилась отдельно из-за остаточного `vctip.exe` после линковки. У всех пяти шагов в таблице подтверждены код 0, EOF обоих потоков и пустой Job без принудительного завершения. Хеши пяти образов совпали до и после обоих прогонов тестов.

## Терминал

После сохранения согласия в Setup выберите `/route auto <local_ipv4> <peer_ipv4>`. В JSONL той же операции соответствует `route_lan_auto` с полями `local_ipv4` и `peer_ipv4`. Устройства в выбранной LAN могут заметить периодические объявления. Setup сохраняет точные IP, GUID адаптера, Private Network ID и префикс из одного типизированного наблюдения Windows. Неоднозначная, отсутствующая или просроченная привязка отклоняется. Для выбора режима нужен остановленный runtime. `/route lan <local_ipv4> <peer_ipv4>` и `/route loopback` сохраняют ручной режим. Их JSONL-эквиваленты — `route_lan` и `route_loopback`.

После `/quit` в Setup запустите `kilogram-cli chat --setup <directory>`, выполните `/connect start` и используйте `/status`. В режиме `--jsonl` подключение выполняет операция `connect` с `args.mode = "start"`. Auto поднимает локальный IPC до запуска сетевого контроллера. История и разрешённая локальная очередь доступны без LAN. Статусы Auto показывают ожидание выбранной сети, ожидание текущей policy, поиск, аутентификацию или повтор и подключение обоих контекстов. Ручные импорт и повтор локаторов требуют Manual. Экспорт работает как диагностическая операция, пока наблюдается действующий локальный endpoint.

## Жизненный цикл

- Setup и Profile2 строго проверяют режим. Старые профили без поля режима используют Manual. Session сверяет профиль с аутентифицированным IPC и сохранённой привязкой.
- Auto с начала создаёт Owner2 с закрытым допуском. Супервизор создаёт shared proof на каждое наблюдение исходной Private сети. Он открывает exclusive multicast socket и один Iroh endpoint на поколение. При потере сети сначала отзывается proof, затем отменяются сокеты и сетевые задачи. Возврат той же сети открывает новое поколение. Ошибка bind оставляет локальный owner в работе.
- NLM проверяется раз в 2 с на отдельном потоке с COM apartment. Наблюдение действительно максимум 4 с от начала проверки. Если следующая проверка задержалась до следующего тика, текущий proof отзывается без ожидания команды Owner2. Пока прежний NLM-вызов не вернулся, новых потоков проверки нет. Изменение Windows между наблюдениями видно после проверки или истечения proof.
- В уже начатом staged commit финальный guard читает shared proof. Его также проверяют сетевые попытки, раунды admission, lease, peer commits и разрешение на отправку. Owner2 владеет состоянием пары и локаторами. Датаграмма не может изменить локальный профиль. После ротации отправляются оба точных подписанных локатора с HPKE. Входной сокет проверяет peer IPv4, multicast destination и ifIndex, затем пакет снова открывает Owner2 по текущей policy.
- Входная очередь вмещает 4 датаграммы. Сокет и Owner2 ограничивают входные проверки до 8 за скользящую секунду. Квота Owner2 применяется после ожидания в очереди до криптографической проверки. Две попытки или соединения делят общий семафор. У каждого контекста один исходящий цикл. Очередь отправки объявления вмещает один кадр. Отправка идёт каждые 2–5 с при поиске и 14–16 с при двух действующих lease. Повторы используют jitter и backoff 0,5–15 с. Изменение пары пробуждает цикл, дополнительная HPKE-обёртка той же пары сохраняет текущую цель.
- При остановке супервизор отзывает proof, закрывает транспорт и ждёт свои задачи и worker. Таймаут cleanup возвращается как ошибка. Владелец удаляет свой IPC-дескриптор через существующий путь. Существующая проверка владельца сохраняет файлы локаторов с чужой подписью.

## Воспроизведение

Windows NativeToken, Rust 1.98.0 MSVC, `verification`, Cargo Idle, один worker. Команды запускаются из корня проекта. Ограниченные дочерние шаги, логи и квитанции лежат в `.tmp/terminal-t1-3d`. Обёртка `owner.ps1` учитывает окончание процесса, EOF и освобождение Windows Job. Для нового прогона создайте новые имена шагов, чтобы сохранить исходные квитанции.

```powershell
cargo build --locked --offline --jobs 1 --profile verification --package kilogram-bootstrap
$packages = @('kilogram-cli', 'kilogram-first-dialog')
& scripts/run-cargo-tests-stable.ps1 -Package $packages -Profile verification -CargoJobs 1 -SkipCacheStatus
$env:KILOGRAM_CS_TEST_IMAGE_ROOT = (Resolve-Path 'target/verification').Path
$env:KILOGRAM_CS_TEST_FIXTURE_ROOT = (Resolve-Path '.tmp/terminal-t1-3d').Path
& scripts/run-cargo-tests-stable.ps1 -Package $packages -Profile verification -CargoJobs 1 -SkipCacheStatus -Filter 't13d_' -NoCapture -Run
& scripts/run-cargo-tests-stable.ps1 -Package $packages -Profile verification -CargoJobs 1 -SkipCacheStatus -Filter 'ordered_dialog::runtime2::runtime::tests::' -NoCapture -Run
```

`terminal03` тем же runner проверяет `t12d2_route_commands_require_literal_ipv4_and_exact_args`. `terminalclose03` использует `t11c_control_and_close_do_not_wait_on_queue_or_writer` и `t11e2a_background_preemption_waits_for_retirement_and_close_has_a_bound`.

`lintcli03` и `lintnative01` используют `cargo clippy --locked --offline --profile verification --jobs 1 --package <package> --lib -- --no-deps -D warnings`. Format проверяется `rustfmt --check --edition 2024 --config skip_children=true` по исходникам из allowlist.

## Покрытие

- Строгий round-trip Manual/Auto и отказ при неизвестном режиме, null, добавочных полях, неверном GUID и префиксе. Один явный выбор Auto в TTY и JSONL. Синтетические сохранённые параметры загрузились из Setup, прошли через profile-create и открылись в дочернем runtime.
- Реальный authenticated IPC с недоступной выбранной сетью. Статус показал отсутствие endpoint и live lease при прежних кэшированных локаторах. Одиночный импорт отклонён реальным owner. История, очередь и чтение статуса запроса прошли. Shutdown удалил IPC-дескриптор. Отдельная проверка Session подтвердила штатный выход дочернего процесса и EOF.
- Отзыв общего доказательства внутри реального staged Locator commit оставил оба durable high-water без изменений. В новом поколении старый callback отклонён. Свежий callback записал оба слота. Следующий отзыв сделал непригодной уже выданную цель до команды detach. Регрессии T1.3c проверяют partial commit, ротацию и повтор той же пары.
- Управляемый metadata worker оставался один при задержке 160 мс и интервале 30 мс. Он присоединён при закрытии. Очередь принимает четыре записи и отклоняет пятую. Квота отклоняет девятый пакет до границы скользящей секунды, включая доставку накопленного пакета напрямую Owner2. Проверены диапазоны jitter и backoff, а также завершение трёх задач OD2 транспорта в пределах 3 с.

## Границы полевой проверки

Сценарии запуска использовали синтетическую недоступную LAN-привязку. Проверки TLS и сети данных OD2 использовали настоящий loopback. Поведение exclusive bind при занятом порте и освобождение UDP-сокета подтверждены нативной проверкой T1.3b с кодом 10048. Текущий супервизор открывает сокет до создания endpoint и оставляет owner доступным при ошибке bind. Для T1.3e остаются положительный Auto selected-path, совместный цикл перезапусков двух хостов, возврат физической сети и работа с включённым Firewall. Управление сетью, адаптерами и Firewall в этом срезе не выполнялось. [Нативная проверка T1.3b](terminal-t1-3b-result.md)

## Результаты исходного кандидата

| Шаг | Лимиты, с | Проверка |
| --- | ---: | --- |
| [check05](../../.tmp/terminal-t1-3d/check05.receipt.json) | 340 / 380 | Типы тестовых целей CLI и first-dialog. |
| [lintcli03](../../.tmp/terminal-t1-3d/lintcli03.receipt.json) | 340 / 380 | Формат и строгий Clippy библиотеки CLI. |
| [lintnative01](../../.tmp/terminal-t1-3d/lintnative01.receipt.json) | 480 / 520 | Строгий Clippy Windows LAN adapter. |
| [build06](../../.tmp/terminal-t1-3d/build06.receipt.json) | 480 / 520 | Сборка bootstrap и compile-only пяти стабильных harnesses. |
| [focused03](../../.tmp/terminal-t1-3d/focused03.receipt.json) | 480 / 520 | 8 целевых тестов T1.3d. |
| [regression03](../../.tmp/terminal-t1-3d/regression03.receipt.json) | 480 / 520 | 11 тестов Runtime2, включая 8 прежних проверок. |
| [terminal03](../../.tmp/terminal-t1-3d/terminal03.receipt.json) | 480 / 520 | 1 регрессия ручной команды Setup. |
| [terminalclose03](../../.tmp/terminal-t1-3d/terminalclose03.receipt.json) | 480 / 520 | 2 регрессии отмены и закрытия терминала. |

В холодных сборках `build01`, `build03` и `build05` дочерний процесс вернул 0, а Windows Job потребовал завершить оставшийся `vctip.exe`. Эти квитанции сохранены отдельно. У всех восьми финальных шагов подтверждены пустой Job, EOF обоих потоков и код 0 без принудительной очистки. Хеши трёх стабильных harnesses и двух дочерних EXE совпали до и после каждого финального запуска тестов.

Запуски широкого Clippy сохранились с ошибками прежних участков. Для `kilogram-first-dialog` зафиксированы 19 диагностик на неизменённых строках, для `kilogram-runtime-ipc` — одна `chunks_exact_to_as_chunks`, для бинарной цели CLI — те же 39 диагностик, что на базе T1.3c. [Аудит диагностик](../../.tmp/terminal-t1-3d/baseline-lint-audit.json)

## Proposed publication allowlist

Точные Git blobs, SHA-256 рабочих файлов, режимы, квитанции и хеши образов включены в [review.json](../../.tmp/terminal-t1-3d-review-01/review.json). Предыдущий манифест сохранён, исходный отчёт и прежние образы скопированы в каталог доработки. Commit и push ожидают явной приёмки.

```text
ai-docs/tasks/terminal-t1-3d-result.md
apps/kilogram-cli/Cargo.toml
apps/kilogram-cli/src/main.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/admission.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/config.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/discovery_tests.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/discovery.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/lan_metadata.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/lan_runtime.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/mod.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/network.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/owner.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/runtime_tests.rs
apps/kilogram-cli/src/ordered_dialog/runtime2/runtime.rs
apps/kilogram-cli/src/ordered2_cli.rs
apps/kilogram-cli/src/terminal_chat.rs
apps/kilogram-cli/src/terminal_chat/coordinator/tests.rs
apps/kilogram-cli/src/terminal_setup/service.rs
apps/kilogram-cli/src/terminal_setup/tests.rs
apps/kilogram-cli/src/terminal_setup/wire.rs
Cargo.lock
crates/kilogram-first-dialog/src/chat.rs
crates/kilogram-first-dialog/src/connection_tests.rs
crates/kilogram-first-dialog/src/connection.rs
crates/kilogram-first-dialog/src/connection/locators.rs
crates/kilogram-runtime-ipc/src/ordered2.rs
crates/kilogram-transport-iroh/src/ordered2.rs
crates/kilogram-windows-lan/src/lib.rs
crates/kilogram-windows-lan/src/native.rs
crates/kilogram-windows-lan/src/native/binding.rs
crates/kilogram-windows-lan/src/native/socket.rs
```
