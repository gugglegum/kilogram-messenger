# A3-S3: Codec2, Guard2 и closed storage seam

2026-10-25, Asia/Yekaterinburg. Component implementation для root review.
Commit/push до приёмки не выполнялись. Production actor/transport/migration не включены.

Основание: [implementation task](long-lived-dialog-a3-protocol-s3-implementation-task.md),
[frozen plan](long-lived-dialog-a3-protocol-s3-plan.md),
[review2](long-lived-dialog-a3-protocol-s3-plan-review-2.md),
[read-budget amendment](long-lived-dialog-a3-protocol-s3-read-budget-amendment-1.md) и
[provenance clarification](long-lived-dialog-a3-protocol-s3-provenance-test-clarification-1.md).
Дополнительно принято [явное time-budget amendment](long-lived-dialog-a3-protocol-s3-time-budget-amendment-1.md),
SHA256 `69d3166a4481540273501273ae2a77a63491934de89616a13afcaa8e69d796e6`.
Baseline HEAD — `e6f9b8ff6c7e9305d7216a94b142f0220b0c4a25`, parent
`e136c59b829c6ecc5eb501273f25091e9b9e4789`.

## Что реализовано

| Слой | Результат и граница |
|---|---|
| Protocol | Typed layouts21 Frame kinds, Core2/Event2/Inner2, Consent2/ModeProof2/controls/Ready2/receipt/descriptor; canonical CBOR, domains и caps. Parsed objects immutable. Root/Device/static selected-Q verification отделены от native MAC и current policy. Mixed data/receipt dispatch выбирает единственный decoder по явному tag; Receipt1 сохраняется exact, Receipt2 может ссылаться на format1 под выбранным existing Q. |
| Ratchet | Fixed two-lane Guard2 без serialized seen-set; checked n/sequence/g/j/root/native/T, Active/Inactive provenance, prepared transition и atomic trial apply. Authenticated external Seen observations. Bounded metadata encode/restore и pure L1 facts/source fold. Native byte adapter/config1/public snapshot API не менялись. |
| State | Новый closed Protocol2 operation family: Init/Genesis/TextBatch1..4/Pending/Acknowledge. Writer выводит fields/span/IDs/Seen из canonical source. O/A/F/R hash/certificate correspondence, mandatory original/admission capture, immutable F/raw-receipt links и exact retry evidence. Native/Seen/source/projection/Auth/credits/receipt проходят общий managed atomic writer, H1/H2 и shared Author fences. |
| CLI tests | cfg(test) owner adapter связывает настоящие native accounts/sessions, Codec2/Guard2 и indexed AEAD vaults. Runtime dispatcher, ALPN2 listener, live current-policy owner и file migration отсутствуют. |

Код и точный diff перечислены в
[source-manifest.json](long-lived-dialog-a3-protocol-s3-evidence/source-manifest.json)
и [implementation-source.diff](long-lived-dialog-a3-protocol-s3-evidence/implementation-source.diff):
25 source/data paths, из них24 Rust files и один shared JSON corpus. В существующих
runtime files изменения ограничены exports/wiring/private managed integration;
основная реализация и тесты находятся в новых modules.

Основные entrypoints:

- [protocol2](../../crates/kilogram-protocol/src/ordered_dialog_v2.rs),
  [authority/receipts](../../crates/kilogram-protocol/src/ordered_dialog_v2/authority.rs),
  [frames/mixed dispatch](../../crates/kilogram-protocol/src/ordered_dialog_v2/frame.rs),
  [static selection](../../crates/kilogram-protocol/src/ordered_dialog_v2/mode.rs).
- [Guard2/import facts](../../crates/kilogram-ratchet/src/ordered_v2.rs).
- [closed state lowering/resolver](../../crates/kilogram-state/src/vault/indexed_dialog/history_protocol2.rs),
  [четыре structural readers](../../crates/kilogram-state/src/vault/indexed_dialog/history_source_v2.rs).
- [real native/index driver](../../apps/kilogram-cli/src/ordered_dialog/protocol2_component_tests.rs),
  [rollback driver](../../apps/kilogram-cli/src/ordered_dialog/protocol2_failure_tests.rs),
  [generated L1 provenance](../../apps/kilogram-cli/src/ordered_dialog/protocol2_legacy_tests.rs).

