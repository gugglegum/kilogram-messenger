# RFC-0027: DB-only device identity layout

Статус: реализовано в M0.8.15 (2026-09-02).

## 1. Цель

После M0.8.14 прикладные команды уже читали device signing key и device
encryption key только из authenticated vault, но их plaintext-копии
`device-secret.key` и `device-encryption-secret.key` продолжали жить в
retained filesystem shadow. Они также участвовали в exact gate и могли быть
восстановлены из БД после crash. Это сохраняло корректность, но не улучшало
фактическую защиту этих ключей at rest.

M0.8.15 делает encrypted vault единственным normal persistent storage для
обеих device identity records. Остальное состояние пока сохраняет
compatibility shadow.

## 2. Инварианты

1. Schema/layout переключается до удаления plaintext-файлов.
2. Upgrade никогда не создаёт новую identity и не использует filesystem
   fallback.
3. Отсутствующий raw identity file при upgrade допустим, если соответствующая
   authenticated DB record существует.
4. Существующий raw identity file удаляется только после byte-exact сравнения
   с DB record. Несовпадение завершает операцию fail-closed.
5. После cutover exact shadow gate сравнивает filesystem только для
   shadow-managed kinds; две identity records остаются частью authenticated
   snapshot/manifest, но не частью shadow.
6. Primary-shadow recovery не пишет identity secrets на filesystem.
7. Device identity не входит в обычный mutation API. Ротация ключа остаётся
   отдельной authority-операцией.

## 3. Vault schema v3

Schema v3 сохраняет encrypted manifest index и record envelopes schema v2.
Новое значение schema является authenticated layout marker:

- schema v1 — legacy manifest без encrypted index;
- schema v2 — indexed vault с полным retained shadow;
- schema v3 — indexed vault, где `DeviceIdentity` является DB-only.

Смена v2 → v3 не меняет snapshot ID: набор record, их content hashes и bytes
остаются теми же. Generation увеличивается, чтобы recovery witness видел сам
layout transition. Старый клиент не понимает schema v3 и обязан остановиться,
а не принять отсутствие raw-файлов как удаление identity из snapshot.

## 4. Upgrade и crash semantics

Перед обычной CLI-командой существующие pending primary-shadow/mirror intents
сначала завершаются по правилам исходной schema. Затем выполняется retirement:

1. полностью аутентифицировать текущие manifest, index, generation и records;
2. byte-exact сравнить все non-identity shadow records;
3. если raw identity files ещё существуют, сравнить их с DB records;
4. одной immediate redb transaction опубликовать schema v3 и следующую
   generation;
5. удалить совпавшие raw identity files и синхронизировать state directory;
6. повторить exact layout-aware verification.

Crash до шага 4 оставляет старую schema и не удаляет ключи. Crash после шага 4
может оставить одну или две лишние plaintext-копии, но identity уже полностью
доступна из authenticated DB. Следующий `state-vault-migrate` или обычный CLI
guard безопасно завершает удаление без нового generation.

Если matching raw copy появилась во время уже открытого mirror intent, final
gate удаляет её перед построением effective snapshot. Mismatched copy не
удаляется и не импортируется: команда останавливается до явного вмешательства.

## 5. Effective snapshot и recovery

Для schema v3 full compatibility operations строят effective record set из:

- non-identity records, прочитанных из retained filesystem shadow;
- неизменяемых `DeviceIdentity` records текущего authenticated DB snapshot.

Поэтому обычный checkpoint/final mirror не интерпретирует отсутствие raw keys
как их удаление. `primary-shadow-intent` recovery восстанавливает только
shadow-managed records и удаляет случайно появившиеся identity files вместо
их публикации.

Typed shadow diagnostics честно сообщают для `device-identity` ноль shadow
records; полный `VaultReport` по-прежнему включает две DB records и их bytes.

## 6. Явный plaintext restore

`state-vault-restore` остаётся явной recovery/export операцией и создаёт новый
legacy directory, включая device identity plaintext. Это не normal shadow и не
автоматический fallback. Такой output является чувствительным секретом; после
проверки его следует немедленно мигрировать в новый vault либо безопасно
удалить. Обычные команды schema v3 эти файлы не создают.

## 7. Ограничения

- Обычное удаление имени файла не гарантирует secure erase старых SSD blocks,
  filesystem journal, backup или cloud-sync history.
- Ratchet pickle secrets, account-root secret и часть прочего локального state
  ещё могут оставаться в retained shadow либо иметь отдельный provider.
- DPAPI CurrentUser защищает offline key material, но не от процесса с
  полномочиями того же Windows user.
- Downgrade к pre-v3 binary не поддерживается.

