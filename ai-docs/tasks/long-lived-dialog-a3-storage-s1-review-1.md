# A3-S1 implementation review 1

2026-10-24. **Не принято; требуется repair iteration 2.** Контракт A3 остаётся
принятым. Исполнитель тот же OD1 / GPT-6.1 Sol Max / default без Fast. Commit/push
ещё не разрешены; wire2, migration и live runtime остаются за границей S1.

## Проверенная сдача

Baseline HEAD `f59d285d5704dc2c6969b6f8e416f204223e27cc`, index пуст.
[Отчёт исполнителя](long-lived-dialog-a3-storage-s1-implementation.md) SHA-256
`c9f9e50b2fcd9462eb20a15bbce38c96ba4bb44e94e3415fbbc291c07c314d62`.
[Source manifest](long-lived-dialog-a3-storage-s1-evidence/source-final.json)
SHA-256 `e545b25d3fc161f119bb5d2ee229bd89d873b7a37c412ee9a578db83d417bcf6`;
[validation result](long-lived-dialog-a3-storage-s1-evidence/validation-result.json)
SHA-256 `8b9544a33c344a093cf11b572d9735d68f0a656bd83c3ab60c5bc7626105502d`.
Root независимо сверил все12 source SHA/Git blobs и15 evidence hashes, прочитал
storage/index/backend/types/credits и relevant tests. Исходная сдача и её журналы
сохраняются как evidence этой итерации, включая честно оставленные OPEN gates.

Положительные результаты: authenticated radix paths и summaries, namespace-bound
AEAD, atomic anchor/data/ledger, owner/fence separation, реальные injected faults
и owned-child kills, production allocator growth и exact capacity refusal/retry.
44/44 reported library checks, fmt/Clippy подтверждены представленными журналами;
root не выдаёт их за собственный повтор. Повтор всего дорогого набора сейчас не нужен.

## R1 [P1]: повторные mutable updates теряют часть retained charge

`crates/kilogram-state/src/vault/indexed_dialog.rs:1059–1075` вычисляет old charge из
текущего overlay, затем перезаписывает `new_charge[key]` последней разницей.
Несколько Put одного Native/Current/Request в одном plan разрешены, но первый рост
теряется. `credits::cost` списывает эту уменьшенную сумму и Readable возвращает
слишком большой remainder в protected repair pool.

Независимый public-API regression создаёт funded head Pending, затем переоткрывает
store с quota ровно used+reserved+unissued+repair_free. Event/Projection/Receipt и
единственный Native2048 проходят. Тот же окончательный retained state с двумя Put
Native1024→Native2048 получает **Err(Quota)**. Последний delta равен1024 вместо
2633B полной новой leaf charge. Потеря составляет **1609B** =1024+585.
Таким образом уже принятый и обеспеченный резервом frame не получает обещанный
progress при нулевой ordinary capacity. При свободной capacity ошибка скрывается
за расходованием ordinary quota. Это библиотечный дефект; live runtime ещё не подключён.

Исправить учёт относительно committed baseline всего плана, а не последней
промежуточной версии ключа. Сохранить immutable conflict/exact duplicate semantics,
checked arithmetic и однократное распределение charge по credit transitions.
Проверить grow→grow, grow→shrink, same-size/content changes, существующий mutable
key, zero ordinary capacity и batch с несколькими frames. После отказа/retry/reopen
root/data/ledger должны оставаться согласованными. Не маскировать проблему большей
quota или пропуском conservation checks.

## R2 [P2]: Readable принимает квитанцию другого ordinal

`crates/kilogram-state/src/vault/indexed_dialog/credits.rs:628–632` требует лишь
наличия любой ReceiptA/B в refs. В отличие от Acknowledge, ordinal не сопоставляется
с удаляемым Pending. Независимый тест с Pending ordinal1 и единственной receipt
ordinal2 возвращает **Ok**, удаляя Pending и освобождая credit/head.

Storage API знает обе ordinal и должен проверять их связь до изменения ledger,
как уже делает для Undelivered retirement. Добавить matching ordinal/existence
проверку; reject сохраняет Pending/root/generation/reserves, matching control
проходит. Signature, role/EventId binding внутри opaque receipt и current-policy
validation по-прежнему относятся к будущему adapter: этот пункт не требует
реализации wire/crypto в S1 и не утверждает наличие удалённого exploit.

## R3: закрыть ресурсную неопределённость перед приёмкой S1

