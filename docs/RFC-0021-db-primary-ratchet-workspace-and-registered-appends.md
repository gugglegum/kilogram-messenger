# RFC-0021: DB-primary ratchet workspace and registered appends (M0.8.9)

Статус: реализовано в M0.8.9

## 1. Задача и граница

M0.8.8 читал `next-sequence` из authenticated vault, но `RatchetState` всё ещё
загружал retained `ratchet/**`, а append-only delta находил новые records вторым
обходом всех history directories после операции.

M0.8.9 вводит два связанных контракта:

- каждая ratchet-операция начинает работу с authenticated DB generation;
- каждый append-only writer заранее регистрирует точные canonical paths в
  активной `StateTransaction`.

Retained filesystem пока остаётся crash-журналированным staging и exact shadow.
Ratchet crate ещё не пишет непосредственно в redb, а active DB manifest всё ещё
полностью decrypt/re-hash при commit.

## 2. DB-primary ratchet workspace

`read_mutable_primary_canary` теперь разрешает `ratchet` и `sequence` при тех же
fail-closed условиях M0.8.8: authenticated manifest/generation, exact active
outer intent и отсутствие pending primary-shadow marker.

`CommandTransactionContext::load_ratchet_state`:

1. читает только ratchet records из vault;
2. проверяет root, kind, canonical path и отсутствие duplicates;
3. создаёт durable primary backup внутри активного crash journal;
4. заменяет retained `ratchet/**` DB-снимком;
5. только после этого открывает `RatchetState` над staging tree.

После операции direct commit сравнивает staging ratchet с DB baseline, а не с
предкомандным filesystem shadow. Поэтому retained drift не становится входом
encrypt/decrypt/prekey операции. При штатной ошибке и после process crash
journal восстанавливает DB-primary backup, а не потенциально изменённый shadow.
Тот же primary-backup invariant теперь применяется к `next-sequence`.

Внешний `VaultDualWriteGuard` по-прежнему сначала требует exact shadow. Drift,
существовавший до открытия authenticated intent, отклоняется ещё раньше и
должен проходить явный recovery path.

Диагностика:

```text
vault_mutable_read_kind=ratchet
vault_mutable_read_source=db-primary
vault_mutable_read_generation=3
vault_mutable_read_record_count=6
vault_ratchet_workspace=prepared
vault_ratchet_workspace_committed=true
```

## 3. Явный append-only write-set

`StateTransaction::register_append_only_write` принимает только paths под:

- `events/**`;
- `local-messages/**`;
- `history-rewraps/**`;
- `history-recovery/**`.

CLI регистрирует `.event` и `.authorization` для каждого `AuthorizedEvent`,
local projection, rewrap bundle/transfer и recovery checkpoint до записи.
Delivery, acknowledgement, connect, sync, history import и seed-history
переведены на этот API.

При построении direct delta transaction больше не перечисляет append-only
directories второй раз. Он проверяет существование baseline paths и читает
payload только у зарегистрированных paths. Повторная регистрация идемпотентна.
Изменение уже committed append-only payload отклоняется сравнением с
authenticated DB record; removal baseline record также остаётся запрещён.

Начальный directory walk пока сохраняется в crash journal для rollback списка,
а post-commit exact confirmation всё ещё сканирует retained tree. Пропущенный
caller registration поэтому не попадёт в DB delta и будет выявлен final exact
confirmation, но production repository-owned write receipts остаются следующим
усилением API.

Диагностика commit:

```text
vault_primary_write_mode=typed-registered-delta
vault_append_write_set_count=3
```

## 4. Production wiring

Ни одна live CLI ratchet operation больше не создаёт `RatchetState` до
transaction context. Listener prekey publication, delivery decrypt, connect
fan-out, sync projection creation, prekey export и seed-history используют
DB-primary workspace на каждой transaction boundary.

Sync больше не держит filesystem-backed `RatchetState` между rounds. Каждый
committed batch загружает свежее active vault generation, применяет decrypt
изменения и атомарно коммитит ratchet, projections и events в одной границе.

## 5. Проверки

State/vault tests доказывают:

- DB ratchet snapshot заменяет изменённый retained shadow до использования;
- operation rollback восстанавливает ratchet и sequence из primary backup;
- registered new records входят в typed delta, незарегистрированный path не
  участвует во втором scan, потому что второго scan больше нет;
- append-only removal и modification-in-place отклоняются;
- wrong root/kind, duplicate и pending marker остаются fail-closed.

CLI test повреждает ratchet secret после открытия outer intent, затем успешно
загружает рабочий ratchet из DB, коммитит один зарегистрированный canary event и
получает exact generation `1 -> 2`.

Release process smoke `.tmp/m089-smoke-20260901-193740` выполнил Alice/Bob
delivery и acknowledgement. Обе стороны сообщили DB-primary ratchet и sequence,
режим `typed-registered-delta`; append write-set был `3` у client и `5` у
listener. Source/listener generations завершились как `4/3`, history совпала,
final compatibility mirror остался `already-current`.

Все 92 workspace tests, rustfmt, strict Clippy и release build проходят.

## 6. Ограничения и следующий этап

Это ещё не native redb ratchet repository: vodozemac работает над временно
гидратированным retained staging tree. Ratchet backup bounded размером session/
prekey state, но filesystem остаётся участником write path.

Начальный append baseline walk, active DB full decrypt/re-hash и post-commit
exact shadow scan сохраняют `O(state)` части. Trust reads/writers используют
filesystem compatibility bridge. Development master key лежит рядом с DB.

Этот следующий этап реализован в M0.8.10 и описан в
[`RFC-0022`](RFC-0022-repository-write-receipts-and-manifest-index.md): domain
writers возвращают exact receipts, а schema-v2 direct commit обновляет
authenticated index без чтения неизменённых DB payload records. Initial/final
shadow gates пока остаются full-state. Следующим storage cutover становится
DB-primary trust repository; protected key provider, rollback witness и bounded
backup остаются отдельными security stages.
