# RFC-0032: consent-bound history recovery retry coordinator (M0.9.5)

Статус: реализовано в M0.9.5

Дата: 2026-09-02

## 1. Задача

M0.9.4 обнаруживает свежий source descriptor, но каждый запуск всё ещё требует
ручного accept. Простое фоновое принятие любой найденной ссылки нарушило бы
границу consent. M0.9.5 разделяет два действия:

1. recipient один раз явно проверяет SAS и подписывает bounded recovery plan;
2. retry coordinator без нового prompt ищет только свежие descriptors, полностью
   совпадающие с этим планом, и возобновляет его signed checkpoint chain.

Это bounded CLI coordinator для интеграции с будущим platform scheduler. Он не
регистрирует OS service, не просыпается сам и не определяет тип сети или питание
по недокументированным platform API.

## 2. Recipient-signed approved plan

План создаётся строго без network connection:

```powershell
kilogram-cli history-recovery-plan-approve `
  --state-dir .\recipient `
  --link-file .\recovery.link `
  --conversation example `
  --confirm-sas 123-456-789-012 `
  --plan-file .\approved-recovery.plan
```

До подписи CLI выполняет M0.9.4 preflight: source/root signatures, expiry, exact
recipient, local conversation membership и authority anti-rollback. Затем exact
recipient device подписывает versioned plan, который связывает:

- Account ID и полный root-signed device list;
- source/recipient Device IDs и полный role-bound SAS digest;
- Conversation ID;
- approved range и page size;
- route policy;
- network/power execution policy;
- approval time и plan expiry.

Plan file ограничен 64 KiB и публикуется no-clobber. Он не содержит E2EE keys,
ratchet state или plaintext, но раскрывает account/device/conversation metadata
и поэтому не должен считаться анонимным. Default lifetime — 24 часа, допустимый
диапазон `1..=168` часов. В M0.9.5 удаление файла было единственной локальной
отменой; M0.9.6 добавил отдельный terminal recipient-signed cancel record.
Удалённого revocation protocol для уже скопированного state пока нет.

## 3. Fresh endpoint without trust expansion

Recovery URI действует недолго и привязана к transport Endpoint ID. Coordinator
не переиспользует просроченную URI: в каждой попытке он запускает M0.9.4 LAN
scan и требует свежую source-signed ссылку.

Новый descriptor может отличаться от исходного только ephemeral полями:

- Endpoint coordinates;
- issued/expiry timestamps;
- Link ID и source signature, неизбежно изменившиеся вместе с payload.

Account/device list, source/recipient, SAS, conversation, range, page size и
route policy должны точно совпасть с recipient-signed plan. Более новая
authority revision, другой range или privacy route требуют нового явного
approval. Если найдено ноль либо несколько matching descriptors, connection не
открывается и coordinator переходит к следующей bounded попытке.

## 4. Network and power policy

При approval defaults таковы:

- Ethernet: разрешён;
- Wi-Fi: разрешён;
- mobile/metered: запрещён;
- unknown network: запрещён;
- battery: разрешена.

Флаги `--deny-ethernet`, `--deny-wifi`, `--allow-mobile`,
`--allow-unknown-network` и `--require-external-power` меняют policy до её
подписи. Нельзя создать plan, запрещающий все network classes.

Run требует явный runtime context:

```powershell
kilogram-cli history-recovery-plan-run `
  --state-dir .\recipient `
  --plan-file .\approved-recovery.plan `
  --conversation example `
  --network-class wifi `
  --power-source battery
```

Допустимые network classes: `ethernet`, `wifi`, `mobile`, `unknown`; power:
`external`, `battery`, `unknown`. В M0.9.5 значения передавал caller, и CLI
печатал `history_recovery_network_context_source=caller-supplied`. M0.9.7
заменил default path на Windows-native probe; прежняя пара аргументов сохранена
как явный development override. Полный контракт — RFC-0034.

Blocked policy завершается до UDP bind и Iroh connection с diagnostics
`history_recovery_scheduler_discovery_attempted=false` и
`connection_attempted=false`.

## 5. Retry and state semantics

Один run имеет hard bounds:

- `1..=8` attempts, default 3;
- `1..=30` секунд LAN discovery на attempt, default 3;
- `0..=300` секунд fixed delay между attempts, default 5;
- `1..=64` atomic recovery pages на authenticated connection, default 64;
- каждый discovery сохраняет прежние caps: 512 датаграмм и 8 scheduler
  candidates из общего M0.9.4 maximum 16.

M0.9.6 сохраняет этот per-process attempt cap, но заменяет fixed delay на
persistent exponential equal-jitter backoff; см. RFC-0033.

Перед каждым attempt повторно проверяется plan expiry. Descriptor проходит
source signature, exact recipient и exact-plan matching. Перед самим connect
локальные certificate, authority и membership заново проверяются под state
lock. Invalid page/transfer не коммитится; уже committed pages и recipient-
signed checkpoints переживают failed attempt и используются следующим.

Source protocol/transport failure может быть повторён до hard attempt cap, но
не ослабляет verifier: ошибочный ответ не становится допустимым от количества
retry.

## 6. Concurrency boundary

CLI обычно сериализует device state на всю команду. Для background coordinator
это означало бы удержание lock во время discovery и backoff до десятков минут.
M0.9.5 исключает plan runner из outer command lock и берёт его только в коротких
critical sections:

- local plan/trust/checkpoint preflight;
- активная authenticated recovery attempt и её vault mirror;
- post-attempt checkpoint check.

UDP scan и retry sleep выполняются без state lock и без открытого vault mirror
intent. Foreground client может использовать тот же device state в это время.
Активный transfer по-прежнему эксклюзивен, чтобы ratchet, projections, events и
checkpoint оставались crash-consistent.

## 7. Совместимость и проверки

Recovery URI v1, ticket v9, ALPN `kilogram/m0/sync/7`, wire messages и checkpoint
format не изменились. Plan v1 — локальный coordinator artifact, не сетевой wire
message. Новых dependencies нет.

- unit tests проверяют recipient signature, bounded expiry, fresh endpoint
  matching и fail-closed network/power matrix;
- все 112 workspace tests проходят;
- Windows process smoke `.tmp/m095-smoke-20260902-022156` подтвердил policy
  block до discovery, первый `no-candidate`, свободный foreground state access
  во время backoff и второй attempt после source restart;
- fresh Endpoint ID был принят только при полном совпадении signed plan;
- один authenticated connection передал две atomic pages, создал два
  checkpoints и byte-identical source/recipient history.

## 8. Что остаётся дальше

- настоящий OS background service/task с wakeup и network-change events;
- macOS/Linux/mobile adapters и Windows event subscriptions поверх M0.9.7;
- внешний rollback witness для persistent scheduler state из M0.9.6;
- wide-area privacy-preserving descriptor lookup вместо LAN-only multicast;
- plan listing/revocation UI и защищённое локальное хранение metadata;
- source listener, рассчитанный на несколько последовательных recovery sessions;
- multi-source selection и recovery от устройства собеседника.
