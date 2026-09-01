# RFC-0020: typed journal delta and mutable sequence canary (M0.8.8)

Статус: реализовано в M0.8.8

## 1. Задача и граница

M0.8.7 сделал encrypted vault точкой необратимого commit, но собирал его delta
полным чтением retained legacy tree после каждой journal-операции. Mutable
author sequence также читался из `next-sequence` на filesystem, хотя commit уже
происходил в DB.

M0.8.8 вводит два связанных вертикальных среза:

- `StateTransaction` сообщает canonical typed set изменённых records;
- `next-sequence` читается из authenticated vault generation и только
  публикуется в retained filesystem как shadow.

Ratchet и trust reads пока не переведены на DB adapters. Этот RFC не объявляет
полный repository cutover и не удаляет legacy files.

## 2. Typed journal delta

`StateTransaction::staged_mutations` строит изменения только для roots,
которыми уже управляет crash journal:

- сравнивает текущий bounded `ratchet/**` с journal backup;
- сравнивает единственный `next-sequence` с его backup;
- для append-only `events/**`, `local-messages/**`, `history-rewraps/**` и
  `history-recovery/**` читает payload только у новых paths;
- отклоняет удаление baseline append-only record.

Каждая mutation несёт canonical `StateRecordKind`, relative path и `Some(bytes)`
для upsert либо `None` для removal. Vault повторно вычисляет kind из canonical
path, запрещает `device-identity`, `trust` и `other` в journal write-set,
отклоняет duplicate/unsafe paths и transaction из другого state root.

Текущий authority/contact code пока пишет небольшой bounded набор trust records
вне `StateTransaction`: `account-authority.snapshot`,
`device-certificate.cert`, `conversation-memberships/**` и
`peer-authority/**`. До применения journal write-set direct coordinator
включает текущее содержимое только этих namespaces в ту же vault transaction.
Этот compatibility ingress намеренно уже полного retained-tree scan; удаление
уже committed trust record отклоняется fail-closed. Trust reads и domain write
API при этом остаются filesystem-backed.

Append-only namespace пока перечисляется для нахождения новых paths, но при
построении pre-commit delta payload существующих history records больше не
читается из filesystem. Отдельная post-commit exact shadow confirmation пока
по-прежнему читает весь retained tree.

## 3. Direct vault transaction

`commit_primary_transaction(&StateTransaction)` заменяет CLI-вызов полного
`commit_primary_checkpoint`:

1. проверяет authenticated outer mirror intent и active generation;
2. аутентифицирует active DB records;
3. включает bounded non-journal trust compatibility delta;
4. применяет journal mutations к DB-owned record map;
5. пересчитывает manifest и exact delta;
6. одной `Durability::Immediate` redb transaction публикует encrypted
   upserts/removals, manifest, generation, rotated mirror intent и
   `primary-shadow-intent-v1`;
7. после filesystem journal commit выполняется прежняя exact shadow
   confirmation.

Таким образом retained legacy tree больше не является входом полного payload
snapshot для построения DB delta. Crash contract M0.8.7 не меняется: до DB
commit побеждает filesystem rollback, после DB commit — vault recovery.

`commit_primary_checkpoint` сохранён как migration/compatibility primitive, но
live `run_state_transaction` и `run_store_transaction` его больше не используют.

## 4. Первый mutable DB-primary adapter

`read_mutable_primary_canary([sequence])` разрешён только при:

- полностью authenticated vault manifest/generation;
- отсутствии pending primary-shadow intent;
- наличии outer mirror intent, exact bound к active snapshot.

Он возвращает owned `VaultMutableRead` непосредственно из DB и намеренно не
читает `next-sequence` из retained shadow. `CommandTransactionContext` лениво
захватывает этот record при первой аллокации. Отсутствующий record означает
sequence `0`; единственный допустимый path — `next-sequence`.

`DeviceState::allocate_sequence_from` получает authenticated current value,
записывает `current + 1` в filesystem staging и возвращает current. Несколько
аллокаций в одной transaction используют локальный cursor. На ошибке journal
возвращает shadow к прежнему состоянию; на успехе sequence входит в тот же
direct DB commit, что ratchet/event/projection.

Never-migrated state по-прежнему использует filesystem allocator.

Диагностика:

```text
vault_mutable_read_kind=sequence
vault_mutable_read_source=db-primary
vault_mutable_read_generation=1
vault_mutable_read_record_count=1
vault_primary_write_mode=typed-journal-delta
```

## 5. Fault и integration проверки

State tests доказывают:

- journal выдаёт changed/added/removed ratchet records, sequence и только новые
  append-only records;
- baseline append-only removal отклоняется;
- DB-primary sequence возвращает committed value даже после изменения shadow;
- injected abort direct redb delta не меняет generation и не создаёт marker;
- успешный direct commit с одним новым trust record даёт точные
  `4 upsert / 1 remove / 1 unchanged`;
- pending primary-shadow marker блокирует следующий mutable read;
- transaction от другого root и ещё не разрешённый ratchet mutable read
  отклоняются.

CLI test изменяет retained `next-sequence` с `2` на `99` после подготовки
authenticated outer intent. Реальная transaction выделяет `2` из DB, сохраняет
shadow `3`, коммитит ровно один typed upsert, повышает generation `1 -> 2` и
завершает final mirror как `already-current`.

Все 90 workspace tests, rustfmt, strict Clippy и release build проходят. Release
process smoke `.tmp/m088-smoke-20260901-185655` завершил Alice/Bob delivery и
acknowledgement с `vault_mutable_read_source=db-primary`,
`vault_primary_write_mode=typed-journal-delta`, source/listener generations
`4/3` и точным совпадением final vault/legacy histories.

## 6. Ограничения и следующий этап

Active DB records пока полностью decrypt-ятся для проверки manifest и его
пересчёта, append-only directories перечисляются дважды journal-ом, а exact
shadow confirmation после DB commit снова читает все legacy payloads. Поэтому
общая CPU/path сложность ещё не `O(changed)`: устранён именно полный
pre-commit staging scan при построении delta. Ratchet tree сравнивается с
backup целиком, но он ограничен локальным session/prekey state.

Ratchet и trust читаются через retained filesystem shadow; нет direct storage
adapter, write-set регистрации от domain repositories, incremental authenticated
manifest index или production migration schema. Development master key всё ещё
лежит рядом с DB.

M0.8.9 реализован в
[`RFC-0021`](RFC-0021-db-primary-ratchet-workspace-and-registered-appends.md):
ratchet transaction начинается с authenticated DB snapshot, rollback использует
primary backup, а explicit append write-set убирает второй directory walk.
Trust cutover, protected key provider, rollback witness, versioned migrations и
bounded backup остаются отдельными security stages.
