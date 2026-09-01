# RFC-0018: command-local sync read overlay (M0.8.6)

Статус: реализовано в M0.8.6

## 1. Задача и граница

M0.8.4–M0.8.5 перевели read-only history и history-rewrap source на
authenticated immutable snapshot из encrypted vault. Такой snapshot нельзя
было напрямую использовать в sync: первый round мог принять события и
проекции, а следующий round продолжал бы видеть только состояние начала
команды.

M0.8.6 переводит sync inventory и events-by-ID reads на двухслойное
command-local представление:

```text
authenticated immutable base + successfully committed command overlay
```

Legacy filesystem пока остаётся write-primary и crash-recovery shadow. Этот
этап не делает direct DB writes и не удаляет retained tree.

## 2. Граница снимка

Sync-клиент захватывает `ImmutableReadRepositories` после загрузки собственной
certificate/authority, но до наблюдения ticket authority/prekey и до любых
sync writes.

Listener ещё не знает тип application request до transport authorization и
локальных authority/prekey операций. Поэтому он захватывает один immutable
read-set в начале любой команды `listen`. Если request оказывается sync, тот же
snapshot становится base overlay; если это approved history rewrap, он остаётся
source snapshot. Инициализированный, но повреждённый, stale или drifted vault
по-прежнему блокирует команду без filesystem fallback.

Состояния без vault сохраняют compatibility: base представлен проверяющими
filesystem adapters.

## 3. Два command-local overlay

`kilogram-store` предоставляет:

- `CommandEventReadOverlay` над `EventReadRepository`;
- `CommandLocalMessageReadOverlay` над `LocalMessageReadRepository`.

Event overlay объединяет base и committed layer, затем повторно проверяет:

- membership и account/device authorization каждого события;
- вычисленный Event ID;
- уникальность `(author_device_id, author_sequence)`;
- отсутствие conflicting value для существующего Event ID;
- deterministic inventory, events-by-ID и causal frontier.

Projection overlay адресуется Event ID, предпочитает committed value base
value и отклоняет conflicting ciphertext/provenance.

Оба слоя существуют только внутри одного процесса sync. После завершения
команды они не нужны: их записи уже находятся в committed filesystem
transaction и затем зеркалируются в vault штатным M0.8.2/M0.8.3 guard.

## 4. Stage → durable commit → publish

Overlay не принимает запись сразу. Контракт состоит из двух фаз:

1. `stage_committed` валидирует batch и возвращает owned staged records, не
   меняя видимый read-set;
2. CLI выполняет существующий `StateTransaction`: проверяет authorization,
   последовательно decrypt-ит ratchet messages, сохраняет local projections и
   immutable authorized events;
3. только после успешного filesystem commit вызывается `commit_staged`, и
   следующий sync round видит новые records.

При ошибке decrypt, отсутствующем recipient slot, конфликте event/projection
или filesystem I/O M0.7.7 откатывает legacy state, а staged records просто
отбрасываются. Network response формируется только после этого durable commit,
как и раньше.

`events_by_id` также может восстановить отсутствующую local projection для
полученного ранее события. Такая проекция проходит отдельную filesystem
transaction и публикуется в projection overlay только после её commit.

## 5. Multi-round sync

`DecryptingSessionStore` теперь разделяет:

- `event_writes` / `local_message_writes` — retained filesystem repositories;
- `event_reads` / `local_message_reads` — immutable base плюс command overlay.

`inventory` и `events_by_id` читают только второй слой. `put_events` пишет через
первый слой и публикует результат во второй. Поэтому после batch 64 следующий
round строит inventory уже с этими 64 Event ID и запрашивает только остаток, а
не повторяет первый batch.

CLI печатает физический source и размер реально опубликованного overlay:

```text
sync_primary_read=encrypted-vault
sync_shadow_read=legacy-verified
sync_overlay_committed_events=73
sync_overlay_committed_local_projections=73
```

Нулевой размер допустим: например, сторона только отправляла уже имевшиеся
события или legacy-compatible base сам увидел committed filesystem records.

## 6. Проверки M0.8.6

Unit/integration проверки доказывают:

- staged event/projection невидимы до `commit_staged`;
- после commit inventory, events-by-ID, projection lookup и frontier включают
  overlay;
- conflicting writer sequence отклоняется;
- два последовательных sync batches поверх пустого immutable vault base дают
  inventory 2, затем 3;
- повторная запись идемпотентна;
- event без local recipient slot не меняет ни filesystem, ни overlay;
- mirror guard после нескольких batches публикует одну согласованную vault
  generation.

Все 85 workspace tests, rustfmt, strict Clippy и release build проходят.

Release process smoke `.tmp/m086-smoke-20260901-160635` передал 73 события
двумя rounds `64 + 9`. Listener начал с пустого encrypted-vault event/projection
snapshot, завершил с overlay `73/73`, vault generation 2 и историей из 73
событий, побайтно совпадающей с source event history. Отдельный projection
shadow drift дал exit code 1 без legacy fallback.

## 7. Ограничения и следующий этап

Overlay не является persistent database, межпроцессным журналом или заменой
M0.7.7. Он решает только видимость records, committed в течение одной sync
команды. Full vault authentication/shadow comparison всё ещё `O(state)`, writes
сначала попадают в legacy files, а development master key лежит рядом с DB.

Следующий логичный этап M0.8.7 — ввести transactional vault-primary write
repositories для immutable events и local projections с retained legacy shadow,
failure injection и доказанной атомарностью. Ratchet/trust/sequence cutover,
защищённый key provider, rollback witness, migrations и bounded backup остаются
отдельными этапами.
