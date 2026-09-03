# RFC-0044: Existing-account device link

Статус: реализовано в M0.9.17 (2026-09-04).

## 1. Цель и граница этапа

M0.9.17 позволяет добавить новый device в уже существующий Account без передачи
seed-фразы, Account Root key или device private keys между машинами. Церемония
разделена на три независимых действия:

1. новый device создаёт короткоживущий подписанный request;
2. машина с существующим Account Root после ручного сравнения SAS выпускает
   certificate и новый полный device list;
3. только запросивший device расшифровывает и устанавливает authorization.

Результат — новый device с DB-primary encrypted vault, текущей Root-signed
authority и свежим prekey pool. Его история по-прежнему пуста. Она восстанавливается
существующими recipient-bound recovery links/plans отдельно для каждого source;
несколько устройств могут дать независимые resumable plans, после чего уже
реализованный reconciliation сравнивает их signed inventory claims.

## 2. Provisional request

На новой машине выполняется:

```powershell
kilogram-bootstrap device-link-request `
  --workspace-dir .\kilogram-linked-device `
  --account-id <ACCOUNT_ID>
```

Helper в same-parent staging создаёт новый signing key, HPKE encryption key,
ratchet state, signed 16-entry prekey pool и encrypted vault. После проверки
vault один no-clobber rename публикует workspace:

```text
WORKSPACE/
    device/                       # encrypted DB-primary device state
    public/prekey-pool.bin
    device-link/
        request.kdl               # public signed request
        request-receipt.json      # public metadata
```

Request v1 содержит точный Account ID, random 256-bit nonce, Device ID, HPKE
public key, signed prekey pool и окно времени. Он подписан новым device key,
имеет default lifetime 10 минут, hard maximum 30 минут и общий cap 256 KiB.
`request_id` — domain-separated digest всего signed request. Из него получается
12-digit SAS вида `1234-5678-9012`.

Request не содержит private material и сам по себе ничего не разрешает. Его
можно проверить offline командой `device-link-inspect`; expired request остаётся
читаемым для диагностики, но authorization fail-closed запрещён.

## 3. Root authorization и публикация

На машине владельца Account Root выполняется:

```powershell
kilogram-bootstrap device-link-authorize `
  --account-root-dir .\kilogram-account\account-root `
  --request-file .\kilogram-linked-device\device-link\request.kdl `
  --confirm-sas 1234-5678-9012 `
  --response-file .\response.kdl `
  --device-list-file .\kilogram-account\public\account-device-list.snapshot
```

До изменения authority helper проверяет request/device/prekey signatures,
account, bounds, freshness и точный ручной SAS. Account Root enrolment защищён
от параллельного writer отдельным OS file lock. Операция:

- возвращает уже опубликованный exact certificate при безопасном retry;
- запрещает повторный Device ID с другим HPKE key/capabilities;
- запрещает восстановление навсегда revoked Device ID;
- выпускает certificate на следующем monotonic authority sequence;
- атомарно заменяет только полный Root-signed device-list snapshot.

Certificate, существующий лишь в памяти после сбоя, не авторизует устройство:
проверяющая сторона принимает его только внутри полного device list. Поэтому
сбой до atomic replace может оставить безвредный sequence gap, но не
«наполовину добавленный» device и не rollback. Повтор команды либо завершает
новую публикацию, либо возвращает тот же уже опубликованный certificate/list.

Root дополнительно подписывает authorization, привязанный к точному request ID,
certificate и полному device list. Весь signed authorization HPKE-шифруется на
public key из request. Внешний response envelope раскрывает Account ID и request
ID, но не certificate/device-list payload; подложенный или чужой response не
пройдёт decrypt/binding/signature checks.

## 4. Принятие на новом device

```powershell
kilogram-bootstrap device-link-accept `
  --workspace-dir .\kilogram-linked-device `
  --response-file .\response.kdl
```

Accept загружает identity только из authenticated DB-primary vault и требует
совпадения локального request, Device ID и HPKE key. После расшифрования он
проверяет Root signature, exact certificate, полный list и его revision.
Certificate и authority snapshot коммитятся через существующий trust workspace:
vault-primary transaction, crash-consistent retained shadow и подтверждение
mirror. Public certificate/list и terminal receipt пишутся после этого
идемпотентно. Повторный accept безопасен; response другого device и tampering
отклоняются.

## 5. Восстановление истории с нескольких источников

Device link не объявляет локальную историю полной и не копирует plaintext или
ratchet sessions. После accept новый certificate находится в актуальном полном
device list, поэтому новый device может быть точным recipient существующих
`history-recovery-link-*` и `history-recovery-plan-*` команд.

Для двух старых устройств создаются два отдельных recipient-specific plans.
Каждый источник передаёт свои bounded pages с recipient-signed checkpoint chain;
обрывы возобновляются независимо. `history-rewrap-reconcile` затем различает
`incomplete`, `single-source`, `agreed` и `divergent`. Даже `agreed` не является
доказательством глобальной полноты; UI должен показывать это честно.

Автоматический поиск/параллельный запуск нескольких plans и desktop wizard не
входят в M0.9.17. Следующий UI этап должен оркестрировать эти уже проверенные
примитивы, а не вводить новый менее строгий recovery protocol.

## 6. Security properties и ограничения

- Seed/Root key не покидает owner machine и не становится enrollment bearer.
- Новый private device key никогда не покидает новый device.
- SAS подтверждает конкретный signed request, но пользователь всё равно должен
  сверять его по независимому доверенному каналу.
- Response конфиденциален для exact HPKE recipient и дополнительно Root-signed.
- Authority anti-rollback сохраняется; public list нельзя заменить более старой
  revision или другим snapshot той же revision.
- DPAPI CurrentUser защищает at-rest keys от offline copy, но не от malware под
  тем же Windows user.
- Удаление/revocation нового device остаётся отдельной Root authority операцией.

## 7. Проверки

Unit/integration tests покрывают полный request → inspect → authorize → accept,
двух-device complete list revision, encrypted recipient binding, wrong SAS,
tampering, wrong recipient, exact retry и повторный accept. Identity test отдельно
проверяет idempotent Root enrollment и конфликт key material. Обязательны
workspace formatting, strict all-target/all-feature Clippy, 154 tests, release
build и process smoke четырёх helper-команд. Реальный release smoke
`.tmp/m0917-smoke-20260904-014515` получил authority revision 2 и принял
recipient-encrypted response размером 911 bytes.

## 8. Следующий этап

M0.9.18 должен добавить desktop wizard для обеих ролей device-link ceremony:
drag-and-drop request/response, крупный SAS confirmation, обновление launch
profile после accept и явный список recovery sources/plans с прогрессом,
`agreed`/`divergent` состоянием и без обещания глобальной полноты.
