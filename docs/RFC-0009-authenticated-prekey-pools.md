# RFC-0009: authenticated prekey pools and concurrent initiation

- Статус: **Implemented spike (M0.7.6)**
- Дата: 2026-09-01
- Область: свежие наборы Olm one-time keys, anti-rollback discovery и
  детерминированное разрешение crossed pairwise initiation

## 1. Цель этапа

M0.7.3–M0.7.5 публиковали ровно один `SignedPrekeyBundle` устройства. После
его расходования новый first message, созданный по старой копии bundle, уже не
мог открыть session. Два устройства, одновременно создавшие outbound session
друг к другу, сохраняли разные session под одним peer Device ID и не могли
расшифровать crossed PreKey message.

M0.7.6 заменяет сетевую single-OTK границу подписанным пулом, запоминает
максимальную увиденную публикацию каждого peer device и допускает две session
только на время разрешения одновременной инициализации. Сетевой `RatchetText`
event и его ciphertext slots не меняются.

## 2. SignedPrekeyPool

Один `SignedPrekeyPool` содержит:

- стабильную device-signed `SignedRatchetIdentity`;
- монотонный `generation`;
- `published_at_unix_seconds` и `expires_at_unix_seconds`;
- от 1 до 64 `SignedPrekeyBundle`, по умолчанию 16;
- внешнюю Device signature над identity, generation, временем и всеми entries.

Sequence всех entries образуют непрерывный возрастающий диапазон. OTK внутри
пула уникальны, каждый внутренний bundle сохраняет собственную Device signature
и ту же ratchet identity. Максимальный подписываемый срок — 30 дней; CLI по
умолчанию публикует пул на 7 дней. При проверке часов допускается 5 минут skew.

Публичный `pool_id` — domain-separated BLAKE3 digest полного кодирования
подписанного пула. Он является идентификатором immutable публикации, но не
секретом и не глобальным consensus checkpoint.

## 3. Выбор и расходование OTK

Для первого сообщения sender детерминированно выбирает entry по hash от своих
Device ID, Device ID получателя и generation пула. Повтор одной стороны поэтому
выбирает тот же OTK, а разные sender devices обычно распределяются по пулу.
Коллизия остаётся возможной и приводит к отказу второй инициализации после
расходования ключа, а не к повторному использованию private OTK.

`vodozemac::olm::Account` удаляет private OTK только после успешной проверки и
расшифрования PreKey message. Kilogram затем немедленно создаёт новый пул:
generation увеличивается на один, а sequence продолжается после последнего
sequence прежнего пула. Неиспользованные private keys старых публикаций временно
остаются в bounded Olm account для уже находящихся в пути сообщений. Просроченный
текущий пул автоматически заменяется при следующей публикации.

## 4. Freshness и discovery high-water

Каждое устройство хранит под
`STATE_DIR/ratchet/peer-prekey-pools/<DEVICE>.pool` максимальную проверенную
публикацию peer device:

- ratchet identity key;
- generation;
- первый и последний sequence;
- publication/expiry time;
- pool ID.

Новая generation обязана иметь sequence строго после ранее увиденного диапазона
и не уменьшать publication time. Более низкая generation отклоняется как
rollback; другой pool ID на той же generation — как device equivocation; смена
ratchet identity существующего Device ID также fail-closed.

Connection ticket v9 содержит root-complete device list и ровно один свежий
pool для каждого listed device. `connect` и `sync` запоминают directory сразу
после проверки ticket, даже если не отправляют новое сообщение. Listener
добавляет свой текущий пул автоматически; public pool других устройств аккаунта
пока передаётся через повторяемый `--peer-prekey-pool-file`.

Это authenticated M0 discovery boundary, но не глобальный discovery service.
При первом контакте старый, ещё не истёкший и корректно подписанный pool нельзя
отличить от самого нового без gossip/witness/DHT policy.

## 5. Concurrent pairwise initiation

Persistent session record v2 хранит:

- peer Device ID и его стабильный Curve25519 identity key;
- active session ID и признак подтверждения;
- одну active и не более одной retained session;
- роль каждой session: outbound, inbound или migrated legacy.

