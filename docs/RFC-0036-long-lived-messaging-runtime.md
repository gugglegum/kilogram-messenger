# RFC-0036: Long-lived messaging runtime (M0.9.9)

Статус: реализовано в M0.9.9

## 1. Мотивация

До M0.9.9 команда `listen` создавала новый Iroh endpoint, обслуживала ровно
одну delivery/sync-сессию и завершалась. Такой режим полезен как диагностический
one-shot, но не соответствует обычному мессенджеру: запущенный клиент должен
оставаться доступным, принимать следующие сообщения и синхронизации без ручного
перезапуска listener и переноса нового ticket после каждого события.

M0.9.9 вводит первый runtime-срез. Он не является GUI или окончательным daemon
API, но задаёт правильную границу процесса и локального состояния, поверх
которой можно строить contact manager, исходящую очередь и клиентский UI.

## 2. Команда и жизненный цикл

`kilogram-cli runtime`:

- создаёт один Iroh endpoint и сохраняет его Endpoint ID до завершения процесса;
- публикует один подписанный ticket v9 и обслуживает через него последовательные
  `DeliverEvent` и `SyncInventory` сессии;
- заново выполняет Account Root/device authorization для каждого connection;
- после ошибочной или неавторизованной сессии закрывает только её и продолжает
  принимать следующие подключения;
- штатно завершается по `Ctrl+C`, optional idle timeout или optional session
  bound, закрывая endpoint;
- после restart создаёт новый endpoint и атомарно заменяет ticket-файл.

`--max-sessions 0` и `--idle-seconds 0` означают обычный режим до `Ctrl+C`.
Ненулевые значения предназначены для bounded deployments и воспроизводимых
process tests; они ограничены соответственно 65 536 сессиями и 24 часами idle.

Существующая `listen` остаётся one-shot диагностикой и не меняет семантику.

## 3. Ticket и транспорт

Весь runtime использует один session binding, производный от текущего Endpoint
ID. Transport route из подписанного ticket проверяется до открытия state
transaction. Direct/relay selection и E2EE application frames остаются теми же,
что у `listen`, `connect` и `sync`.

Ticket является публичным addressing/authorization artifact и не содержит
endpoint secret или ключей расшифрования. Если указан `--ticket-file`, runtime
записывает его через temporary file в том же каталоге, `fsync` и atomic replace.
Это не позволяет наблюдателю filesystem получить частично записанный новый
ticket во время restart.

## 4. Граница локального состояния

Runtime намеренно исключён из общего outer state lock и outer vault mirror
intent. Ожидание сети не должно блокировать foreground-команду клиента.

Граница записи следующая:

1. начальная публикация authority/prekey directory проходит под кратким
   `StateDirectoryLock` и обычным vault dual-write;
2. endpoint ждёт connection без state lock;
3. route policy проверяется без state lock;
4. перед device authorization и application stream runtime получает lock;
   только typed `StateError::AlreadyLocked` повторяется каждые 25 ms не более
   15 секунд;
5. одна accepted application session выполняется под отдельным vault
   dual-write guard; commit/rollback и mirror завершаются до освобождения lock;
6. runtime возвращается к ожиданию следующего connection без lock.

Поэтому обычная локальная команда может работать между входящими сессиями, а
краткое столкновение не маскирует другие I/O или integrity ошибки. Application
sessions пока обслуживаются последовательно: это сохраняет существующий
single-writer sequence/ratchet invariant.

## 5. Поддерживаемые запросы

Runtime использует тот же обработчик, что one-shot listener:

- signed ratchet delivery с durable local commit до acknowledgement;
- bounded bidirectional sync с immutable command-local read snapshot;
- полная device/account/membership проверка на каждой сессии.

History recovery rewrap требует отдельного exact consent/SAS окна и в M0.9.9
остаётся в специализированном M0.9.1–M0.9.8 flow. Runtime не превращает
долгоживущий endpoint в неограниченный recovery source.

## 6. Проверка

Локальный process smoke M0.9.9:

- запустил Bob runtime один раз с `--max-sessions 3`;
- выполнил два независимых `connect` и один `sync` через один Endpoint ID;
- получил три completed sessions, четыре одинаковых события у Alice и Bob и
  штатный `runtime_stop_reason=session-limit`;
- перезапустил runtime с тем же device state, получил новый Endpoint ID и
  атомарно заменённый ticket;
- доставил ещё одно сообщение после restart; обе истории сошлись на шести
  событиях.

Отдельный unit test проверяет создание parent directory и замену существующего
ticket-файла.

## 7. Ограничения и следующий этап

M0.9.9 ещё не реализует:

- локальный IPC/API для GUI;
- persistent contact descriptor и автоматическое обновление peer endpoint;
- исходящую очередь, reconnect/backoff и automatic sync trigger;
- параллельную обработку нескольких state-mutating sessions;
- OS autostart/background registration.

OS registration не является условием работы мессенджера: когда обычное
приложение запущено, runtime живёт внутри него; когда пользователь полностью
закрыл приложение, networking прекращается. Autostart/background mode может
позже стать отдельной явной пользовательской настройкой.

Следующий срез M0.9.10 должен добавить persistent contact/runtime descriptor и
локальную исходящую очередь с reconnect/sync, не меняя криптографический wire
protocol M0.9.9.
