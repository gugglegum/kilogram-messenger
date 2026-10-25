# A3-S5: контракт переноса существующего L1 диалога

2026-10-25. Пользователь поручил следующий этап S5 после окончательной приёмки
и публикации S4. Первая сдача — конкретный контракт для архитектурного review.
Реализация, её приёмка и публикация следуют после принятия контракта.

Исполнитель: прежний OD1 чат `01a1653c-0154-7611-8e8c-044fbe1e4d7a`,
GPT-6.1 Sol Max, без Fast. Оркестратор/root:
`01a164ec-589c-7b41-bc34-3ba158d1f22b`.

## База и нормативные входы

Implementation baseline — опубликованный S4 commit
`0c1baa887008f784a86fd7ebabdcd5145fb4d1c0`, parent
`9547ed171c64bf5ea529b4ba487acf6aa1a61b5b`.
До новых root task/status edits HEAD=origin/master, index/worktree были чистыми.
S4 принят и независимо проверен по всем528 paths/blobs и actual GitHub master.
Root task/README/coordination/next-stage edits относятся к новому S5 dossier.

Минимальное чтение, остальное — адресно по необходимости:

- [A3 contract](long-lived-dialog-a3-contract.md), прежде всего §3, §7–9,
  и принятые [уточнения](long-lived-dialog-a3-contract-review-2.md).
