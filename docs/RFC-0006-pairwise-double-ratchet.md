# RFC-0006: authenticated pairwise Double Ratchet

- Статус: **Implemented spike (M0.7.3)**
- Дата: 2026-08-31
- Область: асинхронное установление и сохранение pairwise ratchet-сессии

> M0.7.4 расширил этот single-recipient контракт до полного account-wide
> fan-out. Актуальные device-list и event-инварианты описаны в
> [`RFC-0007`](RFC-0007-multi-device-ratchet-fanout.md). M0.7.5 добавил
> same-account recovery старых projections в
> [`RFC-0008`](RFC-0008-authenticated-history-rewrap.md), а M0.7.6 заменил
> сетевой single-OTK контракт signed pools и concurrent-session resolution в
> [`RFC-0009`](RFC-0009-authenticated-prekey-pools.md).

## 1. Цель этапа

M0.7.2 отделил реплицируемый ciphertext от читаемой локальной истории, но
шифровал каждый входящий event долговременным HPKE-ключом получателя. Утечка
этого ключа раскрывала все ранее записанные peer boxes.

M0.7.3 заменяет сетевой HPKE box на Olm/Double Ratchet из Rust-библиотеки
`vodozemac` 0.10.0. Собственная реализация ratchet-примитивов исключена. HPKE
остаётся только у `LocalTextProjection`, которая никогда не реплицируется.

## 2. Криптографическая граница

Новый crate `kilogram-ratchet` — единственный слой, который работает с
`vodozemac::olm::Account`, `Session` и encrypted pickle. Остальные crates видят
только:

- `SignedRatchetIdentity`;
- `SignedPrekeyBundle`;
- валидированный непрозрачный `RatchetCiphertext`;
- `DecryptedMessage`, который нельзя сконструировать вне успешного decrypt;
- диагностический результат с session ID, типом `pre-key`/`normal` и признаком
  создания сессии.

Используется совместимая с Olm конфигурация `SessionConfig::version_1`. Это
проверяемый прототип, а не заявление о финальном выборе pairwise-протокола.

## 3. Аутентификация ключей

Olm account имеет постоянные Curve25519 и Ed25519 public keys. Kilogram
связывает их с существующим application `DeviceId` отдельной Ed25519-подписью
device key и domain separation.

`SignedPrekeyBundle` содержит:

- подписанную ratchet identity;
- монотонный локальный bundle sequence;
- один Curve25519 one-time key;
- вторую device signature над всем bundle.

Connection ticket v7 включает bundle listener. Проверка ticket сначала
проверяет root-signed `DeviceCertificate` и authority snapshot, затем обе
device signatures ratchet-объектов и совпадение их Device ID. Таким образом,
Iroh endpoint, Account/Device authorization и асинхронный prekey находятся в
одном подписанном ticket envelope.

## 4. Установление и развитие сессии

Если persistent session с peer ещё нет, отправитель создаёт outbound session из
подписанных Curve25519 identity/one-time keys получателя. Первое и все
последующие сообщения до ответа имеют тип `PreKey` и несут данные 3DH
установления.

Получатель принимает только `PreKey`, если session отсутствует. Vodozemac
проверяет MAC, создаёт inbound session и только после успешного decrypt удаляет
использованный one-time private key. Kilogram сразу сохраняет session/account и
публикует следующий подписанный one-time key.

Первый ответ получателя уже `Normal`. После его decrypt исходная сторона
считает session установленной; следующие сообщения также `Normal` и двигают
симметричные и DH ratchets. Session выбирается по peer Device ID; смена
подписанного Curve25519 identity для того же Device ID отклоняется fail-closed.

## 5. Реплицируемый event и локальная история

`EventPayload::RatchetText` v4 содержит:

- единственный recipient Device ID;
- подписанную ratchet identity автора;
- Olm message type и ciphertext.

Device signature `SignedEvent` покрывает recipient, ratchet identity,
ciphertext, conversation, author sequence и causal parents. Plaintext и
статический recipient HPKE box в event отсутствуют.

Автор создаёт `LocalTextProjection` из исходного текста. Получатель может
создать её только из `DecryptedMessage`, возвращённого ratchet-слоем. Повторная
доставка уже принятого event читает существующую projection и не пытается
повторно использовать уничтоженный message key.