Dependency graph сохранён: protocol→ratchet; state не зависит от protocol/ratchet.
Общие raw fixtures — test data, а не новая production dependency. Cargo.toml/lock,
Codec1, native adapter, Guard1/PrivateState и HPB1 encoding/predicate не изменены;
это проверяет [source-review.json](long-lived-dialog-a3-protocol-s3-evidence/source-review.json).

### R1, R2 и read/retry correspondence

Wire A0.request = OfferID[..16], B0.request = GrantID[..16]. Только private HPB1
genesis request нормализуется в прежний `[0;16]`; signed raw/Core/span/ID остаются
исходными. Text request сохраняется exact и сопоставляется с FIFO queue.
Bound2 projection/parent/first/tail/Seen/checkpoint reads повторяют whole-Event
correspondence. Согласованные hashes для чужого Core fragment недостаточны.
Mandatory proof/evidence failure даёт Integrity; downgrade по parseable Core отсутствует.

O извлекается из source, A фиксируется при Pending, F связан с EventId через Control,
R извлекается из exact signed receipt и проверяется против противоположного certificate.
Renewal меняет AuthRef, сохраняя certificate/Q. Inputs sorted/unique/hash-checked/used;
прежние stored bundles повторно проверяются по hash/certificate. Duplicate capture/ACK
не копирует новый current snapshot в permanent event evidence.

Control cardinality bound2 равна `2 + source_count + receipt_marker_count`:
Identity/proof, F на каждый source, raw receipt на каждый marker. Для unbound остаётся1.
Это дополнительный exact-count check; key/value correspondence проверяется отдельно.

При exact commit-id/full-digest retry перед zero-write outcome проверяются сохранённые
source/projection/F/receipt/Native либо Pending/captures/authority. Missing R не обходится
даже exact last-commit retry. Другой full digest под тем же commit_id даёт Conflict.
Test owner публикует trial с исходной root generation и Native before hash; RAM меняется
после успешного managed commit.

## Фактические проверки S3

Новые tests имеют prefix `a3_s3_`. Длинные cases запускались отдельно; bounded final
выборка исполнила23 tests: CLI5, protocol6, ratchet3, state9. Вместе с N01/N02 —25
named tests. Counts ниже обозначают cases/vectors внутри них, а не25 разных E2E traces.

| Matrix | Факт |
|---|---|
| C01 |28 roundtrips:21 Frames +3 Event/Inner +2 proof origins +2 receipt formats; дополнительно Receipt2→legacy source под existing Q |
| C02/C03 |12 malformed CBOR;13 cap/cap+1 pairs =26 helper vectors. Largest structural tuples: Core500B, Event12904B, Receipt363B, existing proof1055B, descriptor585B; descriptor Device signature проверена. Text4096B UTF-8 проходит,4097B отвергается. |
| C04–C09 |8 version/dispatch,12 independent disagreements,8 crypto/binding,8 Q/mode,6 collection и4 signed genesis negatives; rejection path не выполняет mutating native calls |
| Arithmetic/phase |24 pure vectors и8 phase vectors; same-chain при T=M, Inactive new-DH и inconsistent used_own_first проверяются через Guard2 preflight, без huge native states |
| Import |8 negative categories,9 rejection calls: missing Head и wrong AAD — два calls одной категории. Config1 positive control проходит тот же owner entrypoint; config2 отвергается точным UnsupportedOrigin/config до restore/open/import. Отдельно1 public-config serde rejection disabled V2. Две positive pre-session shapes OfferedA/GrantedB. |
| G01 |7 categories: exact/conflicting duplicate, gap, old key reuse, missing current Seen, typed Integrity/IO lookup failures; no consume после failed lookup |
| G02 |6 post-trial failures;10 attempted message calls, из них3 successful decrypts с последующим inner rejection и1 bad MAC. Полный trial discard; funded queue и accepted points/checkpoint неизменны;6 следующих valid transitions прошли. Collision/cap — явные post-native lookup/serializer injections. |
| S01/S02 |8 canonical contextual source variants +8 receipt disagreements отвергнуты. Те же8 shared D05 raw bytes отдельно отвергнуты closed writer. Debt не снимается. |
| S03/S04 |6 atomicity cases;4 retry categories, включая ambiguous recovery в S05. C2 с двумя genesis не изменяется. |
| S05 |before_write → old root; before_sync/after_sync → coherent new root в измеренном run. Reopen + exact retry без re-encrypt. Configured physical-capacity open refusal сохраняет ранее funded queue. |
| S06/S07 |6 bypasses отвергнуты;2 nonzero-prefix genesis commit/read/reopen,2 owner signed-prefix negatives C09,2 unbound HPB1 compatibility vectors и1 fragment-corruption vector. Old HPB1 predicate сохранён. |
| A01–A04 |2 authority traces: O=A=F и renewal O=A1/A=A2/F=A3/R=B2;3 duplicate traces;8 resolver negatives;1 bad Root signature boundary до commit. Renewal fixture после дополнительного undelivered source:12 sources/10 Text/23 message calls. |
| D01–D09 |Ровно37 comparisons у обеих реализаций:19 extracted-field comparisons +18 malformed rejects. Protocol проверяет реальные `id()`/`core_span()` getters; state независимо читает те же saved bytes. Четыре reader entrypoints, без Frame decoder в state. |

