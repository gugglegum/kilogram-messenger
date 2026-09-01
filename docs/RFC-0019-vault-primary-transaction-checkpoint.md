# RFC-0019: vault-primary transaction checkpoint (M0.8.7)

Статус: реализовано в M0.8.7

## 1. Задача и граница

До M0.8.7 даже DB-primary read paths записывали новое состояние сначала в
retained legacy files, завершали filesystem journal, а encrypted vault обновляли
один раз в конце команды. Network response уже зависел от filesystem commit,
но не от успешного vault commit.

M0.8.7 делает encrypted vault точкой необратимого commit для всех операций,
которые уже входят в M0.7.7 `StateTransaction`. Это delivery,
acknowledgement, sync batches, seed-history, history-rewrap import/recovery и
prekey/ratchet updates.

Legacy tree пока не удаляется. Внутри операции он служит staging area, а после
vault commit — exact compatibility shadow. Mutable reads и часть trust updates
вне `StateTransaction` ещё используют filesystem; это не полный repository
cutover.

## 2. Почему checkpoint включает не только event/projection

Ratchet message нельзя коммитить отдельно от связанного состояния:

- inbound event зависит от consumed/advanced ratchet и local projection;
- authored event зависит от advanced ratchet, allocated author sequence и
  authored projection;
- acknowledgement зависит от allocated sequence и event sidecar.

Поэтому vault-primary checkpoint фиксирует exact delta всего staged state tree,
а не только файлы `events` и `local-messages`. Это делает event/projection
commit атомарным с ratchet/sequence, но ещё не переводит их read adapters на
DB.

## 3. Нормальный протокол commit

Одна journal-операция проходит следующие состояния:

```text
authenticated outer mirror intent
  -> prepared filesystem journal
  -> write legacy staging tree
  -> immediate redb delta commit
       records + manifest + generation
       rotated mirror intent
       authenticated primary-shadow intent
  -> filesystem committed marker + journal cleanup
  -> exact DB/legacy comparison
  -> clear primary-shadow intent
  -> network response / visible CLI result
```

`commit_primary_checkpoint` проверяет исходный authenticated mirror intent и
активную generation, собирает staged tree, вычисляет delta и одной
`Durability::Immediate` redb transaction:

- upsert-ит changed/new encrypted records;
- удаляет removed records;
- публикует новый keyed manifest и monotonic generation;
- заменяет обычный mirror intent на binding к новой generation;
- создаёт отдельный authenticated `primary-shadow-intent-v1`.

После filesystem commit `confirm_primary_shadow` заново сравнивает все paths и
contents, проверяет оба intent против активного snapshot и удаляет только
primary-shadow marker. Обычный command intent остаётся, поэтому последующие
transactions могут создать ещё один checkpoint, а финальный guard может
зеркалировать нетранзакционные изменения.

## 4. Crash recovery: vault всегда побеждает после primary commit

До redb commit staged files не считаются authoritative. Ошибка checkpoint
откатывает M0.7.7 journal, а неуспешная redb transaction невидима.

После redb commit authoritative становится vault:

- crash до filesystem committed marker: следующий `StateDirectoryLock`
  сначала откатывает prepared filesystem journal, затем vault guard видит
  authenticated primary-shadow intent и восстанавливает весь retained tree из
  committed vault snapshot;
- crash после filesystem marker, но до confirmation: восстановление
  идемпотентно подтверждает или перепубликует тот же shadow;
- active filesystem journal блокирует преждевременную shadow recovery внутри
  ещё не завершённой операции;
- повреждённый marker, generation binding, manifest, ciphertext или shadow
  останавливает recovery fail-closed.

Восстановление пишет только authenticated vault paths, отклоняет unsafe paths и
symlinks, удаляет отсутствующие в primary snapshot legacy files и повторно
выполняет exact comparison до очистки marker. Crash во время самого restore
безопасен для повторения: marker остаётся в БД.

## 5. CLI commit barrier

`run_state_transaction` и `run_store_transaction` теперь используют общий
`PendingVaultPrimaryWrite`:

1. operation closure формирует staged filesystem state;
2. если vault инициализирован, checkpoint обязан успешно закоммититься;
3. при ошибке checkpoint filesystem journal откатывается;
4. filesystem transaction публикует shadow;
5. shadow confirmation должна завершиться до возврата результата caller.

Поэтому delivery/sync code отправляет protocol frame только после vault commit
и shadow confirmation. Never-migrated states сохраняют прежний filesystem-only
режим.

Каждая изменившая состояние journal transaction теперь может увеличить vault
generation. Одна CLI-команда с несколькими sync rounds или отдельным
acknowledgement может увеличить generation несколько раз. Финальный M0.8.2
mirror обычно сообщает `already-current`; если команда меняла state вне
journal, он по-прежнему применяет разрешённый legacy-to-vault delta.

Диагностика checkpoint:

```text
vault_primary_write=committed
vault_primary_write_upserted_records=10
vault_primary_write_removed_records=0
vault_primary_write_unchanged_records=6
vault_primary_write_generation=3
vault_primary_shadow=confirmed
```

Recovery дополнительно печатает `vault_primary_shadow_recovery=restored`.

## 6. Проверки M0.8.7

State fault test доказывает:

- injected failure внутри redb delta не меняет generation и не создаёт
  primary-shadow marker;
- успешный checkpoint блокирует DB-primary read до confirmation/recovery;
- recovery не запускается поверх active filesystem journal;
- симуляция crash откатывает prepared legacy staging, после чего vault
  восстанавливает event и связанный ratchet state;
- повторная recovery идемпотентна;
- после confirmation обычный outer mirror может применить последующий trust
  delta;
- marker с неверным authenticator отклоняется.

CLI integration test выполняет два последовательных sync batches поверх vault:
каждый batch создаёт собственную generation, overlay видит committed records, а
финальный mirror остаётся `already-current`.

Все 86 workspace tests, rustfmt, strict Clippy и release build проходят.

Release process smoke `.tmp/m087-smoke-20260901-170711` выполнил реальную
authenticated direct delivery. У клиента vault достиг generation 4, у listener
generation 3. На обеих сторонах `vault_primary_write=committed` и
`vault_primary_shadow=confirmed` появились до `sent_event_id` либо
`received_event_id`. Финальные mirrors были `already-current`, vault verify
прошёл, а DB-primary histories содержали одинаковые delivery и acknowledgement
events.

## 7. Ограничения и следующий этап

Filesystem ещё используется как staging input, а checkpoint собирает полный
state tree, поэтому scan остаётся `O(state)`. Нетранзакционные trust operations
могут обновляться прежним recoverable legacy-first mirror. Ratchet, sequence и
trust reads ещё не используют vault adapters. Development master key остаётся
рядом с DB.

M0.8.8 реализован в
[`RFC-0020`](RFC-0020-typed-journal-delta-and-mutable-sequence.md): live CLI
использует typed delta активного journal вместо полного filesystem payload
checkpoint, а `next-sequence` стал первым mutable DB-primary adapter. Ratchet и
trust cutover, protected key provider, rollback witness, versioned migrations
и bounded backup остаются отдельными security stages.
