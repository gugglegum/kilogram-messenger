# RFC-0026: DB-primary device identity

Статус: реализованный M0.8.14 spike.

## 1. Цель среза

До M0.8.14 vault уже был источником истины для immutable history, mutable
sequence/ratchet и trust state, но каждая прикладная команда продолжала
создавать `DeviceState` прямым чтением двух чувствительных файлов:

- `device-secret.key` — Ed25519 signing secret устройства;
- `device-encryption-secret.key` — X25519 encryption secret устройства.

Это оставляло filesystem compatibility shadow фактическим источником
криптографической identity. Подмена shadow могла изменить ключ, используемый
командой, ещё до проверки certificate/trust state.

M0.8.14 вводит отдельный immutable `DeviceIdentityStateRepository` и переводит
все production-команды на authenticated DB-primary secret material после
инициализации vault. Filesystem остаётся только временной compatibility-копией
для crash/dual-write gate и больше не является входом прикладной identity.

## 2. Выбор источника

CLI использует единый `load_command_device_state`:

1. если vault ещё не инициализирован, `DeviceState::load_or_create` читает или
   создаёт два legacy-файла;
2. если vault инициализирован, открывается только existing vault;
3. repository аутентифицирует manifest/index/generation и выбирает записи
   `StateRecordKind::DeviceIdentity`;
4. должны существовать ровно два ожидаемых пути и каждый plaintext обязан
   иметь ровно 32 bytes;
5. `DeviceState::from_secret_material` строит signing/encryption identities из
   полученных bytes без чтения и создания shadow-файлов.

Для initialized vault отсутствует silent filesystem fallback. Missing record,
неверная длина, неожиданный identity path, повреждение index/ciphertext,
несовпадение manifest или generation завершают команду fail-closed.

## 3. Аутентифицированный selected read

Для schema v2 repository:

- проверяет AEAD-protected manifest index целиком;
- сверяет manifest commitment и authenticated generation;
- загружает и расшифровывает только записи kind `DeviceIdentity`;
- повторно проверяет path, plaintext length и hash выбранных records;
- проверяет согласованность активного mirror intent с прочитанным snapshot.

Schema v1 сохраняет совместимость через полную проверку snapshot и последующую
фильтрацию identity records. Повреждённая schema v2 автоматически не
перестраивается и не принимается из filesystem.

## 4. Работа с секретами

`VaultDeviceIdentityRead` содержит два фиксированных 32-byte массива и
zeroize-ится при уничтожении. Временные decrypted `Vec<u8>` и промежуточные
массивы repository обёрнуты в zeroizing containers. Конструктор
`DeviceState::from_secret_material` также zeroize-ит переданные копии после
создания криптографических identity objects.

Secret bytes, их hashes и ciphertext не выводятся. CLI публикует только:

```text
vault_device_identity_read_source=db-primary
vault_device_identity_read_generation=<N>
vault_device_identity_record_count=2
```

До инициализации vault диагностируется
`device_identity_read_source=filesystem`.

## 5. Неизменяемость identity

Device identity не добавляется в разрешённый direct mutation set. После
миграции обычная команда может читать ключи из DB, но не может незаметно
заменить их через filesystem или typed transaction. Замена/ротация device key
должна быть отдельной authority-операцией с новым device certificate, а не
обновлением существующей записи.

Account Root identity этим RFC не затрагивается: её lifecycle и seed/recovery
authority остаются отдельной задачей.

## 6. Compatibility shadow и граница безопасности

M0.8.14 меняет источник чтения, но намеренно не удаляет два raw shadow-файла.
Текущий pre-command dual-write guard всё ещё требует exact соответствия всего
retained filesystem snapshot authenticated vault и тем самым обнаруживает
внешнюю подмену до запуска команды. Crash recovery также пока умеет
восстанавливать эти файлы из DB.

Следствия:

- прикладная identity больше не зависит от plaintext filesystem read;
- подменённый shadow не становится ключом команды;
- но at-rest confidentiality device secrets пока не улучшилась, потому что raw
  compatibility copies физически остаются на диске;
- process с доступом к памяти клиента или DPAPI текущего Windows user остаётся
  вне защищаемой границы;
- secure erase старых файлов на SSD/journaled filesystem гарантировать нельзя.

Следующий отдельный этап должен удалить identity secrets из normal retained
shadow, адаптировать exact gate/recovery к DB-only records и выполнить
production shadow retirement без смешивания с key rotation.

## 7. Проверки

Автоматические regressions доказывают:

- создание `DeviceState` из caller-authenticated bytes не читает, не создаёт и
  не изменяет shadow-файлы;
- repository возвращает DB bytes при подменённых filesystem copies;
- отсутствие одного из двух records отклоняется;
- AEAD-порча выбранного identity ciphertext отклоняется;
- CLI helper при initialized vault сохраняет исходные device ID и encryption
  public key, даже если оба shadow-файла подменены после открытия mirror intent.

Windows release process smoke должен дополнительно запустить реальную команду
`identity` на копии schema-v2 vault и подтвердить DB-primary diagnostics,
неизменные device IDs и неизменный authenticated vault report.
