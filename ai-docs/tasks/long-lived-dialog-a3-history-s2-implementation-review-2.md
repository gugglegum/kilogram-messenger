# A3-S2 implementation review 2

2026-10-24. **Ограниченные cache/node изменения проверены; S2 пока не принят.**
Воспроизведённого функционального дефекта в проверенных сценариях нет. Следующий
шаг — один final 100k run по явному [time-budget amendment1](long-lived-dialog-a3-history-s2-time-budget-amendment-1.md).
Commit/push продукта пока не поручается.

Вход: [repair2 report](long-lived-dialog-a3-history-s2-implementation-2.md), SHA
`7229edbe77da5349f56f733cc2ba22057c7e38267069ff1cb226f6632de6643b`, blob
`fe95738df1bdcb92cc58f18e3738028915e81e89`; HEAD
`61524df3a77203d73673f47d38decf6b817615e3`. Executor turn
`01a16e2a-b7cd-7131-9b23-3760839c4a66` completed; новых concurrent edits нет.

## Проверка реализации и evidence

Root сверил bytes/SHA всех 111 evidence files, 19 текущих build inputs с их snapshots,
3 stable EXE с сохранёнными копиями; расхождений нет. Quality SHA
`76d05cd07573329fdb11019cea70436f7a11ac590753d04a45829ce516b524ed`.
Финальный library EXE SHA
`bccec3ff1b4cce66aea2e54a1d496496222125f7bf46c746b85e56d543a8c946`.

Прочитаны changes относительно сохранённого source-before, verified value cache,
node FIFO/deferred sealing, cache regressions, profiling/reuse helper и связь с
writer/commit. Проверены следующие свойства:

- Value reuse ограничен одним Work, physical slot/key, authenticated root/version,
  account/context/incarnation. Mutable/negative entries обновляются; global aliases
  имеют общий physical key, owner prefix проверяется и при cached return.
  ≤128 entries и ≤1MiB plaintext capacities; Zeroizing и metadata accounting сохранены.
- Node FIFO включается только коротким history read, ≤512 entries и conservative
  1MiB; reuse требует scope/full Summary, mutation выключает и очищает FIFO.
  Межстраничного кэша/snapshot нет. Candidate proof map по-прежнему очищается.
- Новые generated nodes остаются в bounded decoded overlay. `seal_changes` вызывается
  common commit до plan bytes/backend begin/write; exact envelope length/cap проверены.
  Старые persisted nodes проходят AEAD/proof. Удалённые и промежуточные версии не
  попадают в publication. Formats/KDF и durable protocol не менялись.
- Profiling opt-in/cfg(test); обычный scale выполняется без него. Повторные synthetic
  samples явно отделены от нормативных измерений. Old S1 R1/R2 и H1/H2 semantics
  не заменены упрощённым fixture importer.

Отложенный node seal затрагивает common writer, поэтому root дополнительно повторил
реальные owned-child kill windows, пропущенные исполнителем в этой итерации.

| Независимая проверка root | Результат |
|---|---|
|S2 history/cache small, scale явно исключён|19/19 PASS, 10.48 s|
|S1 owned-child kills before_commit / after_commit / after_extension_sync|PASS, 2.11 s; PIDs 9180/37468/61260, recovered records 0/2/256|
|Public cursor после append и нового OS process|PASS; parent 0.63 s, child 0.03 s, PID39452|

Запуск — только stable runner, verification/cache_metrics, locked/offline, Jobs2,
BelowNormal, test-threads1. Root product/test source не менял. Executor final
14 affected S1 +5 independent R1/R2 и strict Clippy/fmt подтверждены совпавшим
evidence; production-growth test заново не выполнялся и новым PASS не объявляется.

Root [verification JSON](long-lived-dialog-a3-history-s2-review-evidence-2/review-verification-2.json),
SHA `1582d23ff4d1c17d619368594a574acfb7978e0401c4cecd39c3ee56f54e72dd`,
содержит команды, hashes и ссылки на три raw logs.

## Что остаётся до приёмки S2

100k не запускался; нормативные 100k memory/ratio/consistency gates UNAVAILABLE.
Small 188.486 s и прогноз 1851.9 s не заменяют измерение. В 10k small ordinary
append p95 вырос 49.981 → 59.383 ms; отдельная причина не установлена. Полный
100k/1k page/append ratio≤4 остаётся обязательным.

Точный hash EXE прежнего node-profile/small timing не был зафиксирован тогда.
Два последующих изменения только cfg(test), product/scale snapshots совпадают;
это ограничение report сохранено. Final correctness EXE нельзя приписывать прежним
timings. Следующий run фиксирует final build hash до запуска и закрывает provenance
для нормативного результата, без косметических повторов старого profiling.

## Поручение следующей итерации

Тот же OD1, GPT-6.1 Sol Max/default без Fast. Применить только test-only бюджет
2400 s по amendment1 и его явное отражение в новом evidence; product algorithms,
memory/ratio limits/profile/distribution не менять. Выполнить один fresh full
1k/10k/100k run на окончательной pinned сборке и сохранить fixture для reuse.
Достаточные small prechecks уже получены; отдельная новая малая benchmark-серия
и повторные unchanged crash/regression suites не требуются. Проверить изменённый
harness обычными locked compile/Clippy/fmt/whitespace checks.

Новый report/evidence с суффиксом `-3`: all stages/gates, phase timings, real I/O,
memory/cache, ordinary append, checkpoints/restart, exact source/EXE provenance.
При failure/ceiling/missing metrics — один фактический callback и остановка.
После полного PASS — также один callback и остановка до root acceptance/allowlist.
Wire/native continuation/Root2/migration/runtime/UI/A2 deferred scope не начинать.

Все прежние reports/evidence/reviews/manifests, L1 EXE и S1/S2 fixtures сохранить.
`development-environment.md` clean SHA53074ff… согласно прямому human override;
старый discarded330ad3… блок не возвращать. Index пуст, HEAD61524df…; S2/A3
не объявлены завершёнными. Обычные commit/push принятой задачи разрешены пользователем,
но точное поручение на них последует после review полного результата.
