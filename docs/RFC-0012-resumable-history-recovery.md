# RFC-0012: возобновляемое восстановление истории (M0.7.9)

Статус: реализовано в M0.7.9
Дата: 2026-09-01

## 1. Задача

M0.7.8 передавал только один ограниченный диапазон истории за один запуск
listener. Для длинного разговора пользователь должен был вручную вычислять
следующий индекс, а после обрыва не существовало аутентифицированной локальной
записи о том, какая страница уже импортирована.

M0.7.9 добавляет orchestration-слой поверх неизменного криптографического
протокола M0.7.8:

- signed checkpoint на recipient device для каждого явно выбранного source;
- страницы по 1–256 событий, каждая с новым session-bound signed request;
- атомарный import `bundle + transfer + events + projections + checkpoint`;
- безопасный retry после обрыва или перезапуска процесса;
- накопление независимых source claims и выбор inventory только при согласии
  как минимум двух наблюдавшихся источников.

## 2. Явные границы доверия

Recovery остаётся только same-account операцией. Recipient обязан задать
`--source-device`, `--expect-account` и ранее сверенный полный SAS через его
12-значное представление. Ticket от другого source, другой device list или
другого аккаунта не продолжает существующий plan.

Source consent теперь задаёт окно `[range_start, range_start + count)`, а не
обязательно одну страницу. Каждый фактический request должен целиком лежать в
этом окне и всё ещё ограничен `MAX_HISTORY_REWRAP_ENTRIES = 256`. Listener
обслуживает одну страницу и завершается; для следующей страницы source запускает
новый listener с тем же окном и выдаёт новый ticket.

Это намеренно не является source discovery и не разрешает клиенту молча
перебирать все устройства аккаунта.

## 3. SignedHistoryRecoveryCheckpoint v1

Checkpoint подписывается persistent signing key recipient device и содержит:

- Account ID, Conversation ID, направленные source/recipient Device ID;
- полный `HistoryRewrapSas` digest;
- утверждённое окно и фиксированный page size;
- следующий ожидаемый canonical inventory index;
- после первой страницы — source-signed inventory count и digest;
- hash предыдущего checkpoint.

`HistoryRecoveryId` вычисляется из неизменяемой части plan. `checkpoint_id`
хэширует полный подписанный checkpoint. Последовательность поэтому является
проверяемой hash chain: пропущенный, переставленный, подменённый или forked
локальный checkpoint отклоняется.

Checkpoint не является секретом и не содержит plaintext сообщений. Он
аутентифицирует локальный прогресс, но не защищает от полного удаления всей
цепочки владельцем файловой системы и не доказывает глобальную полноту истории.

## 4. Authenticated pagination

Команда `history-recovery-resume` сначала восстанавливает последнюю корректную
цепочку checkpoint. Затем она:

1. проверяет source certificate, root-signed device list, Account ID, точный
   source Device ID и SAS из нового ticket;
2. запрашивает диапазон от `next_range_start` длиной не более `page_size`;
3. проверяет recipient signature/session binding исходного request и source
   signature точного transfer;
4. требует неизменный `(inventory_event_count, inventory_digest)` относительно
   первой импортированной страницы;
5. импортирует страницу и новый checkpoint одной journal transaction.

Если соединение оборвалось до шага 5, checkpoint не продвигается и повторный
запуск запрашивает ту же страницу. Если commit завершился, следующий запуск
начинается с нового индекса. Перекрывающий retry остаётся идемпотентным за счёт
immutable content-addressed event/projection stores.

Plan завершён при достижении меньшего из конца утверждённого окна и заявленного
source inventory count. Повторный запуск завершённого plan не открывает сетевое
соединение.

## 5. CLI workflow

Source один раз одобряет всё окно, но создаёт свежий ticket для каждой страницы:

```powershell
.\kilogram-cli.exe listen `
  --state-dir .\source `
  --allow-account $AccountId `
  --device-list-file .\account.devices `
  --peer-prekey-pool-file .\recipient.pool `
  --ticket-file .\page-1.ticket `
  --history-rewrap-conversation chat `
  --history-rewrap-recipient-device $RecipientDeviceId `
  --history-rewrap-approve-sas $Sas `
  --history-rewrap-range-start 0 `
  --history-rewrap-count 1000
```

