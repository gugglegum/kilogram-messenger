# RFC-0030: bounded history recovery QR ceremony (M0.9.3)

Статус: реализовано в M0.9.3

Дата: 2026-09-02

## 1. Задача

M0.9.2 определил компактную подписанную recovery URI, но называл её только
QR-ready: source и recipient всё ещё обменивались строкой или текстовым файлом.
M0.9.3 реализует полный image round-trip для CLI:

- listener сразу создаёт PNG QR вместе со своей recovery-сессией;
- отдельная команда рисует QR из ранее полученной подписанной ссылки;
- offline inspect и network accept читают ссылку непосредственно из PNG/JPEG;
- результат QR decode проходит тот же криптографический verifier, exact-device,
  conversation и SAS preflight, что текстовый M0.9.2 input.

Это image-file ceremony. Live camera, clipboard capture, GUI scanner и OS deep
link handler пока не реализованы.

## 2. Генерация

`listen --history-recovery-qr-file <NEW.png>` создаёт source-signed link и
публикует QR до перехода в `status=listening`. Текстовый
`--history-recovery-link-file` при этом необязателен. Для повторного рендера без
listener используется:

```powershell
kilogram-cli history-recovery-link-qr-render `
  --link-file .\recovery.link `
  --qr-file .\recovery.png
```

Renderer использует:

- standard QR Code, не Micro QR;
- error correction level L, чтобы гарантированно вместить весь M0.9.2 limit;
- четыре pixels на module;
- обязательную quiet zone шириной четыре modules;
- grayscale PNG;
- no-clobber publication через same-directory temporary file и atomic persist.

Максимальная разрешённая URI длиной 2953 bytes проверена как один Version 40-L
QR. Меньший реальный smoke payload 1202 bytes дал Version 25 и изображение
500×500.

Renderer никогда не перезаписывает существующий output. Это не позволяет
случайному повторному запуску заменить уже переданный QR другим endpoint или
recovery plan под тем же именем.

## 3. Декодирование

`history-recovery-link-inspect` и `history-recovery-link-accept` принимают ровно
один из `--link`, `--link-file` или `--qr-file`.

Image input имеет явные M0 bounds:

- только PNG или JPEG, определённые по содержимому, а не расширению;
- обычный файл размером `1..=16 MiB`;
- ширина и высота не больше 4096 pixels;
- image decoder получает allocation budget 64 MiB;
- detector должен найти ровно один QR;
- payload обязан быть ASCII, иметь versioned Kilogram prefix и не превышать
  2953 bytes.

Ноль QR отклоняется. Несколько QR также отклоняются вместо неявного выбора
первого: пользователь должен однозначно видеть, какой descriptor он принимает.
После image decode `SignedHistoryRecoveryLink::decode_text` заново проверяет
Root/source signatures и весь M0.9.2 contract. QR error correction не заменяет
криптографическую целостность.

## 4. Пользовательский поток

Source запускает прежний consent-gated listener, добавляя PNG output:

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
  --history-recovery-qr-file .\recovery.png
```

Recipient может сначала проверить QR строго offline:

```powershell
kilogram-cli history-recovery-link-inspect --qr-file .\recovery.png
```

После независимого сравнения показанного SAS он явно принимает тот же image:

```powershell
kilogram-cli history-recovery-link-accept `
  --state-dir .\recipient `
  --qr-file .\recovery.png `
  --conversation example `
  --confirm-sas 123-456-789-012
```

Никакие Account ID, source Device ID, range, page size, route policy или
endpoint не перепечатываются вручную: они берутся из подписанного QR payload.

## 5. Неизменные границы доверия

QR является только переносчиком публичного descriptor. Он не является bearer
capability и не содержит ключей или plaintext истории. До сети accept требует
exact recipient certificate, local conversation ID и SAS. После сети listener
повторно выполняет device authentication и независимо применяет exact local
consent window. Page requests, source transfers и checkpoint chain не менялись.

Фото QR может раскрывать account/device/conversation metadata и endpoint
coordinates любому, кто его увидит. Короткий expiry ограничивает полезность
старой копии, но не делает изображение секретным или анонимным.

## 6. Совместимость и зависимости

Wire protocol, ticket v9, recovery URI v1 и ALPN `kilogram/m0/sync/7` не
изменились. Текстовые `--link`/`--link-file` остаются совместимы.

CLI использует `qrcode 0.14.1` для генерации, `rqrr 0.10.1` для detection/decode
и `image 0.25.10` с отключёнными default formats и только PNG/JPEG features.
Они являются заменяемыми локальными UI/codec dependencies, а не частью
криптографического wire protocol.

## 7. Проверки

- unit tests покрывают PNG round-trip, no-clobber, wrong prefix, payload over
  limit, exact maximum Version 40, JPEG, ambiguous multi-QR, oversized file и
  oversized dimension;
- Windows direct process smoke `.tmp/m093-smoke-20260902-012850` проверил
  listener PNG, standalone rerender, offline scan, no-clobber и wrong-device
  rejection до сети;
- exact recipient восстановил две atomic pages одним authenticated connection;
- source и recipient получили byte-identical verified history и два immutable
  checkpoint.

## 8. Что остаётся дальше

- live camera/clipboard scanner и GUI confirmation screen;
- OS registration для `kilogram://` deep links;
- authenticated LAN publication/discovery свежих source descriptors реализован
  в M0.9.4; wide-area privacy-preserving discovery остаётся открытым;
- background scheduler с retry, power и metered-network policy;
- multi-source selection и recovery claim UI.
