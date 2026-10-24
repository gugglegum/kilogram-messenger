# A3-S1: indexed storage, integrity и ресурсы

2026-10-24, Asia/Yekaterinburg. Первая библиотечная сдача по
[S1 task](long-lived-dialog-a3-storage-s1-task.md) и
[принятому контракту](long-lived-dialog-a3-contract.md). На review, без commit/push.
Это фундамент хранения; новый messenger runtime ещё не подключён.

## Scope и происхождение

Baseline `f59d285d5704dc2c6969b6f8e416f204223e27cc`. Iroh1.1.0/redb4.2.0/
vodozemac0.10.0, Cargo.lock и dependencies не менялись. Actual turn подтвердил
`gpt-6.1-sol/max`; global config `service_tier=default`, отдельный tier в turn
metadata отсутствует. Fast/priority не включался, новых чатов/агентов нет.

До product edit сохранён `target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe`,
61,531,648B, SHA-256
`d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`.
Этот baseline не заменяется новым storage harness.

Добавлен публичный библиотечный `kilogram_state::indexed_dialog`, не CLI/profile
enrollment. `IndexedDialogVault` требует lifetime `StateDirectoryLock`, отдельный
DB file lock, новый explicit account/claim и переданные owner device keys. Create
принимает только пустой каталог с lock-файлом. Fence-only/чужой/существующий root
не усыновляется; open проверяет защищённый account/claim и никогда не создаёт root
при его отсутствии/повреждении. Device keys сохраняются в encrypted anchor;
master key использует существующий provider (на Windows DPAPI CurrentUser).

Wire2/consent2/Olm engine, live L1 migration, onboarding и runtime activation не
реализованы. Синтетические fixtures не получают runtime profile/enrollment и не
объявляются зашифрованными E2E диалогами.

## Код и интерфейс

| Файл под `crates/kilogram-state/src/` | Ответственность |
|---|---|
| `vault/indexed_dialog.rs` | owner/schema5 create/open, AEAD/KDF, fixed anchor, point/range, generation/commit identity, actual old/new plan и primary commit |
| `vault/authenticated_index.rs` | compressed radix, authenticated absence/membership/range, split/merge, bounded per-operation verified-node cache |
| `vault/indexed_dialog/types.rs` | fixed typed keys/namespaces, bounds, projection codec, errors/reports |
| `vault/indexed_dialog/credits.rs` | queue/pending/debt, grant bitmap, protected head, captures и conservation |
| `vault/indexed_dialog/backend.rs` | locked file-backed redb backend, actual read/write/resize/sync, capacity/free-space policy |
| `vault/indexed_dialog/*tests.rs` | local synthetic component, corruption, I/O, owned child kill, scale/growth checks |
| `lib.rs`, `vault.rs` | export и узкий recovery fence; generic schema validator остаётся закрытым для5 |

`get`/authenticated absence, `range(first,1..128)`/min и byte-bounded pages,
`commit(expected_generation,commit_id,mutations,credits)`, `pending`, resource/stats
readout. Numeric keys сохраняют numeric order. Изменяемые payload namespaces —
Request/Current/Native; остальные immutable put-if-absent-or-byte-identical.
Произвольный retirement отвергается; transient удаления выполняют typed transitions.
Event/Projection/Auth/Receipt/Admission/Author не GC.

API локален доверенному owner actor. Opaque Event/Auth/Receipt/Current/Native bodies
не заменяют будущую проверку подписи, native MAC/inner, Core binding и current policy.
Существование storage receipt здесь не означает сетевую авторизацию или Delivered.

## Точный storage format5

Все fixed counters/lengths в новых canonical records big-endian. Logical key32:
hash IDs без преобразования, numeric n — BE u64+24 нулевых bytes; Request16+16 нулей.
Native имеет только key0. Context selector0/1 связан с сохранённым C32.

| Namespace code | Key / payload |
|---|---|
|0 Event | EventId32 / exact opaque event≤16KiB |
|1/2 LaneA/B | n / EventId32 |
|3 Projection | EventId / version5,text_len16,UTF8≤4096,binding_len16,adapter binding≤2304; max6405B |
|4 Request | request16 / bounded mapping≤128B; engine semantics пока вне S1 |
|5 Queue | enqueue_seq / managed ticket с принятым UTF8 body≤4096 |
|6 Undelivered | n / managed materialized ticket с immutable source refs |
|7/8 ReceiptA/B | n / opaque receipt≤1KiB |
|9 Pending | n / managed ticket, exact event≤16KiB и capture refs |
|10/11 SeenA/B | key32 / g64+first_id32 |
|12 Auth | AuthRef / immutable captured bytes≤16KiB |
|13 Control,14 Current,15 Native | control id/current id/key0;≤512KiB/512KiB/128KiB, aggregate control cap4MiB/C |
|16 Credit | reserved namespace, generic writes запрещены |
|17/18/19 Author0/1/2 | global signer slot+n / C32+EventId32; cross-C fork test |
|20 Admission | EventId / `[5,original_auth32,admission_auth32]`,65B; создаётся при readable retirement |

