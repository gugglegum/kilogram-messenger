# A1 integration — приёмка repair iteration 2

2026-10-23. HEAD `3e82e6380c30d0f9443b6692ea34faccaacddef7` + рабочий diff.
[Отчёт реализации](lossless-sync-a1-integration.md),
[замечания итерации 1](lossless-sync-a1-integration-review-1.md).

**Согласованный первый single-chain integration slice принят. I1–I4 закрыты.**
Новых блокеров в проверенных исправлениях не обнаружено; ещё один repair cycle
не требуется. A1 целиком остаётся открыт. Product code не менялся в этом review.
Изменения четырёх Rust файлов остаются незакоммиченными; commits/push/PR не делались.

## Закрытие замечаний

| Пункт | Проверенное исправление и evidence |
| --- | --- |
| I1 | Явный allowlist bad-input errors; operational/native-state/maintenance failures выходят из транзакции с rollback. Local trust load отделён от pure input authorization. Реальная ошибка account write после session save и ошибка trust read после успешного prefix сохраняют pending bytes, pre-state native/frontier, без quarantine/ACK. Retry читает сообщение ровно один раз. |
| I2 | Account сохраняется в явно известном формате `ordered_access`; fallible чтение disk tag не определяет формат. После save/reopen prefix остаётся, обычный loader и cached legacy encrypt/decrypt отказывают. |
| I3 | Head расходует byte reserve в пределах hard cap; future admission снова резервирует MAX_EVENT. Проверены head progress, превышение cap на один байт и сохранность accepted pending. |
| I4 | Runtime gate стоит под существующим exclusive lock после vault/shadow recovery, до generic directory/listener/authority preparation. Настоящий RuntimeFromProfile с newer valid own list отказывает без изменения generation/snapshot/authority и без создания runtime artifacts; проверен также восстановленный shadow tag. |

Просмотрены исходники, новые assertions и red/green evidence. Исходные defects
ловились новыми tests до исправлений; setup failures в отчёте отделены от
подтверждённых red outcomes. Два независимых review по crypto и runtime приняли
соответственно I1/I2 и I3/I4 без новых blockers.

## Независимая проверка оркестратора

- SHA-256 четырёх исходников, manifests/lock и stable EXE совпали с
  [environment.json](lossless-sync-a1-integration-iteration2-evidence/environment.json).
  Full patch reverse/whitespace check и git diff --check прошли.
- CLI regressions `-Filter ordered_ingress_i`: **4 passed, 0 failed**, 29.90 s;
  [review-cli-regressions.log](lossless-sync-a1-integration-iteration2-evidence/review-cli-regressions.log).
  Проверены настоящие Windows write/read faults, I3 и оба I4 recovery варианта.
- Ratchet tests: **12 passed, 0 failed**, 9.04 s, включая I1 classifier/I2 и
  обычные ratchet scenarios;
  [review-ratchet.log](lossless-sync-a1-integration-iteration2-evidence/review-ratchet.log).
- Оба запуска только через `scripts/run-cargo-tests-stable.ps1`, jobs=2,
  locked/offline, BelowNormal, workspace TEMP/TMP. Сетевые EXE напрямую не запускались.

Полный 256 acceptance, все 8 CLI negatives, ordinary runtime IPC smoke,
readiness 188/188 и strict Clippy приняты по evidence исполнителя, связанному с
проверенными source/binary hashes; повторно всё это в review не запускалось.
256 test: четыре hash pages, пять child-process reopens, 256 exact plaintexts
через history API, pending=0. Его 1095.55 s — измерение функционального теста,
не доказательство пригодной для пользователя скорости.

## Граница приёмки и дальнейшей работы

Теперь имеется реальный vault-backed ordered ingress с durable pending,
согласованными ratchet/projection/frontier/ACK transitions и проверками отказов.
Он работает в согласованном изолированном synthetic профиле с известным genesis
и одной receiving chain. Обычные пользовательские профили не мигрированы,
публичного включения нет. Смена DH chain по-прежнему явно блокируется.

256 evidence использует настоящие session/vault/history компоненты с trusted
transport binding; сетевые runtime actors, Iroh/relay/mailbox, доставка ACK,
две физические машины и все power-loss windows не проверены этим slice.
Нужны последующие решения/реализация для обычного двустороннего диалога через
смены ключей, разных contexts и delivery paths, затем пользовательская проверка.
Это дальнейший scope, не незавершённые исправления I1–I4.

Разрешённый цикл этой задачи завершён: реализация → review с четырьмя замечаниями
→ repair → повторная приёмка. Новая задача исполнителю не отправлена, пустое
подтверждение через межчатовый callback не нужно. Для следующего этапа использовать
эту приёмку как baseline, сохранив открытые ограничения A1/MVP.
