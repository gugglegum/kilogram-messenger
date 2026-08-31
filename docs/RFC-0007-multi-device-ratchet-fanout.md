# RFC-0007: signed multi-device ratchet fan-out

- Статус: **Implemented spike (M0.7.4)**
- Дата: 2026-09-01
- Область: полный список устройств аккаунта и отдельный pairwise ciphertext для
  каждого устройства

## 1. Цель этапа

M0.7.3 устанавливал persistent ratchet только с устройством, которое создало
connection ticket. Если у Bob было два устройства, сообщение Alice содержало
ciphertext только для Bob-1; Bob-2 мог синхронизировать event, но не мог его
расшифровать.

M0.7.4 добавляет проверяемый account-wide fan-out. Один логический text event
по-прежнему имеет один Event ID, causal parents и author sequence, но содержит
отдельный Olm ciphertext для каждого устройства peer account.

## 2. Root-signed account device list

`AccountDeviceListSnapshot` содержит:

- полный `AccountAuthoritySnapshot` с revision и revocations;
- от 1 до 32 root-signed `DeviceCertificate`;
- root signature над всем каноническим списком.

Сертификаты сортируются по Device ID. Дубликаты, неверный account, отсутствующие
messaging capabilities, certificate вне authority revision и revoked device
отклоняются. `AccountRootState` сохраняет опубликованный snapshot и не позволяет
подписать другой список на той же authority revision. Enrollment или revocation
увеличивает authority revision и разрешает публикацию нового списка.

M0 CLI получает список сертификатов явно через `account-device-list`. Root state
пока не ведёт собственный реестр выданных сертификатов, поэтому оператор обязан
передать все активные certificate files. После подписи именно опубликованный
snapshot становится утверждением Root о полноте списка.

## 3. Authenticated prekey directory

`AccountPrekeyDirectory` соединяет root-signed device list с ровно одним
`SignedPrekeyBundle` каждого перечисленного Device ID. Проверка требует точного
совпадения отсортированных Device ID: нельзя добавить outsider, удалить одно
устройство или подставить bundle другого устройства.

Root не подписывает быстро меняющиеся prekeys. Каждый bundle подписан своим
device key, уже авторизованным root-signed certificate. Connection ticket v8
дополнительно подписывает конкретную сборку directory ключом online listener и
связывает её с Iroh Endpoint ID, route policy и allowed requester Account ID.

Listener автоматически добавляет собственный текущий bundle. Public bundles
остальных устройств передаются через повторяемый
`--peer-prekey-bundle-file`. Это временная ручная distribution boundary, а не
production discovery service.

## 4. Fan-out event

`EventPayload::RatchetText` v5 содержит:

- root-signed `recipient_device_list`;
- общую device-signed ratchet identity автора;
- канонический массив `RatchetRecipient { device_id, ciphertext }`.

Event validation требует, чтобы recipient slots точно и в том же порядке
совпадали со всеми сертификатами embedded device list. Полнота fan-out поэтому
проверяется и после store/sync, без исходного ticket. Внешняя подпись автора
покрывает device list, все ciphertext, conversation metadata и causal parents.

Перед созданием event sender продвигает отдельную persistent Olm session для
каждого recipient Device ID. Для нового offline device получается `PreKey`, для
существующей сессии — `Normal`. В event нет sender box и нет общего ключа,
который позволил бы одному устройству расшифровать слот другого.

## 5. Delivery и sync

Online listener выбирает слот своего Device ID, расшифровывает его и сохраняет
local projection до event/acknowledgement. Перед расшифрованием client также
требует, чтобы embedded device list принадлежал Account ID его установленного
certificate; одного совпадающего Device ID недостаточно. Остальные slots
остаются opaque.

Offline устройство позднее получает тот же immutable event через обычный sync,
повторно проверяет embedded root-signed device list, выбирает собственный слот,
двигает свою pairwise session и создаёт собственную local projection. Event ID
и общая история у устройств совпадают; локальные encrypted projections и
ratchet sessions различаются.

Устройство, отсутствующее в embedded list, не может создать projection. Новый
device, добавленный после создания event, намеренно не получает старый slot;
M0.7.5 передаёт ему доступную историю отдельным authenticated rewrap из
[`RFC-0008`](RFC-0008-authenticated-history-rewrap.md).

## 6. Версии

Несовместимое изменение повышает границы:

- SignedEvent/Event ID — v5;
- sync envelopes и signature domains — v6;
- device session authorization — v6;
- Iroh ALPN — `kilogram/m0/sync/6`;
- connection ticket — v8.

M0.7.3 single-recipient events и tickets не мигрируются автоматически.
Ratchet account/session pickle и LocalTextProjection остаются совместимыми.

## 7. Проверенные свойства

- Root публикует один канонический device list на authority revision;
- revoked, duplicate, omitted и mismatched device/bundle отклоняются;
- prekey directory покрывает список сертификатов точно один-к-одному;
- event с неполным recipient set не подписывается;
- два recipient devices расшифровывают разные ciphertext одного event;
- outsider без slot не может создать local projection;
- process smoke: Alice доставляет Bob-1 один event с двумя slots, Bob-2 затем
  получает event и acknowledgement через sync;
- Alice хранит две pairwise sessions, Bob-1 и Bob-2 — по одной;
- histories всех трёх devices совпадают, plaintext marker отсутствует в
  event/projection/account/session files.

## 8. Ограничения

- public bundle distribution пока ручная; после расходования OTK новое bundle
  другого устройства нужно снова получить и передать listener;
- replay старого корректно подписанного prekey в первую очередь создаёт риск
  недоступности; max-seen prekey sequence и gossip freshness ещё не реализованы;
- один OTK и одна session на pair Device IDs не решают concurrent initiation;
- fan-out линейно увеличивает event, поэтому M0 ограничивает account 32 devices;
- файловые изменения нескольких ratchet sessions, projection и event не
  объединены общей транзакцией;
- добавление нового устройства не даёт ему старую историю;
- компрометация Account Root позволяет подписать новый malicious device list;
  recovery/quorum/hardware-root policy пока отсутствует.

## 9. Следующий этап

M0.7.5 реализовал authenticated history rewrap от живого устройства к новому
авторизованному device с явным диапазоном, provenance и признаком неполного
source inventory. Актуальный контракт описан в
[`RFC-0008`](RFC-0008-authenticated-history-rewrap.md). Он не возвращает static
sender boxes в replicated events и не обещает восстановление после потери всех
живых projections.
