# RFC-0014: recoverable shadow dual-write (M0.8.2)

Статус: реализовано в M0.8.2

## 1. Задача и граница

M0.8.1 создал проверяемый encrypted snapshot, но после миграции любая live
запись делала vault устаревшим. Простое «сначала записать filesystem, затем
DB» оставляет неоднозначность после crash: расхождение могло быть как
легитимным committed update, так и внешней подменой legacy state.

M0.8.2 вводит recoverable shadow dual-write:

- legacy filesystem остаётся primary read/write store;
- инициализированный vault зеркалит состояние после каждой live CLI-команды;
- перед командой в vault атомарно сохраняется authenticated write intent;
- committed snapshot получает monotonic mirror generation;
- после crash разрешено автоматически принять текущее legacy state только при
  наличии валидного intent, связанного с exact предыдущим snapshot;
- расхождение без intent остаётся fail-closed.

Этот этап не переключает EventStore, projections или ratchet на DB reads и не
обещает эффективный incremental write. Он создаёт recovery protocol между
двумя atomic domains, необходимый до такого переключения.

## 2. Repository boundary

`kilogram-state` экспортирует trait `StateMirrorRepository`:

```rust
pub trait StateMirrorRepository {
    fn begin_dual_write(&self) -> Result<VaultReport, StateError>;
    fn finish_dual_write(
        &self,
    ) -> Result<(VaultMirrorOutcome, VaultReport), StateError>;
    fn recover_pending_dual_write(
        &self,
    ) -> Result<Option<(VaultMirrorOutcome, VaultReport)>, StateError>;
}
```

`EncryptedStateVault` является первой реализацией. CLI зависит от lifecycle
trait, а не от деталей таблиц `redb`. Будущая typed/incremental repository
реализация может сохранить тот же coordinator contract.

`VaultReport` дополнен `mirror_generation`. Generation начинается с 1 и
увеличивается только когда canonical legacy snapshot действительно изменился.
Read-only команда всё равно проходит intent lifecycle, но возвращает
`already-current` без увеличения generation.

## 3. Versioned authenticated metadata

M0.8.2 не меняет encrypted record schema v1 и совместим с vault M0.8.1. В
`vault-meta-v1` добавлены два versioned metadata record:

- `snapshot-generation-v1` — generation, snapshot ID и keyed authenticator;
- `mirror-intent-v1` — base generation, exact base snapshot ID и keyed
  authenticator.

Authenticator вычисляется keyed BLAKE3 с отдельным domain-separated subkey.
Поэтому один DB-файл без `state-vault.key` недостаточен, чтобы изготовить
intent, разрешающий принять произвольное изменение legacy state.

Vault M0.8.1 без generation record читается как generation 1. Первый успешный
M0.8.2 lifecycle атомарно добавляет authenticated generation metadata. Это
совместимый metadata upgrade без перешифрования records.

Generation — локальный sequencing marker, а не внешний rollback witness.
Замена всей согласованной пары DB+key старой копией по-прежнему не
обнаруживается.

## 4. Нормальный lifecycle команды

Все CLI-команды с device `--state-dir`, кроме vault maintenance, выполняются
под прежним exclusive state lock:

1. `StateDirectoryLock` завершает recovery M0.7.7 filesystem journal.
2. Если vault не инициализирован, команда работает как раньше.
3. Если найден pending authenticated intent, выполняется crash recovery из
   текущего уже восстановленного legacy state.
4. Active vault полностью проверяется и сравнивается с legacy state.
5. В отдельной immediate-durability DB transaction записывается intent,
   связанный с active generation/snapshot ID.
6. Выполняется исходная CLI-команда и все её M0.7.7 transactions.
7. Независимо от успеха команды собирается фактически committed legacy state.
8. Если snapshot не изменился, generation record подтверждается, а intent
   удаляется одной transaction.
9. Если snapshot изменился, все encrypted records, новый manifest,
   `generation + 1` и удаление intent коммитятся одной transaction.

Completion запускается и после ошибки команды: delivery/connect может успеть
durably сохранить локальное событие до последующей сетевой ошибки. Если и
команда, и mirror completion ошиблись, CLI сохраняет исходную ошибку и явно
добавляет ошибку dual-write; intent остаётся для следующего запуска.

