# RFC-0004: pairwise HPKE-шифрование payload

- Статус: **Implemented spike (M0.7.1)**
- Дата: 2026-08-31
- Область: E2EE содержимого сообщения между двумя конкретными устройствами

> Исторический контракт M0.7.1. M0.7.2 удалил sender box из реплицируемого
> event и вынес читаемую историю в local-only encrypted projection; текущая
> граница описана в [RFC-0005](RFC-0005-local-encrypted-history-projection.md).

## 1. Задача и граница этапа

До M0.7.1 подписи и authorization chain защищали целостность событий и право
автора участвовать в разговоре, но `Text` сохранялся и передавался открыто.
Relay, transport peer и любой читатель каталога events могли увидеть тело
сообщения.

M0.7.1 делает первый узкий E2EE-срез: тело одного сообщения шифруется отдельно
для устройства отправителя и одного устройства получателя. Подписанный causal
event, account/device authorization, membership и sync остаются внешним
проверяемым конвертом.

Это намеренно **не** Double Ratchet и не production-протокол личного чата.
Этап проверяет композицию E2EE с существующим event DAG, storage и P2P sync.

## 2. Ключ устройства и сертификат

Каждый `DeviceState` содержит две независимые долговременные идентичности:

- Ed25519 signing key, публичная часть которого образует `DeviceId`;
- 32-байтовый секретный seed для HPKE X25519 keypair.

HPKE seed сохраняется в `device-encryption-secret.key`. В development state он
пока не защищён паролем или OS keystore, но никогда не входит в ticket, wire
event или authorization sidecar.

Формат `DeviceCertificate` v2 добавляет `encryption_public_key`. Account Root
подписывает одним сертификатом Account ID, Device ID, X25519 public key,
authority sequence и capabilities. При установке и загрузке своего сертификата
устройство требует совпадения обеих локальных публичных частей. Поэтому peer не
должен принимать отдельный неподписанный ключ шифрования.

## 3. Ciphersuite и аутентификация

Используется HPKE Base mode по RFC 9180:

- KEM: DHKEM(X25519, HKDF-SHA256);
- KDF: HKDF-SHA256;
- AEAD: ChaCha20-Poly1305.

Каждый recipient box создаётся отдельной single-shot HPKE операцией с новым
эфемерным encapsulated key. Base mode сам по себе не аутентифицирует отправителя.
В Kilogram аутентификацию даёт внешняя Ed25519-подпись `SignedEvent`, которая
покрывает полный список recipient boxes, а `AuthorizedEvent` связывает автора с
Account Root и conversation membership.

HPKE `info` имеет отдельный domain separator. В AEAD AAD входят:

- версия event format;
- conversation ID;
- author Device ID и author sequence;
- causal parents;
- Device ID конкретного получателя box.

Изменение этих метаданных либо ciphertext обнаруживается подписью события и/или
AEAD open. Event ID не входит в AAD, потому что он вычисляется от уже готового
подписанного ciphertext event.

## 4. Формат encrypted event

Вариант plaintext `EventPayload::Text` удалён. Вместо него
`EncryptedText` содержит ровно два canonical отсортированных уникальных box:

1. для текущего устройства автора — чтобы оно могло читать собственную
   отправленную историю;
2. для одного конкретного peer device из проверенного root-signed certificate.

Каждый box содержит recipient Device ID, 32-байтовый encapsulated key и
ChaCha20-Poly1305 ciphertext. Максимальный plaintext — 64 KiB; wire validator
ограничивает соответствующий ciphertext размером plaintext плюс AEAD tag.

Acknowledgement пока остаётся незашифрованным служебным событием. Оно содержит
только Event ID подтверждаемого сообщения, causal parent и metadata автора, но
не тело сообщения.

## 5. Delivery, history и sync

- `connect` берёт ключ собственного устройства из установленного сертификата,
  а ключ listener — из проверенного ticket v5, создаёт два box и только после
  этого подписывает событие.
- Listener проверяет membership/authority/signature, находит свой box и обязан
  успешно расшифровать UTF-8 **до** сохранения и acknowledgement.
- `history` повторно проверяет authorization chain и расшифровывает box
  текущего устройства. Отсутствующий box, чужой key или повреждённый AAD/ciphertext
  дают fail-closed ошибку.
- Event store и sync работают с тем же immutable ciphertext. Sync-узлу не
  требуется plaintext для deduplication, Event ID, author sequence и causal DAG.
- CLI оборачивает sync store локальным decrypting adapter: весь входящий batch
  обязан иметь корректный box для текущего Device ID и полностью пройти AEAD
  open до атомарной записи. Нечитаемое событие не отравляет локальную историю.
- Development-only `seed-history` требует публичный peer certificate через
  `--peer-certificate-file` и создаёт те же два encrypted box, поэтому helper не
  оставляет plaintext events.

`.authorization` sidecar содержит только публичные certificate/snapshot proofs.
Команда `history` выводит расшифрованный текст в консоль, но на диске event body
остаётся ciphertext.

## 6. Несовместимые версии

M0.7.1 намеренно повышает границы протокола:

- DeviceCertificate — v2 и новый signature domain;
- SignedEvent/Event ID — v2;
- sync envelopes/signature domains — v3;
- device session authorization — v3;
- Iroh ALPN — `kilogram/m0/sync/3`;
- connection ticket — v5.

Старые tickets/events/certificates не должны молча декодироваться новым
клиентом. Автоматической миграции plaintext history нет. Для M0.7.1 нужен новый
device state и новый сертификат: существующая установка сертификата immutable и
не заменяется неявно.

## 7. Проверенные свойства

- HPKE round trip работает, а чужой secret key и изменённый AAD отклоняются;
- подписанный encrypted event расшифровывается обоими указанными устройствами,
  но не посторонним Device ID;
- serialized event и фактический `.event` не содержат тестовый plaintext;
- изменение ciphertext нарушает event signature;
- certificate persistence связывает X25519 key с тем же device/account;
- все прежние membership, revocation, authorization, store и multi-round sync
  tests работают с ciphertext events.
- sync batch без box текущего устройства отклоняется до записи на диск.

## 8. Явные ограничения и threat model

- Статический recipient key позволяет расшифровать всю записанную историю после
  будущей компрометации этого key. Forward secrecy и post-compromise security
  **нет**.
- Ровно один peer device означает, что fan-out на все устройства аккаунта,
  device list discovery и добавление нового устройства ещё не реализованы.
- Нет prekey bundles, asynchronous first message, session ratchet, skipped-key
  handling или replay window уровня мессенджера.
- Conversation ID, account/device certificates, author sequence, causal links,
  размеры, время и сетевые адреса не скрыты этим content-encryption слоем.
- Локальные key files и расшифрованный текст в памяти/консоли не защищены от
  компрометации endpoint. Production keystore и encrypted database отсутствуют.
- Нет attachments, offline blind mailbox, disappearing-message key erasure и
  cryptographic member removal.
- Используемый Rust HPKE crate и вся композиция требуют независимого security
  review до любых заявлений о production security.

## 9. Следующая криптографическая работа

Следующий pairwise этап должен выбрать и прототипировать асинхронное
установление сессии и ratchet с явными forward-secrecy/PCS свойствами, включая
fan-out по device list. HPKE M0.7.1 остаётся полезным проверяемым baseline и не
должен называться заменой Double Ratchet. Для групп removal и rekey по-прежнему
проектируются вместе с MLS epochs.
