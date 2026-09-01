# RFC-0013: encrypted transactional state vault (M0.8.1)

Статус: реализовано в M0.8.1

## 1. Задача и граница среза

M0.7.7 сделал файловое состояние crash-consistent, но для каждой операции
создаёт полный backup mutable-файлов и baseline имён append-only roots. Кроме
того, имена файлов и часть локальных объектов остаются видимы at rest.

M0.8.1 вводит первый безопасный шаг к production-oriented storage:

- весь существующий device `STATE_DIR` копируется в одну embedded ACID-базу;
- путь и содержимое каждой записи шифруются до передачи базе;
- миграция фиксируется одной durable transaction;
- vault можно полностью проверить и восстановить в новый каталог;
- действующий filesystem store не переключается и не удаляется.

Это **теневая snapshot-миграция**, а не завершённая замена persistence layer.
Она сначала доказывает формат, атомарность и обратимость на реальном состоянии,
не подвергая работающий M0 store риску destructive in-place migration.

## 2. Выбранная embedded database

Vault использует `redb` 4.2: pure-Rust embedded ACID/MVCC database со
стабильным on-disk format и copy-on-write B-tree. Для snapshot commit явно
выбирается `Durability::Immediate`.

Причины выбора для этого среза:

- нет отдельного процесса, системной библиотеки или C toolchain;
- одна write transaction атомарно публикует records и manifest;
- reader видит только committed state;
- API достаточно узок, чтобы позднее спрятать его за storage repository traits.

Выбор `redb` для M0.8.1 не делает его необратимой production-зависимостью.
Migrations, нагрузочные характеристики, mobile support и backup semantics ещё
должны быть проверены до публичного релиза.

