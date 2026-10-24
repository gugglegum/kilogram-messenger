# A3-S2: оптимизированная verification сборка для полного scale gate

2026-10-24. Root принимает изменение **методики S2 scale verification** после
[attempt3](long-lived-dialog-a3-history-s2-implementation-3.md) и его
[review3](long-lived-dialog-a3-history-s2-implementation-review-3.md).
Whole2400s, integrity/memory/ratio limits и dataset сохраняются. S2 ещё не принят.

## Основание и граница решения

Unoptimized verification достиг2400s после100k range/16 appends, не завершив suffix.
Одна matched pair дала mean page141.521→10.412ms и point12.190→0.477ms в release
при одинаковых logical operation/source-check counts. Node/value AEAD sections
ускорились47.9×/70.0×. При этом release выключает debug assertions, а redb в debug
добавляет open traversal/cache warming. Эти профили нельзя объявлять эквивалентными
по memory/open semantics или просто переносить прежние нормы на release.

Для следующего полного S2 результата выбирается отдельный Cargo profile:

```toml
[profile.verification-scale]
inherits = "verification"
opt-level = 3
debug-assertions = true
overflow-checks = true
```

`debug = "line-tables-only"` и `incremental = false` наследуются от verification.
Обычный verification, release, S1 benchmark и остальные defaults не менять.
Проверки включены и для runtime dependencies; никаких package overrides/RUSTFLAGS
для отключения checks. Дополнительные LTO/CPU-specific настройки не поручаются.

Cargo позволяет независимо задавать optimization, debug assertions и overflow
checks: [официальная документация](https://doc.rust-lang.org/cargo/reference/profiles.html).
Это явный root выбор после измерений, в рамках разрешённых A3 review/repair циклов.
Исторический unoptimized FAIL остаётся FAIL; предыдущие reports/контракты не
переписываются. Новый результат прямо называет profile и его отличие.

## Что остаётся обязательным

Один свежий persistent store и процесс, последовательные1k/10k/100k стадии,
реальные validators/ledger/AEAD/durable commits, payload/distribution/private batch
прежние. Full range,16 ordinary appends, old snapshots/полный suffix/reopen/C2
обязательны. Native E2E=0. Public batch≤4, plan≤16MiB, gets≤274/page32, candidates≤2.

Process≤384MiB, app≤192MiB, cache64MiB;100k−1k raw≤96MiB **и** residual≤32MiB
в обеих warm phases; page/append p95 ratios≤4; complete scenario≤2400s.
Сравнивать все точки внутри нового profile/run, не смешивать старые1k с новыми100k.
Debug-only redb open traversal остаётся включённым; это не production cold-I/O proof.
Малый запас residual240KiB из attempt3 не служит разрешением поднять memory limit.

## Не терять завершённые измерения

В `history_scale.rs` вывести bounded summary каждой завершённой фазы до следующей
длительной фазы: fill, range/point, ordinary append, reopen/checkpoint, suffix.
Сохранять фактические timers/p95/I/O/counters и explicit status даже при последующем
budget error. Непройденные шаги остаются incomplete; не объявлять whole PASS заранее.
Не добавлять N-sized copies/oracle vectors, не расширять memory/plan limits.

Вывести `cfg!(debug_assertions)` и профиль run в provenance; нормативный путь при
выключенных debug assertions отвергается. Effective opt-level/debug_assertions/
overflow_checks для kilogram-state и redb подтвердить compiler artifact metadata
или точными compiler flags, а не только именем profile. Runtime identity/data
проверки остаются без изменений. Old raw logs не исправлять задним числом.

## Порядок следующей итерации

1. Только workspace Cargo profile и test-harness reporting/guard; product algorithms,
   dependencies/lock graph, fixtures, profiling helper и encoding semantics не менять.
2. Locked/offline compile через stable workflow, Jobs12/BelowNormal; зафиксировать
   sources/EXE до исполнения. Постоянные EXE в отдельном target/verification-scale.
3. В новом profile выполнить19 малых S2 tests и root public OS-process cursor test.
   Затем один bounded read helper на отдельной synthetic copy сохранённой10k fixture,
   максимум12 pages+12 points, чтобы подтвердить ожидаемый эффект optimization при
   сохранённых debug checks. Это precheck, не нормативный scale результат. При
   неожиданно слабом выигрыше/другом blocker остановиться для review.
4. После precheck один fresh full1k/10k/100k run без профилирования, параллельной
   тяжёлой нагрузки и дополнительного отдельного small fill. Все фазы и final gate
   должны завершиться; прежнюю100k fixture сохранить для дальнейшего reuse.
5. После PASS или любого failure/ceiling — report/evidence4, один callback и stop.
   Автоматических повторов, новых оптимизаций и изменения лимитов нет. Strict
   Clippy/fmt/whitespace по изменённым inputs; unchanged heavy suites не повторять.

Новый fresh run нужен для сопоставимых1k→100k memory/ratio данных на одном процессе
и store после изменения compilation profile. Частичные числа attempt3 и release
probe не подменяют этот результат. Приёмка и exact commit/push — отдельный root review.
