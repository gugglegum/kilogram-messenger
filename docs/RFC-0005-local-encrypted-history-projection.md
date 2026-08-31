# RFC-0005: локальная зашифрованная проекция истории

- Статус: **Implemented spike (M0.7.2)**
- Дата: 2026-08-31
- Область: разделение реплицируемого E2EE-event и читаемой локальной истории

## 1. Почему этот слой нужен до Double Ratchet

В M0.7.1 один `EncryptedText` содержал два static-key HPKE box: для peer и для
самого автора. Второй box позволял автору читать отправленную историю прямо из
реплицированного event, но делал будущий ratchet бессмысленным для записанного
исходящего трафика: компрометация долговременного sender encryption key
раскрывала бы все ранее перехваченные sender boxes.

Double Ratchet специально создаёт отдельный ключ для каждого сообщения и
удаляет его после использования. Поэтому ciphertext log и долговременно
читаемая локальная история должны быть разными объектами. M0.7.2 вводит это
разделение до выбора конкретной ratchet-библиотеки.

## 2. Реплицируемый event

`EventPayload::EncryptedText` v3 содержит ровно один HPKE box для конкретного
peer Device ID. Device автора не может быть получателем этого box. Полный
ciphertext и recipient ID остаются под Ed25519-подписью `SignedEvent`; HPKE AAD
по-прежнему связывает conversation, автора, sequence, parents и recipient.

Event store, authorization sidecar, Event ID, causal DAG и sync реплицируют
только этот объект. На relay, peer storage или wire нет отдельной статической
копии, расшифровываемой ключом автора.

## 3. LocalTextProjection

Каждое участвующее устройство хранит отдельный local-only sidecar:

- версия projection format;
- Event ID;
- локальный Device ID;
- HPKE ciphertext, зашифрованный на локальный device encryption key.

AAD projection связывает version, Event ID и локальный Device ID. Файл имеет
имя `<event-id>.local-text`, записывается атомарно, не перезаписывается и
проверяется при чтении. Повтор exact projection идемпотентен; иной ciphertext
для уже занятого Event ID считается immutable conflict.

Sidecar не является частью `AuthorizedEvent`, inventory или sync wire format.
Это локальная materialized projection: sender создаёт её из исходного текста,
recipient — только после успешного AEAD open сетевого box.

## 4. Порядок записи и fail-closed правила

- `connect` подписывает recipient-only event, затем сохраняет локальную
  projection и только после этого event перед отправкой;
- listener проверяет membership/authority/signature, расшифровывает recipient
  box, сохраняет projection, затем event и только потом acknowledgement;
- sync проверяет authorization всего event, требует существующую читаемую
  projection либо расшифровывает recipient box и создаёт её до event batch;
- повторная доставка использует существующую projection и сравнивает plaintext,
  поэтому случайный HPKE nonce не создаёт ложный immutable conflict;
- `history` больше не пытается расшифровать replicated event своим ключом: она
  требует sidecar и fail-closed проверяет его против Event ID и Device ID.

Если process завершится между sidecar и event write, может остаться безопасный
orphan sidecar. Обратное состояние — event без читаемой projection — не
создаётся штатным write path. Production database должна заменить этот M0
двухфайловый порядок одной транзакцией.

## 5. Sync и восстановление

Обычная двусторонняя синхронизация сохраняется:

- автор уже имеет projection исходящего event;
- peer создаёт свою projection при delivery или входящем sync;
- acknowledgements не содержат message body и projection не требуют.

Полная утрата sender projection является честной границей M0.7.2. Sender не
может расшифровать recipient-only event, возвращённый peer, и sync отклонит его
без заранее существующей projection. Будущее восстановление нового/очищенного
устройства требует authenticated history rewrap: устройство, у которого есть
plaintext, должно заново зашифровать его для нового разрешённого device. Нельзя
возвращать sender box в общий event ради удобства recovery.

## 6. Версии

Несовместимое изменение повышает границы:

- SignedEvent/Event ID — v3;
- sync envelopes/signature domains — v4;
- device session authorization — v4;
- Iroh ALPN — `kilogram/m0/sync/4`;
- connection ticket — v6.

DeviceCertificate остаётся v2: долговременная X25519 public key binding не
изменилась. M0.7.1 events не мигрируются автоматически.

## 7. Проверенные свойства

- serialized event содержит один peer recipient и не расшифровывается автором;
- sender и recipient создают разные local-only projections и читают их своим
  ключом;
- projection bytes и event bytes не содержат fixture plaintext;
- отсутствующая, повреждённая, чужая или относящаяся к другому event projection
  отклоняется;
- sync создаёт projection до сохранения входящего recipient event;
- повторный sync того же event идемпотентен;
- event без recipient box текущего устройства и без собственной projection не
  попадает в event store.

## 8. Что этот этап не обещает

- Recipient box всё ещё использует static HPKE key, поэтому входящие сообщения
  пока не имеют forward secrecy или post-compromise security.
- Local projection зашифрована, но ключ лежит рядом в development state без OS
  keystore/passphrase; это структурная граница, а не production at-rest security.
- Нет prekey bundle, asynchronous first message, ratchet session, skipped-key
  limits, device-list fan-out или history rewrap.
- Metadata event DAG и traffic analysis не скрыты.

## 9. Следующий pairwise этап

M0.7.3 должен прототипировать проверенную Rust-реализацию asynchronous session
establishment и Double Ratchet поверх recipient-only slot. Кандидат
`vodozemac::olm` предоставляет pre-key/normal messages, persistent session
pickle и double-ratchet API; до выбора нужно отдельно проверить протокол
аутентификации prekey bundle нашим Account/Device chain, replay semantics,
лицензию и границы Matrix-specific формата. Реализация Signal primitives с
нуля исключена.

Нормативная модель свойств ratchet: <https://signal.org/docs/specifications/doubleratchet/>.
Документация рассматриваемого Rust-кандидата:
<https://matrix-org.github.io/vodozemac/vodozemac/olm/index.html>.
