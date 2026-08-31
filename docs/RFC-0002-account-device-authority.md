# RFC-0002: Account Root и авторизация устройств

- Статус: **Implemented draft (M0.6.1)**
- Дата: **2026-08-31**
- Область: Account ID, device certificates, capabilities, revocation snapshots
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

M0.6.1 заменяет передаваемые вручную отдельные файлы отзывов полным
`AccountAuthoritySnapshot`. Snapshot подписан root key, содержит монотонную
revision и канонический полный набор permanent revocations на эту revision.
Устройства сохраняют максимальную увиденную revision каждого peer account и
отклоняют rollback или два разных snapshot с одинаковой revision.

M0.7.1 расширяет сертификат до v2 и root-подписанно связывает отдельный
X25519 public key устройства. Полный payload-контракт описан в
[`RFC-0004`](RFC-0004-pairwise-hpke-payload.md).

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
8. Сертификат и каждый отзыв должны иметь `authority_sequence < revision`
   используемого snapshot.
9. После принятия revision `N` устройство никогда не принимает revision `< N`;
   разные валидно подписанные состояния с revision `N` считаются root
   equivocation и тоже отклоняются.

## 3. Ключи и идентификаторы

В M0.5.1 `AccountId` — 32-байтовый Ed25519 verifying key, отображаемый как 64
hex-символа. Это осознанно прямой публичный ключ, а не хеш: проверяющая сторона
может сразу проверить root signature без дополнительного key-resolution слоя.

Корневое состояние хранится отдельно от device state:

```text
ACCOUNT_DIR/
    account-root-secret.key    # 32-byte Ed25519 secret, development plaintext
    next-authority-sequence    # следующий локальный sequence
    authority-log-version      # версия durable authority layout
    revocations/
        <DEVICE_ID>.revocation # полный durable permanent-revocation set

STATE_DIR/
    device-secret.key          # существующий device signing secret
    device-encryption-secret.key # HPKE X25519 key seed, development plaintext
    device-certificate.cert    # публичный root-signed certificate
    account-authority.snapshot # snapshot собственного аккаунта
    peer-authority/
        <ACCOUNT_ID>.snapshot  # max revision, увиденная для peer account
    ...
```

`account-root-secret.key` в текущем прототипе хранится plaintext. Это не
production-решение и не seed/recovery implementation. Не следует синхронизировать
`ACCOUNT_DIR` через облачный диск или передавать его другому устройству.

## 4. DeviceCertificate v1/v2

Подписываемое содержимое:

```text
version: u8
account_id: AccountId
device_id: DeviceId
encryption_public_key: EncryptionPublicKey # добавлено в v2
authority_sequence: u64
capabilities: sorted unique list<DeviceCapability>
```

M0.5.1 определяет две capabilities:

- `sign-events` — устройство может подписывать прикладные события;
- `sync-history` — устройство может участвовать в синхронизации истории.

Пустой, повторяющийся или неканонически отсортированный список отклоняется.
Подпись Ed25519 текущего v2 вычисляется над domain prefix
`kilogram:device-certificate-signature:v2\0` и Postcard-представлением
содержимого. Исторический v1 не содержал encryption key и использовал domain
v1. Postcard остаётся временным Rust-only codec и не фиксирует
будущий публичный межъязыковой wire format.

`authority_sequence` выдаётся корневым состоянием монотонно, начиная с нуля.
Он задаёт порядок root operations и пригодится для будущего authority log, но
M0.6.1 всё ещё предполагает единственный локальный writer Account Root и не
решает конкурирующую работу нескольких копий root authority.

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

## 6. AccountAuthoritySnapshot v1

Подписываемое содержимое:

```text
version: u8
account_id: AccountId
revision: u64
revocations: sorted unique list<DeviceRevocation>
```

`revision` равна следующему невыданному `authority_sequence`. Поэтому любой
сертификат или отзыв, покрываемый snapshot, имеет меньший sequence. Revocations
сортируются по `DeviceId`; повтор, другой Account ID, sequence из будущего или
невалидная вложенная root signature отклоняются до проверки внешней подписи.
Domain prefix внешней подписи:
`kilogram:account-authority-snapshot-signature:v1\0`.

Root сохраняет каждый отзыв до публикации snapshot. Старый root directory, в
котором уже были authority operations, но ещё не было durable revocation log,
не может безопасно объявить свой набор полным и отклоняется как legacy state.
Автоматическая миграция такого состояния в M0.6.1 намеренно отсутствует.

## 7. Алгоритм проверки

Для авторизации устройства проверяющая сторона:

1. принимает доверенный `expected_account_id`;
2. декодирует сертификат и проверяет version, canonical capabilities и root
   signature;
3. требует точного совпадения Account ID;
4. требует capabilities, нужные текущей операции;
5. проверяет root signature snapshot, его Account ID, canonical full revocation
   set и покрытие certificate/revocation sequences указанной revision;
6. сравнивает revision с максимальной сохранённой для этого аккаунта и
   отклоняет rollback/equivocation;
7. сохраняет более новый snapshot атомарной заменой;
8. отклоняет устройство, если хотя бы один валидный отзыв указывает его
   `DeviceId`;
9. отдельно проверяет device signature над событием или session proof.

