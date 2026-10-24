# A3-S1: indexed storage, целостность и ресурсы

2026-10-24. **Реализация этой части разрешена.** Это первая логическая часть
[принятого A3](long-lived-dialog-a3-contract.md), не новый отдельный product milestone.
[Review2](long-lived-dialog-a3-contract-review-2.md) закрыл архитектурные R1–R3.

## Основание и состояние

Прямое human поручение `01a16bf9-89cd-7cb3-8661-36eef244e0fc` в root
`01a164ec-589c-7b41-bc34-3ba158d1f22b` разрешает contract→implementation review,
repair cycles и commit/push после приёмки. Ранее проверенные human разрешения на
substantive callbacks сохраняются; при необходимости читать первичный human turn.
Постоянное AGENTS commit/push правило действует, повторное разрешение не требуется.

Baseline HEAD/origin/master `f59d285d5704dc2c6969b6f8e416f204223e27cc`, parent
`860b351fb747562e1fe12d469b1ad053217a7c0f`. Root независимо проверил ровно7 committed
paths/blobs документации и фактический remote master. В опубликованном контракте
SHA-256 `70af8abcbbf12d553a8a4ae0751292d8780eea94b0dea350e1d17d68f54b8941`;
нормативная схема принята. Iroh1.1.0/redb4.2.0/vodozemac0.10.0 сохраняются.

Executor — прежний OD1 чат `01a1653c-0154-7611-8e8c-044fbe1e4d7a`, local,
«Подготовить baseline lossless sync». GPT-6.1 Sol Max, default без Fast. Новые
чаты/субагенты не нужны. Перед edit подтвердить actual model/effort/config и baseline.

До первой product edit сохранить baseline EXE d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1
из `target/field-tests/ordered-dialog/kilogram-cli.exe` в постоянный
`target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe` и проверить hash.
Если сохранённый baseline уже есть, сверить его, не перезаписывать другой binary.
Не путать с более старым pre-A2 EXE3f42245... . Evidence фиксирует hash содержимого.

## Конечный результат этой сдачи

Работающий библиотечный `IndexedDialogVault` на существующем redb/protected vault
key, с authenticated point/range доступом, atomic root/data/credits и проверяемыми
границами ресурсов. Его можно отдельно проверить до подключения нового wire/runtime.
Не нужна новая общая storage platform и не нужно снова выбирать архитектуру.

1. Реализовать typed namespace/key/value API, canonical layouts/domains и schema5
   RootAnchor, compressed radix index и encrypted envelopes по §4 контракта.
   Все bounds проверять до неограниченных allocation. Root и summaries защищены;
   expected missing node/value не превращается в логическое отсутствие записи.
   Namespace/context/profile/type/key связываются с proof и AAD. Numeric ranges
   используют сохранённый numeric order, а не порядок hash адресов.
2. Atomic mutation plan с expected generation/commit identity, immutable
   put-if-absent-or-identical, mutable updates и типизированным retirement только
   разрешённых transient записей. Point/range/absence работают по verified paths;
   range/min не могут незаметно пропустить leaf. Ошибка до commit сохраняет прежний
   root/data/counters; ambiguous commit требует reopen и разрешения outcome.
   Не вводить fallback создания нового root при повреждении существующего.
3. Ledger used/reserved и per-C isolation: promises не оплачиваются дважды, quota
   shrinking не забирает уже принятые обязательства. Реализовать storage-level
   transitions queue→materialized debt и grant→pending→retained/remainder по §6,
   либо минимальный typed интерфейс с теми же проверяемыми conservation predicates.
   Проверить protected repair pool/отдельный head slot и отказ при exhausted credits.
   Wire catch-up/ACK decisions пока не подключать; они остаются будущими adapters.
4. File-backed StorageBackend/stats/cache64MiB, actual encoded16MiB plan и128MiB
   explicit-write budget, actual set_len policy §6.1, profile quota/headroom.
   Не возрождать64MiB growth limit, не включать cfg(test/fuzzing) redb geometry в
   production. Не парсить/переписывать redb headers своим кодом. OS free preflight —
   наблюдение, не гарантированная reservation; actual I/O errors обрабатывать отдельно.
   F/retained accounting не добавляет сканирование всей истории к каждой mutation.
5. Минимальные mode-aware owner/schema/generic-writer fences для безопасного
   использования новой библиотеки и её tests. Существующий L1/legacy path сохраняется.
   Schema5 пока не подключать к пользовательскому enrollment/runtime. Library
   creation требует явного нового owner claim и проверяемого состояния; не усыновляет
   чужой fence/profile и не создаёт второй live native domain. Root key protection
   остаётся прежней; synthetic fixture credentials не получают production fallback.

