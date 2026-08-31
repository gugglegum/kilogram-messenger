# RFC-0002: Account Root и авторизация устройств

- Статус: **Implemented draft (M0.5.1)**
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

Этот срез намеренно не меняет M0.4 connection ticket и session protocol.
Применение новой authority-модели при delivery и sync выделено в M0.5.2.

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

M0.5.1 реализует шаги 1–6 в `kilogram-identity`. Шаг 7 уже существует для
M0-events и session messages, но связывание двух проверок в сетевом протоколе
относится к M0.5.2.

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

## 8. Граница M0.5.1

Реализовано:

- отдельное создание и загрузка Account Root;
- root-signed device certificate с capability checks;
- установка и загрузка сертификата из device state;
- root-signed permanent device revocation;
- публичная проверка account/device authorization;
- отрицательные тесты на tampering, другой Account ID, другой Device ID,
  недостающую capability и попытку повторной выдачи после отзыва.

Не реализовано:

- derivation или восстановление root key из seed-фразы;
- защита root secret средствами ОС или аппаратного хранилища;
- recovery quorum, root rotation и разрешение конкурирующих authority events;
- распространение полного актуального revocation view;
- отдельные device encryption/session keys;
- срок действия и обновление сертификатов;
- account-authorized membership разговоров;
- проверка сертификатов и отзывов в connection ticket, delivery и sync;
- окончательный codec и crypto-agility.

## 9. Следующий срез: M0.5.2

M0.5.2 должен заменить временное `--allow-device` / known-author правило на
публично проверяемую цепочку:

```text
trusted AccountId
    -> root-signed DeviceCertificate
    -> device-signed ticket/session proof/event
    -> current root-signed revocation view
    -> conversation membership policy
```

Минимальный integration test должен доказать, что сертифицированное устройство
может доставлять и синхронизировать события, несертифицированное устройство
отклоняется до раскрытия истории, а новый session после получения revocation
отклоняет ранее действительный device key.
