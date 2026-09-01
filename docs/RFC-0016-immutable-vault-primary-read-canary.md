# RFC-0016: immutable vault primary-read canary (M0.8.4)

Статус: реализовано в M0.8.4

## 1. Задача и граница

M0.8.3 доказал exact typed equivalence между encrypted vault и retained legacy
tree, но прикладные команды продолжали возвращать данные только из filesystem.
M0.8.4 выполняет первый реальный primary-read cutover для самого узкого и
обратимого сценария:

- только read-only команда `history`;
- только immutable `event` и `local-projection` repositories;
- только после полной authentication vault и exact legacy shadow comparison;
- без silent fallback, если инициализированный vault повреждён или расходится с
  legacy.

Все live writes, network sync, ratchet, trust, sequence, recovery и rewrap
repositories остаются legacy-primary. Отсутствие vault целиком сохраняет
совместимый legacy режим; частично инициализированный или невалидный vault
считается ошибкой.

## 2. State-side primary snapshot

`TypedStateRepository::read_primary_canary` принимает явный список
`StateRecordKind`. M0.8.4 разрешает только:

- `event` — `.event` и обязательные `.authorization` sidecars;
- `local-projection` — device-local encrypted `.local-text` values.

Пустой список и любой mutable/неизвестный kind отклоняются. Метод:

1. полностью расшифровывает и аутентифицирует active vault;
2. проверяет manifest и authenticated generation;
3. если live command уже создала mirror intent, требует exact совпадения intent
   с active generation/snapshot;
4. побайтно сравнивает весь vault и legacy tree;
5. возвращает только выбранные DB records, их kind/path/content и per-kind
   shadow inventory.

Matching intent разрешён специально для read-only canary: общий CLI lifecycle
создаёт intent до запуска команды. Если legacy изменился после intent, exact
comparison завершается ошибкой и stale DB bytes не возвращаются. Intent с
другим base также отклоняется.

`VaultPrimaryRead` является owned authenticated snapshot. После завершения
shadow comparison прикладной decoder получает bytes именно из DB, а не путь к
legacy файлу.

## 3. Store-side read adapters

`kilogram-store` экспортирует узкие object-safe interfaces:

```rust
pub trait EventReadRepository {
    fn load_authorized_conversation(...);
    fn frontier(...);
}

pub trait LocalMessageReadRepository {
    fn get(...);
}
```

Их реализуют как существующие filesystem stores, так и новые owned snapshots:

- `ImmutableEventReadSnapshot`;
- `ImmutableLocalMessageReadSnapshot`.

Snapshot constructors принимают DB bytes и fail-closed валидируют canonical
layout:

- ровно 64 lowercase hex символа в Conversation ID / Event ID;
- ровно два path components для events и один для projections;
- только `/`, без empty, `.`, `..` или backslash components;
- только известные `.event`, `.authorization`, `.local-text` extensions;
- отсутствие duplicate paths.

При чтении заново проверяются event signature и Event ID, соответствие имени
файла и conversation directory, уникальность `(author device, sequence)`,
обязательный authorization sidecar, Account Root/membership authorization,
projection Event ID и authenticated projection encoding. Поэтому DB-primary
не обходит прежнюю прикладную проверку store.

## 4. CLI policy

`history` выбирает repository один раз после state lock и M0.8.2 recovery:

- vault отсутствует полностью — `legacy-filesystem`, для обратной совместимости
  с ещё не мигрированным state;
- vault инициализирован — только `encrypted-vault` плюс
  `legacy-verified` shadow;
- любая ошибка open/authentication/shadow/layout/decode — команда завершается
  ошибкой; попытки открыть legacy repository после ошибки нет.

Успешный canary печатает:

```text
history_primary_read=encrypted-vault
history_shadow_read=legacy-verified
history_vault_generation=2
history_vault_event_records=6
history_vault_local_projection_records=3
```

Затем история, frontier и plaintext projection строятся из snapshot objects.
Read-only mirror completion очищает intent с delta `0/0/N`, не меняя
generation.

## 5. Security properties

Получено:

- первое прикладное чтение, фактически возвращающее authenticated DB bytes;
- exact shadow gate до выдачи результата;
- отсутствие silent downgrade для инициализированного vault;
- повторное применение event/membership/projection verification к DB values;
- запрет mutable kinds на уровне state API;
- совместимость с authenticated intent read-only lifecycle.

Не получено:

- DB-primary writes;
- transactional overlay для команд, которые читают и затем пишут events;
- cutover sync, rewrap, ratchet, trust или sequence;
- отказ от полного `O(state)` shadow scan;
- удаление legacy files или защита master key;
- внешний rollback witness.

## 6. Проверки M0.8.4

State tests покрывают:

- primary selection только event/projection;
- запрет empty и mutable selection;
- successful read без intent и с matching authenticated intent;
- typed mismatch после изменения legacy projection до mirror;
- recovery после отказа stale read.

Store test создаёт валидные event, authorization и projection, загружает их в
owned snapshots, физически удаляет исходные legacy files и после этого успешно
проверяет authorized history, frontier и projection. Unsafe snapshot path
отклоняется.

CLI seeded-history test мигрирует три события, запускает matching intent и
получает те же verified events/projections через trait objects из vault.

Все 85 workspace tests, rustfmt, strict Clippy и release build проходят.

Release process smoke `.tmp/m084-smoke-20260901-120000` на реальном generation
2 state прочитал из vault 6 event/authorization records и 3 projections,
восстановил 3 сообщения и frontier, подтвердил `legacy-verified`, затем завершил
mirror как `0/0/23`. Post-canary vault verify сохранил generation 2. Отдельная
копия с изменённой legacy projection завершилась typed mismatch и exit code 1;
вывод не содержал legacy fallback.

## 7. Следующий этап

M0.8.5 реализован в
[`RFC-0017`](RFC-0017-vault-primary-history-rewrap.md): оба read-only
source-history пути history rewrap используют общий authenticated owned
snapshot и не открывают legacy stores после cutover. Read trait подготовлен к
sync inventory/events-by-ID, но mixed read/write sync остаётся legacy-primary.

M0.8.6 должен добавить явный overlay новых event/projection records либо direct
transactional DB write; нельзя просто переиспользовать snapshot начала команды
и потерять записи, созданные позже.

Mutable ratchet/trust/sequence cutover, protected key provider, migrations,
backup и rollback witness остаются отдельными security stages.