Primary evidence — [bounded-final.log](long-lived-dialog-a3-protocol-s3-evidence/bounded-final.log)
и [результат/EXE hashes](long-lived-dialog-a3-protocol-s3-evidence/bounded-final-result.json).
Structural corpus [s3-differential.json](../../crates/kilogram-protocol/tests/data/s3-differential.json)
содержит construction oracles, не результаты protocol parser. Opaque Olm/signature bytes
в structural event vectors не являются native MAC evidence. State negative tests используют
opaque Native markers и доказывают storage correspondence/atomicity; real native evidence
идёт отдельно через CLI component driver. Fixture setup message calls codec tests также
отделены от нулевых calls на самих rejection paths.

### N01/N02/N03: настоящая native session

| Case | Sources / Text / successful message calls | Reopen, generation, размер | Harness elapsed / runner wall |
|---|---|---|---|
| N01 |642 /640 A,0 B /1284 | Reload обоих после256/512; A.g1/B.g0; Seen[2,1] на обоих vaults; max complete sealed checkpoint3063B, Guard346B |311.149s /337.544s |
| N02 |642 /320 A+320 B /1284 |20 Text DH cycles/role; exact cycle7/8 late-tail с reload, также cycles5/10/15; g20/20, Seen[21,21]; max sealed4873B, Guard347B |423.412s /455.668s |
| N03 |20 /18 /40 | Generated L1 A0/B0+8/8, provenance+pure import cuts9/9, затем1/1 Event2; account/session/keys/D/E/SID прежние; новый checkpoint restore |1.489s, внутри bounded run |

N01 j у A Text равен n−1 до639, crossing511 не создаёт session/epoch/genesis.
В late-tail N02 own A8 остаётся Active/T16 и own lane не меняется при B98..112.
Все saved signed raw bytes/IDs, plaintext/requests и indexed Seen first/g проверены
после delivery/reopen. Public history возвращает только Text. На каждом accepted
checkpoint проверяется128KiB cap полного envelope и фиксированный Guard cap1024B.

Native accounts/keys создаёт OS RNG; deterministic schedule/text/request не заменяют
entropy. Один account/C/role, одна OTK B, без refill/reset. В calls считаются
encrypt/inbound/decrypt; fork/snapshot/restore и signature operations к этим числам
не добавлены. N03 — in-memory conversion fixture, не storage/profile/live migration.

Логи и exact per-run hashes:
[N01](long-lived-dialog-a3-protocol-s3-evidence/n01-final.log),
[N01 result](long-lived-dialog-a3-protocol-s3-evidence/n01-final-result.json),
[N02](long-lived-dialog-a3-protocol-s3-evidence/n02-final.log),
[N02 result](long-lived-dialog-a3-protocol-s3-evidence/n02-final-result.json).
Сохранённые successful fixtures находятся в `.tmp/a3-s3/`:
N01 `component-0-MlrDib`/`component-1-v1aohy`, N02 `component-0-Ivv0TK`/`component-1-iJjjui`.