Local trie slots=`24*context+code`; Author slots48..50 глобальны. Остальные из64
roots пусты. Привязка трёх signer slots к реальным Device credentials — обязанность
будущего admitted actor, не новая произвольная система signers.

Envelope=`5 || random_nonce24 || XChaCha20Poly1305(ciphertext+tag16)`. KDF contexts:
`kilogram a3 v5 ` + `node encryption`, `value encryption`, `integrity`, `lookup`.
Cursor key пока не используется. Data/node AAD связывает schema domain, incarnation,
slot, C/account, kind и derived address. Root AAD имеет отдельный anchor domain и
incarnation; root envelope дополнительно начинается с incarnation32.

Node plaintext: prefix_bits16,prefix32,kind8; leaf — value_hash32,length64,
charge64,reserved64,owner8; branch — два Summary. Summary=digest32,count64,
used64[2],reserved64[2] (72B). Leaf envelope133B, branch220B, hard cap256B.
Unused prefix bits обязаны быть0. Keyed digest покрывает canonical node/scope;
address не входит в digest, но входит в AEAD. При split/merge переносится только
node root, child addresses сохраняются. Negative prefix/empty proof даёт absence;
missing expected node/value, другой key/scope/digest — Integrity, не NotFound.

Anchor≤32KiB: version, incarnation/account/claim, owner keys, два C, generation,
last commit id/request digest,64 Summary и два bounded ledgers. Все эти поля
зашифрованы/аутентифицированы. Sentinel в прежнем META table имеет schema5,
count/bytes0 и hash encrypted anchor. Generic/old writers отвергают schema5;
existing ASCII account fence также остаётся. Перед prepared legacy recovery новый
schema/anchor probe отказывается выполнять старый journal поверх5.

## Atomicity и bounds операции

Обычный путь не перечисляет paths/records/directory и не пересобирает manifest.
Старый Node/Value проверяется по path; все affected nodes/values и anchor публикуются
одной redb transaction, Immediate durability. Generation и exact old anchor — CAS.
Commit identity связан с bounded canonical request digest. Exact retry последнего
commit разрешается после reopen; другой body под тем же id — Conflict. Last-id
другого commit означает unknown, а не доказанный abort; permanent request/event
mapping при runtime integration остаётся необходимым.

Plan реально кодирует header(version,old/new generation,id), затем entries
table8,key_len16,key,old?[flag8,len64,bytes],new?[flag8,len64,bytes]. Включены old/new
nodes, values, encrypted anchor и sentinel. Проверяется actual encoded≤16MiB,
не сумма только новых payload. Preflight errors не меняют root. После storage I/O
error workspace poisoned; get/range/credits/last_commit требуют reopen. Outcome
разрешается штатным redb recovery и commit/request lookup до publication/ACK.

Normal operations O(b·d·h), range O(k·d·h+bytes), d≤256, h — redb height. API допускает
bounded trusted bulk≤4096 mutations для fixtures/maintenance; native actor batch≤4
ещё не подключён. Per-operation cache только уже verified/computed nodes, bound16MiB;
он проверяет expected Summary при hit, инвалидируется при mutation и уничтожается
с read transaction. Между pages/commits полного index/cache/history в приложении нет.

