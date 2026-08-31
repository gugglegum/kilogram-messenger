# RFC-0011: network authenticated history rewrap

- Статус: **Implemented spike (M0.7.8)**
- Дата: 2026-09-01
- Область: согласованный пользователями перенос локально читаемой истории между
  устройствами одного аккаунта через authenticated device-to-device session

## 1. Цель этапа

M0.7.5 определил подписанный и зашифрованный `HistoryRewrapBundle`, но передавал
его вручную через файл. M0.7.8 переносит тот же неизменяемый bundle в
существующую Iroh-сессию, не вводя серверное хранение plaintext или ключей.

Новый срез должен одновременно доказать:

1. recipient запросил конкретный conversation и bounded range;
2. запрос относится к текущей transport session, а не воспроизведён из старой;
3. source явно разрешил тот же recipient, conversation, range и SAS;
4. source подписал ответ и связал его с точным запросом;
5. recipient импортировал bundle, transfer provenance, events и projections
   crash-consistently;
6. claims нескольких источников сравниваются без объявления локального
   наблюдения глобальным consensus.

## 2. Граница доверия и user consent

Network rewrap разрешён только разным устройствам одного Account ID. Оба
`DeviceCertificate` обязаны присутствовать в одном проверенном root-signed
`AccountDeviceListSnapshot`. Recipient certificate должен в точности совпадать
с certificate в source ticket; source certificate — с certificate listener.

Перед запуском пользователи независимо получают 12-значный SAS, представленный
четырьмя группами по три цифры. SAS domain-separated BLAKE3 связывает:

- полный canonical encoding root-signed device list;
- source Device ID в роли source;
- recipient Device ID в роли recipient.

Перестановка ролей меняет SAS. Полный 256-bit digest входит в подписанный
recipient request; десятичное сокращение используется только для сравнения
людьми по отдельному каналу. SAS не скрывает metadata и не доказывает, что
source обладает глобально полной историей.

Source запускает listener только с явными
`--history-rewrap-conversation`, `--history-rewrap-recipient-device`,
`--history-rewrap-approve-sas`, `--history-rewrap-range-start` и
`--history-rewrap-count`. Recipient обязан передать совпадающий
`--confirm-sas`. Без полного source approval обычный listener возвращает
`NotApproved` и не открывает local projections.

## 3. Signed request

`SignedHistoryRewrapRequest` версии 1 содержит:

- conversation ID;
- source и recipient Device ID;
- `SyncSessionBinding`, полученный из listener Endpoint ID;
- `range_start` и `max_event_count`;
- полный SAS digest.

Recipient подписывает весь content своим Ed25519 Device key. Source проверяет
подпись, текущую session binding, уже авторизованный requester Device ID,
same-account authority, SAS и точное совпадение с локальным approval. Count
обязан быть в диапазоне `1..=256`, а арифметика range проверяется на overflow.

## 4. Source-signed transfer

`SignedHistoryRewrapTransfer` версии 1 содержит исходный signed request и
`HistoryRewrapBundle` версии 1. Source Device signature покрывает оба объекта.
Проверка transfer требует:

- точного равенства request тому, который recipient только что отправил;
- совпадения conversation/source/recipient;
- `bundle.range_start == request.range_start`;
- `bundle.range_end <= range_start + max_event_count`;
- SAS, повторно вычисленного из embedded root-signed device list;
- всех прежних source signatures, HPKE AAD и event authorization проверок
  bundle.

Таким образом, отдельная валидная запись rewrap не может быть переставлена под
другой запрос или transport session. Wire ALPN повышен с
`kilogram/m0/sync/6` до `kilogram/m0/sync/7`; event v5, sync/session v6 и ticket
v9 не изменились.

## 5. Bounded transport

Application frame остаётся ограничен 8 MiB. Максимум bundle — 256 events, но
реальный размер зависит от исходных `AuthorizedEvent` и fan-out ciphertexts.
Source кодирует и проверяет полный transfer до отправки. Если он превышает
wire limit, recipient получает `TransferTooLarge` и должен повторить операцию с
меньшим `--count`; неограниченного allocation или неявного усечения нет.

## 6. CLI flow

Обе стороны получают SAS из одного public device list:

