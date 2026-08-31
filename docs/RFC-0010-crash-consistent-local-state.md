# RFC-0010: crash-consistent local state transaction

- Статус: **Implemented spike (M0.7.7)**
- Дата: 2026-09-01
- Область: локальное состояние одного устройства
- Не меняет: wire protocol, Event ID, ticket v9, sync/session v6

## 1. Проблема

До M0.7.7 одна логическая операция записывала несколько независимых файлов:

1. расходовала или продвигала Olm/Double Ratchet state;
2. увеличивала `next-sequence`;
3. создавала зашифрованную локальную plaintext projection;
4. сохраняла immutable event и authorization sidecar;
5. при inbound PreKey могла ротировать опубликованный prekey pool.

Падение между этими шагами могло оставить использованный message key без event
или event без читаемой локальной projection. Повторный запуск обнаружил бы часть
конфликтов, но не мог бы безопасно восстановить состояние до операции.

M0.7.7 вводит минимальную файловую транзакцию для текущего M0 store и
эксклюзивную блокировку всего `STATE_DIR`.

## 2. Инвариант

После завершения команды или recovery локальная операция находится ровно в
одном из двух состояний:

- **до операции**: исходные ratchet и sequence восстановлены, а все новые
  event/projection/rewrap files удалены;
- **после commit**: все записи операции сохранены, journal можно только удалить.

Prepared journal без commit никогда не трактуется как успешная операция.
Committed journal никогда не откатывается, даже если процесс упал при его
последующей очистке.

## 3. Эксклюзивный state lock

Каждая CLI-команда с `--state-dir` перед чтением или записью:

1. создаёт и canonicalize-ит каталог;
2. открывает `STATE_DIR/.kilogram-state.lock`;
3. получает неблокирующий exclusive OS file lock;
4. под lock выполняет recovery незавершённой транзакции;
5. держит file handle до полного завершения async-команды.

Второй процесс завершается с явной ошибкой `state directory ... is already
locked`. Lock действует только для процессов, которые соблюдают этот контракт;
он не защищает от ручной правки файлов или другой программы.

Account Root каталоги пока не используют этот lock: RFC охватывает device
`STATE_DIR`, а не параллельные offline root operations.

## 4. Journal layout

```text
STATE_DIR/
  .kilogram-state.lock
  .kilogram-transactions/
    active/
      manifest.json
      prepared
      committed        # появляется только после успешной операции
      rolled-back      # появляется после полностью выполненного rollback
      backup/
        ratchet/
        next-sequence
```

Одновременно допускается только одна active transaction. До marker `prepared`
прикладная мутация не начинается.

Manifest v1 фиксирует:

- существовал ли `ratchet` до операции;
- существовал ли `next-sequence`;
- полный список файлов в append-only roots на момент begin.

Symlink в transaction-managed дереве отклоняется. Пути из manifest обязаны быть
относительными и состоять только из normal path components.

## 5. Что откатывается

Mutable state копируется целиком:

- `STATE_DIR/ratchet`;
- `STATE_DIR/next-sequence`.

Для append-only roots сохраняется только baseline имён файлов:

- `STATE_DIR/events`;
- `STATE_DIR/local-messages`;
- `STATE_DIR/history-rewraps`.

Эти stores уже используют immutable/noclobber записи. Поэтому rollback не
переписывает существующие данные, а удаляет только файлы, отсутствовавшие в
baseline. Такой алгоритм также убирает temporary files, оставшиеся после
аварийно прерванной immutable write.

Journal не содержит plaintext message bodies. Ratchet backup дублирует уже
зашифрованные pickle/session files, а projection остаётся зашифрованной локальным
device key.

## 6. Протокол состояний

```text
нет active
   |
   | snapshot + manifest + prepared
   v
prepared -----------------------> committed -----> cleanup
   |
   | operation error / recovery
   v
restore + remove new files -----> rolled-back ---> cleanup
```

Recovery:

- active без `prepared` — операция ещё не начиналась, удалить journal;
- `prepared` без terminal marker — повторить идемпотентный rollback;
- `committed` — сохранить новое состояние и удалить journal;
- `rolled-back` — считать восстановление завершённым и удалить journal.

До удаления текущего mutable state recovery проверяет version, все manifest
paths и наличие/type нужных backups. Повреждённый journal останавливает запуск
fail-closed и не начинает частичный destructive restore.

Marker пишется через `create_new`, затем file `sync_all`. Manifest и backups
также синхронизируются до `prepared`. Это M0 crash-consistency contract для
обычной локальной файловой системы, а не защита от повреждения носителя,
ложного `fsync`, потери всего диска или злонамеренной offline-модификации.

## 7. Границы прикладных операций

M0.7.7 использует transaction для:

- listener prekey publication/rotation;
- connect prekey high-water observation;
- connect sequence + ratchet fan-out + authored projection + event/sidecar;
- listener decrypt/prekey consumption + received projection/event +
  acknowledgement sequence/event;
- получаемого или локально материализуемого sync batch;
- всего `seed-history` batch;
- `history-rewrap-import` bundle + projections + events;
- явного `ratchet-bundle` и `ratchet-prekey-pool` state update.

Сетевой send выполняется после local commit. Если процесс падает после commit,
но до peer acknowledgement, повторная доставка опирается на immutable Event ID
и существующую local projection, а не повторно расшифровывает то же ratchet
ciphertext.

Ошибка sync после rollback завершает текущую команду. Это важно: in-memory
ratchet уже мог продвинуться и не используется повторно после восстановления
его disk snapshot.

## 8. Проверки

Автоматические tests подтверждают:

- второй file lock того же canonical `STATE_DIR` отклоняется, после drop lock
  снова доступен;
- явный operation error восстанавливает ratchet/sequence и удаляет новые
  event/projection files;
- оставленный prepared journal автоматически откатывается следующим lock;
- committed marker после симулированного падения сохраняет новое состояние;
- unsafe relative path в повреждённом manifest отклоняется fail-closed;
- CLI wrapper выполняет тот же rollback для всех managed roots;
- существующие delivery, sync, ratchet, history rewrap и authority tests
  продолжают проходить.

## 9. Ограничения

- Snapshot всего ratchet tree и полный список append-only файлов имеют стоимость
  `O(local state)` и предназначены только для M0 filesystem store.
- Production storage должен заменить это одной зашифрованной transactional DB
  с WAL, bounded recovery и проверяемыми migrations.
- Authority snapshots, certificates, memberships и произвольные output files
  вне managed roots не входят в эту transaction.
- Lock не координирует разные устройства и не является distributed consensus.
- Hardware failure, bit rot и hostile local administrator требуют backups,
  authenticated at-rest structures и отдельной threat model.

## 10. Следующий этап

M0.7.8 выполнен в [`RFC-0011`](RFC-0011-network-history-rewrap.md): authenticated
history rewrap перенесён с ручной файловой границы в device-to-device session,
добавлены явное согласие обеих сторон, SAS, bounded request/transfer и
reconciliation claims нескольких источников без обещания глобальной полноты.
