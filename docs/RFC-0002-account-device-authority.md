# RFC-0002: Account Root и авторизация устройств

- Статус: **Implemented draft (M0.5.2)**
- Дата: **2026-08-31**
- Область: Account ID, device certificates, capabilities, revocation
- Связанные материалы: [`RFC-0001`](RFC-0001-core-architecture.md),
  [`../ai-docs/decisions.md`](../ai-docs/decisions.md)

## 1. Резюме

M0.5.1 вводит первую проверяемую границу между аккаунтом и его устройствами.
Аккаунт имеет отдельный корневой Ed25519-ключ. Его публичный ключ является
`AccountId`; корневой secret подписывает только сертификаты и отзывы устройств
и не используется для обычных сообщений.

Каждая установка клиента по-прежнему имеет самостоятельный `DeviceId` и
device secret. `DeviceCertificate` связывает этот ключ с аккаунтом и явно
перечисляет capabilities. `DeviceRevocation` навсегда отзывает конкретный
device key. Сертификат и отзыв проверяются только по публичному `AccountId`,
поэтому проверяющей стороне не нужен root secret.

M0.5.2 применяет эту модель в connection ticket и отдельном session
authorization handshake до любых delivery/sync данных. Временные
`--allow-device` и known-author больше не используются.

## 2. Инварианты

1. Account Root и Device Identity — разные ключи и разные state directories.
2. Root key не подписывает сообщения, transport handshakes или sync inventory.
3. Сертификат действителен только для указанного `AccountId`, `DeviceId` и
   набора capabilities.
4. Подмена любого подписанного поля обнаруживается.
5. Отзыв постоянен для конкретного device key. Более поздний сертификат не
   оживляет этот ключ; повторное добавление требует нового device key.
6. Наличие сертификата не отменяет проверку владения device secret: событие или
   session proof всё равно должно быть подписано самим устройством.
7. Неизвестный, неверно подписанный или относящийся к другому аккаунту отзыв
   не должен влиять на авторизацию и должен завершать проверку ошибкой.

## 3. Ключи и идентификаторы

В M0.5.1 `AccountId` — 32-байтовый Ed25519 verifying key, отображаемый как 64
hex-символа. Это осознанно прямой публичный ключ, а не хеш: проверяющая сторона
может сразу проверить root signature без дополнительного key-resolution слоя.

Корневое состояние хранится отдельно от device state:

```text
ACCOUNT_DIR/
    account-root-secret.key    # 32-byte Ed25519 secret, development plaintext
    next-authority-sequence    # следующий локальный sequence

STATE_DIR/
    device-secret.key          # существующий device signing secret
    device-certificate.cert    # публичный root-signed certificate
    ...
```

`account-root-secret.key` в текущем прототипе хранится plaintext. Это не
production-решение и не seed/recovery implementation. Не следует синхронизировать
`ACCOUNT_DIR` через облачный диск или передавать его другому устройству.

## 4. DeviceCertificate v1

Подписываемое содержимое:

```text
version: u8
account_id: AccountId
device_id: DeviceId
authority_sequence: u64
capabilities: sorted unique list<DeviceCapability>
```

M0.5.1 определяет две capabilities:

- `sign-events` — устройство может подписывать прикладные события;
- `sync-history` — устройство может участвовать в синхронизации истории.

Пустой, повторяющийся или неканонически отсортированный список отклоняется.
Подпись Ed25519 вычисляется над domain prefix
`kilogram:device-certificate-signature:v1\0` и Postcard-представлением
содержимого. Postcard остаётся временным Rust-only codec и не фиксирует
будущий публичный межъязыковой wire format.

`authority_sequence` выдаётся корневым состоянием монотонно, начиная с нуля.
Он задаёт порядок root operations и пригодится для будущего authority log, но
M0.5.1 ещё не решает конкурирующую работу нескольких копий root authority.

## 5. DeviceRevocation v1

Подписываемое содержимое:

```text
version: u8
account_id: AccountId
device_id: DeviceId
authority_sequence: u64
```

Domain prefix подписи:
`kilogram:device-revocation-signature:v1\0`.

Любой валидный отзыв данного `device_id` в текущем account view делает этот
ключ недействительным независимо от sequence сертификата. Такая монотонная
семантика не допускает двусмысленного «возврата» уже скомпрометированного ключа.
Чтобы снова добавить то же физическое устройство, клиент создаёт новую пару
device keys и получает новый сертификат.

## 6. Алгоритм проверки

Для авторизации устройства проверяющая сторона:

1. принимает доверенный `expected_account_id`;
2. декодирует сертификат и проверяет version, canonical capabilities и root
   signature;
3. требует точного совпадения Account ID;
4. требует capabilities, нужные текущей операции;
5. проверяет root signature и Account ID каждого полученного revocation;
6. отклоняет устройство, если хотя бы один валидный отзыв указывает его
   `DeviceId`;
7. отдельно проверяет device signature над событием или session proof.