M0.5.1 реализует шаги 1–6 в `kilogram-identity`. M0.5.2 добавляет
`SignedDeviceSessionAuthorization`: устройство подписывает сертификат вместе с
binding текущего listener Endpoint ID. `kilogram-session` связывает proof,
сертификат, требуемые capabilities и подписанный authority snapshot до того, как
listener принимает event или inventory. Подпись самого event/inventory затем
обязана принадлежать уже авторизованному device key.

## 8. CLI lifecycle

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
cargo run -p kilogram-cli -- account-snapshot `
  --account-dir .tmp/account-root `
  --snapshot-file .tmp/account.snapshot
cargo run -p kilogram-cli -- device-authority-update `
  --state-dir .tmp/alice `
  --snapshot-file .tmp/account.snapshot
cargo run -p kilogram-cli -- device-authorize `
  --state-dir .tmp/alice `
  --account-id <ACCOUNT_ID>
```

Последняя команда обязана завершиться ошибкой. Exported certificate и
revocation являются публичными подписанными объектами; root secret в них не
входит. CLI не перезаписывает существующие export-файлы и не заменяет уже
установленный сертификат другим.

`device-enroll` автоматически создаёт и устанавливает snapshot сразу после
выдачи сертификата. После последующих root-операций устройство получает более
новую версию через `account-snapshot` + `device-authority-update`.

## 9. Реализованная граница

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

M0.6.1 дополнительно реализует:

- durable полный набор permanent revocations у Account Root;
- root-signed snapshot с canonical completeness на конкретной revision;
- atomically persisted own/peer snapshots и защита от rollback/equivocation;
- ticket v4 с listener snapshot и session authorization v2 с requester snapshot;
- pinning snapshot до certificate authorization, поэтому revoked device может
  доставить подписанное состояние, которое немедленно запретит его же session;
- CLI export/update lifecycle без `--peer-revocation-file`.

M0.6.2 применяет эту authority chain к каждому автору conversation history.
Owner-signed membership и `AuthorizedEvent` определены отдельно в
[`RFC-0003`](RFC-0003-conversation-membership.md).

M0.7.1 добавляет отдельный persistent encryption key, certificate v2 и ticket
v5/session authorization v3. Сертификат больше нельзя заменить без явной
миграции device state; для текущего development spike нужен свежий state.

Не реализовано:

- derivation или восстановление root key из seed-фразы;
- защита root secret средствами ОС или аппаратного хранилища;
- recovery quorum, root rotation и разрешение конкурирующих authority events;
- discovery/gossip/witness-механизм, гарантирующий получение глобально самой
  свежей revision при первом контакте;
- session ratchet/prekey keys с forward secrecy и post-compromise security;
- срок действия и обновление сертификатов;
- discovery/gossip свежих account и conversation snapshots;
- окончательный codec и crypto-agility.

## 10. Сетевой контракт M0.6.1

Ниже зафиксирован исторический M0.6.1 contract. Текущий M0.7.1 переносит тот же
authority смысл в несовместимые ticket v5 и session authorization v3 из-за
DeviceCertificate v2.

M0.5.2 заменяет временное `--allow-device` / known-author правило на цепочку:

```text
trusted AccountId
    -> root-signed DeviceCertificate
    + root-signed complete AccountAuthoritySnapshot(revision)
    -> device-signed ticket/session proof/event
    -> persistent max-seen revision per peer account
```

Ticket v4 содержит endpoint, public listener certificate, listener authority
snapshot, разрешённый requester Account ID и route policy. Listener device
подписывает весь ticket. Клиент проверяет device signature ticket, root
signatures, Account ID из `--expect-account`, pin/rollback state и certificate
authorization. Listener аналогично принимает только certificate + snapshot
аккаунта из `--allow-account`.

После QUIC/path establishment клиент открывает отдельный authorization stream.
Он отправляет certificate, snapshot и device signature над ними + binding
текущего listener Endpoint ID. Listener сначала проверяет proof и Account ID,
затем сохраняет snapshot и только после этого проверяет revocation certificate.
Listener отвечает `DeviceAuthorized` или общим
`DeviceAuthorizationRejected`; лишь после успешного ответа открывается stream с
event или inventory. Следующие sync rounds используют уже авторизованный device
на том же connection.

Snapshot доказывает полноту permanent revocations только на подписанную
revision. Anti-rollback доказывает, что после встречи с revision `N` узел не
вернулся назад. Но первая встреча со старым, корректно подписанным snapshot не
позволяет узнать, существует ли где-то revision `N+1`. До появления
authenticated discovery/gossip/witness это явная граница, а не обещание
мгновенного глобального отзыва.

Локальный process smoke подтвердил delivery между двумя отдельными аккаунтами,
recovery sync двух events на новое устройство того же requester account без
предыдущего авторства и отказ нового session после передачи listener валидного
root-signed revocation.

## 11. Следующий срез

M0.6.2 завершил минимальный signed conversation membership и проверку каждого
автора history. M0.7.1 добавил static-key pairwise HPKE payload baseline.
Authenticated gossip/witness для first-contact freshness, asynchronous ratchet
с FS/PCS, membership removal/MLS epochs, seed/recovery и root rotation остаются
отдельными срезами.
