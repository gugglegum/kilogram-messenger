# RFC-0003: подписанное состояние участников разговора

- Статус: **Implemented draft (M0.6.2)**
- Дата: 2026-08-31
- Область: минимальная авторизация авторов событий и history sync

## 1. Задача

Account Root и device certificate отвечают на вопрос «какое устройство вправе
действовать от имени аккаунта», но сами по себе не дают аккаунту доступ к
конкретному разговору. До M0.6.2 любой авторизованный account/device мог
создать корректно подписанное событие, а sync не имел проверяемого списка
аккаунтов, чьи события разрешено принимать.

M0.6.2 вводит минимальное состояние участников и проверяет цепочку:

```text
локально доверенный ConversationMembershipSnapshot
    -> AccountId автора входит в members
    -> AccountAuthoritySnapshot принадлежит этому AccountId
    -> DeviceCertificate разрешён snapshot и имеет sign-events
    -> DeviceId сертификата совпадает с автором SignedEvent
    -> device signature события валидна
```

Это authorization slice. Шифрование payload, MLS epochs, discovery и
production-группы в него не входят.

## 2. Модель владельца M0

У разговора есть один `owner_account_id`. Его Account Root создаёт и подписывает
полный `ConversationMembershipSnapshot` со следующими полями:

- версия формата;
- `conversation_id`;
- монотонная `revision`, начиная с 1;
- `owner_account_id`;
- canonical отсортированный список уникальных `AccountId` участников;
- Ed25519-подпись owner Account Root с отдельным domain separator.

Owner всегда автоматически входит в список. Список в M0.6.2 **только
расширяется**: новая revision обязана быть строгим или равным superset прежней.
Удаление участника намеренно не эмулируется, потому что безопасное удаление
должно быть связано с ordered security event и новой криптографической эпохой.

## 3. Локальная установка и anti-rollback

Membership распространяется пока вне протокола — файлом от владельца разговора.
Устройство устанавливает snapshot только если его собственный сертифицированный
Account ID присутствует в `members`.

Для каждого `conversation_id` хранится один max-seen snapshot:

- повтор тех же bytes и revision идемпотентен;
- меньшая revision отклоняется как rollback;
- другое подписанное состояние той же revision отклоняется как equivocation;
- новая revision от другого owner отклоняется;
- новая revision, удаляющая прежнего участника, отклоняется как нарушение
  add-only контракта.

Файл membership является локальным trust anchor разговора. Peer не может
прислать самоподписанный список и тем самым добавить себя.

## 4. Авторизованное событие

Wire/storage envelope `AuthorizedEvent` содержит:

- исходный `SignedEvent`;
- root-signed `DeviceCertificate` автора;
- полный root-signed `AccountAuthoritySnapshot` автора.

Conversation membership не дублируется в каждом событии. Получатель использует
свой установленный snapshot и требует совпадение `conversation_id` и членство
`AccountId` автора. Для direct delivery и acknowledgement дополнительно
проверяется точное совпадение account/device с уже авторизованной transport
session. Для sync то же правило применяется к каждому событию в обе стороны,
включая события третьих участников.

Event ID по-прежнему вычисляется только от неизменяемого `SignedEvent`.
Authorization envelope сохраняется рядом отдельным immutable
`EVENT_ID.authorization` sidecar. История считается пригодной для M0.6.2 лишь
если для каждого `EVENT_ID.event` присутствует совпадающий sidecar и вся цепочка
проверяется относительно локального membership.

## 5. Wire compatibility

`SyncEventBatch`, `SyncDiff`, delivery и acknowledgement теперь передают
`AuthorizedEvent`, а не голый `SignedEvent`. Версия sync envelope и домены
подписей повышены до v2; Iroh ALPN повышен до `kilogram/m0/sync/2`.
Структура ticket v4 не изменилась.

Старое M0.6.1 event state не имеет authorization sidecars и поэтому fail-closed
отклоняется командами delivery, `history`, `sync` и `seed-history`. Автоматической
миграции нет: старый event не содержит достаточного доказательства Account ID
автора. Для тестирования нужен новый state или явный будущий migration protocol.

## 6. CLI lifecycle

Owner создаёт snapshot и включает Account ID собеседника:

```powershell
kilogram-cli conversation-create `
  --account-dir .\alice-account `
  --conversation friends `
  --member-account <BOB_ACCOUNT_ID> `
  --membership-file .\friends-v1.membership
```

Каждый участник устанавливает один и тот же публичный файл:

```powershell
kilogram-cli conversation-membership-install `
  --state-dir .\alice-state `
  --membership-file .\friends-v1.membership
kilogram-cli conversation-membership-install `
  --state-dir .\bob-state `
  --membership-file .\friends-v1.membership
```

Добавление участника создаёт новую revision и новый export-файл:

```powershell
kilogram-cli conversation-member-add `
  --account-dir .\alice-account `
  --conversation friends `
  --member-account <CAROL_ACCOUNT_ID> `
  --membership-file .\friends-v2.membership
```

Новый snapshot требуется установить на устройствах участников до обмена с
добавленным аккаунтом.

## 7. Проверенные свойства M0.6.2

- root signature, owner, conversation ID и canonical members проверяются;
- membership переживает restart и не допускает rollback/equivocation/removal;
- событие аккаунта вне `members` отклоняется protocol и sync слоями;
- event store требует неизменяемый совпадающий authorization sidecar;
- финальный release Iroh smoke между двумя отдельными Account IDs создал общий
  membership, доставил Text/Acknowledgement, затем одним sync round передал 3
  дополнительных events и получил две одинаковые авторизованные истории из 5
  events;
- transport-independent multi-round и reconnect sync продолжают сходиться с
  новым envelope.

## 8. Явные ограничения

- Один owner и add-only список — временная M0-модель, не governance групп.
- Удаление, ban, передача owner/admin прав и конфликтующие membership changes
  требуют ordered security log и связи с MLS epoch.
- Membership-файл раскрывает список Account IDs тому, кто его получил.
- Нет автоматической доставки/поиска свежей membership revision.
- Authority snapshot в событии доказывает authorization на своей revision, но
  не доказывает реальное время создания события. Без authenticated
  gossip/witness, expiry или epoch-bound authorizations украденный отозванный
  ключ и старый корректный snapshot остаются проблемой при first contact и при
  импорте «исторических» событий.
- В границе M0.6.2 payload и local store были plaintext. M0.7.1 заменил этот
  формат pairwise HPKE ciphertext, а M0.7.2 вынес читаемую копию в local-only
  encrypted projection; см. [`RFC-0004`](RFC-0004-pairwise-hpke-payload.md) и
  [`RFC-0005`](RFC-0005-local-encrypted-history-projection.md).

Pairwise HPKE spike теперь определён отдельно в RFC-0004, но ratchet и ключевые
эпохи остаются следующей криптографической задачей. Модель удаления участников
должна проектироваться вместе с MLS, а не расширением add-only snapshot задним
числом.