- [S4 contract §10](long-lived-dialog-a3-runtime-s4-contract.md#10-existing-l1-version-fences-и-точный-successor-s5)
  и deferred A3 gates из §11. Это точная согласованная граница S5.
- [S4 final acceptance](long-lived-dialog-a3-runtime-s4-implementation-review-5.md),
  [implementation](long-lived-dialog-a3-runtime-s4-implementation.md) и текущий код.
- [S3 plan](long-lived-dialog-a3-protocol-s3-plan.md) и
  [S3 acceptance](long-lived-dialog-a3-protocol-s3-implementation-review-1.md):
  existing ModeProof/mixed codecs уже есть; pure import не является миграцией файла.
- Текущие CLI UnsupportedMigration/UnsupportedHandoff, runtime2 existing-origin
  fences, V4 OrderedVault/index/journal/shadows/Guard1, indexed storage/runtime
  purpose guards, actual OD1 engine и Native public persistence APIs.

Не копируй заново большие A3/S3/S4 документы. Контракт должен содержать S5-решения,
точные отличия и ссылки на уже принятые invariants. Не перепроектируй согласованный
wire или Native crypto. Обнаруженное противоречие укажи как explicit decision для
root со ссылками на фактические entrypoints, а не обходи новой неописанной моделью.

Выявленный пробел старых readable A/F refs разобран в
[root provenance decision1](long-lived-dialog-a3-migration-s5-provenance-decision-1.md).
Контракт обязан формально учесть это migration-only различие и его negative gates;
новые live captures и current-policy требования не ослабляются.

## Продуктовая граница S5

1. Явный storage upgrade существующего V4 под lifetime lock: recovery/preflight,
   atomic Fence5+MigrationIntent/mode fence в той же DB, streamed authenticated
   copy/index с checkpoint resume, atomic Ready и retirement прежних current pointers.
   До fence безопасна отмена; после fence — forward-only resume/readable old history.
2. Сохранение Device/account/native SessionKeys, D/E/SID, точных старых ciphertext,
   EventId/request/receipt/history bytes и всех queue/pending/debt/reservation.
   Без нового genesis, account/OTK/session или параллельного writable native domain.
3. Реальная indexed L1 compatibility с прежним удалённым peer; затем отдельный
   per-context existing Consent2 и P/A/K/Committed handoff. Замораживается только
   send materialization. Старые receive/repair/ACK продолжаются; Selected принимает
   format2, но не отправляет новый Text до durable Committed.
4. Mixed Event1/Receipt1 ниже cuts и Event2 выше cuts, неизменный selected Q,
   current policy/lease отдельно от исторического proof, непрерывная история.
   После handoff сохранённая очередь продолжается ровно один раз. Второй C остаётся
   рабочим при миграции/паузе/блокировке первого в рамках согласованной модели.

Нужно реализовать все четыре части, а не назвать pure conversion, отказ старому
writer или fresh S4 trace завершённым S5. Допустимо предложить последовательные
логические implementation slices с отдельными review/commit, но финальная S5
приёмка обязана закрыть оставленные A3-1/A3-7/A3-8 обязательства.

## Что должен решить контракт

- Source inventory и точные accepted V4 origins/phases, validation/recovery order,
  pre-fence cancellation, незавершённые journals, lock ownership. Отдельно generic,
  corrupt, wrong-owner, partial genesis и unfinished initial handshake.
- Точное encoding/layout/namespace/purpose решение относительно уже принятого
  RuntimeRoot6. Таблица V4 d9b, S3 schema5, текущего S4, нового S5 и malformed/future
  inputs: busy/live и cold paths, отказ прежнего writer до mutation, никакого
  header/profile relabel/fallback/adoption. Исторические записи не переписывать.
- Atomic write sets для fence/intent, каждого batch checkpoint, Ready/current
  pointers и каждого P/Prepared/Selected/Committed перехода. Kill/IO/abort ambiguity,
  reopen/resume/exact retry должны иметь однозначные исходы и source provenance.
- Bounded V4Ref/streamed validation: old index authentication, hashes/path/type/AAD,
  cross-index completeness, seen mirror, existing Native checkpoint provenance;
  прежние keys/optional IDs/OTK reservation и late-chain tails сохраняются.
- Credit reconciliation: legacy lifetime reservation, queued/pending/receipt debt,
  новый repair pool, exhausted511/next512, quota/capacity отказ и продолжение после
  увеличения capacity. Без невидимого списания долга или искусственного refill.
- Existing control machine с exact retransmit, cut/pending/source watermarks,
  immutable Q/consent proof, renewed/revoked current policy, lost Committed и обоими
  рестартами. Старый peer, смешанные per-C modes, B-only progress, late old ACK и
  пользовательские requests должны иметь конкретный путь до проверяемого результата.
- Конкретные typed repository/owner/transport/IPC/CLI seams. Profile JSON не задаёт
  authoritative per-C mode; Root secrets не загружаются в live runtime. Existing
  history/page/cursor/isolation bounds и S4 resource/atomicity guards сохраняются.
- Однозначные operator commands/status: explicit storage-upgrade отдельно от
  consent/handoff; pause/resume после fence, old read-only history при blocked copy,
  profile publication после Ready и восстановление после сбоя этой публикации.

## План проверки и экономия повторных прогонов

Матрица связывает каждый gate с реальным entrypoint, fixture origin, assertions,
kill/IO point и ожидаемыми native/source/history counters. Обязательно:

- Новые реальные L1 fixtures от неизменного baseline d9b: active, exhausted511,
  accepted queue, pending64+gap/funded head, late DH/old receipts и неполный initial
  handshake. Не синтетическая подмена saved Native, истории или контрольных счётчиков.
- Old remote compatibility до handoff, exact cuts и отказ downgrade после него;
  actual old local/live writer и exact S4 predecessor gates. Сравнение protected
  logical state до/после отказа, positive controls с правильными owner/config.
- Real parent kills у Fence/copy/Ready и P/Prepared/Selected/B Active/Committed;
  выбранные изменившиеся native/receive/receipt IO windows. Подтвердить целые
  transitions, а не принять early child exit за kill в заданной точке.
- Полный resumed-L1 диалог по A3-1: ≥640 Text каждой стороны, ≥16 receiving DH,
  unchanged E/D/SID при crossing511, exact IDs/requests/UTF8 и old/new cursor history.
  Указать counts до/после миграции явно. Закрыть применимые offline/2C/relay свойства
  либо обосновать точное source-scoped reuse уже принятого S4 evidence.

Предложи небольшой startup/migration/own-relay smoke до длительного сценария.
Учти найденные в S4 Windows read-only sync_all в legacy OD1 test_support::checkpoint
и ordered-vault pause hooks: определить, используются ли они новой матрицей; если
да, предложить отдельное проверяемое исправление до длинных runs. S4 helper repairs
уже приняты. Не повторять эти ошибки на конце получасового прогона.

Сохраняются verification/locked/offline, Jobs12/BelowNormal и stable Windows EXE
paths. Сетевые harnesses compile --no-run, запуск через run-cargo-tests-stable.ps1.
Предложить фиксированные пути S5/old helpers без hash в имени; существующие L1/S4/
S3 images и frozen inputs сохраняются. Нельзя перезаписывать работающий stable EXE;
текущий wrapper копирует их даже при Run. Параллельность — только независимые
fixtures/processes и уже подготовленные разные EXE, без writer/target collisions.

Составить bounded verification plan с оценкой на основе измеренного S4 и ceilings
A3 §9. Вклад fixture creation/build/runner overhead и final contributing sets
показать явно. Никакого скрытого увеличения deadlines, quiet rerun или сборки PASS
из префикса и суффикса. Не запускать заново100k/долгий S3 corpus без изменения
соответствующего пути. Reuse обосновывать исходниками, не только старой строкой PASS.
Изменение ceiling или normative gate — отдельное решение root до запуска.

## Первая сдача и граница остановки

Создать `long-lived-dialog-a3-migration-s5-contract.md`: рекомендуемое решение,
таблицы состояний/совместимости/проверок, source map с точными ссылками, implementation
cuts и открытые решения для root. При необходимости — небольшие S5-only notes,
но не многочисленные дубли нормативного текста.

На этой сдаче только чтение исходников и S5 documentation. Product Rust/tests/Cargo,
runtime/DB/EXE/fixtures, migration и heavy tests не менять/не запускать. Сначала
проверить local links/diff и вернуть список изменений/hash/выводы на review.
Root task/status files сохранить как root-owned; чужие изменения не затрагивать.
Не изменять принятые исторические manifests/evidence и development-environment.md
(clean SHA53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f).

После готовности отправить один содержательный callback root: baseline, paths/hash,
ключевые решения/риски/вопросы и explicit stop before implementation. Прямое human
разрешение callback в чате исполнителя `01a16faf-e3a0-76a3-912a-1249ed4ca1e5`
сохраняет силу до отзыва; указатель не подменяет сам источник разрешения.
На существенной блокировке сообщить раньше. Не ждать копирования пользователем.

После review контракт будет опубликован по exact allowlist, затем root поручит
реализацию. После её проверки — исправления при необходимости, final acceptance,
commit/push каждого принятого логического изменения и независимая root сверка SHA/
diff/remote. До явной приёмки конкретного пакета commit/push не делать.
GUI/groups/multi-device, public field rollout, M1.2.63 и deferred crafted A2 DoS
не входят в S5 по умолчанию. Whole A3/MVP не объявлять закрытым без своей матрицы.
