# RFC-0025: Portable vault-key recovery and rollback witness

Статус: реализованный M0.8.13 spike.

## 1. Цель среза

M0.8.12 привязал локальный `state-vault.key` к Windows user/machine через
DPAPI CurrentUser. Это защищает master key at rest, но потеря Windows profile
или перенос vault на другой компьютер делали `state-vault.redb` недоступным.

M0.8.13 добавляет явный переносимый recovery lifecycle:

- master key экспортируется только в отдельный passphrase-зашифрованный файл;
- recovery-файл содержит внешний witness поколения и snapshot ID;
- импорт сначала проверяет package AEAD и всю vault DB candidate key;
- rollback и same-generation fork отклоняются до изменения локального key file;
- прошедший проверку key заново заворачивается локальным provider, то есть на
  Windows снова становится DPAPI CurrentUser envelope.

Recovery package не является серверным backup, не попадает в `state-dir` и не
создаётся автоматически.

## 2. Команды

```text
kilogram-cli state-vault-key-export \
  --state-dir <DIR> \
  --output-file <NEW_EXTERNAL_FILE> \
  --passphrase-file <FILE>

kilogram-cli state-vault-key-import \
  --state-dir <DIR> \
  --recovery-file <EXTERNAL_FILE> \
  --passphrase-file <FILE>
```

Passphrase не передаётся аргументом командной строки и не попадает в shell
history/process listing. CLI читает не-symlink regular file размером не более
4098 bytes, снимает ровно один конечный LF или CRLF и передаёт bytes в
zeroizing buffer. Библиотечная граница принимает от 16 до 4096 bytes.
Минимум 16 bytes — только fail-fast floor, а не оценка стойкости человеческого
пароля; для реального backup нужен длинный случайный пароль или достаточно
длинная случайная фраза из password manager.

## 3. Формат recovery package

```text
16-byte magic "KILOGRAM-VRECOV1"
postcard RecoveryEnvelope {
  header: {
    version: 1,
    algorithm: Argon2id,
    argon2_version: 0x13,
    memory_kib: 65536,
    iterations: 3,
    parallelism: 1,
    salt: [u8; 16],
    nonce: [u8; 24],
  },
  ciphertext: XChaCha20-Poly1305(...),
}
```

Максимальный размер всего файла — 64 KiB. Для v1 принимается только точный
набор KDF parameters; attacker-controlled memory/time values не запускаются.
Argon2id выводит 256-bit recovery key, которым XChaCha20-Poly1305 шифрует:

```text
RecoveryPlaintext {
  version: 1,
  vault_master_key: [u8; 32],
  vault_schema_version: u64,
  mirror_generation: u64,
  snapshot_id: [u8; 32],
}
```

Magic-independent AAD включает domain separator и весь header. Поэтому version,
KDF parameters, salt и nonce нельзя изменить независимо от ciphertext.
Неверный passphrase и tampered ciphertext возвращают одну authentication
ошибку. Derived key, сериализованный plaintext, decoded master key и CLI
passphrase zeroize-ятся после использования.

Реализация использует RustCrypto `argon2` 0.5.3 и его raw-output
`hash_password_into` API. Документация подтверждает поддержку Argon2id v0x13
и caller-provided output buffer:
[RustCrypto Argon2](https://docs.rs/argon2/0.5.3/argon2/)
и
[`Argon2::hash_password_into`](https://docs.rs/argon2/0.5.3/argon2/struct.Argon2.html#method.hash_password_into).

## 4. Экспорт

Перед экспортом vault обязан не иметь pending mirror intent и пройти полную
DB-only authentication. Witness берётся из этого же проверенного `VaultReport`.

Output path разрешается через существующий canonical ancestor и обязан быть
вне canonical `state-dir`. Existing output не перезаписывается. Package
сначала записывается в same-directory temporary file, синхронизируется и
публикуется через no-clobber rename; затем синхронизируется parent directory.
Это исключает частичный успешный package и случайную замену прежнего witness.

Passphrase file является отдельным чувствительным секретом. Пользователь
должен хранить или безопасно удалить его независимо; сам recovery package
без passphrase рассчитан на offline хранение, но обе части вместе дают доступ
ко всей истории, защищённой данным master key.

## 5. Импорт и порядок доверия

Импорт выполняется под exclusive state-directory lock, но не запускает обычный
dual-write guard: локальный key envelope может отсутствовать или быть
неоткрываемым именно потому, что выполняется recovery.

Порядок действий фиксирован:

1. canonical recovery path обязан быть вне `state-dir`, symlink запрещён;
2. package bounded, format и точные KDF parameters проверяются до Argon2;
3. AEAD открывает candidate master key и witness;
4. `state-vault.redb` открывается с candidate key без изменения key file;
5. полностью проверяются manifest, encrypted index, records, generation auth;
6. current DB report сравнивается с external witness;
7. текущий local provider создаёт candidate envelope и проверяет его
   protect/unprotect roundtrip ещё до записи;
8. только после всех проверок envelope атомарно устанавливается как local key.

Package другого vault, неправильный passphrase, corrupt DB, rollback, fork,
symlink или ошибка локального provider оставляют прежний `state-vault.key`
byte-exact либо, если он отсутствовал, не создают его.

## 6. Rollback witness semantics

Пусть package содержит witness `(G, S)`, а проверенная DB — `(g, s)`:

- `g < G` — rollback, импорт запрещён;
- `g == G` и `s != S` — другая ветка того же поколения, импорт запрещён;
- `g == G` и `s == S` — exact recovery разрешён;
- `g > G` — DB новее package, импорт разрешён после полной AEAD-проверки.

Schema version также сравнивается при равном поколении. Package тем самым
является переносимым внешним witness, но не глобальным monotonic service:
согласованный rollback одновременно DB и старого recovery package не
обнаруживается. Пользователь должен обновлять и независимо хранить свежий
package после значимых изменений. Аппаратный counter, прозрачный witness log
или согласование между устройствами остаются будущими этапами.

## 7. Диагностика

Экспорт и импорт печатают только путь/format/KDF profile, schema, generation и
snapshot ID. Secret, salt, nonce, ciphertext, derived key и passphrase не
выводятся. Успешный импорт дополнительно сообщает current DB generation,
snapshot ID, локальный provider и `rollback_check=passed`.

## 8. Граница безопасности

Срез решает переносимость vault master key и обнаружение части offline
rollback-сценариев. Он не решает:

- восстановление Account Root seed, device identity или иных secret lifecycle;
- защиту от процесса с доступом к passphrase и памяти клиента;
- secure erase package/passphrase на SSD или journaled filesystem;
- согласованный rollback DB вместе со старым внешним package;
- автоматическое резервирование, ротацию master key или разделение backup по
  устройствам;
- production macOS Keychain/Linux Secret Service provider.

## 9. Проверки

Автоматические regressions покрывают exact export/import, запрет output внутри
state tree, no-clobber, short/wrong passphrase, package tamper, package другого
vault, отсутствие мутации key file при ошибке, rollback поколения и fork при
одинаковом generation. Windows release process smoke должен дополнительно
удалить DPAPI envelope у копии реального vault, импортировать package и
подтвердить прежние generation/snapshot/record count новым DPAPI envelope.
