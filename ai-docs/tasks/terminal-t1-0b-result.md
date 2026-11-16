# T1.0b — штатный Windows runtime из локального и SSH-сеансов

## План до запуска

Проверить неизменённые `kilogram-bootstrap.exe` и `kilogram-cli.exe` из принятой сборки CS4.1 на основном ПК и в ранее переданном комплекте ноутбука. Сверить SHA‑256 файлов на месте. Использовать отдельные каталоги `.tmp/terminal-t1-0b-20261116-a01` и `C:/Users/test/KilogramLab/terminal-t1-0b-20261116-a01`, на ноутбуке — прямой SSH с pinned host key без PTY, Windows PowerShell с `-EncodedCommand`.

В каждом каталоге создать два одноразовых аккаунта штатным `kilogram-bootstrap create-ordered2` и использовать только публичный `bootstrap-ready.json`. Поток с фразой и диагностический вывод bootstrap подавить. Участники на одном хосте нужны для достижения Active2: установка Fresh consent создаёт слот, а история инициализируется при установлении контекста. Провести `ordered context2-propose/cosign/import` для обоих слотов и создать два Profile2 через `runtime-profile-create` с DirectOnly и `127.0.0.1:0`. Ключи и профили остаются на своём хосте. Ни LAN, ни смена брандмауэра не входят в проверку. [Bootstrap](clean-start-cs2-recoverable-bootstrap-recipe.md) · [Consent и loopback](clean-start-cs1-cli-recipe.md)

Запустить обе роли через `runtime-from-profile`. Связать подписанный локальный descriptor с ожидаемыми account/device, PID ребёнка и контекстами через `ordered status2`. Импортировать только сгенерированные для этой попытки публичные адреса на том же хосте. Проверить Active2 и завершённые пустые страницы `ordered history2` для обоих слотов. Затем выполнить аутентифицированный `ordered shutdown`, подтвердить exit 0, EOF, отсутствие принадлежащих проверке процессов и удаление descriptor. Открыть первый профиль повторно, подтвердить другой runtime instance и те же history view, завершить штатно. Отказ при отсутствующем или просроченном descriptor проверяется отдельно, если позволяет CLI. Проверка `Client2` с неверной expected identity и проверка pending intent остаются отдельными целевыми задачами.

Контрольные процессы принадлежат одному Windows Job Object на хост. На работу отведено до 180 с, общий предел 210 с. Штатный путь завершается через IPC. При ошибке инструментарий ограниченно завершает только свои процессы и сохраняет результаты. Повтор использует новый каталог.

## Результат от 2026-11-16

Квалификация прошла на рабочей базе `2e765d23c2446b1a1a642710018215fccc24f8a9`. Одинаковый инструмент `scripts/test-terminal-t1-runtime.ps1` выполнен локально в NativeToken и на ноутбуке через прямой pinned SSH без PTY. Повтор `a04` создал две новые пары. Транспорт каждого запуска оставался в одном хосте на loopback. [Инструмент](../../scripts/test-terminal-t1-runtime.ps1)

| Наблюдение | Основной ПК | Ноутбук через SSH |
| --- | --- | --- |
| Внешнее время lifecycle и код выхода | 20,313 с, 0 | 62,328 с, 0 |
| Bootstrap и согласие | Два Root с `windows-dpapi-current-user`, оба consent установлены в обоих профилях | То же |
| Runtime и локальный IPC | Два cold start. Signed loopback descriptor, runtime PID и account/device/context pins сверены. Active2 в обоих слотах на обоих узлах. | То же |
| История | Четыре завершённые пустые страницы. Два view в первом профиле различаются. После restart первый профиль вернул те же два view с новым runtime instance. | То же |
| Shutdown | Три `ordered shutdown` с ack, exit 0, EOF и удалением descriptor. `ordered status2` после остановки вернул 1. Ноль дочерних процессов до принудительной очистки и после неё. | То же |

Совпали SHA‑256 двух ранее принятых образов. `kilogram-cli.exe` — `125838275f2ed3f7258e88a924a4c5499472b2efc3b935941a2cb386aae7e54f`, `kilogram-bootstrap.exe` — `baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4`. На основном ПК использован `target/clean-start-runtime`, на ноутбуке — каталог ранее переданного комплекта `C:/Users/test/KilogramLab/cs4-field-current/Kilogram-CS4`. Новых сборок не было.

Локальные квитанции `a01`–`a03` сохранены. В `a01` probe завершился до bootstrap из-за имени переменной, совпавшего с автоматической переменной PowerShell. В `a02` bootstrap завершился, но проверка ожидала hex там, где публичный JSON хранит байты account/device. В `a03` получен Active2, но чтение истории остановилось из-за имени JSON-поля `F`; Job Object завершил свои оставшиеся процессы и вернул пустой список. Все повторы использовали свежие каталоги. Рабочий `a04` не потребовал принудительного завершения.

Для воспроизведения в свежем `ProbeRoot` с префиксом `terminal-t1-0b-` положить проверенные копии `scripts/test-terminal-t1-runtime.ps1` и `scripts/migration-s5-l1-deadline.ps1` как `probe.ps1` и `deadline.ps1`. Передать абсолютные `ProbeRoot` и `ImageRoot` отдельному Windows PowerShell. Проверенные `-NoProfile -NonInteractive -EncodedCommand` invocation, загрузка штатных модулей, SHA‑256 обоих скриптов и 225-секундный внешний watchdog находятся в `.tmp/terminal-t1-0b-20261116-a04/run.py`. Там же `local-lifecycle.json`, `remote-deploy.json` и `remote-lifecycle.json`. Удалённая пара создана в `C:/Users/test/KilogramLab/terminal-t1-0b-20261116-a04`. Recovery stdout bootstrap отброшен; отчёты не содержат приватных ключей, descriptor token или recovery words.

Результат подтверждает direct SSH для создания тестового состояния, работы runtime с DPAPI и штатного жизненного цикла. Проверка отсутствующего descriptor покрыта обычным CLI. Типизированный отказ по чужой expected identity, protected pending intent, доставка сообщения и терминальный протокол остаются за пределами этого прогона. Текущий полевой setup и настройки хостов сохранены.
