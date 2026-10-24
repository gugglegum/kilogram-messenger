# A3-S2: optimized verification-scale — full gate PASS

2026-10-24. **Один fresh full1k/10k/100k scenario завершил все gates: PASS.**
Scenario401.9873352s, harness401.99s при ceiling2400s. Полные range/suffix,
ordinary appends,3 checkpoints/reopen и C2 assertion выполнены на каждой стадии.
Обе memory comparisons и p95 ratios прошли. Это результат S2 storage verification;
приёмка S2 и commit/push остаются отдельным root review. Native E2E=0.

Основание: [review3](long-lived-dialog-a3-history-s2-implementation-review-3.md),
SHAfdca062b5e886143abc88aab8fb54f1e3ada3c3cdb039559ed889d9aa3f11307,
и [verification-profile amendment1](long-lived-dialog-a3-history-s2-verification-profile-amendment-1.md),
SHA9d9cad04c6dfa8eb5a215de5d89752e41a629b0602f9fc4af16146fb676a42c1.
Baseline HEAD61524df3a77203d73673f47d38decf6b817615e3; index пуст.
OD1 turn01a16ed1-9cf7-70a1-b282-42cacfc8ff77, GPT-6.1 Sol/max, config default/no Fast.

## Точный change-set и effective compiler flags

После attempt3 изменены только2 из19 build inputs:

- Workspace `Cargo.toml`: добавлен отдельный `verification-scale`, inherits=verification,
  opt-level3, debug-assertions=true, overflow-checks=true. Обычные verification/release
  defaults не менялись; line-tables-only/incremental=false наследуются.
- Test-only `history_scale.rs`: нормативный full отвергает debug_assertions=false
  и другой stable-path profile; profile/debug setting выводятся в run JSON. Завершённые
  fill/reopen/point/range/ordinary/visibility/checkpoints/suffix/isolation печатаются
  сразу, до следующих долгих фаз. Существующие latency arrays сортируются на месте,
  новые N-sized vectors/copies не добавлены. Whole PASS остаётся после всех фаз и close.

Product algorithms, profiling helper, dependency/lock graph, кодировки, validators,
durability, dataset/private batch128/public batch≤4 и resource bounds не менялись.
Нет новых package/env overrides, CPU/codegen/LTO flags. Ceiling2400s прежний.
[Profile patch](long-lived-dialog-a3-history-s2-evidence-4/cargo-profile-change-4.patch),
[harness patch](long-lived-dialog-a3-history-s2-evidence-4/harness-phase-change-4.patch).

Не ограничились именем profile. Locked `cargo test --no-run --message-format`
вернул compiler-artifact.profile для **redb4.2.0**, normal library и test library
**kilogram-state**: `opt_level="3"`, `debug_assertions=true`, `overflow_checks=true`,
`debuginfo="line-tables-only"`. [Raw](long-lived-dialog-a3-history-s2-evidence-4/compiler-artifacts-4.log),
[проверенная metadata](long-lived-dialog-a3-history-s2-evidence-4/effective-profiles-4.json).
Debug-only redb open/cache behavior остаётся включённым. Это не release benchmark.

Все Cargo calls locked/offline, Jobs12/BelowNormal. Stable runner компилировал без
запуска,32.23s; strict Clippy state all-targets/cache_metrics PASS7.02s. **До первого
исполнения** зафиксированы19 source inputs/bytes/SHA/blobs/snapshots и3 stable EXE:
[build provenance](long-lived-dialog-a3-history-s2-evidence-4/build-provenance-before-execution-4.json),
SHA795c0db1325039dd578fc10fd23dc0e130437ddc87508022c48504b6d1fb8325.
Library5390336B, SHA
b270073df32ded751368f93e3271a0393af89d5b0a55826a0d7e8cd4a87d89d6;
постоянный path `target/verification-scale/deps/kilogram-state-lib-kilogram_state-tests.exe`,
backup `.tmp/a3-s2/final-4/`. Hashes ещё раз проверены до precheck/full и после run.