CLI lib stable EXE: `target/verification/deps/kilogram-cli-lib-kilogram_cli-tests.exe`.
N01 SHA256 `edbde38c6c16ea2aea99142fb10cdf11b28a253597b5ff32d781ca18e052682a`;
N02/bounded SHA256 `5dccb0aa8429ca2d284bcbb82829fe5b68c663fbaf86c919cacaf028511e3ba9`.
Между этими runs изменились только два **test** files: funded G02 fixture и protocol
differential getter assertions. Production files и N01 driver не изменились;
`*-source.json` сохраняют фактический snapshot каждого run. Hash в имени EXE не используется.

### Read budget и наблюдаемые расходы

Bound2 page predicate — согласованные `40*(limit+2)+3`; unbound — прежние
`8*(limit+2)+2`. Выбор режима использует authenticated Identity/Control root count,
не peer hint/Core. Control.count1 fast path не добавляет proof/source lookup unbound.
Все hget/hrequired, включая absence/Auth/receipt/parents, входят в point_gets; budget
selection не читает данные после snapshot stats. Максимум2 candidates/2 parents,
без recursive ancestry/whole-history cache.

| Fixture / limit | Observed max point_gets | candidate_loads / max simultaneous |
|---|---:|---:|
| Structural bound2,8 Text /1 |106|3 /2|
| Structural bound2,8 Text /32 |241|8 /2|
| Unbound S2,8 Text /1 |26|3 /2|
| Unbound S2,8 Text /32 |60|8 /2|
| Real renewal/receipts, own delivered+undelivered,10 Text /32 |306|10 /2|
| Opposite vault,9 Text /32 |278|9 /2|

Те же maxima получены после reopen; limit1 real fixtures —106/3/2. Actual elapsed
для всех8 small read selections напечатан в bounded log, отдельно от predicate.
Missing/corrupt proof/F/Auth/receipt проверяются до частичной страницы/advanced cursor,
также в noninitial parent, Seen first и checkpoint reads. Missing R запрещает оба
пути duplicate ACK: normal semantic retry и exact last-commit/full-digest retry.
Эти measurements не заменяют S2 throughput/UX evidence.

## Reproduction и контроль scope

Windows x86_64-pc-windows-msvc, rustc1.98.0/LLVM22.1.8, Cargo1.98.0. Profile
`verification`, Jobs12, BelowNormal, model/Fast settings не изменялись.
Native test harnesses компилировались только с `--no-run`, выполнялись исключительно
через stable runner. Wrapper сохраняет source hashes, outcome, elapsed и фактические
fixed-path EXE hashes; failed logs не перезаписываются.

```powershell
$e = './ai-docs/tasks/long-lived-dialog-a3-protocol-s3-evidence'
& "$e/run-check.ps1" -Mode compile -Name reproduce-compile
& "$e/run-check.ps1" -Mode run -Name reproduce-n01 -Filter a3_s3_n01_real_native_indexed_long_chain
& "$e/run-check.ps1" -Mode run -Name reproduce-n02 -Filter a3_s3_n02_real_native_indexed_duplex_late_tails
& "$e/run-check.ps1" -Mode run -Name reproduce-bounded -SkipLongCases
& "$e/run-check.ps1" -Mode clippy -Name reproduce-clippy
cargo fmt --all -- --check
rustfmt --edition 2024 --check crates/kilogram-protocol/tests/support/ordered_dialog_v2.rs
git diff --check
```

Compile command внутри wrapper: `invoke-cargo-friendly.ps1 test` с literal
`--locked --offline --profile verification -p kilogram-protocol -p kilogram-ratchet
-p kilogram-state -p kilogram-cli --no-run`. Clippy — те же packages/profile,
`--all-targets -- -D warnings`. Stable runner использует `-CargoJobs 12 -Run -NoCapture`;
новые final runs также имеют `CARGO_NET_OFFLINE=true`. `-WorkspaceTemp` для existing
tests направляет TEMP/TMP в отдельный workspace temp; значения восстанавливаются.

Source/diff/protection capture:
[capture-source.ps1](long-lived-dialog-a3-protocol-s3-evidence/capture-source.ps1),
[run-check.ps1](long-lived-dialog-a3-protocol-s3-evidence/run-check.ps1),
[protected-files.json](long-lived-dialog-a3-protocol-s3-evidence/protected-files.json).
Имена `fast-*` у промежуточных logs означают короткую test selection, не режим модели.

### Промежуточные failures сохранены

- `check-initial`, `pure-1`, `compile-2`: compile/API/test-helper placement issues;
  corrected getters, support path и public state facade. Самые ранние diagnostic checks
  не имеют полного source manifest; final provenance на них не основан.