Основные ожидаемые места: `crates/kilogram-state/src/vault/indexed_dialog.rs`,
`authenticated_index.rs`, узкие backend/types/test modules при необходимости,
минимальные exports/fences в `vault.rs`/`lib.rs`. Нужные local test/benchmark helpers
допустимы; `main.rs` не превращать в место storage реализации. Dependencies не
менять без конкретного выявленного препятствия в принятой схеме.

Native crypto/protocol2/Root2/handoff, migration живых OD1 profiles и runtime
activation **не входят в эту сдачу**. Если для typed storage нужен history cursor
reader §5, его можно реализовать здесь и проверить на тех же persistent records;
CLI/IPC пока не подключать. Проблему контракта сообщать с конкретным counterexample,
не заменять схему молча и не запускать несвязанный redesign.

## Проверки до сдачи

- **Integrity:** нормальные insert/get/absence/min/range/update/remove transient;
  immutable conflict/exact duplicate, fork/duplicate logical key, corruption/deletion
  node/value/anchor, swapped namespace/C/key/proof, stale generation. Canonical
  empty/single/split/merge и длинный общий prefix; реальный persistent index, не
  только mock map. Проверить, что integrity error не выдаётся как NotFound.
- **Atomicity:** реальные file-backed commits/reopen с root/data/credit consistency;
  адресные injected I/O/sync/ambiguous outcome и real child kill на новых durability
  boundaries, где необходимо. Test process определяется точно; не завершать чужие
  processes. Accepted bytes/debt нельзя потерять или повторно charge при retry.
- **Resources:** maximal encodings и полный old/new affected plan, уменьшенные
  quotas/full pending/head/grant transitions, два C, честный capacity refusal.
  Один production redb growth boundary A3-6a (actual delta≥128MiB) с normal single
  append, reopen/continuation и отдельным отказом ниже exact requested capacity;
  увеличить configured capacity и доказать продолжение прежнего request/state.
  Не заполнять диск пользователя и не выдавать backend quota за full-volume test.
- **Scale:** real persistent point/range/index на1k/10k/100k records, ordinary
  single-append после bounded bulk и reads после reopen/cache clear. Указать число
  логических событий и физических записей раздельно, memory/read/write/grow/sync/
  plan/commit/latency и отсутствие full index traversal в обычном пути. Synthetic
  fixture явно отделить от native E2E и от полного A3-4 history gate; если history
  reader ещё не реализован, отметить этот gate как предстоящий, а не PASS.
- Relevant state/owner/legacy regressions, fmt и strict Clippy по затронутым targets.
  Повтор полного дорогого OD1 process corpus сейчас не нужен, если не затронут его
  writer/owner dispatch; обосновать выбранный набор. После успешных проверок не
  повторять их без новых изменений/сомнений.

Все Windows network Rust harnesses: compile `--no-run`, затем только
`scripts/run-cargo-tests-stable.ps1`, fixed EXE paths; CargoJobs2/BelowNormal/locked.
Этот же runner предпочтителен для state tests. Не выполнять прямой network cargo test.
Ресурсный бюджет измерений из §9 действует; превышение означает анализ bottleneck,
а не скрытый пропуск критерия. Сохранять пригодный результат для последующих частей,
не генерировать100k заново без причины.

## Сдача, сохранность и остановка

Создать `long-lived-dialog-a3-storage-s1-implementation.md` с итоговым scope, touched
modules, точными командами/source hashes, результатами и remaining A3 gates;
новые public evidence в `long-lived-dialog-a3-storage-s1-evidence/`. Public материалы
не должны включать native secrets/IPC tokens/неотфильтрованные private logs.
Кратко объяснить actual performance и worst-case bounds, не заменять их test counts.

Сохранить foreign `ai-docs/development-environment.md` и исключить из публикации:
SHA-256 `330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Root task/README/coordination changes относятся к этой части; исторические accepted
manifests/reviews/evidence не переписывать. A2 остаётся OPEN:7 warnings/strict gate
FAIL и DEFERRED security reproducer не входят в S1, не запускать/не отлаживать его.
External/daily MVP readiness не заявлять.

После реализации и проверок — один содержательный callback root с paths/hashes,
проверенными свойствами и ограничениями, затем остановка до review. Commit/push
только после явной приёмки root, точным allowlist. Дальше root ведёт repair cycles,
проверяет опубликованный diff и поручает следующую часть согласованного A3.
