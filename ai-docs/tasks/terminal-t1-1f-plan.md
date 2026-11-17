# T1.1f. Двуххостовый пилот терминала

База пилота — опубликованный T1.1e2b, коммит `3174a169fa7f2f5aa6888740b3a8d0e6667d5c8c`. Перенос комплекта, изменение правил брандмауэра и запуск пилота выполняются после архитектурной приёмки. [Публикация E2b](../../.tmp/terminal-t1-1e2b/publication-remote.json) · [Результат E2b](terminal-t1-1e2b-result.md)

## Предварительная проверка

| Хост | Наблюдение |
| --- | --- |
| Основной ПК | `DESKTOP-PAUL-RY`, Windows build 26300, PowerShell 5.1.26100.9818. `192.168.0.140/24` на Wi‑Fi «Беспроводная сеть 2», индекс 22, Up, Private. Маршрут до `192.168.0.152` on-link через `192.168.0.140`. UDP `49152–65535`. `vcruntime140.dll`, `vcruntime140_1.dll` и `msvcp140.dll` версии 14.50.35719.0. Процессов `kilogram-*` не было. ActiveStore сообщает `Private.Enabled=False`, правил для точного пути EXE не найдено. |
| Ноутбук | Pinned SSH подтвердил `PAUL_HP_OMEN` и соединение `192.168.0.140 → 192.168.0.152:22`. Windows build 26300, PowerShell 5.1.26100.9818, `192.168.0.152/24` на Wi‑Fi «Беспроводная сеть», индекс 19, Up. UDP `49152–65535`, те же три DLL версии 14.38.33135.0. Процессов `kilogram-*` не было. Системная маска активных профилей `0x2` соответствует Private. Прямой запрос категории конкретного интерфейса отклонён CIM, а NLM вернул ошибку диапазона. `INetFwPolicy2` сообщает `private_enabled=false` и пустой список правил для точного пути EXE. |

