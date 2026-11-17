# T1.2a. Создание локального Setup из терминала

База `81c493b61c54aebbeef9970f01fa048621817133`. Windows-вход `kilogram-cli setup --dir <каталог>` выполняет begin, open, status, создание аккаунта/устройства и подтверждение recovery. TTY и JSONL используют одну машину состояний. Локальный worker вызывает штатный sibling `kilogram-bootstrap.exe` и обслуживает существующий `Setup`. [Контракт](terminal-t1-2a-contract.md) · [Координатор](../../apps/kilogram-cli/src/terminal_setup.rs)

- Создание под `.onboarding.lock` повторно проверяет назначение перед bootstrap. После потери результата проверяются существующие public receipts. Терминал допускает повторный create при отсутствующем workspace и отвергает `plaintext-development` при открытии. Подтверждение слов записывается через `Setup::acknowledge`. Форматы Setup и bootstrap прежние. [Доступ к Setup](../../crates/kilogram-first-dialog/src/model/terminal.rs) · [Исходы helper](../../crates/kilogram-first-dialog/src/helper.rs)
- Слова доставляет только явный show. Для pipe/JSONL требуется opt-in. Секретная сериализация использует заимствованный текст и зануляемый буфер, а Windows writer пишет в отдельный небуферизованный handle. Перед первым байтом ответа освобождается внешний ID. Мутации ждут завершения worker и точного writer ACK, в том числе при немедленном `recovery_ack`. Читаемый `status` во время создания берётся из снимка. Ошибки helper идут в протокол как коды. [Секретный результат](../../apps/kilogram-cli/src/terminal_setup/wire.rs) · [Worker](../../apps/kilogram-cli/src/terminal_setup/service.rs)

## Проверки

| Запуск | Результат |
| --- | --- |
| `build03` | `--locked --offline`, Idle и одно задание, изображения в `target/terminal-t1-setup-checks`. CLI SHA-256 `aff9181d86fd9ccf180afd2d6cfaa9556f030125daecce34b7c04dbe0f39a887`. Bootstrap SHA-256 `c1b44ee7550caf32978aa9c279ae911d443702ac3c386c4519b1ed3d43def6be`. |
| `cli03` | Семь целевых тестов стабильных Windows harness. Повтор ID до writer ACK, старый ACK, частичный/просроченный вывод, привязка cancel, снимок статуса, резерв workspace и lock. Реальный helper подтвердил соответствие фразы account ID в памяти и открытие durable ready без слов после отмены перед принятием результата. |
| `helper03` | Целевой тест общего collector. Проверены ограничение stdout/stderr, одновременное чтение обоих pipe, сбой, таймаут, отмена и ожидание завершения child. |
| `smoke02` | Реальные локальные ConPTY и JSONL с sibling helper. Явный показ 24 слов, подтверждение и повторное открытие, JSONL без opt-in, конкурирующий процесс и немедленный повтор ID для `show → ack`. Durable ACK пережил отказ stdout. Отвергнуты junction и receipts с `plaintext-development`. EOF во время active create освободил Setup и допустил проверку сохранённого состояния. |
| `console01` | Реальный Ctrl+C в ConPTY отменил active create, после чего терминал принял status и quit. Idle Ctrl+C отдельно проверен в `smoke02`. |

Во всех зачётных запусках получены child/owner 0, EOF обоих потоков и пустой Job без принудительной уборки. Зачёт broken stdout проверяет ожидаемый код 2 внутри успешно завершившегося smoke. Захваченные слова проверялись в памяти и очищались. В журнал вошли только результаты проверок, количество слов и хеши исполняемых файлов. [Сквозное свидетельство](../../.tmp/terminal-t1-2a/native02/evidence.json) · [Активный Ctrl+C](../../.tmp/terminal-t1-2a/console-native01/evidence.json)

`build01` обнаружил, что существующий bootstrap-contract был доступен CLI только в dev-зависимостях. Зависимость перенесена в основной target без изменения lockfile. `build02`, `cli01`, `helper01` и `cli02` сохранили квитанции об уборке остаточного `vctip`. Зачёт получен в отдельных запусках с тем же Cargo-набором пакетов. `smoke01` остановился в проверочном скрипте при чтении занятого Windows lock-файла. В `smoke02` проверка учитывает пустой lock-файл, а остальные файлы проверяются на отсутствие фразы. Образы smoke не менялись.

Эта сдача охватывает локальный Setup приглашающей стороны. Обмен invitation/reply, согласие, настройка LAN и переход к chat относятся к следующим срезам. Физический пилот и SSH на этом этапе не повторялись.
