# RFC-0017: vault-primary history rewrap sources (M0.8.5)

Статус: реализовано в M0.8.5

## 1. Задача и граница

M0.8.4 впервые передал прикладному decoder bytes из encrypted vault, но только
для команды `history`. Оба пути, создающие history-rewrap bundle, продолжали
повторно открывать retained legacy `events` и `local-messages`:

- ручной `history-rewrap-export`;
- source-side ответ listener на `HistoryRewrap` request.

M0.8.5 переводит эти два read-only source-history пути на тот же
authenticated owned snapshot. Сформированные bundle/transfer по-прежнему
записываются обычным M0.7.x кодом, а все sync/delivery/import writes остаются
legacy-primary и затем зеркалируются в vault по M0.8.2/M0.8.3.

## 2. Единый immutable read-set

CLI использует `ImmutableReadRepositories`, который один раз выбирает physical
source:

- vault полностью отсутствует — filesystem adapters и явные
  `legacy-filesystem` / `not-enabled` diagnostics;
- vault инициализирован — полная authentication, matching live-intent check и
  exact comparison всего retained legacy tree, после чего только выбранные
  `event` / `local-projection` records превращаются в owned snapshots;
- partial, damaged, stale или drifted vault — ошибка без filesystem fallback.

Read-set содержит `Box<dyn EventReadRepository>` и
`Box<dyn LocalMessageReadRepository>`. Оба traits теперь `Send + Sync`, поэтому
owned snapshot можно безопасно удерживать через async transport awaits без
повторного открытия legacy path.

`EventReadRepository` дополнительно предоставляет verified
`authorized_inventory` и `authorized_events_by_id`. Filesystem store вызывает
свои существующие реализации, а immutable snapshot строит результат через
тот же `load_authorized_conversation`, повторно проверяя membership,
authorization и event signatures. Эти методы подготавливают read-side API для
будущего sync overlay, но M0.8.5 не переключает sync.

## 3. Ручной export

`history-rewrap-export` захватывает immutable read-set до установки переданного
authority snapshot и до построения inventory. Общий
`build_history_rewrap_bundle` принимает только read-only traits, поэтому он не
может обратиться к filesystem-specific API.

После этого неизменными остаются прежние проверки:

- source и recipient находятся в одном root-signed device list;
- source certificate совпадает побайтно;
- conversation membership разрешает source account;
- выбираются только `RatchetText` events;
- каждая local projection открывается для source account/device;
- range ограничен `MAX_HISTORY_REWRAP_ENTRIES`;
- bundle подписан source и зашифрован на recipient certificate.

Успешная команда дополнительно печатает:

```text
history_rewrap_primary_read=encrypted-vault
history_rewrap_shadow_read=legacy-verified
history_rewrap_vault_generation=1
history_rewrap_vault_event_records=6
history_rewrap_vault_local_projection_records=3
```

## 4. Сетевой source

Listener захватывает immutable source snapshot только когда задан полный
explicit rewrap approval. Захват происходит до command-local authority/prekey
operations, accept, requester authority pinning и application request. Поэтому
последующая выдача диапазона использует ровно один подтверждённый source view,
а не состояние, повторно открытое после transport-side mutations.

Listener без rewrap approval не платит стоимость canary и сохраняет прежний
явный `HistoryRewrapRejected(NotApproved)` ответ. При approval, но отсутствующем
snapshot, source завершается ошибкой; downgrade не разрешён.

SAS, same-account restriction, session-bound recipient request, exact approved
window, source-signed transfer и wire-size limit не изменены.

## 5. Почему sync ещё не переключён

History rewrap только читает immutable events/projections, захваченные в начале
команды. Sync одновременно:

1. строит inventory;
2. принимает новые events;
3. создаёт local projections;
4. может в следующем round снова читать уже изменившийся набор.

Owned snapshot начала команды не увидит пункты 2–3. Простая замена filesystem
store на snapshot привела бы к stale inventory, повторным запросам или потере
видимости локально committed records. До cutover нужен один из двух явных
контрактов:

- command-local overlay: immutable vault base плюс verified in-memory/new-record
  layer, который обновляется только после успешного local commit;
- direct transactional DB repository writes с согласованным read transaction и
  crash protocol для retained legacy shadow.

M0.8.5 только расширяет read trait этими будущими sync primitives; write path
и `SessionStore` не меняются.

## 6. Проверки M0.8.5

CLI seeded-history test:

- получает 3 authorized events через vault-primary snapshot;
- проверяет `authorized_inventory` и `authorized_events_by_id`;
- временно убирает исходные `events` и `local-messages` directories;
- успешно строит полный history-rewrap bundle из 3 entries только через owned
  snapshot;
- возвращает retained tree и штатно завершает live mirror intent.

Все 85 workspace tests, rustfmt, strict Clippy и release build проходят.

Release process smoke `.tmp/m085-smoke-20260901-153149` мигрировал реальный
source state, затем:

- manual export прочитал vault generation 1, 6 event/authorization records и
  3 projections и создал 3 entries;
- authenticated Iroh source/recipient session по direct path передала те же 3
  entries; listener сообщил `encrypted-vault` / `legacy-verified`;
- post-transfer vault verify сохранил exact generation 1;
- отдельная копия с изменённой legacy projection завершилась exit code 1, а
  вывод не содержал legacy fallback.

## 7. Следующий этап

M0.8.6 реализован в
[`RFC-0018`](RFC-0018-command-local-sync-read-overlay.md): command-local
event/projection overlay сохраняет authorization/membership validation, видит
только успешно committed writes, корректно обслуживает несколько sync rounds и
не обходит M0.7.7 rollback или M0.8.2 authenticated mirror intent.

Ratchet/trust/sequence DB-primary writes, защищённый key provider, bounded
backup, migrations и внешний rollback witness остаются отдельными этапами.