Справка: [redb README](https://docs.rs/crate/redb/latest/source/README.md) и
[WriteTransaction](https://docs.rs/redb/latest/redb/struct.WriteTransaction.html).

## 3. Файлы и key boundary

После `state-vault-migrate` рядом с прежним состоянием появляются:

```text
STATE_DIR/
  state-vault.redb
  state-vault.key
  ... retained legacy state ...
```

`state-vault.key` содержит случайный 256-bit master key. На Unix новый файл
создаётся с mode `0600`; на Windows он получает ACL каталога. Это development
key provider: шифрование vault защищает от случайного раскрытия или отдельной
утечки DB-файла, но **не** от атакующего, который может прочитать и DB, и key
file. Production-вариант должен заворачивать master key через OS keystore,
аппаратный ключ, passphrase/seed-derived KEK либо их явную комбинацию.

Ключ не записывается в `redb`, не выводится CLI и хранится в zeroizing wrapper
в памяти процесса. Потеря key file делает vault невосстановимым; его backup
должен проектироваться вместе с account/device recovery, а не копироваться в
неуправляемые логи.

## 4. Формат vault v1

В базе две таблицы:

- `vault-records-v1`: keyed BLAKE3(relative path) → encrypted record;
- `vault-meta-v1`: фиксированный ключ `snapshot-manifest` → manifest.

Зашифрованная запись содержит:

- record version;
- нормализованный UTF-8 relative path с `/`;
- точные bytes исходного файла.

Каждая запись получает случайный 192-bit nonce и шифруется
XChaCha20-Poly1305. Отдельный encryption subkey и lookup subkey выводятся из
master key через domain-separated BLAKE3 KDF. В AAD входят schema version и
record lookup key. Поэтому перестановка ciphertext под другим ключом,
модификация envelope или неверный master key отклоняются до декодирования.

Manifest v1 намеренно не содержит открытых путей. Он хранит только:

- schema version;
- количество records;
- суммарный plaintext size;
- keyed BLAKE3 snapshot ID от canonical sequence `(path, content)`.

Manifest аутентифицируется косвенно: после чтения все records расшифровываются,
проверяются и из них заново вычисляется точный manifest. `redb` обеспечивает
атомарность и целостность DB-страниц, а keyed snapshot ID обнаруживает логически
несогласованный набор даже после успешного декодирования.

## 5. Что входит в snapshot

Рекурсивно копируются все обычные файлы device state, кроме:

- `.kilogram-state.lock`;
- `.kilogram-transactions`;
- `state-vault.redb`;
- `state-vault.key`.

Symlink, небезопасный relative path и non-UTF-8 path отклоняются. Текущие
defensive limits: не более 1 000 000 records, 64 MiB на record и 512 MiB
plaintext на snapshot. Это M0 limits, а не выбранные продуктовые лимиты для
медиа: большие вложения должны храниться как отдельные encrypted blobs.

## 6. Миграция и повторный запуск

`state-vault-migrate --state-dir <DIR>` выполняется под существующим exclusive
state lock:

1. собирает и сортирует legacy records;
2. вычисляет manifest;
3. шифрует каждую запись свежим nonce;
4. очищает record table и вставляет весь snapshot в одну write transaction;
5. записывает manifest в ту же transaction;
6. делает immediate-durability commit;
7. повторно открывает read transaction, аутентифицирует все records и сверяет
   пересчитанный manifest.

Если процесс падает до commit, новый snapshot невидим. Unit fault injection
прерывает операцию после первой вставки и подтверждает отсутствие manifest.

Повторная миграция неизменившегося legacy state возвращает
`migration=already-current`. Если committed vault уже существует, а legacy
state изменился, команда fail-closed: M0.8.1 не перезаписывает единственную
подтверждённую snapshot автоматически. Следующий этап должен ввести online
dual-write/versioned migration protocol вместо угадывания, какая сторона
актуальнее.

## 7. Проверка и восстановление

`state-vault-verify --state-dir <DIR>`:

- требует существующие DB и key file;
- проверяет schema/record versions и defensive limits;
- расшифровывает все records и проверяет AEAD/AAD;
- заново вычисляет manifest;
- сравнивает snapshot с retained legacy files.

`state-vault-restore --state-dir <SOURCE> --output-state-dir <NEW_DIR>`:

1. запрещает существующий destination;
2. проверяет vault до записи;
3. создаёт sibling temporary directory;
4. создаёт каждый файл с `create_new`, синхронизирует его;
5. повторно собирает staging snapshot и сравнивает manifest;
6. атомарно переименовывает staging directory в destination;
7. синхронизирует parent directory.

Vault DB и key file в restored legacy directory не копируются. Restore никогда
не заменяет существующий каталог и поэтому не может молча уничтожить live
device state.

## 8. Security properties M0.8.1

Получено:

- атомарная publication полного encrypted snapshot;
- confidentiality путей и содержимого при утечке только DB-файла;
- authentication каждой записи и всего canonical snapshot;
- fail-closed schema, size, duplicate-path и traversal validation;
- безопасная проверяемая обратимость миграции;
- отсутствие изменения wire protocol, Event ID и E2EE semantics.

Не получено:

- защита при совместной краже DB и соседнего key file;
- rollback detection после замены обоих файлов старой согласованной копией;
- secure deletion старых legacy files, WAL/free pages и SSD copies;
- online writes непосредственно в vault;
- versioned data migrations, incremental backup и recovery key ceremony;
- защита Account Root directory, который остаётся отдельной границей.

## 9. Проверки

Unit tests покрывают:

- migrate → verify → idempotent migrate → restore с byte-exact результатом;
- отсутствие fixture plaintext и path в raw DB bytes;
- невидимость незакоммиченной fault-injected transaction;
- обнаружение legacy drift;
- неверный master key;
- запрет restore поверх существующего path.

Process smoke release-бинарником `.tmp/m081-smoke-20260901-070000` мигрировал
23 файла/26,037 plaintext bytes из копии реального M0.7.9 recipient state,
повторно получил `already-current`, проверил vault, восстановил byte-identical
legacy tree и прочитал те же 3 `history` events. Raw DB scan не нашёл
`m078-secret`, `device-secret.key` и `local-messages`.

## 10. Следующий этап

M0.8.2 реализован в
[`RFC-0014`](RFC-0014-recoverable-shadow-dual-write.md): vault получает
versioned generation, authenticated intent и recoverable shadow dual-write
после каждой live CLI-команды. Legacy остаётся primary read path, а changed
command пока пересобирает полный snapshot. Следующий этап должен разбить его на
typed incremental repositories и сравнивать DB/legacy reads до primary cutover.
Protected master-key provider и rollback witness остаются отдельными,
обязательными security slices.
