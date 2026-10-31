# B2: готово к архитектурному review

Baseline HEAD: `f6018d10820e7af0f0b3d2f64d66344d057deff1`. Изменены
16 проверочных исходников: 14 файлов из baseline и два новых. Продуктовые
runtime/Owner7 и чужие правки в этот diff не входят. Индекс пуст.

Добавлены ограниченный durable progress, существующий оптимизированный
verification-scale и явное продолжение для свежего attempt02/E02. Исторические
inventory/rows/BKP неизменны; вход принимает исключительно bit2@9 в двух S DB
при прежней длине и полном совпадении остальных байтов. Нормализация только
в памяти. Сравнения final/history и final/entry разделены; ошибки progress
не прерывают существующий close/final cleanup и не превращаются в PASS.

Строгий CLI Clippy — attempt04; строгий state Clippy, scoped fmt и PS syntax —
attempt08. Малый genuine L1 reader и progress прошли в attempt05: два теста,
1.72s внутри harness. Сам attempt05 сохранён как FAIL из-за неверного ожидания
побайтного равенства после genuine redb repair. На сохранённой синтетической
копии измерены реальные allocator/header изменения. Исправленный тест читает
ровно исходную таблицу с одной записью genuine=1, проверяет чистый заголовок и
сопоставляет реальные bytes/hash обеих осей с отчётом. Ничего не исправляется
вручную в заголовке после открытия.

Attempt09 PASS: positive и wrong-bit/other-byte/backup-change/strict-stays-strict.
Полный digest observation, включая raw hashes, совпал до/после Dispose.
Фактически final/history=false, final/entry отличаются. Финальный аудит:
8 проходов плюс отдельно учтённые два normalization read; slots4/4; handles0,
cleanup_unknown=false, transport cleanup=true; дочерних процессов0.

Все девять попыток сохранены. FAIL01/02 — внутренние cutoff при холодной
scale-сборке; FAIL03 — два lint; FAIL05 — описанный oracle; FAIL06/07 — исправленные
trait imports. Большой raw stderr05 (6,485,350B) сохранён неизменным, дальнейший
просмотр ограничен. Прежние B1 образы, промежуточный worker и draft-исходники
сохранены отдельно. Ничего не выдано за отсутствующие inputs.after/images.

Фактический B2 C1807.5774478 из3000, F0; root ceilingC6000/F4000.
Расчёт для ledger: S5.3 C4448.7369884/F1854.7357734/P0; combined
C5036.1855993/F6571.0559223/P0. Ledger обновляет root.

Машинные результаты: [handoff](C-s53-cold-b2-01.handoff.json),
[16 source pins](C-s53-cold-b2-01.sources.json),
[introduced diff](C-s53-cold-b2-01.introduced.diff),
[точный будущий invocation](Cold-B-B2-combined-private-invocation.json).
В handoff перечислены сохранённые локальные snapshots и все evidence hashes.

Private attempt02 не активирован; commit/push не выполнялись. Следующий шаг —
root review и отдельное поручение точной публикации, затем новое решение об
активации Cold-B. G1/S5.4 и вся S5.3 остаются открыты.
