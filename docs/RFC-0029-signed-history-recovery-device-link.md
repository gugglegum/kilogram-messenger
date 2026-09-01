# RFC-0029: signed history recovery device link (M0.9.2)

Статус: реализовано в M0.9.2

Дата: 2026-09-02

## 1. Задача

M0.9.1 перенёс до 64 страниц истории по одному аутентифицированному соединению,
но recipient должен был вручную получить большой connection ticket и отдельно
ввести Account ID, source Device ID, диапазон и размер страницы. Ticket v9
включает свежие prekey pools всех устройств и в реальном smoke-test занимал
около 36 KiB текста, поэтому он непригоден для QR ceremony.

M0.9.2 вводит компактную recipient-specific ссылку
`kilogram://history-recovery/v1/...`. Source подписывает в ней только
необходимые для bootstrap публичные данные и точный recovery plan. Recipient
может проверить ссылку полностью offline, явно сравнить SAS и одной командой
запустить существующий M0.9.1 coordinator без legacy ticket.

Срез создаёт QR-ready payload, но пока не рисует QR-картинку и не регистрирует
OS deep link handler.

## 2. Содержимое ссылки

Подписанный source device payload содержит:

- Iroh `EndpointAddr` и transport `RoutePolicy`;
- source `DeviceCertificate`;
- полный root-signed `AccountDeviceListSnapshot` без prekey pools;
- точный recipient Device ID;
- точный `ConversationId`;
- одобренное полуоткрытое окно `[range_start, range_end)`;
- рекомендуемый `page_size`;
- время выпуска и истечения.

Подпись source device domain-separated. `link_id` отдельно вычисляется как
domain-separated BLAKE3 digest канонического подписанного объекта. M0 codec —
Postcard, затем base64url без padding и versioned URI prefix. Этот codec остаётся
предварительным и не фиксирует будущий публичный межъязыковой wire format.

## 3. Ограничения

- URI содержит не более 2953 UTF-8 bytes и поэтому помещается в один QR Code
  Version 40-L в byte mode;
- срок жизни положительный и не превышает 3600 секунд, по умолчанию 600 секунд;
- допускается 120 секунд clock skew;
- диапазон непустой и заканчивается не дальше M0 inventory limit 4096;
- `page_size` лежит в `1..=256`;
- source и recipient являются разными устройствами одного точного root-signed
  device list;
- source certificate обязан иметь messaging capability и в точности
  присутствовать в этом list.

Ссылка публична: она не содержит device secret, endpoint secret, ratchet key,
prekey private material или расшифровываемый текст истории. При этом она
раскрывает account/device metadata, conversation identifier, endpoint
coordinates и объём одобренного диапазона; её нельзя считать анонимным токеном.

## 4. Offline inspect и явное принятие

`history-recovery-link-inspect` декодирует объект, проверяет Root/device
подписи, роли, bounds и срок действия, печатает SAS и bootstrap coordinates и
завершается с `connection_attempted=false`.

`history-recovery-link-accept` дополнительно требует:

1. local certificate точного recipient Device ID из ссылки;
2. local conversation label, дающий подписанный `ConversationId`;
3. точное ручное подтверждение role-bound 12-digit SAS;
4. уже установленный owner-signed conversation membership.

Только после этих проверок команда открывает сеть. Account, source, range,
page size и route policy берутся из подписанного payload, а не повторяются в
несвязанных CLI flags.

Пример source:

```powershell
kilogram-cli listen `
  --state-dir .\source `
  --allow-account <ACCOUNT_ID> `
  --device-list-file .\account.devices `
  --peer-prekey-pool-file .\recipient.pool `
  --history-rewrap-conversation example `
  --history-rewrap-recipient-device <RECIPIENT_DEVICE_ID> `
  --history-rewrap-approve-sas 123-456-789-012 `
  --history-rewrap-range-start 0 `
  --history-rewrap-count 256 `
  --history-recovery-link-file .\recovery.link
```

Пример recipient:

```powershell
kilogram-cli history-recovery-link-inspect --link-file .\recovery.link

kilogram-cli history-recovery-link-accept `
  --state-dir .\recipient `
  --link-file .\recovery.link `
  --conversation example `
  --confirm-sas 123-456-789-012
```

## 5. Защита на transport/runtime boundary

Ссылка не является bearer capability и сама по себе не даёт историю. После
подключения сохраняются все прежние проверки:

- Iroh connection аутентифицирует transport Endpoint ID;
- recipient доказывает владение exact device key;
- listener сопоставляет Account Root certificate и current authority snapshot;
- listener независимо хранит локальное consent для того же conversation,
  recipient и диапазона;
- каждый page request и каждый source transfer остаются session-bound и
  подписанными;
- каждая страница атомарно коммитится с recipient-signed checkpoint.

Поэтому найденная или пересланная ссылка не превращает peer в автоматически
доверенный source. Wrong-device link отклоняется до сетевого подключения, а
истёкший, изменённый или подписанный неавторизованным устройством payload
отклоняется fail-closed.

## 6. Совместимость

Wire request/response objects, ticket v9 и ALPN `kilogram/m0/sync/7` не
изменились. `history-recovery-resume` с legacy ticket остаётся доступен. Новый
link-accept преобразует проверенный компактный payload во внутренний bootstrap
и использует тот же M0.9.1 recovery coordinator.

Source всё ещё строит полный ticket внутри listener для старых клиентов, но
link path этот ticket не передаёт recipient. Prekey pools остаются нужны другим
режимам messaging/fan-out и не включаются в QR payload.

## 7. Проверки

- unit regression проверяет round-trip, Root/source signature, exact recipient,
  expiry и tamper rejection;
- Windows direct process smoke `.tmp/m092-smoke-20260902-010717` создал ссылку
  размером 1202 bytes, проверил её offline, отклонил другое устройство до сети
  и одним authenticated connection перенёс две страницы;
- source и recipient получили byte-identical verified history и два immutable
  recovery checkpoint.

## 8. Что остаётся дальше

- QR image renderer/decoder реализованы в M0.9.3 и описаны в
  [`RFC-0030`](RFC-0030-bounded-history-recovery-qr-ceremony.md); live camera,
  clipboard и OS deep-link integration ещё не реализованы;
- authenticated LAN publication/discovery реализован в M0.9.4; wide-area и
  multi-source discovery остаются открытыми;
- постоянный scheduler с retry, power и metered-network policy;
- rotation/revocation-aware descriptor refresh и multi-source UI;
- compact Merkle/range summary вместо bounded full inventory.