- [atomic-1](long-lived-dialog-a3-protocol-s3-evidence/atomic-1.log): старый H2 Control.count1
  не учитывал новые records. Исправлен exact cardinality, без расширения arbitrary keys.
- [atomic-2](long-lived-dialog-a3-protocol-s3-evidence/atomic-2.log): прежний S2 read-work
  bound не вмещал mandatory bound2 evidence; root принял отдельный40/3 predicate,
  legacy8/2 сохранён и измерен.
- [compile-10](long-lived-dialog-a3-protocol-s3-evidence/compile-10.log): public config2
  constructor недоступен в pinned features; метод pure provenance negative согласован root.
- [compile-9](long-lived-dialog-a3-protocol-s3-evidence/compile-9.log): та же config2
  проблема и неверное имя старого Frame signing API; последнее исправлено на existing API.
- [fast-1](long-lived-dialog-a3-protocol-s3-evidence/fast-1.log): несовпадение типов
  Zeroizing<Vec> и slice в test assertion; исправлено сравнение bytes.
- [fast-2](long-lived-dialog-a3-protocol-s3-evidence/fast-2.log): Windows ACL5 при создании
  нового vault в sandbox Temp; новые fixtures перенесены в workspace, successful runs
  используют изолированные directories. Это не сетевой10013 и не product repair.
- Strict Clippy iterations `clippy-1..3` нашли argument grouping, iterator/type/enum-size
  issues. Они исправлены; широких production lint suppressions не добавлено.

Runtime failures не заменены поздними PASS; полная история logs/result/source JSON
лежит в отдельной evidence directory. N01/N02 ранние PASS также сохранены, final
measurements выше относятся к усиленным read/retry checks.
[Индекс failures](long-lived-dialog-a3-protocol-s3-evidence/failed-runs.json) отделяет
9 nonzero wrapper results от3 ранних compile failure logs без result JSON.

## Ограничения и review boundary

Actual config2 SessionPickle regression **не выполнен**: V2 закрыт feature flag,
который не включался. Pure unsupported-origin/config rejection и public config serde
check не выдают себя за такой native regression.

S3 даёт библиотечные/static/storage component гарантии. Current-policy clock/revoke/
lease owner, durable Active2 ceremony, production actor, ALPN2 routes/exporter runtime,
offline scheduling, L1 file migration/live handoff, two-process2C whole-A3640/640 и
offline256 acceptance в эту сдачу не входят. Structural vectors/trusted DTO не являются
MAC/current user authorization. S2/permanent100k/legacy route suites повторно не запускались.

Frozen plan/reviews/accepted manifest и clean development-environment SHA
`53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f` сохранены.
L1 baseline EXE SHA `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`
проверен; frozen S1/S2 DB не открывались. Root-owned task/README/coordination и три
implementation amendments сохранены. Изменение coordination относительно initial snapshot — root-owned;
оно показано отдельно в protection JSON и исключено из source diff.

## Итоговые regressions, budget и provenance

| Selection / result log | Top-level PASS | Harness seconds | Runner seconds |
|---|---:|---:|---:|
| [S2 history](long-lived-dialog-a3-protocol-s3-evidence/regression-s2-history.log) |12|8.52|31.734440|
| [S2 cache](long-lived-dialog-a3-protocol-s3-evidence/regression-s2-cache.log) |5|2.89|25.628968|
| [S2 private H1/H2](long-lived-dialog-a3-protocol-s3-evidence/regression-s2-private.log) |2|0.63|23.176420|
| [S1 ledger](long-lived-dialog-a3-protocol-s3-evidence/regression-s1-ledger.log) |5|19.74|41.555935|
| [S2 process cursor](long-lived-dialog-a3-protocol-s3-evidence/regression-s2-cursor.log) |1|2.51|23.692856|
| [Codec1](long-lived-dialog-a3-protocol-s3-evidence/regression-codec1.log) |2|0.02|23.665070|
| [Guard1 late64/gap](long-lived-dialog-a3-protocol-s3-evidence/regression-guard1.log) |1|0.10|21.317678|
| [byte adapter8 DH/reopen](long-lived-dialog-a3-protocol-s3-evidence/regression-byte-adapter.log) |1|0.04|23.863499|
| [Codec1 maximal wire](long-lived-dialog-a3-protocol-s3-evidence/regression-codec1-bounds.log) |1|0.06|22.492743|

