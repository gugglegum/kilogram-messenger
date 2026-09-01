# RFC-0023: DB-primary trust repository

Статус: реализованный M0.8.11 spike.

## 1. Проблема

После M0.8.10 direct vault commit применял ratchet, sequence и append-only
изменения как explicit journal delta, но на каждом commit отдельно перечислял
retained trust namespaces:

- `device-certificate.cert`;
- `account-authority.snapshot`;
- `peer-authority/*.snapshot`;
- `conversation-memberships/*.membership`.

Это был bounded compatibility ingress: filesystem мог неявно добавить trust
mutation в DB transaction, хотя command не объявлял trust write. Production
reads тех же объектов также шли через `DeviceState` и retained shadow.

## 2. Repository boundary

`TrustStateRepository::read_primary_trust` возвращает owned authenticated
snapshot только записей `StateRecordKind::Trust`. При vault schema v2 он:

1. аутентифицирует encrypted manifest index и его соответствие manifest;
2. выбирает trust paths из index;
3. делает keyed lookup и AEAD-decrypt только выбранных payload records;
4. повторно сверяет canonical path, length и content hash с index;
5. связывает результат с authenticated mirror generation и active intent.

Production CLI декодирует snapshot в domain objects и заново применяет
проверки signature, Account ID, Device ID, encryption key, revision и
Conversation ID. При наличии vault отсутствие требуемой записи является
ошибкой; silent filesystem fallback запрещён. Немигрированное состояние
сохраняет legacy-compatible filesystem path.

## 3. Trust write workspace

Trust mutation выполняется только внутри `StateTransaction`:

1. transaction получает DB-primary trust snapshot;
2. `prepare_trust_workspace` проверяет exact state root и kind каждого record;
3. DB baseline сохраняется в durable primary backup crash journal;
4. retained trust namespace заменяется byte-exact DB baseline;
5. прежние domain validators выполняют install/update/anti-rollback;
6. transaction строит typed `Trust` mutations относительно DB baseline;
7. vault commit публикует payload delta и manifest index до filesystem commit;
8. rollback и next-start recovery возвращают DB-authoritative trust baseline.

Для ещё не мигрированного состояния тот же API явно подготавливает legacy
baseline. Это делает certificate, own/peer authority и membership writes
crash-consistent уже до создания vault.

## 4. Удаление compatibility ingress

`commit_primary_transaction` больше не перечисляет trust directories и не
сравнивает их с active vault. В direct delta попадают только trust mutations
явно подготовленного workspace. Незарегистрированная подмена retained trust
shadow не меняет DB-primary state. Compatibility checkpoint и final
`finish_dual_write` явно отклоняют незарегистрированный `Trust` delta, поэтому
он не может пройти через старый full-snapshot path как authority update;
обязательная exact shadow граница остаётся дополнительной проверкой.

Разрешённые direct kinds теперь включают `Trust`. Append-only immutability
остаётся отдельным правилом и на trust records не распространяется: правила
rollback, equivocation и add-only membership применяет identity domain layer.

## 5. Command cutover

DB-primary reads используются в `listen`, `connect`, `sync`, delivery/history,
seed, prekey publication и history-rewrap/recovery paths. Все production writes
certificate, own authority, peer authority и membership проходят через
transactional trust workspace. Sync projection code получает уже проверенный
local Account ID и больше не перечитывает certificate из filesystem.

Диагностика публикует:

```text
vault_trust_read_source=db-primary
vault_trust_read_generation=N
vault_trust_read_record_count=N
vault_mutable_read_kind=trust
vault_trust_workspace=prepared
vault_trust_workspace_committed=true|false
```

## 6. Crash и fail-closed contract

- wrong root, wrong selected kind, duplicate или non-trust path отклоняются;
- trust workspace всегда начинает с authenticated DB baseline;
- operation error и interrupted prepared journal восстанавливают этот baseline;
- DB commit остаётся irreversible point перед публикацией filesystem shadow;
- final `confirm_primary_shadow` полностью сверяет DB/index/shadow;
- schema-v2 trust read не обязан decrypt unrelated payload records.

Последнее означает, что повреждение unrelated ciphertext обнаруживается общей
verification/confirmation границей, но не превращает выбранный trust record в
неаутентифицированный: его собственный AEAD и index entry проверяются всегда.

## 7. Проверки

Automated tests покрывают DB-primary hydration поверх tampered trust shadow,
rollback и next-start recovery трёх mutable workspaces, explicit trust delta,
игнорирование его direct ingress, отказ compatibility/final full-snapshot
границ принимать незарегистрированный trust delta и CLI repair через domain
validator. Полный process smoke дополнительно должен подтвердить
authority pin, membership read, delivery/ack и одинаковую историю двух fresh
accounts.

## 8. Ограничения и следующий этап

Retained filesystem остаётся compatibility shadow. Pre-command exact gate,
initial append baseline и post-commit confirmation ещё выполняют full-state
проверку. Encrypted manifest index остаётся единым `O(record count)` metadata
blob. Vault master key хранится рядом с DB.

Следующий storage/security slice: protected key provider через OS keystore или
passphrase/seed wrapping, внешний rollback witness, bounded backup/restore и
формализованные versioned production migrations. Paged/Merkle index можно
проектировать отдельно, не смешивая его с key custody.
