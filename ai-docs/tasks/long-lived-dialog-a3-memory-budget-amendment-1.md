# A3: уточнение memory gate 1

2026-10-24. **Принято root как архитектурное уточнение до final S1 benchmark.**
Это дополнение к [контракту A3](long-lived-dialog-a3-contract.md), а не результат
приёмки реализации. Оно заменяет только warm PrivateBytes growth clause в A3-5.
Исходный контракт, его accepted manifest и прежние FAIL/OPEN evidence неизменны.
Дальнейшие A3 задачи читают исходный контракт вместе с этим дополнением.

## Основание и выбор

Исходная цель total PrivateBytes growth≤64MiB почти полностью расходуется на
заполнение явно выбранного64MiB cache. Она оставляет недостаточный запас для
cache metadata, allocator overhead и прочего состояния процесса. По проверенному
[diagnostic](long-lived-dialog-a3-storage-s1-evidence-2/memory-analysis.json) одного
Windows verification процесса, после одинакового полного warm range без N-sized
test vector:

| Значение |1016 записей|100048 записей|Прирост|
|---|---:|---:|---:|
| PrivateBytes |4,513,792|95,612,928|91,099,136B /86.879MiB|
| Measured redb cache data |2,580,800|67,108,864|64,528,064B /61.539MiB|
| PrivateBytes − cache data |1,932,992|28,504,064|26,571,072B /25.340MiB|

Root независимо сверил hashes, арифметику, memory helper и pinned redb4.2.0 source.
`CacheStats::used_bytes` берёт read-cache bytes+write-buffer bytes; настройка
Builder задаёт caching data, не весь heap процесса. `cache_metrics` включает
counters и выдачу статистики, не меняет geometry/production cache policy.
Source anchors: redb `src/db.rs:242–283,1470`,
`src/tree_store/page_store/cached_file.rs:332–359` и `lru_cache.rs`.

Exact native-heap attribution для25.340MiB не доказана. Это residual всего процесса,
а не гарантированно одни cache metadata. После drop store процесс возвращается
к3,579,904B; test vector отдельно добавляет3,207,168B и после drop возвращает память
к95,612,928B. Следовательно исключение vector не устраняет исходное превышение.

Выбран явный bounded overhead allowance, а не настройка cache под один benchmark.
Снижение cache могло бы приблизить старый total64MiB критерий, но меняло бы принятый
working set и I/O/latency режим. Для desktop verification сохраняются cache64MiB и
absolute384MiB, добавляется отдельно проверяемый residual ceiling. Мобильный budget
и произвольная история существенно больше100k этим решением не объявляются готовыми.

## Новые нормативные проверки A3-5

В одном Windows verification setup/process/profile, с production cache64MiB и
одинаковыми payload/page sizes, собрать1k/10k/100k и проверить **оба** warm phases:
после полного range и после одинакового числа ordinary single appends. Одинаковые
фазы сравнивать друг с другом; фактические counts до/после append явно указать.

Для каждой фазы p обозначим M(N,p)=неизменённые process PrivateBytes,
C(N,p)=фактически измеренные redb cache data bytes в той же точке.
Одновременно требуются:

1. `M(100k,p) − M(1k,p) ≤ 96MiB` (100,663,296B).
2. `[M(100k,p) − C(100k,p)] − [M(1k,p) − C(1k,p)] ≤ 32MiB` (33,554,432B).
3. Quiescent measured cache≤64MiB и configured cache64MiB. Каждая измеренная фаза
   процесса≤384MiB, включая fill/reopen; peaks и live buffers показываются отдельно.
4. Прежние app-owned live buffers≤192MiB, bounded point/range/plan, отсутствие
   полного обхода истории в обычной операции и p95 page/append ratio≤4 сохраняются.

В формулах используется signed arithmetic без искусственного clamp/вычитания
неизмеренной памяти. Нельзя взять M и C из разных фаз или выбрать выгодный minimum.
Полный range прогревает только данные этой fixture; дополнительные allocations,
принудительное сокращение working set и искусственное заполнение baseline запрещены.
Test-only allocations показываются явно; final domain validation не создаёт
N-sized expected vector. Малый массив latency samples допустим и не вычитается.

Нулевая/недоступная cache metric без instrumentation не является measured empty
cache или PASS residual gate. Нормативный run включает существующий
`redb/cache_metrics` через stable runner, без изменения production default features.
В обычном run неподдерживаемые platform metrics явно unavailable, не фальшивые нули.

Это **изменение критерия**, исходный64MiB gate не прошёл задним числом. Старые логи
и observed exceed сохраняются. Нарушение новых96/32 bounds требует нового анализа,
а не очередного автоматического повышения. Нельзя выдавать эти samples за доказанный
global heap peak или точную asymptotic memory bound всей будущей runtime интеграции.

## Что ещё требуется

Существующий diagnostic удовлетворяет новым численным границам только как
диагностическое измерение на pre-repair source. Нужен один fresh final-source
initial100k run с fill+read+ordinary append, исходным1200s ceiling и всеми фазами.
Прежняя fixture сохраняется; превышение1200s останавливает run для анализа.
S1 приёмка, A3 history cursor/native E2E/runtime gates и publication этим документом
не разрешены автоматически. Основание решения — [review2](long-lived-dialog-a3-storage-s1-review-2.md).