Cache redb64MiB; explicit write budget128MiB/transaction. Physical policy исполняет
exact set_len(L'), без64MiB ceiling и собственного header parser. Учитываются
L'+F+256MiB headroom≤configured quota (default48GiB). F вычисляется один раз по
key/fence/lock+явно управляемым внешним files, без scan истории на mutation. При
росте наблюдается OS free≥delta+headroom, это не OS reservation. Windows probe
DriveInfo выполняется hidden subprocess только при росте; verbatim drive prefix
обработан. Extension/zero-fill/OS query latency отражается отдельно от explicit
write bytes. ConfiguredCapacity отличается от injected/actual I/O failure. Путь
продолжения — увеличить capacity/место, reopen и exact retry, не truncate/reset.

Clean/unclean open и redb allocator/debug validation измеряются отдельно. В pinned
redb `db.rs:1173–1174` debug allocator walk прогревает малые DB при verification
open. Поэтому нулевые backend reads после такого reopen не объявляются cold I/O;
отдельный release-only probe использует production debug_assertions=false/cache64.
Whole scrub/maintenance и внешний whole-snapshot rollback не получают новых гарантий.

## Credits без потери accepted bytes

Per-leaf used charge = payload+41 envelope+32 key+2×256 node allowance. Default
logical quota16GiB/C; reserved promises, repair64MiB/C и отдельный protected head
temp64KiB учитываются раздельно. Quota shrink ниже used+promises отвергается.

- Queue держит P128KiB и actual body/metadata. Materialize требует immutable source,
  typed projection с **тем же принятым текстом** и полный список положительных
  retained charges. P превращается в used+remaining debt; debt≥48KiB покрывает
  будущие два16KiB snapshots, receipt и metadata. Queue удаляется атомарно.
- Pending держит P+64KiB с вычетом уже used temporary bytes. `CapturePending`
  связывает original/admission Auth refs, оплачивает newly captured bytes из temp,
  не из второго P. Дубликат сохраняет прежние captures/credit ownership.
- Readable требует **byte-exact** перенос pending→Event, projection и receipt.
  Capture refs копируются в immutable Admission record. Permanent cost включает
  admission_paid, новые retained records и positive checkpoint growth; unspent P
  возвращается origin pool, temp/head освобождается отдельно.
- Grant≤256 хранит bitmap и выдаёт ровно funded tokens; existing pending исключены.
  Cancel возвращает только unissued. Retained spend не становится свободным при
  переиспользовании temp slot. Refill требует настоящего свободного quota.
- Debt retirement требует matching receipt ordinal/existence и оплаченных новых
  charges; peer signature/current policy — будущий adapter. Ни один путь не удаляет
  source/history либо accepted pending/queue ради освобождения места.

Полнота positive-charge refs проверяется storage layer. Недостающий metadata/native
growth не скрывается за refund. Полный pending64+head с двумя maximal16KiB snapshots
и checkpoint fixture проходит при zero ordinary capacity; refill без extra quota
отвергается. C2 не тратит protected funds C1. Общий физический I/O/capacity — общий.

## Проверки и evidence

Команды выполняются из workspace, `CARGO_NET_OFFLINE=true`, `RUST_TEST_THREADS=1`,
TEMP/TMP=`.tmp/a3-s1`, CargoJobs2/BelowNormal. Rust harnesses только compile no-run
и stable runner; names постоянные, revision/hash в имени EXE нет.

```powershell
$env:CARGO_NET_OFFLINE='true'
$env:RUST_TEST_THREADS='1'
$env:TEMP=Join-Path (Get-Location) '.tmp/a3-s1'
$env:TMP=$env:TEMP
$env:KILOGRAM_S1_SCALE_DIR=Join-Path (Get-Location) '.tmp/a3-s1/persistent-scale'
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state `
  -SkipTest @('indexed_scale_persistent_100k','indexed_scale_small_verification') `
  -CargoJobs 2 -Run -NoCapture
./scripts/invoke-cargo-friendly.ps1 -CargoCommand clippy -CargoJobs 2 `
  -CargoArguments @('--locked','--offline','--profile','verification',
                    '-p','kilogram-state','--all-targets','--','-D','warnings')
./scripts/invoke-cargo-friendly.ps1 -CargoCommand clippy -CargoJobs 2 `
  -CargoArguments @('--locked','--offline','--profile','release',
                    '-p','kilogram-state','--all-targets','--','-D','warnings')
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state `
  -Filter indexed_scale_small_verification -CargoJobs 2 -Run -NoCapture
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state `
  -Filter indexed_scale_persistent_100k -CargoJobs 2 -Run -NoCapture
./scripts/run-cargo-tests-stable.ps1 -Package kilogram-state -Profile release `
  -Filter indexed_release_cold_backend_pages -CargoJobs 2 -Run -NoCapture
cargo fmt --package kilogram-state -- --check
```

Evidence находится в [S1 evidence](long-lived-dialog-a3-storage-s1-evidence/).
`component-initial.json` — исходный13/13 run с source/EXE hashes;
`source-preservation-counterexample.json` — честный0/1 до исправления pending bytes;
`state-before-capture.log` —43/43; `state-pre-cache.log` —44/44 с capture accounting;
`state-final.log` —44/44 на окончательном library code,89.78s.
`source-pre-cache.json` связывает исходный run с EXE `d00e3b18…`;
`source-final.json` содержит12 raw SHA-256/Git blobs итоговых sources, full44 EXE
`6e92bc66…` и final scale EXE `2d84fa42…`. Между этими последними двумя compilation
менялся только исключённый из full44 `scale_tests.rs`: добавлена проверка **общего**
elapsed time после range/append, production code совпадает. Strict Clippy all-targets
и fmt check проходят на окончательных sources; Cargo.lock/dependencies без изменений.
Private fixture DB/key files остаются под `.tmp`, в public evidence их нет.

Итоговый машиночитаемый [validation-result.json](long-lived-dialog-a3-storage-s1-evidence/validation-result.json),
SHA-256 `8b9544a33c344a093cf11b572d9735d68f0a656bd83c3ab60c5bc7626105502d`,
содержит результаты, ratios, open items и hashes всех остальных evidence files.
[source-final.json](long-lived-dialog-a3-storage-s1-evidence/source-final.json),
SHA-256 `e545b25d3fc161f119bb5d2ee229bd89d873b7a37c412ee9a578db83d417bcf6`,
фиксирует12 source files; release EXE SHA-256
`13bbf0328305eb2e49a783d6fe8ec38bfc68eded312c783decb5fa12740231e1`
зафиксирован отдельно в validation result. Public artifacts не содержат vault DB,
device/private native keys либо IPC tokens.

Последние фактические numbers и hashes зафиксированы в result/source artifacts;
storage fixture, process kill и graceful reopen не смешиваются с native E2E.
Обязательные negative cases: empty/single/split/merge/long prefix, numeric order,
immutable/fork/generation/id conflicts, node/value/anchor deletion/corruption,
namespace/C/key/proof swaps, exact plan overflow, quota/head/grant, IO/sync unknown
outcome, protected owner/schema и prepared old recovery. Parent kill выполняется
только для собственного Child PID на before_commit/after_commit/after_extension_sync.

Actual final fault run: пять адресно сработавших ошибок — StorageFull, before_write,
after_write, before_sync, after_sync. Первые три восстанавливают старый state, две
sync ошибки — новый; принятый ранее Event не исчезает, Queue/reservation совпадают
с recovered root. after_sync indices1/2 не сработали: это два controls, не ещё две
доказанные ошибки. После ошибки доступ требует reopen. Real kills восстанавливают
соответственно0/2/256 records, без mixed root/data/credit state.

Production growth (та же final library):4016 synthetic records, normal batch1,
134,746,112→269,488,128B, Δ134,742,016B, append289.2455ms,
actual explicit writes172,352B,2 syncs,12 node reads, actual plan32,968B.
Setup+growth+reopen/continuation48.8809s; page/region geometry не переопределялись.
Отдельный smaller-file capacity test достигает actual request2,109,440B, F317B;
quota270,545,212B=L'+F+256MiB−1 отвергает его. Увеличение до1GiB и exact retry
после reopen продолжают те же requests до288 records. Это backend capacity test,
не заполнение host volume. Maximal accepted old/new plan15,231,748B/1799 nodes;
переполнение и oversized fields отвергаются до primary commit.

### Scale, cold I/O и обнаруженный bottleneck

Все payload —1024B opaque Event fixture одной C. Hash key детерминирован по n;
full range проверяет exact sorted keys и каждый payload, затем выполняет16 обычных
single appends. Физические nodes/values считаются отдельно от логических records.
Это raw indexed Event API, без native encryption, Projection/history cursor gate.

`scale-pre-cache.log` и `scale-pre-cache-result.json` сохраняют исходный полный
1k→10k→100k run. Он построил reusable fixture и прошёл проверки данных, но занял
1275.31s (**20min ceiling превышен на75.31s**). Прежний тест проверял deadline
в fill loop; итоговая проверка теперь охватывает также reads/append. На100k full
range302.42s, page32 p95=100.71ms, append p95=24.44ms; repeated path AEAD/digest
в одной операции оказался bottleneck. Bounded verified-node cache устраняет эти
повторные проверки. Он не сохраняется между запросами и не меняет disk format.

Исходный run показал PrivateBytes5,951,488→102,805,504B (рост92.37MiB), то есть
нельзя объявлять64MiB warm-growth gate выполненным по этому измерению. Это включает
debug redb/cache/allocator и test fixture verification; полный A3-5 требует отдельной
проверки нормативного warm baseline. Верхний process bound384MiB соблюдён.

Final performance использует новые малые fixtures и **тот же** private100k store,
без повторного построения100k. Reopen/read/single-append повторены из-за изменения
hot path; стоимость initial creation из исходного run сохранена отдельно.

| Final verification, до16 sampled append |1k|10k|100,016 reused |
|---|---:|---:|---:|
| Physical nodes / values |1999 /1000|19999 /10000|200031 /100016|
| Fill, s |2.267|30.424|0, reuse|
| Reopen, ms |30.08|242.28|2273.41|
| Full exact range, s |0.618|6.558|78.310|
| Page32 p50 / p95, ms |19.72 /20.35|20.95 /21.85|24.99 /26.59|
| Single append p50 / p95, ms |11.16 /14.05|13.33 /14.75|17.02 /18.37|
| Max single append node reads / plan B |13 /18101|15 /19087|19 /21233|
| Backend read B при open |2,552,137|24,367,433|244,040,009|
| Backend read B при full page traversal |0, debug-warm|0, debug-warm|714,145,792|
|16 appends: read / write / grow B |0 /1778688 /0|0 /2753536 /0|413696 /3826688 /0|
|16 appends: syncs |16|16|16|
| Process PrivateBytes B |5,558,272|32,870,400|98,963,456|

Final p95 ratios100k/1k: page1.307, append1.307 (≤4). Full100k range ускорился
примерно в3.86 раза; сам проход reused fixture+append81.32s, harness83.68s.
Это **не** новое измерение создания100k с нуля в20min: прежнее превышение не стёрто.
Final observed PrivateBytes growth89.08MiB также **выше64MiB**. Нормативный warm-growth
A3-5 gate остаётся OPEN; bound384MiB проходит. Small/100k verification samples идут
в отдельных процессах на одинаковом profile/setup, first-stage cache малой DB ещё
не заполнен до64MiB; дополнительный production release diagnostic не подменяет
эти verification numbers и не закрывает нормативное сравнение.

Cold storage здесь означает File-backed reads, а не flush OS page cache/проверку
физического носителя. Release-only small probes отдельно требуют ненулевой прирост
backend reads на первой page после reopen; cache64MiB и production geometry те же.
Release reuse100k измеряется отдельно от verification, без построения нового store.

Release cold probes PASS: на1k первая page32 прочитала430,080B/105 backend reads,
на10k675,840B/165; initial open в обоих случаях37,193B. Page latency1.370/2.001ms.
Release reuse100k PASS: target100,032, open65,865B/10 reads/7.101ms,
full range720,228,352B/4.175s, page32 p95=1.689ms, append p95=7.631ms,
process PrivateBytes92,381,184B. Это показывает cold backend access и отдельно
стоимость debug open, но не объясняет всю разницу памяти и не закрывает64MiB gate.
Reused store после обоих final runs содержит100,048 logical Event records,
200,095 node rows и100,048 value rows; private fixture сохранена.

По исходникам normal point/append проходит только radix paths, range — bounded
кандидаты; инструментированные single append13/15/19 node reads подтверждают
отсутствие whole-tree work. File-backed stats считают реальные redb page reads,
включая повторное чтение после cache eviction; full traversal поэтому читает больше
размера DB. Redb debug open/unclean repair сами могут перечислять allocated pages;
это O(N) startup/recovery, не замаскированная per-message операция приложения.

## Граница результата

S1 не закрывает full A3-1…A3-9: real640 Text/сторону, native>512, Root2/offline256,
L1 live upgrade/handoff, causal history cursor и direct/relay runtime ещё впереди.
Также OPEN: нормативный warm-growth64MiB и полное initial-import timing на final
hot path; measured old timing exceed и memory exceed остаются в evidence.
100k synthetic typed storage records не равны100k signed/native E2E messages.
Legacy inventory4096 не менялся. Supporting A2 сохраняется, весь A2 OPEN:7 warnings,
strict advisory FAIL; DEFERRED security reproducer не разрабатывался/не запускался.
External/daily-MVP readiness не заявляется.

Автоматическая проверка отклонила предложенный дополнительный public getter device
signing/encryption secrets как выход за S1 scope. Он не добавлен; secret keys остаются
в protected root, public transport/history views не возвращают их. Для этой сдачи
разрешение на такой API не требуется.

Чужой `ai-docs/development-environment.md` сохранён SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
Исторические accepted contracts/manifests/evidence не переписывались. До приёмки S1
не выполняются commit/push; после substantive callback root — остановка на review.