```powershell
kilogram-cli history-rewrap-sas `
  --device-list-file .\account.devices `
  --source-device <SOURCE_DEVICE_ID> `
  --recipient-device <RECIPIENT_DEVICE_ID>
```

После сравнения source запускает одноразовый listener:

```powershell
kilogram-cli listen `
  --state-dir .\state-source `
  --allow-account <ACCOUNT_ID> `
  --device-list-file .\account.devices `
  --peer-prekey-pool-file .\recipient.pool `
  --ticket-file .\source.ticket `
  --history-rewrap-conversation chat `
  --history-rewrap-recipient-device <RECIPIENT_DEVICE_ID> `
  --history-rewrap-approve-sas 123-456-789-012 `
  --history-rewrap-range-start 0 `
  --history-rewrap-count 256
```

Recipient отправляет signed request и импортирует ответ:

```powershell
kilogram-cli history-rewrap-fetch `
  --state-dir .\state-recipient `
  --ticket-file .\source.ticket `
  --conversation chat `
  --range-start 0 `
  --count 256 `
  --confirm-sas 123-456-789-012 `
  --expect-account <ACCOUNT_ID>
```

## 7. Crash-consistent import и provenance

После полной сетевой проверки recipient закрывает соединение и одной M0.7.7
filesystem transaction сохраняет:

- неизменённые events и authorization sidecars;
- encrypted local projections v2;
- исходный `<rewrap-id>.rewrap`;
- source-signed `<rewrap-id>.transfer` с session-bound recipient request.

Повторный идентичный import идемпотентен, конфликт immutable file отклоняется.
Authority snapshot storage по-прежнему находится вне managed transaction, как
явно оговорено в RFC-0010.

## 8. Multi-source reconciliation

`history-rewrap-reconcile` проверяет все локальные `.rewrap` выбранного
conversation и группирует claims по:

`(source Device ID, inventory_event_count, inventory_digest)`.

Перекрывающиеся подписанные ranges одного claim объединяются. Claim считается
complete только если union без пробелов покрывает `[0, inventory_event_count)`.
Результат имеет четыре состояния:

- `incomplete` — ни один source claim не покрыт полностью;
- `single-source` — есть один полный source claim;
- `agreed` — два или более source подписали одинаковые count и digest;
- `divergent` — полные claims различаются или один source подписал разные
  inventory claims.

CLI всегда печатает `global_completeness_proven=false`. Даже `agreed` означает
лишь согласие наблюдавшихся устройств, а не наличие всех когда-либо созданных
events.

## 9. Проверки

Автоматические tests покрывают:

- role-bound SAS, session mismatch и oversized request;
- recipient request signature, source transfer signature и tampering;
- точное связывание transfer с request;
- source consent с exact SAS и same-account restriction;
- gap/overlap range coverage;
- `single-source`, `agreed`, `divergent` и `incomplete` classification.

Process smoke создал один аккаунт с source/recipient и три старых сообщения,
известных только source. Реальный direct Iroh flow передал 4,674-byte transfer,
recipient восстановил 3/3 plaintext projections и сохранил bundle/transfer.
Reconciliation вернул complete `single-source`; поиск plaintext marker по
recipient state не нашёл совпадений.

## 10. Ограничения

- Listener обслуживает один запрос и завершается; автоматической pagination и
  resume нескольких ranges пока нет.
- Source discovery и выбор нескольких источников выполняются вручную.
- SAS требует отдельного доверенного канала или личного сравнения; QR/device-link
  ceremony ещё нет.
- Source может лгать о полноте или передать только доступную ему историю.
- Потеря всех читаемых projections не исправляется signing key или seed-фразой.
- Bundle раскрывается при компрометации recipient device.
- Relay видит timing/size и endpoint metadata, хотя не видит plaintext.

## 11. Следующий этап

M0.7.9 должен превратить одноразовый range fetch в resumable recovery
orchestration: authenticated pagination с локальным checkpoint, безопасный
retry после разрыва, сбор claims от нескольких явно выбранных устройств и
выбор согласованного inventory без ложного глобального consensus. QR/device-link
ceremony и автоматический source discovery остаются следующими UX/discovery
слоями поверх этого механизма.
