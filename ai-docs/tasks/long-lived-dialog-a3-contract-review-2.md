# A3: повторное review и приёмка контракта

2026-10-24. Baseline `860b351fb747562e1fe12d469b1ad053217a7c0f`.
Исполнитель сдал исправленный [контракт](long-lived-dialog-a3-contract.md),
SHA-256 `66f70540a180804585f6ab9b870c2bcf1d616c24943c6ec946e6d945d7ecc25f`.
Root после проверки изменил в нём только статус и ссылки на это review;
окончательные bytes/Git blob зафиксированы в contract accepted-files manifest.

**Решение: контракт принят. R1–R3 из [review1](long-lived-dialog-a3-contract-review-1.md)
закрыты на уровне архитектуры.** Разрешена публикация точного набора документации;
после независимой сверки commit/push root передаёт первую часть реализации.
Приёмка не означает, что новый runtime/storage уже реализован или протестирован.

## Независимая проверка исправлений

| Замечание | Проверенный результат |
|---|---|
| R1, physical growth | Ложный предел64MiB убран. Backend обслуживает exact allocator set_len, ограниченный реальной configured capacity, retained files и headroom. Условия учитывают фактический запрошенный прирост;8GiB не резервируются на каждый append. Ошибки и ambiguous commit разрешаются штатным reopen/recovery, не обрезкой suffix или повторной encryption. A3-6a проверит реальный production growth boundary и отказ/продолжение после увеличения capacity. |
| R2, единый Q | Origin-tagged ModeProof2 даёт единый Q для Core/Ready/Resume/Receipt. Fresh proof известен до Init и не зависит от SID/genesis/Ready, циклического hash нет. Fresh Active2 требует оба genesis и durable Ready2; existing activation сохраняет P/A/K/Committed. Reconnect повторяет прежние records; hint не выбирает proof, renewal не меняет Q. |
| R3, native counters | Предикат проверяет post-index j+1 и общий turn T=2g+role. Native phase выводится из authenticated guard; недопустимый новый DH в Inactive отвергается до вызова. Поддерживаемый L1 import сохраняет доказанное происхождение checkpoints. Near-limit проверяется арифметикой, обычный >512 run остаётся отдельным обязательством. |

Root повторно прочитал весь исправленный документ, включая unchanged storage,
history, credits, migration/fences и acceptance. Независимо сверены pinned sources:

- redb4.2.0 `page_manager.rs` default/grow/resize/sync и создание с NO_HEADER;
  `layout.rs:154–199,222–239`; `header.rs:208–295` reconciliation после прерванного
  resize; production/test-only Builder API из review1;
- vodozemac0.10.0 `double_ratchet.rs:76–150,204–214,260–275` constructors,
  activation/receive и Inactive special case; ранее проверенные key/counter increments;
- существующий OD1 Ready/Active contract и engine: новый fresh переход сохраняет
  требование durable own+peer Ready и byte-exact retry.

Root отдельно выполнил чистые BigInt вычисления:43 counter/turn checks и96 layout
cases PASS. Пример реального layout v3:134221824→268439552B, delta134217728B.
Это source-backed арифметика, **не** runtime/storage испытания. Новые Rust harnesses
на contract-only этапе не запускались. Не подменять этим будущие process/fault gates.

## Сохраняемые обязательства реализации

1. Authenticated index — отдельная проверяемая часть: exact canonical layouts/domains,
   encrypted/MAC root, membership и absence, range completeness, cross-context/key
   isolation, mutation atomicity, immutable conflict, no unauthorized fallback.
   Получить проверяемый код и отрицательные storage tests до подключения live runtime.
2. Ordinary app append/page не загружают историю целиком. Redb allocator extension,
   OS zero-fill, clean/unclean open и maintenance измеряются отдельно и явно; их
   стоимость не исключать из отчёта о реальной latency под видом маленькой записи.
   Retained-file accounting не должен добавлять whole-history scan к каждой mutation.
3. Offline authorization принимается в рамках нового Root2 permission и текущих
   локальных signed pins. Revocation/expiry/read failure повторно проверяются перед
   commit. Старый L1 не получает новую offline permission без bilateral activation.
4. Existing history, identities, sole ciphertext source, accepted pending/queue/debt
   сохраняются через storage upgrade и wire handoff. Migration не создаёт второй live
   native domain, не обходит current authorization и не разрешает silent reset.
5. Полные A3-1…A3-9, включая640 Text от каждой стороны, ciphertext offline256,
   persistent100k, реальные kills/IO и L1 upgrade, нужны для приёмки всего A3.
   Storage fixture/частичный библиотечный slice не объявлять готовым мессенджером.

## Публикация и следующая работа

Принимается только документация из
`ai-docs/tasks/long-lived-dialog-a3-contract-accepted-files.json` плюс сам manifest.
Обычный commit/push выполняет прежний OD1 исполнитель после root dispatch; проверить
точные staged/committed Git blobs, parent860b351 и сохранить чужую правку. Root
независимо сверяет опубликованный tree/remote. Статусные изменения после freeze
не добавлять в принятый набор без повторной проверки.

Далее реализация идёт последовательными логическими частями с review/repair и
commit/push каждой принятой части. **Первая сдача — indexed storage/integrity,
resource accounting и необходимые schema/owner fences**, включая component/fault
evidence. Сначала root проверяет этот фундамент; новый wire, миграция живых profiles
и runtime activation не включаются в эту первую сдачу. History/100k evidence можно
развивать вместе с typed storage, явно отмечая ещё не подключённые end-to-end gates.
Это порядок выполнения принятого A3, не дополнительный несвязанный milestone.

Executor прежний: GPT-6.1 Sol Max, default без Fast. Windows runners/fixed executable
names, CargoJobs2/BelowNormal и границы task сохраняются. Перед первой product edit
сохранить baseline EXE d9b7ff... по оговорённому постоянному пути и проверить hash.
Supporting A2 принят отдельно, весь A2 остаётся открыт; отложенный security reproducer
не включать. Чужой `ai-docs/development-environment.md` не публикуется, SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798` сохранён.