## Short checks и один bounded precheck

- S2 small19/19 PASS6.79s, scale tests явно skipped.
- Root public OS-process cursor test PASS: parent0.50s, child0.01s/PID61116.
  Child использует current_exe нового profile; bytes root test не менялись.
- Один existing instrumented read helper на новой synthetic copy frozen10k
  `.tmp/a3-s2/history-small-2`, initial SHA7c613e31c74cb78733e5d459161bf83d67fbbc8096d94251c9b53c9df2e01814,
  10016 Text. Original DB не открывалась. Hash/existing owner/scope/count checks PASS.
  Никакого дополнительного small fill, release pair или full traversal для precheck.

На тех же12 cuts/pages +12 points mean page7.488758ms против старого instrumented
verification141.52115ms:18.898×. Mean point0.515958ms. Все phase call counts/namespace
vectors совпали:10746 node opens,1399 value opens,420 source checks; backend reads
5038080B также совпали. Gets274/candidates2. Effective debug/overflow checks сохранены;
precheck дал основание перейти к одному full run. Это instrumented sample, не gate/UX.
[Decision/counters](long-lived-dialog-a3-history-s2-evidence-4/precheck-decision-4.json).

## Fresh full scenario и полные phase measurements

Один unprofiled процесс/PID41048, одна новая `.tmp/a3-s2/history-full-4` с
последовательными1k/10k/100k. Никаких параллельных тяжёлых compile/tests и повторного
heavy run. Прежние source/text1024B, Native256B,2 lanes/до2 parents, own1/16;
private fixture использует настоящие typed validators/ledger/AEAD/durable commits.
Старая history-full-3 не открывалась и сохранила прежний hash.

| Метрика |1k|10k|100k|
|---|---:|---:|---:|
|Fill s|1.673|18.272|314.349|
|Full range s, API+streamed oracle/loop|0.581|2.721|35.177|
|Page32 API p95 ms|5.8509|8.7724|11.9485|
|Point API p95 ms|0.5531|0.5297|1.2751|
|Ordinary append p95 ms, acceptance+source|12.8353|19.5953|18.3583|
|Resume s, checkpoints+midpoint suffix|0.0809|1.1327|17.3902|
|Suffix-only s|0.0679|1.1089|17.3432|
|Suffix pages|16|156|1562|
|Cold reopen ms|15.250|116.836|935.752|
|Resume reopen ms|19.072|117.217|929.981|
|Stage elapsed s|5.139|25.064|371.472|
|Fill transactions /ordinary transactions|71/32|632/32|6327/32|

100k full range вернул ровно100000 ожидаемых user rows. Old midpoint suffix
50016→100000 завершён;16 новых appends остаются вне прежнего snapshot. Новая first,
last point, старый finished cursor и3 checkpoints/reopen проверены. C2 roots/ledger
unchanged на всех3 стадиях. Native E2E=0; trusted synthetic DTO не доказывает wire/native.

Все27 bounded phase records COMPLETE выведены **до** перехода к следующим фазам:
[phase JSON](long-lived-dialog-a3-history-s2-evidence-4/completed-phases-4.json).
Timers/p95/I/O phase records совпали с final stage values при отдельной машинной
сверке; статус не восстанавливался по предположению о control flow.
[Consistency](long-lived-dialog-a3-history-s2-evidence-4/phase-consistency-4.json).

| Clock boundary | Факт |
|---|---:|
|Scenario через явный последний drop(store)|401.9873352s|
|Последний close/drop, включён в scenario|0.0211181s|
|Rust harness elapsed|401.99s|
|Stable runner wall, включая prep/cache status|418.432033s|
|Whole budget|2400s PASS|

Encoding/печать final summary исключены из scenario timer, входят в harness/wall.
Raw [full-history-4.log](long-lived-dialog-a3-history-s2-evidence-4/full-history-4.log),
[полный parsed result](long-lived-dialog-a3-history-s2-evidence-4/full-history-4.json),
[runner wall](long-lived-dialog-a3-history-s2-evidence-4/runner-wall-4.json).