Recipient повторяет одну и ту же команду с очередным свежим ticket. Индекс
страницы вручную не задаётся:

```powershell
.\kilogram-cli.exe history-recovery-resume `
  --state-dir .\recipient `
  --ticket-file .\page-1.ticket `
  --conversation chat `
  --source-device $SourceDeviceId `
  --range-start 0 `
  --count 1000 `
  --page-size 64 `
  --confirm-sas $Sas `
  --expect-account $AccountId
```

CLI печатает `history_recovery_next_range_start` и
`history_recovery_complete`. Изменение source, окна, page size или device list
создаёт другой recovery plan и не может незаметно продолжить прежнюю цепочку.

## 6. Несколько источников и выбор claim

Для второго явно выбранного device recipient запускает отдельный plan с другим
`--source-device` и его role-bound SAS. Все страницы сохраняют прежние
source-signed `.rewrap` claims. `history-rewrap-reconcile` объединяет диапазоны
по source и сравнивает полные inventory claims.

`inventory_selection=agreed` и поля `selected_inventory_*` появляются только
когда минимум два полных source claim имеют одинаковые count и digest и нет
same-source equivocation. `single-source`, `divergent` и `incomplete` не
выбирают inventory автоматически. Даже agreed результат всегда сопровождается
`global_completeness_proven=false`.

## 7. Crash consistency и хранение

Checkpoint-файлы append-only и располагаются в
`STATE_DIR/history-recovery/*.checkpoint`. Этот root добавлен в journal M0.7.7.
Prepared transaction без commit удаляет одновременно новые checkpoint,
`.rewrap`, `.transfer`, events и projections. Каждый файл подписан и сохраняется
через durable temporary file + `persist_noclobber`.

Production по-прежнему должен заменить filesystem journal на encrypted
transactional database/WAL. Metadata checkpoint сейчас не шифруется at rest.

## 8. Совместимость

Wire objects M0.7.8 не изменились: каждая страница использует прежние
`SignedHistoryRewrapRequest` и `SignedHistoryRewrapTransfer`. Поэтому ALPN
остаётся `kilogram/m0/sync/7`, ticket — v9. Новый checkpoint — локальный объект,
который не передаётся peer.

Старый source M0.7.8 ожидал exact range/count и отклонит меньшую страницу из
большого окна. Это fail-closed несовпадение поведения, а не принятие запроса за
границами consent.

## 9. Проверки M0.7.9

- protocol test проверяет подпись, hash-chain link, продвижение, завершение,
  неверного signer и повтор страницы;
- state test проверяет rollback нового `history-recovery` append-only root;
- CLI tests проверяют consent window и границы страниц;
- все workspace tests, rustfmt, строгий Clippy и release build проходят;
- process smoke двумя последовательными direct Iroh sessions импортирует
  диапазоны `0..2` и `2..3`, создаёт два checkpoint, повторно не выходит в сеть
  после completion и получает историю, идентичную source.

## 10. Ограничения и следующий этап

- ticket/source listener пока создаются вручную на каждую страницу;
- автоматического discovery, фонового scheduler и QR/device-link ceremony нет;
- source inventory должен оставаться неизменным в пределах plan; смена claim
  требует осознанно начать новый plan;
- agreement наблюдавшихся sources не является consensus или global checkpoint;
- удаление всей локальной checkpoint chain не обнаруживается без внешнего
  witness/backup;
- metadata, timing, размеры и факт обращения к relay остаются видимыми.

Первый шаг production-oriented локального хранилища реализован M0.8.1 в
[`RFC-0013`](RFC-0013-encrypted-transactional-state-vault.md): encrypted
transactional shadow snapshot, полная verify и safe restore без destructive
cutover. Следующий storage-срез должен перевести live repositories на
versioned dual-write и сравнивать legacy/DB reads до переключения primary
store. После этого можно безопаснее строить постоянный background recovery
coordinator и UX привязки нового устройства.