Итого30 top-level regression PASS, плюс25 S3 = **55 PASS**. Cursor child entry
намеренно ignored при обычном top-level запуске и отдельно исполняется parent test
в дочернем процессе; его вложенный libtest PASS не посчитан второй раз в55 или harness time.
`all_libtest_summaries` сохраняет оба уровня, `harnesses` — только outer results.

**Исходный совокупный ceiling600s превышен.** Отдельные N01/N02<600s этого не исправляют.
После адресного запроса root разрешил1200s на текущий итоговый комплект, сохранив
600s на каждый native scenario и все counts/profile/limits. Сумма12 final runner commands
(N01/N02/bounded +9 regressions) — **1117.3353278s**, то есть18min37.335s, в пределах
явно изменённого1200s. Это сумма отдельных измерений, не время одного continuous run.
Она включает их compile/copy/launch/cache overhead; ничего из неё не вычиталось.
Неудачные/промежуточные runs сюда не включены и сохранены отдельно.

Разделение времени:

- N01 scenario311.149s, outer harness311.17s, runner337.5438557s, входящий compile8.13s.
- N02 scenario423.412s, outer harness423.48s, runner455.6676524s, compile14.25s.
- Bounded outer harnesses67.46s, runner86.9962110s, compile1.72s.
- [Final strict Clippy](long-lived-dialog-a3-protocol-s3-evidence/clippy-final-2.log):
  PASS, checking11.20s, wrapper11.5034973s, отдельно от aggregate.
- Final fmt/helper-rustfmt/whitespace checks: [static-final-result.json](long-lived-dialog-a3-protocol-s3-evidence/static-final-result.json),
  все exit codes0; source protection — [source-review.json](long-lived-dialog-a3-protocol-s3-evidence/source-review.json).

Основную долю занимают два native/persistence scenarios. Эти логи не разделяют CPU,
crypto и backend IO; конкретный внутренний bottleneck не установлен. Ресурсная поправка
не доказывает product latency/throughput и не переносит на bound2 прежние S2 figures.

Машинный состав/сумма:
[verification-summary.json](long-lived-dialog-a3-protocol-s3-evidence/verification-summary.json),
проверяющий script [collect-verification.ps1](long-lived-dialog-a3-protocol-s3-evidence/collect-verification.ps1).
Первая collector draft с null aggregate из-за PowerShell Measure-Object/OrderedDictionary
сохранена как `verification-summary-initial-null-aggregate.json`; итог пересчитан явным
numeric sum. Это исправление collector, не замена failed test поздним PASS.

[Source/EXE correspondence](long-lived-dialog-a3-protocol-s3-evidence/source-exe-correspondence.json)
сопоставляет все12 stable artifacts N01/N02 с их фактическими selected counts. CLI bin
hash одинаков и там исполнялось0 S3 tests. N01 CLI lib отличается по hash из-за позднего
funded G02 test change; исполнявшийся N01 driver и все production dependencies совпадают
с final source. Protocol lib unit artifact N01 также старее из-за differential assertions,
но N01 selection исполнила в нём0 tests. N02/bounded и все9 regressions имеют exact
final source snapshot. Поздний scoped test change не выдаётся за старый EXE hash.

В review входят25 source/data paths, этот report, отдельная evidence directory и три
принятых implementation amendments (read budget, provenance method, aggregate time).
Точный кандидатный список — [review-files.json](long-lived-dialog-a3-protocol-s3-evidence/review-files.json),
hashes/bytes — [artifact-hashes.json](long-lived-dialog-a3-protocol-s3-evidence/artifact-hashes.json),
финальная целостность — [final-integrity.json](long-lived-dialog-a3-protocol-s3-evidence/final-integrity.json).
Metadata можно пересобрать без запуска tests через [finalize-evidence.ps1](long-lived-dialog-a3-protocol-s3-evidence/finalize-evidence.ps1).
Окончательная publication allowlist утверждается root после code review; staged changes
и commit/push отсутствуют. Frozen исходные contract/plan/evidence не переписаны.

Сдача component scope завершена. После callback работа останавливается на root review;
следующие этапы и публикация требуют отдельного указания/acceptance.
