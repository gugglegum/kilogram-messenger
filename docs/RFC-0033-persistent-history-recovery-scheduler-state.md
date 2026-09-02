# RFC-0033: persistent history recovery scheduler state (M0.9.6)

Статус: реализовано в M0.9.6

Дата: 2026-09-02

## 1. Задача

M0.9.5 сохранял consent и recovery checkpoints, но retry loop существовал только
в памяти одного процесса. После restart терялись номер попытки, failure streak и
deadline; fixed delay не защищал от синхронного retry storm. M0.9.6 добавляет
локальное долговечное состояние координатора, не меняя сетевой trust model.

Цели среза:

- продолжать exact approved plan после завершения и перезапуска CLI;
- не запускать discovery раньше подписанного `next_attempt_at`;
- применять bounded exponential backoff с equal jitter;
- не допускать параллельные попытки одного plan;
- сделать явную cancellation терминальным подписанным переходом;
- блокироваться при наблюдаемом откате wall clock.

Это всё ещё platform-neutral CLI coordinator. Он не регистрирует Windows Task
Scheduler/service и по-прежнему получает network/power context от caller.

## 2. Signed append-only state chain

Для каждого `history_recovery_plan_id` создаётся отдельная цепочка под:

```text
STATE_DIR/history-recovery/scheduler/<plan-id>/
  <generation>-<state-id>.scheduler-state
```

Record v1 ограничен 4 KiB и содержит:

- exact Plan ID и recipient Device ID;
- monotonic generation и hash предыдущего signed record;
- transition и lifecycle;
- total attempts и consecutive failures;
- `last_observed_unix_seconds`;
- `next_attempt_at_unix_seconds` и точный scheduled delay.

Каждый record подписывает exact recipient device. Filename проверяется против
signed generation/state ID. Loader читает не более 4096 records, требует
generation 0, непрерывную цепочку без fork/gap, валидные переходы и подписи.
Файлы публикуются atomic no-clobber и никогда не изменяются на месте.

Если encrypted state vault уже инициализирован, каждый переход выполняется под
коротким state lock и recoverable vault dual-write intent. Без vault подпись и
hash chain обнаруживают modification/fork/missing intermediate record, но откат
всего каталога вместе со всеми локальными high-water marks остаётся вне модели:
для этого нужен внешний monotonic witness.

## 3. State machine

Lifecycle имеет четыре состояния:

- `active` — попытка разрешена после deadline;
- `attempting` — записан lease конкретной попытки;
- `cancelled` — терминальное состояние;
- `completed` — терминальное состояние, подтверждённое recovery checkpoint.

Допустимые transitions:

1. `initialized`: generation 0, zero attempts, deadline равен observed time;
2. `attempt-started`: увеличивает total attempts и записывает bounded lease;
3. `attempt-failed`: увеличивает consecutive failures и ставит новый deadline;
4. `attempt-progressed`: сохраняет total attempts, сбрасывает failure streak и
   ставит короткий retry deadline для следующей страницы/session;
5. `cancelled`: сохраняет counters и необратимо запрещает connection;
6. `completed`: терминально связывает scheduler с уже committed checkpoint.

Terminal record не может иметь successor. Два record одной generation или
successor не от текущего State ID считаются fork и отклоняются fail-closed.

## 4. Attempt lease и concurrency

Перед UDP discovery coordinator атомарно пишет `attempt-started`. Lease
вычисляется из реальных hard bounds команды:

- discovery window;
- relay/connect/route waits;
- до 64 page timeouts;
- safety margin.

Итог ограничен двумя часами. Другой процесс видит `attempting` и до lease
deadline не запускает второй scan/connection. Если процесс аварийно завершился,
первый запуск после истечения lease записывает обычный signed failure и новый
backoff deadline, а не создаёт немедленный crash loop.

Cancellation может появиться во время discovery: перед connect runner снова
читает current State ID и в случае terminal cancel не открывает соединение.
Active transfer по-прежнему держит общий state lock; cancellation дождётся его
атомарного завершения и затем станет следующим signed transition.