## 6. Persistent state

Под `STATE_DIR/ratchet` хранятся:

- encrypted Olm account pickle;
- текущий публичный signed prekey bundle;
- по одному encrypted session pickle на peer Device ID;
- случайный 32-byte pickle key.

Обновления pickle записываются во временный файл, синхронизируются и атомарно
заменяют предыдущее состояние. Pickle key пока лежит рядом без passphrase или
OS keystore, поэтому это только защита формата at rest, не защита от
компрометации всего устройства.

M0-файловая модель не может одной транзакцией обновить ratchet, local
projection и immutable event. Session сохраняется до подтверждения операции,
но crash между этими файлами может оставить использованный message key без
event или event без projection. Production storage обязан объединить эти
изменения в транзакционную базу и ввести state-directory lock.

## 7. Sync и восстановление

Sync переносит те же ratchet ciphertext events. Для отсутствующего входящего
event адаптер:

1. проверяет membership, Account/Device authorization и event signature;
2. обрабатывает ratchet text одного автора по `author_sequence`;
3. двигает и сохраняет session;
4. создаёт local projection;
5. только затем сохраняет event batch.

Проверен batch из трёх prekey-events в пустой event store получателя. Повторный
sync идемпотентен благодаря уже существующим projections.

Forward secrecy намеренно меняет recovery semantics. Копии одного device key
недостаточно: без соответствующих Olm account/session pickles старые ratchet
ciphertexts не расшифровываются. Утраченную sender projection также нельзя
восстановить из peer ciphertext. Новому устройству нужен отдельный signed
device/prekey fan-out и authenticated history rewrap от живого устройства.

## 8. Версии

Несовместимое изменение повышает границы:

- SignedEvent/Event ID — v4;
- sync envelopes и signature domains — v5;
- device session authorization — v5;
- Iroh ALPN — `kilogram/m0/sync/5`;
- connection ticket — v7.

Состояние событий M0.7.2 не мигрируется автоматически. DeviceCertificate v2 и
LocalTextProjection v1 сохраняются: их смысл не изменился.

## 9. Проверенные свойства

- real prekey → inbound session → normal reply → normal subsequent message;
- account и обе стороны session переживают перезапуск между шагами;
- signed bundle и event tampering отклоняются;
- peer ratchet identity нельзя молча заменить для существующей session;
- account/session files не содержат fixture plaintext;
- serialized event и local projection не содержат fixture plaintext;
- direct delivery сохраняет session/projection/event до acknowledgement;
- sync создаёт recipient projection до сохранения ratchet event;
- ticket связывает prekey bundle с certified listener Device ID.

## 10. Что этап не обещает

- один опубликованный OTK в M0 не является production prekey pool и плохо
  подходит для одновременных first messages от нескольких devices;
- поддерживается одна session на пару Device IDs; одновременная двусторонняя
  инициация и выбор между несколькими sessions не разрешены;
- нет signed device list, multi-device fan-out, history rewrap или seed recovery;
- нет PQXDH, header encryption, metadata hiding, deniability review или
  формальной верификации композиции с Kilogram event DAG;
- локальная projection сохраняет читаемую историю по продуктовому требованию,
  поэтому компрометация разблокированного клиента раскрывает эту историю даже
  при forward-secret сетевом ciphertext;
- vodozemac/Olm остаётся M0-кандидатом; перед production нужны внешний аудит,
  interop vectors, dependency policy и решение Signal-style ratchet против
  двухучастникового MLS.

## 11. Следующий pairwise этап

M0.7.4 добавил signed device-list fan-out и отдельный ciphertext для prekey
каждого устройства. M0.7.5 добавил authenticated history rewrap с явной
маркировкой неполного source inventory. M0.7.6 добавил signed prekey pools и
crossed-session resolution; транзакционная storage-модель остаётся следующим
отдельным срезом.

Документация использованной реализации:
<https://matrix-org.github.io/vodozemac/vodozemac/olm/index.html>.
Нормативная модель Double Ratchet:
<https://signal.org/docs/specifications/doubleratchet/>.
