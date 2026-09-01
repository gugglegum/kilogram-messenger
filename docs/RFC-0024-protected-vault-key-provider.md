# RFC-0024: Protected vault key provider

Статус: реализованный M0.8.12 spike.

## 1. Цель среза

M0.8.1 хранил случайный 256-bit vault master key как открытые 32 bytes в
`STATE_DIR/state-vault.key`. Шифрование `state-vault.redb` поэтому защищало
только от отдельной утечки DB, но не от копии всего каталога.

M0.8.12 вводит versioned key-provider boundary:

- новый master key не записывается на диск открытым;
- Windows использует DPAPI в CurrentUser scope;
- прежний 32-byte development key мигрируется без re-encryption vault;
- формат, provider и результат загрузки наблюдаемы, но secret не выводится;
- неизвестный или повреждённый envelope завершается fail-closed до открытия DB.

## 2. Формат `state-vault.key`

Файл остаётся рядом с DB, но теперь является несекретным provider envelope:

```text
16-byte magic "KILOGRAM-VAULTK1"
postcard VaultKeyEnvelope {
  version: 1,
  provider: WindowsDpapiCurrentUser | PlaintextDevelopment,
  protected_key: bytes,
}
```

Максимальный размер envelope — 64 KiB. Magic отделяет новый формат от ровно
32-byte legacy key. Version и provider проверяются до unprotect; decoded key
обязан иметь ровно 32 bytes. Key, legacy read buffer и DPAPI plaintext copy
zeroize-ятся после использования.

## 3. Windows provider

Windows build вызывает `CryptProtectData` / `CryptUnprotectData` через
закреплённый safe wrapper `stellar-agent-windows-identity
0.1.0-alpha.6`. Используется CurrentUser scope и
`CRYPTPROTECT_UI_FORBIDDEN`, поэтому CLI не открывает скрытый UI prompt.
Wrapper очищает выделенный DPAPI plaintext buffer перед `LocalFree`; Kilogram
дополнительно zeroize-ит полученный Rust buffer и долгоживущий master-key type.

DPAPI связывает ciphertext с Windows security context. Копия каталога под
другим пользователем или на другой машине не должна автоматически открываться.
Повреждение blob возвращает provider error; fallback к raw filesystem key или
создание нового ключа при существующей DB запрещены.

## 4. Legacy migration

При чтении ровно 32-byte `state-vault.key`:

1. bytes копируются в zeroizing master-key value;
2. provider создаёт защищённый envelope того же ключа;
3. envelope пишется во временный файл в том же каталоге и `fsync`-ится;
4. temporary file атомарно заменяет legacy key file;
5. каталог синхронизируется;
6. только после этого открывается или создаётся `state-vault.redb`.

Crash до replace оставляет прежний raw key и migration повторяется. После
replace остаётся полный envelope. Ошибка protect/write/replace не разрешает
продолжить с частичным файлом. Vault records не перешифровываются, поскольку
master key не меняется.

Операционная система и SSD могут сохранять старые blocks/snapshots; atomic
replace не является обещанием secure erase прежнего raw файла.

## 5. Non-Windows compatibility

Пока macOS Keychain, Linux Secret Service и интерактивный passphrase provider
не реализованы, non-Windows build использует тот же versioned envelope с явно
помеченным `PlaintextDevelopment`. Это не считается production at-rest
защитой. Если такой envelope открывается Windows build, он автоматически
перепаковывается в DPAPI.

Windows DPAPI envelope на другой платформе отклоняется как unavailable
provider; silent downgrade запрещён.

## 6. Диагностика

Vault migration/verification и начало live dual-write публикуют:

```text
vault_key_file_format=protected-envelope-v1
vault_key_protection=windows-dpapi-current-user|plaintext-development
vault_key_load=created|legacy-migrated|already-current
```

Key bytes, DPAPI blob и derived vault keys никогда не выводятся.

## 7. Модель угроз

Срез защищает raw master key при отдельной offline-копии state directory и
обычном чтении файлов другим Windows account/machine. Он не защищает от:

- процесса, уже выполняющегося с полномочиями того же Windows user;
- administrator/kernel compromise, memory scraping или crash dump;
- удаления Windows profile, переустановки ОС или потери DPAPI material;
- rollback согласованной старой пары DB + envelope;
- утечки других пока filesystem-backed device/root/ratchet secrets.

Поэтому это локальная at-rest boundary, а не замена endpoint security,
аппаратного фактора или независимого rollback witness.

## 8. Проверки

Automated regression покрывает fresh protected envelope, reopen, миграцию
existing raw key без смены DB key, invalid magic, tampered provider blob и
wrong key. Полный workspace test и Windows release process smoke должны также
подтвердить миграцию реального M0.8.11 vault и отсутствие 32-byte raw key после
успешного verify.

## 9. Следующие этапы

Отдельно нужны:

1. portable macOS/Linux provider и опциональный passphrase/hardware factor;
2. явный encrypted export/import для device-specific key recovery;
3. внешний monotonic rollback witness для DB + key-envelope generation;
4. bounded backup/restore и rotation без общего decrypt key всех устройств;
5. защита account-root, device-encryption и ratchet pickle keys тем же
   lifecycle, где это не ломает forward secrecy.

Справочные источники: Microsoft рекомендует Windows Credential Manager или
DPAPI для локального хранения секретов и отдельно предупреждает минимизировать
время plaintext в памяти:
[Handling Passwords](https://learn.microsoft.com/windows/win32/secbp/handling-passwords)
и
[CryptProtectData](https://learn.microsoft.com/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata).