Если у Alice есть неподтверждённая outbound session X, а от Bob приходит новый
валидный PreKey для session Y, Alice создаёт inbound Y и сохраняет обе. Bob при
получении crossed сообщения получает тот же набор `{X, Y}`. Обе стороны
независимо выбирают лексикографически меньший session ID как active, поэтому
сходятся без coordinator. Losing session остаётся retained и может расшифровать
уже отправленные до разрешения сообщения; новые исходящие сообщения используют
только active session.

Normal ciphertext сначала пробуется на active session, затем на retained.
Проверка выполняется на временно восстановленной копии pickle, поэтому неудачная
попытка не продвигает состояние. После подтверждения active session новый
неизвестный PreKey не может неявно сбросить ratchet; для recovery потребуется
отдельный будущий reset protocol. Старый session record v1 читается как одна
подтверждённая legacy session и переписывается в v2 при следующем изменении.

## 6. CLI flow

Публичный пул экспортируется или явно ротируется отдельно от listener:

```powershell
kilogram-cli ratchet-prekey-pool `
  --state-dir .\state-bob-2 `
  --pool-file .\bob-2.prekeys `
  --count 16 `
  --valid-for-hours 168
```

`--refresh` принудительно создаёт следующую generation. Для multi-device
account listener получает pools остальных устройств:

```powershell
kilogram-cli listen `
  --state-dir .\state-bob-1 `
  --allow-account <ALICE_ACCOUNT_ID> `
  --device-list-file .\bob-devices.snapshot `
  --peer-prekey-pool-file .\bob-2.prekeys
```

Development `seed-history` аналогично принимает
`--peer-prekey-pool-file`. Старая команда `ratchet-bundle` оставлена только для
диагностики/совместимости состояния и больше не входит в ticket v9.

## 7. Версии

- `SignedPrekeyPool` — v1;
- `AccountPrekeyDirectory` — v2 и содержит pools вместо одиночных bundles;
- persistent session record — v2 с чтением v1;
- connection ticket/signature domain — v9;
- `RatchetText` event/Event ID — по-прежнему v5;
- sync/session authorization и Iroh ALPN — по-прежнему v6 / `sync/6`.

Event schema не повышается: Olm PreKey message уже связывает ciphertext с
конкретным OTK, а pool является session-establishment/discovery metadata.

## 8. Проверенные свойства

- signature, размер, уникальность OTK, непрерывный sequence и freshness пула;
- deterministic selection и автоматическая ротация просроченного пула;
- generation rollback, same-generation equivocation, sequence rollback и
  ratchet identity substitution отклоняются;
- две crossed outbound sessions расшифровывают оба первых сообщения и выбирают
  одинаковый active session ID;
- losing session сохраняется для запоздавшего сообщения, но новое шифрование
  идёт через active session;
- process smoke создаёт Alice/Bob first messages до соединения, синхронизирует
  их 1/1, ротирует оба пула `0 -> 1` и sequence `0..15 -> 16..31`;
- второй ticket сообщает generation 1, после чего generation 0 отклоняется;
- последующая delivery сходится в общей history, plaintext отсутствует в
  event/projection/account/session/pool files.

## 9. Ограничения

- нет DHT, gossip witnesses, blind mailbox или atomic remote OTK reservation;
- first-contact global freshness не доказуема одним device-signed pool;
- wall-clock expiry зависит от корректности локальных часов и допускает 5 минут
  skew;
- hash-selection уменьшает, но не исключает collision разных initiators;
- сохраняются максимум две session на peer, losing session пока не имеет TTL;
- не реализован authenticated session reset после потери state;
- M0.7.7 добавил filesystem transaction и межпроцессный state lock по
  [`RFC-0010`](RFC-0010-crash-consistent-local-state.md); production DB/WAL всё
  ещё не выбрана;
- Olm остаётся M0 reference implementation без PQXDH и внешнего аудита всей
  Kilogram-композиции.

## 10. Следующий этап

M0.7.7 выполнен в [`RFC-0010`](RFC-0010-crash-consistent-local-state.md):
ratchet advancement, local projection, immutable event и prekey rotation
охвачены crash-consistent M0 filesystem transaction, а device state защищён
exclusive lock. Сетевой authenticated history rewrap с user consent/SAS и
multi-source completeness reconciliation реализован M0.7.8 в
[`RFC-0011`](RFC-0011-network-history-rewrap.md).