## Gates и I/O

100k/1k сравниваются внутри одного нового profile/run, без подстановки старых1k.

| Gate | Результат |
|---|---:|
|Page32 p95 ratio≤4|2.042164 PASS|
|Ordinary append p95 ratio≤4|1.430298 PASS|
|Raw growth after range≤100663296B|89948160B PASS|
|Residual growth after range≤33554432B|29995328B PASS; headroom3559104B|
|Raw growth after append≤100663296B|89690112B PASS|
|Residual growth after append≤33554432B|30654464B PASS; headroom2899968B|
|Observed process peak paged/private≤384MiB|106905600/102522880B PASS|
|App bound≤192MiB|31604366B PASS|
|Actual redb cache≤64MiB|67108864B PASS|
|Actual bulk/ordinary plan≤16MiB|1943944/47405B PASS|
|Page gets≤274 /candidates≤2|274/2 во всех стадиях PASS|
|Полный range/suffix/checkpoints/reopen/C2|PASS|

Все21 memory samples/cache metrics доступны. Relative memory allowance не менялся.
Residual после append имеет около2.77MiB запаса; это один измеренный run, не обещание
большого запаса на других нагрузках. Raw/residual и p95 ratios независимо пересчитаны
из raw-derived JSON: [gate recalculation](long-lived-dialog-a3-history-s2-evidence-4/gate-recalculation-4.json).
Failures/unavailable в final gate пустые; итоговый harness status PASS.

| Backend bytes/syncs |1k|10k|100k|
|---|---:|---:|---:|
|Fill reads B|0|17301504|7708676096|
|Fill writes B|32053440|856491520|16309007552|
|Fill sync calls|76|635|6330|
|Range reads B|0|28442624|7178334208|
|Ordinary writes B|6301696|9246720|13629440|
|Ordinary sync calls|32|32|32|

Counters measured, не размеры user payload. Fill100k фазы добавляет89984 rows:
6327 transactions; в ней sync175.562s из314.349s, growth0.568s. Поэтому оставшееся
время нельзя объяснять одной дешифровкой текста: есть реальные durable commits,
encrypted index/value work и backend write amplification. Время sync уже входит в
fill; суммы nested counters не добавляются к whole timer.

Debug redb open traversal/cache warming сохранён. Range35.177s включает streamed
oracle/loop и first-page memory query; page p95 измеряет API. Это не production cold-I/O
proof и не абсолютная UX приёмка открытия чата. Relative ratio≤4 и выполнение scale
gate не заменяют будущий unprofiled UI/runtime/native latency acceptance.

## Fixture, preservation и review boundary

Закрытая history-full-4 сохранена для reuse:100016 user Text, DB1182535680B,
SHAea5603cf5f06b87b23bae03b8bf31a4c1d1ba200641ede24f18cb941b2930e1e.
После run только hash, без нового DB open. [Post-full preservation](long-lived-dialog-a3-history-s2-evidence-4/post-full-preservation-4.json).
Старая history-full-3 — byte-exact bc3410…; старый2400s FAIL остаётся FAIL.

[Preservation4](long-lived-dialog-a3-history-s2-evidence-4/preservation-final-4.json):
42 frozen evidence3 files,217 ранее pinned checks и6 прежних verification/release
EXE/snapshot pairs совпали. L1 EXE, S1/S2 fixtures, предыдущие reports/evidence/reviews,
source/EXE snapshots и timing uncertainty сохранены. Profile amendment/review3/root
verification3 hashes совпали. development-environment.md clean53074ff… по human override.

Locked/offline compile/strict Clippy/fmt/whitespace PASS. Unchanged heavy/crash suites
не повторялись; прежнее evidence root review2/3 не объявляется новым PASS.
Commands и полный inventory находятся в evidence4. HEAD61524df…/index empty;
commit/push не выполнены. **Сдача одного результата root и остановка до review**.
Wire/native continuation/Root2/migration/runtime/UI и соседние задачи не начинались;
whole A3/MVP не объявляются завершёнными.