## 5. Crash matrix

| Точка crash | Состояние при следующем запуске | Recovery |
|---|---|---|
| До записи intent | vault/legacy совпадают | Команда запускается обычно |
| После intent, до legacy write | Active snapshot прежний, legacy прежний | Intent очищается, generation не меняется |
| Во время prepared M0.7.7 journal | Intent есть, legacy частично изменён | Сначала filesystem rollback, затем intent очищается |
| После legacy commit, до vault mirror | Intent есть, legacy новее active vault | Новый encrypted snapshot коммитится как generation + 1 |
| Во время vault snapshot transaction | Старый active snapshot и intent остаются видимыми | Transaction abort, следующий запуск повторяет mirror |
| После vault commit | Vault/legacy совпадают, intent удалён | Recovery не нужен |

DB readers никогда не видят очищенную наполовину record table: records,
manifest, generation и intent removal входят в одну `redb` write transaction.

## 6. Fail-closed drift

Без pending intent `begin_dual_write` сначала выполняет полное сравнение vault
и legacy. Любое расхождение возвращает `VaultLegacyStateChanged` и не запускает
команду. Поэтому автоматический recovery не превращается в общий «принять
текущее содержимое диска».

Intent проверяется по:

- metadata version;
- keyed authenticator;
- exact active generation;
- exact active snapshot ID.

Неверный MAC, другой base или отсутствующий intent отклоняются отдельными
ошибками. Hostile administrator, читающий и меняющий одновременно DB, key file
и процесс, остаётся вне security boundary текущего development key provider.

## 7. CLI

После `state-vault-migrate` обычные live-команды автоматически печатают:

```text
vault_dual_write_intent=prepared
vault_dual_write_base_generation=1
...
vault_dual_write=mirrored
vault_mirror_generation=2
```

При restart recovery дополнительно выводится
`vault_dual_write_recovery=mirrored` или `already-current`.

Команда:

```text
state-vault-recover --state-dir <DIR>
```

явно повторяет recovery только при authenticated pending intent. Без intent она
ничего не переписывает и требует точного совпадения legacy/vault.
`state-vault-verify` и `state-vault-restore` при pending intent fail-closed,
чтобы пользователь не принял устаревший active snapshot за завершённое зеркало.

Restore дополнительно запрещён внутрь source `STATE_DIR`: иначе восстановленные
файлы сами немедленно изменили бы canonical source snapshot.

## 8. Проверки M0.8.2

Unit tests покрывают:

- generation 1 → unchanged 1 → changed 2;
- crash после intent и recovery generation 2;
- injected abort во время snapshot transaction: active generation остаётся 2,
  intent сохраняется, retry создаёт generation 3;
- поддельный intent с неверным authenticator;
- external legacy drift без intent;
- CLI guard normal completion, simulated process drop и next-start recovery;
- запрет restore внутрь source state.

Release process smoke `.tmp/m082-smoke-20260901-080000`:

- мигрировал 23 файла реального M0.7.9 recipient state как generation 1;
- `identity` прошёл полный lifecycle как `already-current`, оставив generation 1;
- live `ratchet-prekey-pool --refresh` увеличил persistent state с 26,037 до
  31,750 bytes и автоматически создал generation 2;
- verify и explicit recover сообщили exact match/no pending recovery;
- restore побайтно совпал со всеми 23 legacy files и прочитал те же 3 history
  events;
- raw DB scan не нашёл fixture plaintext/path markers.

## 9. Ограничения и следующий этап

- changed command пока заново читает и перешифровывает весь state: `O(state)`;
- read-only command делает две небольшие metadata transactions;
- live reads всё ещё идут только из legacy files;
- generation не защищает от полного согласованного rollback DB+key;
- соседний key file не является production keystore;
- Account Root directories не входят в device-state coordinator.

Следующий этап M0.8.3 должен разбить snapshot на typed repositories и
incremental encrypted transactions для mutable ratchet/sequence/trust state и
immutable events/projections/recovery records. Сначала каждый DB read должен
сравниваться с legacy read в shadow mode; primary read cutover допустим только
после equivalence/fault tests и versioned migration policy.