M0.5.1 реализует шаги 1–6 в `kilogram-identity`. M0.5.2 добавляет
`SignedDeviceSessionAuthorization`: устройство подписывает сертификат вместе с
binding текущего listener Endpoint ID. `kilogram-session` связывает proof,
сертификат, требуемые capabilities и доверенный revocation view до того, как
listener принимает event или inventory. Подпись самого event/inventory затем
обязана принадлежать уже авторизованному device key.

## 7. CLI lifecycle

Development CLI позволяет проверить модель без сети:

```powershell
cargo run -p kilogram-cli -- account-create --account-dir .tmp/account-root
cargo run -p kilogram-cli -- account-show --account-dir .tmp/account-root
cargo run -p kilogram-cli -- device-enroll `
  --account-dir .tmp/account-root `
  --state-dir .tmp/alice `
  --certificate-file .tmp/alice.cert
cargo run -p kilogram-cli -- device-authorize `
  --state-dir .tmp/alice `
  --account-id <ACCOUNT_ID>
cargo run -p kilogram-cli -- device-revoke `
  --account-dir .tmp/account-root `
  --device-id <DEVICE_ID> `
  --revocation-file .tmp/alice.revocation
cargo run -p kilogram-cli -- device-authorize `
  --state-dir .tmp/alice `
  --account-id <ACCOUNT_ID> `
  --revocation-file .tmp/alice.revocation
```

Последняя команда обязана завершиться ошибкой. Exported certificate и
revocation являются публичными подписанными объектами; root secret в них не
входит. CLI не перезаписывает существующие export-файлы и не заменяет уже
установленный сертификат другим.

## 8. Реализованная граница

Реализовано:

- отдельное создание и загрузка Account Root;
- root-signed device certificate с capability checks;
- установка и загрузка сертификата из device state;
- root-signed permanent device revocation;
- публичная проверка account/device authorization;
- отрицательные тесты на tampering, другой Account ID, другой Device ID,
  недостающую capability и попытку повторной выдачи после отзыва.

M0.5.2 дополнительно реализует:

- ticket v3 с public listener certificate и разрешённым requester Account ID;
- явный `--expect-account`, предотвращающий незаметную замену listener другим
  самоподписанным аккаунтом;
- session proof, подписанный requester device и привязанный к текущему Endpoint;
- проверку caller-supplied root-signed revocations обеими сторонами;
- отказ до event/inventory для неверного account, proof или revoked device;
- sync нового сертифицированного устройства с пустой локальной историей без
  synthetic event и known-author bootstrap.

Не реализовано:

- derivation или восстановление root key из seed-фразы;
- защита root secret средствами ОС или аппаратного хранилища;
- recovery quorum, root rotation и разрешение конкурирующих authority events;
- автоматическое распространение полного актуального revocation view и
  доказательство его freshness/completeness;
- отдельные device encryption/session keys;
- срок действия и обновление сертификатов;
- account-authorized membership разговоров и проверка полномочий каждого автора
  получаемой history;
- окончательный codec и crypto-agility.

## 9. Сетевой контракт M0.5.2

M0.5.2 заменяет временное `--allow-device` / known-author правило на цепочку:

```text
trusted AccountId
    -> root-signed DeviceCertificate
    -> device-signed ticket/session proof/event
    -> caller-supplied root-signed revocation view
```

Ticket v3 содержит endpoint, public listener certificate, разрешённый requester
Account ID и route policy. Listener device подписывает весь ticket. Клиент
сначала проверяет root signature сертификата, Account ID из
`--expect-account`, capabilities, предоставленные ему revocations и затем
device signature ticket. Listener аналогично принимает только сертификат
аккаунта из `--allow-account`.

После QUIC/path establishment клиент открывает отдельный authorization stream.
Он отправляет certificate и device signature над certificate + binding текущего
listener Endpoint ID. Listener отвечает `DeviceAuthorized` или общим
`DeviceAuthorizationRejected`; лишь после успешного ответа открывается stream с
event или inventory. Следующие sync rounds используют уже авторизованный device
на том же connection.

Для revocation enforcement проверяющая сторона получает публичные файлы через
повторяемый `--peer-revocation-file`. Объекты проверяются криптографически, но
M0.5.2 не умеет доказать отсутствие более свежего отзыва: пустой или устаревший
набор не становится полным только потому, что его предоставил peer. До
реализации authenticated authority-log synchronization это явная операционная
граница, а не обещание мгновенного глобального отзыва.

Локальный process smoke подтвердил delivery между двумя отдельными аккаунтами,
recovery sync двух events на новое устройство того же requester account без
предыдущего авторства и отказ нового session после передачи listener валидного
root-signed revocation.

## 10. Следующий срез

Следующий identity/security этап должен определить conversation membership и
authenticated распространение свежего authority/revocation state. Pairwise
E2EE может начинаться поверх уже существующей Account → Device → Session
цепочки, но не должен считать M0.5.2 production revocation service.
