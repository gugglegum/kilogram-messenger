# RFC-0035: Bounded Windows recovery worker (M0.9.8)

Статус: реализовано в M0.9.8

Дата: 2026-09-02

## 1. Задача

M0.9.6 сохраняет recipient-signed retry deadline и terminal cancellation, а
M0.9.7 получает Windows-native network/power snapshot. До этого coordinator
всё равно требовал внешнего запуска: он не ждал подписанный deadline, не
реагировал на смену сети/питания и проверял policy только один раз.

M0.9.8 добавляет ограниченный worker-процесс, но намеренно не регистрирует его
как Windows service или Task Scheduler task. Регистрация меняет состояние ОС и
остаётся отдельным platform integration этапом.

## 2. Команда и пределы

```powershell
kilogram-cli history-recovery-plan-watch `
  --state-dir .\recipient `
  --plan-file .\approved-recovery.plan `
  --conversation example
```

Worker имеет два независимых hard bound:

- runtime: `1..=86400` секунд, по умолчанию 3600;
- meaningful wakeups: `1..=1024`, по умолчанию 64.

Проверка внешней signed cancellation выполняется не реже
`--cancel-poll-seconds` (`1..=30`, default 5). Polling timeout не считается
meaningful wakeup. Worker завершается чисто при runtime/wakeup bound,
`Completed` или `Cancelled` и не превращается в бесконечный daemon.
Если runtime заканчивается внутри network attempt, future прекращается, а
оставшийся signed `Attempting` lease немедленно переводится в signed failure
перед выходом; короткая fail-closed cleanup может следовать после deadline.

## 3. Источники пробуждения

Windows adapter подписывается через WinRT на:

- `NetworkInformation.NetworkStatusChanged`;
- `PowerManager.PowerSupplyStatusChanged`;
- `PowerManager.BatteryStatusChanged`;
- `PowerManager.EnergySaverStatusChanged`.

Callback только увеличивает process-local sequence и будит Tokio waiter. Он не
сохраняет profile name, SSID, adapter ID, IP или иной сетевой metadata. RAII
registrations снимают все event tokens при завершении worker.

Помимо native events worker ждёт точный `next_attempt_at_unix_seconds` из
проверенного recipient-signed scheduler state. Внешняя смена scheduler state
также обнаруживается по signed state ID. Между наблюдениями worker не держит
exclusive lock `STATE_DIR`; краткий typed lock conflict с cancel/foreground
командой повторяется с ограниченным ожиданием.

## 4. Повторная policy-проверка

Native context теперь читается заново:

1. непосредственно перед созданием attempt lease и UDP discovery;
2. после выбора единственного signed descriptor, непосредственно перед Iroh
   connection.

Если первая проверка запрещает текущую сеть/питание, lease, UDP и connection не
создаются. Если context изменился после discovery, connection не открывается,
started lease завершается signed failure и получает bounded retry deadline.
Worker распознаёт такую race как policy block и снова ждёт native event, не
ослабляя recipient-signed policy.

Manual paired context override сохранён только для deterministic development
tests. Его `refreshed()` остаётся тем же caller-supplied value; production
worker всегда использует native provider.

## 5. Cancellation и expiry

Обычный active scheduler state требует ещё действующий plan. Однако уже
подписанный terminal `Cancelled`/`Completed` state можно проверить и обработать
после expiry plan: worker должен безопасно завершиться, а не продолжать ждать
просроченный consent. Это не разрешает новую попытку после expiry.

Cancellation отдельного процесса может совпасть с коротким worker-read. Worker
повторяет только typed `StateError::AlreadyLocked`; текстовые или иные I/O
ошибки не маскируются. После terminal cancel он больше не выполняет discovery
или connection.

## 6. Совместимость и проверки

Recovery plan v1, scheduler state v1, URI v1, ticket v9, ALPN
`kilogram/m0/sync/7`, wire messages и recovery checkpoints не изменились.
Добавлена только Tokio `sync` feature для process-local event notification.

- unit tests проверяют no-lost-wakeup signal, bounded wait calculation и typed
  lock-contention recognition;
- M0.9.6 regression `.tmp/m096-smoke-20260902-171150` снова прошёл persistent
  attempt, deferred restart, recovery двух pages, identical history и cancel;
- Windows worker smoke `.tmp/m098-smoke-20260902-173045` зарегистрировал native
  event subscriptions, выполнил no-candidate attempt, освободил state lock во
  время ожидания и принял signed cancellation из второго процесса за 1.733 s;
- отдельный active discovery был прерван runtime bound, а Attempting lease
  немедленно записан как signed failure перед выходом за 4.022 s с cleanup;
- после cancel worker завершился с `connection_attempted=false`.

## 7. Что остаётся дальше

- явная установка/удаление Windows Task Scheduler task или service с
  least-privilege identity и безопасной передачей plan path;
- wake компьютера из sleep и обработка logon/logoff/reboot lifecycle;
- macOS/Linux/Android/iOS providers и native event loops;
- monotonic rollback witness для всей scheduler chain;
- privacy-preserving wide-area discovery/mailbox вместо LAN-only multicast.