## 5. Exponential equal-jitter backoff

Новые параметры run:

- `--retry-base-seconds`, default 5, maximum 300;
- `--retry-max-seconds`, default 300, maximum 3600;
- legacy alias `--retry-delay-seconds` указывает на base для старых smoke scripts.

Для failure streak `n` вычисляется:

```text
cap = min(max, base * 2^(n-1))
delay = ceil(cap / 2) + deterministic_jitter(0..floor(cap / 2))
```

Jitter доменно разделён и зависит от Plan ID, failure exponent и persistent
attempt ordinal. Он не является security secret; его задача — развести retry
разных планов без дополнительного RNG state. Delay всегда находится между
половиной exponential cap и cap. `base=max=0` оставлен только для
детерминированных локальных тестов.

`--max-attempts 1..=8` теперь ограничивает один процесс, а не lifetime плана.
После исчерпания локального бюджета команда сохраняет следующий deadline,
печатает `status=history-recovery-scheduler-scheduled` и успешно завершается.
Platform scheduler может разбудить новый процесс позднее.

## 6. Time rollback

Каждый successor обязан иметь `last_observed` не меньше предыдущего. Если
текущее wall-clock время меньше signed high-water mark, runner печатает
`history_recovery_scheduler_clock_rollback_detected=true`, не запускает UDP и
завершается fail-closed. Он также не интерпретирует старый deadline как уже
наступивший.

Это защищает локальную семантику от обычного перевода часов назад, но не является
trusted time и не обнаруживает согласованный rollback всего state directory.
Production-варианту всё ещё нужны platform monotonic clock для одного boot и
внешний witness для rollback между восстановленными snapshots.

## 7. Explicit cancellation

Команда:

```powershell
kilogram-cli history-recovery-plan-cancel `
  --state-dir .\recipient `
  --plan-file .\approved-recovery.plan `
  --conversation example
```

проверяет подпись plan, exact local recipient и Conversation ID, затем добавляет
recipient-signed terminal record. Отмена разрешена даже после expiry plan, не
делает discovery/connection и идемпотентна. Возобновить тот же Plan ID нельзя;
нужны свежий descriptor, SAS approval и новый signed plan.

Удаление только внешнего plan file больше не является единственным механизмом
отмены: скопированный plan может сохраниться. Cancellation локальна конкретному
device state; remote revocation protocol и синхронизация cancel между rollback-
копиями устройства пока не реализованы.

## 8. Совместимость и проверки

Recovery plan v1, URI v1, ticket v9, ALPN `kilogram/m0/sync/7`, wire messages и
recovery checkpoints не изменились. Scheduler state — только локальный artifact;
новых dependencies нет.

- unit tests проверяют signed chain restart, fork rejection, clock rollback,
  bounded jitter и terminal cancellation;
- все 114 workspace tests, rustfmt, strict Clippy и release build проходят;
- Windows process smoke `.tmp/m096-smoke-20260902-130246` выполнил attempt 1 в
  одном процессе, отклонил немедленный restart до UDP, а после deadline принял
  свежий endpoint как persistent attempt 2;
- один authenticated connection передал две atomic pages; пять scheduler
  records, два checkpoints и source/recipient history совпали;
- отдельная recipient state прошла `failed -> cancelled -> restart` без
  discovery и connection;
- прежний M0.9.5 process smoke также проходит без изменения команд благодаря
  compatibility alias.

## 9. Что остаётся дальше

- platform adapter boundary и первый Windows network/power/metered probe;
- регистрация настоящего OS background task/service и wakeup/cancel events;
- внешний monotonic rollback witness для scheduler state;
- compaction signed chain без потери rollback evidence;
- wide-area privacy-preserving descriptor lookup вместо LAN-only multicast;
- plan listing/revocation UI и защищённое отображение metadata;
- source listener для нескольких последовательных recovery sessions;
- multi-source selection и recovery от устройства собеседника.