Read-only проверка использовала прежний Ed25519 pin `SHA256:vvR2wQIJmbHD5033l5xVMbrSGyUcucNCy711PYU4oyc`. Первая попытка обновления превысила предел длины команды Windows. После сокращения команды SSH вернул сетевые сведения до 20-секундного дедлайна. Отдельный короткий запрос завершился с кодом 0 и сохранил данные брандмауэра. Перед полевым запуском сверить, что адреса, профили и политика соответствуют снимку. [Сетевые данные](../../.tmp/terminal-t1-1f/preflight-readiness.json) · [Брандмауэр ноутбука](../../.tmp/terminal-t1-1f/preflight-policy.json) · [Маска профиля Windows](https://learn.microsoft.com/en-us/windows/win32/api/icftypes/ne-icftypes-net_fw_profile_type2) · [SSH/DPAPI](terminal-t1-0a-result.md) · [Runtime из SSH](terminal-t1-0b-result.md) · [CS4 в LAN](clean-start-cs4-field-2026-11-16.md)

## Комплект и изоляция

- Зарезервировать постоянные каталоги образов `E:/AI-Projects/kilogram-p2p-messenger/.tmp/terminal-t11f/bin` и `C:/Users/test/KilogramLab/terminal-t11f/bin`. Имена `kilogram-cli.exe`, `kilogram-bootstrap.exe` и `t11f-fixture.exe` сохраняются при перезапусках. Различать сборки по SHA-256. Windows PowerShell 5.1 запускает `host.ps1` и `deadline.ps1` из этих же каталогов с проверкой SHA-256. Python 3.11+ нужен только на основном ПК.
- Источник CLI — `target/terminal-t1-checks/kilogram-cli.exe`, SHA-256 `9a1875c6631bf3b979f7e15c053db2bf70a68e66aa9e867ac0b7d232776c10cf`. Источник bootstrap — `target/clean-start-runtime/kilogram-bootstrap.exe`, SHA-256 `baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4`. Собрать fixture командой `cargo build --locked --offline --jobs 1 --target-dir target/s54-diagnostics-source-checks --profile verification -p kilogram-first-dialog --example t11f-fixture` из процесса Idle. Артефакт `target/s54-diagnostics-source-checks/verification/examples/t11f-fixture.exe`. [Манифест подготовки](../../.tmp/terminal-t1-1f/preparation-r2.json)
- После приёмки разместить пять проверенных файлов в `bin` на каждом хосте. Для ноутбука использовать SFTP с отдельным SSH identity, pinned known_hosts, `StrictHostKeyChecking=yes`, `HostKeyAlgorithms=ssh-ed25519`, `UpdateHostKeys=no`, `BatchMode=yes` и подтверждённым `BindAddress`. Сверить SHA-256 каждого файла после доставки. Копии скриптов именуются `host.ps1` ([исходник](../../scripts/test-terminal-t1-1f-host.ps1)) и `deadline.ps1` ([исходник](../../scripts/migration-s5-l1-deadline.ps1)). Драйвер повторно сверяет образы перед запуском. [Учётная запись и путь ключа](../development-environment.md#L3125)
- Состояние каждого прогона создаётся только в `runs/<attempt>/setup` под отдельным base своего хоста. Путь должен отсутствовать до прогона. Штатные помощники создают одноразовые Setup, invitation/reply и двухслотовый consent. Recovery words очищаются в локальном помощнике. Финальный consent подписавшей стороны уже сохранён локально. Fixture сохраняет начальный LAN record и проверяет его чтением через Setup. Публичные invitation, reply, proposal, final и locators передаются по одному, с проверкой типа и SHA-256. Базы, DPAPI/Root, seed, IPC descriptor и действующие CS4 Setup остаются на своих исходных путях. Empty-install onboarding терминала остаётся отдельной задачей. [Fixture](../../crates/kilogram-first-dialog/examples/t11f-fixture.rs)

## UDP и брандмауэр

`FreshRoute2::new` принимает LAN только для `direct-only`, конкретного IPv4, `bind.port() == 0` и `relay == None`. `advertisement` проверяет один реально привязанный UDP-сокет, а проверка selected path сверяет сокет с подписанными локаторами. LAN выключает relay и порт-маппинг. Fixture задаёт `192.168.0.140:0 → 192.168.0.152` и зеркальное направление при подтверждении этих адресов. При каждом новом запуске драйвер сверяет IP и фактический UDP-порт процесса из Job до обмена локаторами. [Настройка транспорта](../../crates/kilogram-transport-iroh/src/ordered2.rs#L40)

На момент снимка Private Firewall отключён на обоих хостах. При подтверждении этой политики добавлять правила для пилота не требуется. Если перед запуском Private Firewall включён и требует разрешения входящего UDP, вынести на отдельное согласование строки ниже. Одна пара scoped правил покрывает оба перезапуска. Старое разрешение на 63506/63894 относилось к другим портам.

| Хост | Путь EXE | LocalAddress → RemoteAddress | LocalPort / RemotePort | Profile |
| --- | --- | --- | --- | --- |
| ПК | `E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f\bin\kilogram-cli.exe` | `192.168.0.140` → `192.168.0.152` | `49152-65535` / `49152-65535` | Private, «Беспроводная сеть 2» |
| Ноутбук | `C:\Users\test\KilogramLab\terminal-t11f\bin\kilogram-cli.exe` | `192.168.0.152` → `192.168.0.140` | `49152-65535` / `49152-65535` | Private, «Беспроводная сеть» |

Если согласовано создание, администратор задаёт значения из своей строки и имя `Kilogram-T11f-<attempt>-PC-UDP` или `Kilogram-T11f-<attempt>-LAPTOP-UDP`. Проверить правило в ActiveStore. После завершения или отказа удалить правило этой попытки по точному имени и подтвердить отсутствие имени.

```powershell
New-NetFirewallRule -Name $rule -DisplayName $rule -Program $program -Direction Inbound -Action Allow -Protocol UDP -Profile Private -InterfaceAlias $interface -LocalAddress $localIp -RemoteAddress $peerIp -LocalPort $localRange -RemotePort $peerRange
Get-NetFirewallRule -PolicyStore ActiveStore -Name $rule
# После завершения или отказа
Remove-NetFirewallRule -Name $rule
```

## Драйвер и критерии

После приёмки скопировать шаблон в `pilot-config.json`, сверить `base`, `ipv4`, `hostname`, диапазоны UDP, SSH-параметры и все пять SHA-256. Для каждой попытки задать новый `attempt` и новый абсолютный каталог `evidence`. Выполнить на основном ПК. [Шаблон](../../.tmp/terminal-t1-1f/pilot-config-r2.template.json) · [Драйвер](../../scripts/test-terminal-t1-1f-driver.py)

```powershell
python scripts/test-terminal-t1-1f-driver.py --config '.tmp/terminal-t1-1f/pilot-config.json' --run-reviewed-pilot
```

1. Открыть оба owner через `chat --jsonl --setup …`. Получить свежие локаторы, передать публичные bundles и импортировать их через terminal. До отправки оба `status` подтверждают `live_admitted` и `watching` для обоих контекстов. Сверить пустую историю. Передать четыре сообщения, по одному в каждом направлении для каждого контекста. Сверить ID delivery, incoming (context, instance, epoch, текст) и по одной записи в истории обоих хостов.
2. Остановить owner ноутбука. После подтверждённого выхода и исчезновения дескриптора отправить с ПК сообщение в очереди. Зафиксировать `request_id` и проверить pending. После перезапуска ноутбука дождаться `watching` на его сохранённой истории до передачи свежих локаторов. Проверить один incoming и одну строку на каждом хосте.
3. Перезапустить sender на ПК, сверить новый instance, снова обменяться локаторами. `request_status` и `retry` с сохранённым `request_id` возвращают прежний delivered event ID. После обеих перезагрузок сверить всю отправленную историю.
4. Присоединить observer ноутбука к точному instance owner и дождаться `watching`. Выполнить `disconnect` и `quit` у observer. Owner остаётся доступен и получает дополнительное сообщение. Сверить итоговый набор из шести incoming с delivery, в том числе после retry. Драйвер разделяет события и результаты, сверяет correlation ID и повторяет status/quit в пределах дедлайна при `session_closing`.
5. Считать полную историю обоих контекстов на двух хостах. Сверить точное число строк, уникальные ID и текст с ожидаемыми delivery. Строки для каждого ID должны совпасть по всем полям на двух хостах. Завершить owners через `quit`. Зафиксировать коды child/wrapper, EOF stdout/stderr, пустой Job и отсутствие IPC-дескрипторов.
6. Сверить `pilot.json`, Job квитанции, сокеты и SHA публичных файлов. `messages` связывает `request_id` с delivery и terminal tag получателя. `processes[].incoming` содержит фактические уведомления с контекстом, instance, epoch, ID и тестовым текстом для каждой пары host/tag. `final_history` сохраняет считанные строки по хосту, терминалу и контексту. Отчёт пишется также при аварийном завершении и включает уже полученные уведомления.

Контроллер отводит 700 с на работу. Каждый fixture ограничен Windows Job 105/135 с, каждый terminal — 760/790 с, включая уборку. Владение проверяется по process handle. При исключении контроллер закрывает stdin и ждёт штатный выход. По истечении grace закрывается только собственный wrapper/SSH-клиент. На ноутбуке действует независимый Job watchdog. Принудительная уборка, неполная квитанция или неподтверждённый брандмауэр исключают зачёт. Службы и scheduled tasks не создаются.

## Проверка подготовки

Локальный `offline03` создал два свежих Setup и выполнил обмен четырьмя публичными артефактами, импорт consent и проверку LAN record. CLI обработал `ready` и `quit` без `connect`. Проверены штатные коды и пустые Jobs во всех восьми квитанциях. Отдельная проверка на синтетическом JSONL-потоке подтвердила разделение событий и результатов и конечные повторы `session_closing`. Для правки драйвера синтетические проверки приняли полную историю, отвергли лишнюю строку, дубль, пропуск, неверный текст и несовпадение роли/ordinal на каждом хосте. Имитированный сбой сохранил уведомление с нужным host/tag в `pilot.json`. Физический сетевой сценарий и квитанция живого UDP-сокета ждут полевого запуска. [Локальный результат](../../.tmp/terminal-t1-1f/offline03/offline-result.json) · [JSONL-проверка](../../.tmp/terminal-t1-1f/parser-result.json) · [Сверка истории и аварийного отчёта](../../.tmp/terminal-t1-1f/evidence-regression.json)
