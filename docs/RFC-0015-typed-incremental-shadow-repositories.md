# RFC-0015: typed incremental shadow repositories (M0.8.3)

Статус: реализовано в M0.8.3

## 1. Задача и граница

M0.8.2 сделал encrypted vault recoverable live-зеркалом, но при любом
изменении заново шифровал и записывал весь device state. Это сохраняло
корректность, однако не давало безопасной основы для будущих небольших
транзакций и переключения primary reads.

M0.8.3 вводит два независимых свойства:

- encrypted transaction изменяет только новые, изменённые и удалённые records;
- каждый record относится к стабильному логическому типу, а shadow read
  побайтно сравнивает DB и retained legacy view отдельно по этим типам.

Legacy filesystem всё ещё является primary read/write store. Этот RFC не
разрешает DB-primary cutover и не удаляет legacy files.

## 2. Типизированный inventory

`StateRecordKind` задаёт девять исчерпывающих категорий:

| Kind | Legacy paths |
|---|---|
| `device-identity` | `device-secret.key`, `device-encryption-secret.key` |
| `ratchet` | `ratchet/**` |
| `event` | `events/**` |
| `local-projection` | `local-messages/**` |
| `history-rewrap` | `history-rewraps/**` |
| `history-recovery` | `history-recovery/**` |
| `trust` | authority snapshot, device certificate, conversation memberships |
| `sequence` | `next-sequence` |
| `other` | неизвестные будущие записи |

`other` намеренно не отбрасывается: новый неизвестный файл остаётся частью
authenticated snapshot и видим в диагностике. Это позволяет exact
сравнивать его содержимое и не потерять при mirror; ненулевой `other` сам по
себе не блокирует shadow mode, но не получает DB-primary adapter.

Публичный `TypedStateRepository` возвращает для каждого kind число records и
plaintext bytes. Перед отчётом vault полностью аутентифицируется, затем для
каждого relative path сравниваются exact path, record version и content из DB
и legacy. Первое расхождение сообщает kind и локальный relative path.

CLI-команда:

```text
state-vault-shadow-read --state-dir <DIR>
```

печатает все девять строк `vault_shadow_kind=...`, после чего только при полном
равенстве возвращает `shadow_reads_equal=true`. Pending mirror intent должен
быть сначала восстановлен; устаревший active snapshot не принимается за
успешный shadow read.

## 3. Incremental delta

После live-команды coordinator читает authenticated active records и текущее
legacy state, строит map по canonical relative path и разделяет результат на:

- `upserted_records`: новый path или изменённый content;
- `removed_records`: path был в active vault, но отсутствует в legacy;
- `unchanged_records`: exact path+content совпадают.

Только upsert records получают новый XChaCha20-Poly1305 nonce и ciphertext.
Неизменённые DB values не затрагиваются. Для удалений вычисляется тот же keyed
BLAKE3 lookup key и удаляется только соответствующая запись.

Upserts, removals, новый manifest, authenticated `generation + 1` и удаление
mirror intent входят в одну `redb` transaction с immediate durability. Поэтому
читатель видит либо старое поколение вместе с intent, либо полностью новое
поколение без intent. Fault до commit не публикует даже частично выполненный
delta, а следующий запуск повторяет его из retained legacy state.

`VaultMirrorCommit` заменяет нерасширяемую пару outcome/report и содержит:

- `VaultMirrorOutcome`;
- итоговый `VaultReport`;
- `VaultMirrorDelta` с тремя счётчиками.

Live CLI выводит эти счётчики после normal completion и crash recovery.
Read-only lifecycle возвращает `0 upsert / 0 remove / N unchanged`, не меняет
generation и очищает только intent metadata.

## 4. Сложность и гарантии

M0.8.3 уменьшает объём шифрования и записей в БД с `O(state)` до
`O(changed + deleted)`. Сбор legacy tree, расшифровка active records и exact
сравнение пока остаются `O(state)`. Это сознательный безопасный промежуточный
срез: оптимизация чтения не должна опережать доказанный typed equivalence.

Incremental commit не меняет:

- vault schema v1 и формат encrypted record;
- keyed manifest и snapshot ID;
- M0.8.2 intent/generation recovery protocol;
- wire protocol, Event ID, ticket или ALPN;
- E2EE semantics прикладных событий.

## 5. Fail-closed поведение

- Необъяснённое DB/legacy расхождение без intent блокирует live-команду.
- Typed shadow mismatch называет категорию, но не делает legacy автоматически
  авторитетным.
- Pending intent блокирует verify/shadow-read/restore до recovery.
- Неизвестный path классифицируется как `other`, а не игнорируется; silent
  primary cutover для него запрещён.
- Отмена delta transaction оставляет прежние records, manifest, generation и
  intent видимыми как одну согласованную версию.

Cryptographic collision keyed lookup/snapshot hash остаётся принятой
стандартной предпосылкой. Generation по-прежнему не является внешним rollback
witness: согласованная замена DB и key старой копией не обнаруживается.

## 6. Проверки M0.8.3

Unit tests доказывают:

- changed/new/deleted paths дают точные `2/1/7` delta counters;
- ciphertext неизменённой identity-записи byte-for-byte сохраняется;
- ciphertext изменённой ratchet-записи заменяется;
- удалённый event отсутствует в active table;
- девять typed categories дают ожидаемый inventory;
- намеренный drift local projection возвращает typed mismatch;
- injected abort сохраняет прежнее active generation и authenticated intent;
- crash recovery и unchanged lifecycle сохраняют прежние свойства M0.8.2.

Все 84 workspace tests, rustfmt, strict Clippy и release build проходят.

Release process smoke `.tmp/m083-smoke-20260901-100000` мигрировал 23 файла
реального M0.7.9/M0.8.2 state (31,750 bytes) как generation 1. `identity`
вернул delta `0/0/23`. Реальная ротация signed prekey pool изменила только два
ratchet records: `2/0/21`, generation 2 и 34,332 bytes. Typed shadow inventory
совпал по всем девяти категориям; restore дал 23 byte-exact legacy files и те
же 3 читаемых history events. Raw DB scan не нашёл `m078-secret`,
`device-secret.key` или `local-messages`.

## 7. Ограничения и следующий этап

- primary reads/writes всё ещё идут через filesystem;
- exact shadow comparison всё ещё `O(state)` на каждую live-команду;
- `other` — диагностический fallback, а не разрешение DB-primary adapter для
  неизвестной schema;
- master key остаётся соседним development-файлом;
- secure deletion, external rollback witness и versioned migrations не
  реализованы;
- Account Root directories не входят в device-state coordinator.

M0.8.4 должен начать ограниченный DB-primary canary с наименее рискованных
immutable repositories (`event` и `local-projection`): typed adapter читает DB,
одновременно проверяет legacy shadow result и при расхождении fail-closed без
тихого fallback. Mutable ratchet/trust/sequence остаются legacy-primary до
отдельных fault и migration tests. Protected key provider и rollback witness
остаются самостоятельными security stages.