Отчёт корректно не объявляет PASS: final verification PrivateBytes1k→100k выросли
на89.08MiB при цели≤64MiB; первый complete100k run занял1275.31s при ceiling1200s.
После node-cache исправления final initial fill не измерен; повтор существующего
store81.32s не отвечает на вопрос initial import. Absolute384MiB и ratios≤4 прошли.

Нужен адресный diagnostic, а не автоматическое увеличение limits:

1. На одном profile/setup описать warm baseline и измерить1k/10k/100k с раздельными
   fill/reopen/ordinary page+append phases. Указать cache/live app buffers, memory
   до/после test verification allocations и по возможности peak/settled значения.
   `expected: Vec<Id>` для100k и retained test allocations не считать production
   state; одновременно не скрывать реальную cache/allocator память процесса.
   Нельзя искусственно заполнять1k baseline посторонними данными/allocations,
   принудительно урезать working set или смешивать release/verification для ratio.
2. Найти источник роста. Исправить ненужное удержание/рост, если оно в S1. Если
   production cache64MiB с overhead делает accepted64MiB delta несовместимым с
   измеренной реализацией, представить точные результаты и узкое предложение
   root для review, не менять молча контракт/cache geometry/gate на PASS.
3. После последней затрагивающей hot path правки выполнить один complete final
   initial100k fill+read+ordinary append run с исходным1200s ceiling и фазовыми
   метриками. Изменение hot path и ранее превышенный бюджет обосновывают этот один
   новый run. Сохранить прежний reusable store `.tmp/a3-s1/persistent-scale`; новый
   fixture иметь собственный постоянный путь, не удалять прежние evidence.
   Если ceiling повторно достигнут — остановка/анализ, без бесконечных повторов.

Полный A3 history cursor/native E2E gate остаётся будущим. Текущие storage measurements
не переименовывать в100k сообщений реального диалога.

## Независимые regressions и воспроизведение

Добавлен [integration test](../../crates/kilogram-state/tests/a3_s1_root_review.rs)
через public storage API, только synthetic local credentials. Production source
из проверенного manifest root не изменял. Baseline test SHA-256
`94c0ebb3909d38e663fe64b7772c471438de11cdb8da066f09d4ccee908e544b`.
Результат **0/2**, оба assertions воспроизводят R1/R2; control R1 single Put PASS.
[Лог](long-lived-dialog-a3-storage-s1-review-1-evidence/independent-regressions-baseline.log)
SHA-256 `12a43e97ba8ec4b74023cb01ac2ee84156a85341c7ccb4d1f390aa2a49421fe0`.
В review evidence также сохраняется exact baseline test source.

```powershell
$env:TEMP = Join-Path (Get-Location) '.tmp/a3-s1-review'
$env:TMP = $env:TEMP
$env:CARGO_NET_OFFLINE = 'true'
$env:RUST_TEST_THREADS = '1'
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Filter a3_s1_review_ -CargoJobs 2 -Run -NoCapture
```

Runner сам компилирует --no-run и исполняет fixed-path EXE. Выполнение0.70s.
Первый запуск использовал sandbox Temp и завершился PermissionDenied при создании
fixture до assertions; сохранён отдельным `sandbox-temp-init-error.log`, не считается
product regression. Workspace TEMP/TMP решают это без escalation. SHA этого
диагностического лога `0343fe4140d4a976bc9095a4154d2c97e75f21a3d3bb66b6b9f5cdcaf3259091`.

## Поручение repair iteration 2 и сохранность

R1/R2 исправить в текущем S1 scope, root tests сохранить как regressions и дополнить
нужными conservation cases. R3 измерить/закрыть либо вернуть конкретный contract
conflict с предложением. Relevant fast state/credit/fault checks, fmt и strict
Clippy повторить на final source; heavy checks только при затронутом пути/новом
обоснованном сомнении. Source hashes должны включать все реальные source/test files.

Новый отчёт `long-lived-dialog-a3-storage-s1-implementation-2.md`, новые evidence в
`long-lived-dialog-a3-storage-s1-evidence-2/`. Исторические report/evidence/review1
не переписывать. После содержательного callback root остановиться до review.
Commit/push только после явной приёмки root и точного accepted allowlist.

Foreign `ai-docs/development-environment.md` сохранён root SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Предложенный ранее extra secret getter не добавлять и не обходить отказ;
он не нужен S1 и не блокирует эти исправления. A2 по-прежнему OPEN:7 warnings,
strict gate FAIL; deferred security reproducer не запускать/не разрабатывать.
