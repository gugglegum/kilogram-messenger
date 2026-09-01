# RFC-0022: repository write receipts и authenticated manifest index

Статус: реализованный M0.8.10 spike.

## 1. Проблема

M0.8.9 передавал direct vault transaction точный append-only write-set, но CLI
сам строил пути `.event`, `.authorization` и `.local-text`. Такое дублирование
позволяет repository и coordinator незаметно разойтись после изменения layout.

Кроме того, `commit_primary_transaction` перед применением малого journal delta
расшифровывал все active DB records, собирал полный `BTreeMap` и заново хешировал
все payload. DB write amplification уже была пропорциональна delta, но commit
read/crypto amplification оставалась `O(state)`.

## 2. Repository-owned receipts

`kilogram-store` возвращает `AppendOnlyWriteReceipt` с точными абсолютными
путями, которые writer создал или byte-exact проверил:

- `LocalMessageStore::put_with_receipt` — один `.local-text`;
- `EventStore::put_authorized_with_receipt` — `.event` и обязательный
  `.authorization` sidecar;
- `EventStore::put_authorized_batch_with_receipt` — объединённый ordered
  receipt всего batch.

CLI history writers возвращают тот же receipt для `.rewrap`, `.transfer` и
`.checkpoint`. `StateTransaction::register_append_only_receipt_path`
канонизирует существующий файл, требует принадлежность exact transaction root,
повторно классифицирует kind и применяет прежний symlink/path allowlist.

Receipt описывает и `Inserted`, и idempotent `AlreadyPresent`: direct delta сам
решает, изменился ли authenticated record. API больше не требует, чтобы CLI
знал repository extension или conversation-directory layout.

## 3. Vault schema v2

Schema v2 сохраняет в `vault-meta-v1` новый AEAD-encrypted metadata record
`snapshot-manifest-index-v1`. После расшифрования он содержит отсортированные
entries:

```text
version
entries[]:
  canonical_relative_path
  plaintext_length
  BLAKE3(plaintext_payload)
```

Path и hash не появляются в DB plaintext. Index шифруется отдельным
XChaCha20-Poly1305 key domain и отдельным AAD domain. Snapshot ID вычисляется
keyed BLAKE3 от version и canonical ordered entries. Manifest, index, changed
record ciphertexts, generation и оба recovery intents публикуются одной
`redb` transaction с immediate durability.

Record envelope/AAD остаётся v1-совместимым, поэтому schema upgrade не требует
перешифровать неизменённые payload records.

## 4. Incremental direct commit

В срезе M0.8.10 при schema v2 normal typed commit:

1. проверяет encrypted index и его exact соответствие outer manifest;
2. применяет bounded trust compatibility ingress и journal mutations к index;
3. для append-only existing path сравнивает length/hash и запрещает изменение;
4. шифрует только реально новые или изменённые payload records;
5. атомарно публикует новый index, manifest, generation и recovery markers.

Неизменённые DB payload envelopes не перечисляются и не расшифровываются.
`VaultMirrorCommit` сообщает:

```text
vault_manifest_index_mode=incremental|rebuilt
vault_payload_records_loaded=N
vault_repository_write_receipt_path_count=N
```

Для обычного schema-v2 direct commit `vault_payload_records_loaded=0`.
Index как один metadata blob пока декодируется и кодируется целиком, поэтому
metadata CPU/memory остаются `O(record count)`, хотя payload I/O/AEAD стали
`O(changed + removed)`.

## 5. Fail-closed и crash contract

Оптимизированный pre-commit намеренно не обнаруживает повреждение unrelated
record ciphertext. Без чтения payload это невозможно. Безопасность сохраняют
следующие границы:

- index и manifest аутентифицированы master key;
- changed records и metadata публикуются одной DB transaction;
- network-visible result разрешён только после filesystem journal commit;
- обязательная `confirm_primary_shadow` полностью расшифровывает DB records,
  сверяет их с index и exact retained shadow;
- ошибка confirmation оставляет authenticated primary-shadow intent и требует
  fail-closed recovery, а не сообщает command success.

Initial crash-journal baseline и pre-command outer shadow gate тоже пока
остаются full-state. RFC не утверждает, что весь command path уже `O(changed)`.

## 6. Migration

Schema-v1 manifest и record envelopes продолжают проверяться прежним snapshot
алгоритмом. Первый primary commit или explicit migration:

1. полностью проверяет v1 records;
2. строит encrypted index;
3. повышает generation и атомарно публикует schema v2;
4. сохраняет неизменённые record ciphertexts при delta-upgrade.

Отсутствующий или повреждённый index при schema v2 отклоняется. Silent rebuild
из потенциально повреждённых records не выполняется.

## 7. Проверки

Automated tests проверяют exact event/authorization/projection receipts,
receipt другого state root, schema-v1 one-time rebuild, AEAD tamper index,
нулевое число загруженных DB payload records для schema-v2 typed commit,
immutable modification rejection и прежние injected-failure/recovery cases.

## 8. Ограничения и следующий этап

Trust writers всё ещё используют filesystem compatibility ingress, а ratchet —
гидратированный staging adapter. Следующий storage slice должен дать trust
отдельный DB-primary read/write repository и убрать authority mutations вне
`StateTransaction`. После этого можно проектировать protected key provider,
rollback witness, bounded backup/restore и versioned production migrations.

Этот следующий срез реализован в
[`RFC-0023`](RFC-0023-db-primary-trust-repository.md): M0.8.11 удаляет bounded
trust ingress и переводит production trust reads/writes на DB-primary boundary.
