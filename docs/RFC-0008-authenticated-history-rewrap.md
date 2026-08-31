# RFC-0008: authenticated history rewrap

- Статус: **Implemented spike (M0.7.5)**
- Дата: 2026-09-01
- Область: перенос доступной локальной истории на новое устройство того же
  аккаунта без изменения старых events

## 1. Цель этапа

M0.7.4 шифрует новый text event отдельно каждому устройству, которое входило в
root-signed recipient device list в момент отправки. Устройство, добавленное
позже, закономерно отсутствует в старых ciphertext slots. Ни seed, ни текущий
device signing key не должны давать универсальный ключ расшифрования старого
ratchet history: это отменило бы forward secrecy.

M0.7.5 вводит отдельный authenticated history-rewrap flow. Живое устройство,
у которого сохранились читаемые local projections, явно выбирает диапазон
истории, шифрует plaintext на persistent encryption key нового устройства и
подписывает происхождение своим application Device key. Старые `SignedEvent`,
Event ID, causal parents и сетевые ratchet ciphertexts не изменяются.

## 2. Граница доверия

Текущий срез разрешает rewrap только между двумя различными устройствами одного
Account ID. Оба certificate обязаны присутствовать в одном свежем для сторон
`AccountDeviceListSnapshot`. Root-signed список проверяет account, authority
revision, revocations и messaging capabilities; source Device signature
доказывает, какое авторизованное устройство заявило и передало историю.

Получатель по-прежнему использует установленный owner-signed conversation
membership как локальный trust anchor. Каждый перенесённый `AuthorizedEvent`
повторно проходит membership → author account → author device → event проверку.
Один лишь rewrap bundle не добавляет новый account в разговор.

Peer-assisted recovery от устройства собеседника, cross-account policy и
пользовательское подтверждение fingerprint отложены. Они не должны появиться
как неявное расширение same-account доверия.

## 3. Canonical source inventory и диапазон

Source загружает всю локальную авторизованную историю разговора, оставляет
только `RatchetText` events и сортирует их по Event ID. Для каждого события
обязана существовать читаемая local projection. Manifest содержит:

- conversation ID;
- root-signed account device list;
- source и recipient Device ID;
- число всех text events в source inventory;
- domain-separated BLAKE3 digest conversation ID и полного списка Event ID;
- полуинтервал `[range_start, range_end)` в этом canonical inventory.

Один bundle содержит от 1 до 256 последовательных events. Значение
`source_inventory_complete=true` означает только то, что bundle покрывает весь
inventory, заявленный и подписанный source (`0..count`). Это не доказательство,
что source видел глобально всю существующую историю: злонамеренное или
отставшее устройство может иметь неполную реплику. Provenance, digest и range
делают эту границу явной, но discovery/gossip/checkpoints ещё нужны.

## 4. Шифрование и подпись

Каждая `HistoryRewrapEntry` содержит исходный `AuthorizedEvent`, canonical
inventory index и HPKE Base mode ciphertext. Получатель определяется отдельным
X25519 encryption public key из его root-signed `DeviceCertificate`.

HPKE AAD связывает ciphertext с полным manifest, index и Event ID. Source
Device Ed25519 signature покрывает manifest, event, index и sealed message.
Подмена recipient, диапазона, source inventory digest, event metadata или
ciphertext поэтому обнаруживается до записи history.

Bundle и ciphertext не содержат ratchet session keys. Компрометация нового
устройства раскрывает только переданный ему plaintext и его будущую локальную
историю, но не создаёт универсального способа открыть общий replicated log.

## 5. Импорт и local projection v2

`history-rewrap-import` выполняет все проверки до мутаций, затем:

1. устанавливает embedded authority snapshot с обычной защитой от rollback и
   equivocation;
2. проверяет target certificate и локальный conversation membership;
3. открывает каждую entry локальным encryption key;
4. создаёт immutable local projection v2 с сохранёнными signed provenance,
   manifest и source ciphertext;
5. сохраняет соответствующий неизменённый `AuthorizedEvent` и authorization
   sidecar;
6. сохраняет исходный bundle под
   `STATE_DIR/history-rewraps/<rewrap-id>.rewrap`.

Старые direct projections версии 1 продолжают декодироваться без миграции.
Rewrapped projection открывается только при совпадении установленного local
Account ID, Device ID, event и source signature. Если projection уже существует
и раскрывается в тот же plaintext, повторный или перекрывающийся import
идемпотентен; другой plaintext отклоняется.

После импорта обычный sync может дополнить acknowledgements и другие events.
Если старый text event повторно приходит по sync, существующая rewrapped
projection проходит ту же проверку и event сохраняется без ratchet decrypt,
который для нового устройства невозможен.

## 6. CLI flow M0

Source экспортирует диапазон:

```powershell
kilogram-cli history-rewrap-export `
  --state-dir .\state-bob-1 `
  --conversation chat `
  --device-list-file .\bob-devices.snapshot `
  --recipient-device <BOB_2_DEVICE_ID> `
  --range-start 0 `
  --count 256 `
  --bundle-file .\bob-2-history.rewrap
```

Новое устройство сначала устанавливает тот же conversation membership, затем
импортирует файл:

```powershell
kilogram-cli history-rewrap-import `
  --state-dir .\state-bob-2 `
  --conversation chat `
  --bundle-file .\bob-2-history.rewrap
```

File exchange является M0 transport boundary. Bundle уже E2EE и подписан, но
production UI должен передавать его по авторизованной device-to-device session,
показывать source, диапазон и признак неполноты до согласия пользователя.

## 7. Проверенные свойства

- пустой, обратный, выходящий за inventory или превышающий 256 диапазон
  отклоняется;
- tampering bundle/signature/ciphertext и чужой recipient key отклоняются;
- local projection v1 остаётся читаемой, v2 сохраняет и повторно проверяет
  provenance;
- partial `1..2` явно помечается как неполный source inventory;
- полный `0..3` восстанавливает новому устройству три старых text events;
- перекрывающийся partial/full import идемпотентен;
- устройство другого account не может импортировать bundle;
- удалённый после import event повторно принимается обычным sync через уже
  существующую rewrapped projection;
- source и recovered history совпадают, plaintext marker отсутствует в event,
  projection и rewrap files.

## 8. Ограничения

- source может передать только projections, которые сам способен открыть;
- `source_inventory_complete` является подписанным утверждением source, а не
  глобальным consensus checkpoint;
- M0.7.8 добавил session-bound network transport, явный user consent, SAS и
  multi-source claim reconciliation в [`RFC-0011`](RFC-0011-network-history-rewrap.md);
- нет cross-account recovery, автоматического source discovery и политики
  выбора более доверенного source;
- bundle и projections раскрывают историю при компрометации target device;
- bundle persistence, projections и event store ещё не объединены одной
  транзакцией; операции идемпотентны, но crash может оставить безопасный orphan;
- local encryption/root/ratchet secrets всё ещё не защищены OS keystore;
- member removal и MLS epochs не реализованы.

## 9. Следующий этап

M0.7.6 реализовал authenticated prekey pools, sequence/freshness high-water и
детерминированное разрешение crossed pairwise initiation. Актуальный контракт
описан в [`RFC-0009`](RFC-0009-authenticated-prekey-pools.md). Следующим срезом
оставалась общая crash-consistent транзакция ratchet/projection/event/prekey.
Она реализована M0.7.7 в [`RFC-0010`](RFC-0010-crash-consistent-local-state.md),
а сетевой rewrap с consent и reconciliation — M0.7.8 в
[`RFC-0011`](RFC-0011-network-history-rewrap.md).
