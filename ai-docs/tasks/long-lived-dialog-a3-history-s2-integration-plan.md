# A3-S2: integration contract для causal history

2026-10-24. Контракт принят [root review2](long-lived-dialog-a3-history-s2-integration-review-2.md).
Product code/tests/fixtures ещё не менялись; ниже принятая спецификация S2.
Основание: [S2 task](long-lived-dialog-a3-history-s2-task.md), [A3 §4–5](long-lived-dialog-a3-contract.md),
[memory amendment1](long-lived-dialog-a3-memory-budget-amendment-1.md).
Baseline HEAD/origin/master `4d1d8b191f6e84c812c10ed3a2fa17a8dfa03629`.
Repair H1/H2 по [integration review1](long-lived-dialog-a3-history-s2-integration-review-1.md)
(SHA-256 `73befc0e890ecda2eed6e36c3d6e648016f4d488b40179aa98c8946d2553052c`).
Actual repair turn `01a16d6c-a4d0-7b91-8781-7b69676024cd`: gpt-6.1-sol/max;
config service_tier=default, отдельный turn tier отсутствует, Fast не включался.

## 1. Stored view и граница доверия

S1 уже защищает bytes, scopes и atomic ledger, но `ProjectionValue.binding` opaque,
а generic Put Lane не доказывает contiguous prefix. Поэтому H2 требует explicit
`initialize_history(context,D,E,SID,own_role)` и одного typed writer. Fresh predicate
в §1.1 исключает все прежние per-C obligations/checkpoints, не только visible history.
Root.contexts задаёт C; incarnation берётся внутри vault.
Populate старый raw S1 fixture как историю автоматически нельзя: HistoryUnavailable;
его data сохраняется. Будущая explicit L1 migration — отдельная часть A3.
AlreadyInitialized требует identical identity **и** coherent persisted heads/tails
по §1.1; identity-only match недостаточен. Иная identity — Conflict, не новый view.

Новых namespaces/RootAnchor version нет. Две reserved logical keys:
`H("kg:history2:scope-key\0")` в Control и `H("kg:history2:heads-key\0")` в Current.
Они запрещены generic Put всегда. Identity immutable; heads изменяет только history
writer. Generic fence вычисляет все touched C из ordinary keys, deletions, каждого
credit context/refs и derived keys, включая credit-only commit. Наличие bound либо
partial/corrupt identity/heads любого touched C запрещает generic mutation; reserved
keys запрещены всегда. Только полностью unbound touched C сохраняют S1 raw API.
Для global Author учитываются key.context, authenticated leaf owner и C из value,
а не только объявленный operation context; их несогласованность означает отказ.
Non-history операции перечислены закрытой matrix §2. Это fence от второго writer,
а не альтернативный runtime importer.

Exact local codecs ниже: BE unsigned, flags0/1; optional Id — flag+32B с нулевым
padding при None, никаких trailing bytes. S1 AEAD/AAD/leaf charge остаются прежними.

| Record | Canonical plaintext |
|---|---|
| Control identity,177B | `HVI1,inc32,C32,D32,E32,SID_ASCII43,own_role8,semantics8=2` |
| view_id | `H("kg:history2:view\0" || identity177)` |
| Current heads,118B | `HHD1,view_id32,F_A64,F_B64,last_source_A?33,last_source_B?33` |
| Local Request,94B | `HRQ1,state8(Queued=0/Materialized=1),queue_seq64,request16,body_hash32,event?33` |
| Projection.binding,≤255B | `HPB1,format8,view_id32,kind8,role8,n64,sequence64,request16,previous?33,core_offset16,core_len16,core_hash32,source_hash32,parent_count8,parents[≤2]` |
| Parent,41B | `EventId32,role8,n64`, sorted unique by EventId, как signed parent list |

Formats1/2, roles0/1; kind1 Init(A)/2 Confirm(B) имеют n0, request16=zero16, empty text/parents и None
previous; kind3 Text имеет n≥1. `ProjectionValue` сохраняет outer codec5, text≤4096B,
binding≤2304B; H2 максимум `5+4096+255=4356B`, ниже8KiB. Scope/counters/tails
постоянного размера: initial heads F=(1,1), tails=None. Typed genesis commit может
установить Lane(role,0)/Event/typed empty Projection и tail, не двигая Text F.
Text1 требует этот genesis predecessor; для Text n>1 предыдущий Text — implicit edge.
Это отделяет Core.previous/native cause от signed user parents, которые здесь только Text.

EventId пересчитывается из exact source с существующим `kg:od1:event-id\0` либо
принятым `kg:od2:event-id\0`. `source_hash=H("kg:history2:source\0"||format8||raw)`;
`core_hash=H("kg:history2:core\0"||format8||raw[core_span])`. Span checked, len1..2048,
source≤16KiB. Writer получает bounded `PreparedReadable` с exact source, Core span,
normalized summary и text. **Это trusted owner/adapter DTO, не cryptographic proof.**
Storage проверяет hashes/span, typed scope/slots/parents и atomicity; оно не объявляет,
что произвольные bytes уже прошли wire verification. Будущий verified adapter обязан
доказать canonical source/Core extraction, совпадение summary с Core, signature/ID,
native header/MAC/inner==outer, plaintext text и current policy до commit purpose.
Никакого нового wire2 decoder/native call в S2. Fixtures подают synthetic trusted DTO
через тот же validator/commit; это0 native E2E, без runtime enrollment/fallback.

### 1.1. H2: fresh/bootstrap и repeat

В одном authenticated root snapshot, под owner lease, до native/domain mutation:
1) прочитать две known identity/heads keys; 2) при обеих absent проверить **все24**
local summaries выбранного C на canonical empty; 3) для global Author slots48..50
требовать used[C]=reserved[C]=0 (positive leaf charge доказывает отсутствие owner-C
записей), unused global roots51..63 empty; 4) ledger[C] pristine:
repair_free=64MiB, outstanding=pending=0, head=None, grant=None; aggregate
used[C]=reserved[C]=0. Canonical empty — Summary::default(): zero digest/count и обе
пары used/reserved=0; одного count0 недостаточно.
Quota может быть owner-configured, но обязана оплатить init обычным S1 preflight.
Это fixed summaries/point proofs, без namespace/directory scans. Ничего не удаляется.

Разрешённый bootstrap — **только** существующие authenticated Root account/claim,
protected device keys, incarnation, два C и owner quotas плюс обычные key/fence/lock
файлы. Другой C может быть активен и иметь owner-C2 Author entries: они сохраняются.
Ни одного pre-init per-C Control/Current/Native/Seen/Request/receipt/Auth/queue/pending
record не допускается. Existing session/accepted state требует будущей explicit
migration purpose вне S2. Init выполняется до durable native setup; secrets/session
не создаются initializer и не переиздаются.

| Persisted state выбранного C | Результат init/repeat |
|---|---|
| Обе keys absent, fresh predicate выполнен | Atomic identity+heads(F=1/1,tails=None), native отсутствует |
| Только profile bootstrap/работающий C2, target fresh | То же; C2/shared signer allocation не сбрасываются |
| Pending-only/Queue-only/Undelivered/Native/Seen/Request/domain/Author-owner-C или non-pristine ledger | ExistingStateRequiresMigration, даже без readable Event |
| Identity-only/head-only, bad proof/codec, чужой stored scope/incarnation | IntegrityBlocked; не AlreadyInitialized/fresh repair |
| Обе keys valid, identity совпадает, границы coherent | AlreadyInitialized, без изменения heads/native/ledger |
| Stored view coherent, requested D/E/SID/role другой | Conflict; source/session не усыновляются |
| Heads/tail/source/link/count bound нарушен | IntegrityBlocked без writes |

Repeat сначала проверяет persisted coherence, затем сравнивает requested identity.
Control/Current содержат ровно known identity/heads (count1 каждого в S2); unsupported
дополнительный domain/control key — отказ. Для каждой lane: tail=None ⇒ F=1/count0;
tail=Some ⇒ count=F≥1, Lane0/genesis и Lane(F−1)/tail с Event/typed Projection/hash
точечно совпадают по scope/role/n/kind. Authenticated first-at-or-after(F) отсутствует:
canonical numeric keys, count=F и suffix absence доказывают slots0..F−1 без holes.
Event.count=Projection.count=сумме lane counts (checked); иной count — IntegrityBlocked.
Нет genesis ⇒ Native absent; есть genesis ⇒ ровно один nonempty Native(key0)≤128KiB.
B0 без A0 запрещён; Text в любой lane требует оба genesis. Нормальные queued/pending/
seen/request records уже bound view допустимы при repeat: pristine predicate относится
только к fresh init, он не сбрасывает живой ledger.
Identity/view и F/tail predicates обязательны, failure не лечится перезаписью.
Это bounded boundary validation, не whole DAG/source scrub: corruption внутренних
непрочитанных records остаётся integrity-on-access S1, не сертификат здоровья всего vault.

## 2. Один atomic writer

`commit_history(context,expected_generation,commit_id,operation)` использует те же
Work/plan/redb transaction и poison/reopen rules. **Один C на commit**, никакого
validated-by-C1 изменения C2. Наружу не передаются raw effects/credits/retention refs;
facade выводит их из закрытых typed operations. Runtime TextBatch1..4, прочие по одному.
`initialize_history` — typed Init entry point того же engine.

| Operation (исчерпывающе) | Source rows / derived credits | Native и прочие разрешённые effects |
|---|---|---|
| Init |0 /none | Только identity+heads после §1.1; Native запрещён |
| Genesis |1 новый n0 /none; A0 перед B0 | Только generated Event/Projection/Lane/tail, связанный Native transition и row extras; ordinary bootstrap quota |
| TextBatch |1..4 **новых** Text /ровно1 Materialize или Readable на row | Generated rows/heads, local Request transition, peer receipt, связанные row extras и ровно1 Native transition всего batch |
| QueueAccept |0 /один Queue | Generated local Request и ticket; Native/Seen/Author/heads запрещены |
| AcceptPending |0 /один Pending, optional CapturePending | Peer n≥1; exact ciphertext, только Auth records/refs для связанных captures; Native/Source/Projection/heads запрещены |
| CapturePending |0 /один CapturePending | Только existing pending(C,n,EventId) и его два bounded Auth records/refs; Native/heads запрещены |
| RepairCredit: Grant/CancelGrant/Refill |0 /соответствующий один action | Только ledger/grant; никакого generic effect |
| Acknowledge |0 /один Acknowledge | Existing own Undelivered +matching source-role receipt/auth; Native/heads запрещены |

Иного empty/maintenance/admin purpose нет. Opaque Control/Current patches, native-only
refresh/reset/rekey и migration не разрешены S2; будущая необходимость требует
отдельного typed transition review. Row extras — только referenced Auth, Seen и Author:
все keys/refs выбранного C, Author value=C/EventId и sequence/slot связан с конкретным
row; Seen role/key/g/first связан с row native-header claim. Неподтверждаемый crypto
смысл этих claims проверяет будущий adapter. Другие namespaces/effects запрещены.
Private lowering повторно проверяет touched-C census по всем generated keys/actions/
refs: global Author только с owner-C binding. Unselected C ledger/records неизменны.

**H1 bijection до S1 credits::apply:** consuming Materialize/Readable ↔ новые Text rows,
один-к-одному; sets `(C,ticket_kind,ticket_key)`, `(C,role,n)` и `(C,EventId)` unique. Нельзя
добавить extra consume/refs, погасить два tickets одним row или использовать ticket
дважды. Existing Event/Projection/Lane не считается новым row, даже byte-identical.
Private validator проверяет равенство обоих множеств row↔consume после lowering;
empty batch с Materialize/Readable отвергается до S1, включая review trace A1/R1→q2/R2.

| Row origin | Обязательные storage matches и generated consume |
|---|---|
| Local, role=own_role | Queue(C,q),stage1; ticket.id=request16||zero16; typed Request Queued(q,request,body_hash); queue body **byte-exact** Projection text; q — authenticated min Queue. Materialize(C,q,handle,n,EventId,generated refs), все n/EventId/request из нового row |
| Peer, role=1−own_role | Pending(C,n),stage2; ticket.id=EventId, body **byte-exact** exact row source, capture ownership прежнее. Readable(C,n,EventId,generated refs), matching Receipt(role,n). Request относится к тому же saved source/Core claim verified adapter, не к чужому local Request map |

AcceptPending для n<F_peer не создаёт старый ticket: exact existing readable tuple
даёт AlreadyReadable без writes; conflicting ID/bytes — отказ. Иначе head/ahead/grant
и capture conservation прежние S1. Acknowledge требует existing own Text и debt с
тем же n/EventId; key receipt source-role/n выводится facade. Duplicate подтверждённого
ACK — no-op по exact immutable receipt/source, не повторное списание debt.

QueueAccept по request16: existing identical body возвращает existing result без
writes, conflict не создаёт ticket. Новый q=expected Root generation (один enqueue
на commit, checked, strictly increasing), key absent. Local Request94 хранит
body_hash=H("kg:history2:request-body\0"||UTF8), q/request; Queued→Materialized меняет
лишь state/event. Key request совпадает с value, Queued требует event=None,
Materialized — Some(exact EventId); retry text сверяется с Queue либо typed Projection.
Размер `4+1+8+16+32+33=94B≤128B`. Новый Request оплачивается ordinary quota при enqueue, mapping
не удаляется. Peer request не переписывает local map. Это уточнение94B в уже имеющемся
namespace≤128B; остальные layouts, cursor и response maxima не изменились.

Native mutation без нового **Text** допускается только Genesis с новым n0 source;
без нового source row вообще запрещена. A0 требует Native absent, B0 — committed A0
и existing Native; Text — оба genesis. Transition несёт before_hash текущего Native,
nonempty after_blob≤128KiB и ordered frame_ids **ровно** этого Genesis/Text batch.
before_hash=None допустим только для A0 при absent Native, иначе Some(exact hash).
Storage проверяет presence/hash/C/frame correspondence и generation; доказать, что
opaque blob — правильный native post-state, обязан будущий verified adapter, не S1
raw Put. Другие purposes не могут незаметно изменить checkpoint через Current.

Для каждого нового Text в bounded batch: проверить scope/view/role, n==F[role],
previous==last_source, checked(n+1); разрешить≤2 parents точечно через их typed
Projection и exact Lane(role,n). Parent должен иметь тот же view, kind Text и n<F
своей lane в текущем committed prefix либо в уже проверенном более раннем batch item.
Unknown/future/self/cross-C parent и cycle отвергаются без writes; весь DAG не читается.
Сохранить все signed parent edges, добавить implicit lane order, не менять source bytes.
Записать immutable Event/Projection/Lane и новый fixed heads в **одной** transaction
с native/request/author/receipt effects и credit transitions. No holes; Pending без
readable projection не двигает F. Own Materialize и peer Readable различаются по
pinned own_role; accepted Pending bytes совпадают с Event byte-exact.

Init identity/heads оплачивается ordinary admin quota до accepted frame promises.
Дальше heads fixed-size; все положительные charges и будущий Native growth проходят
прежние R1 final-minus-committed-baseline/spent checks. Materialize/Readable refs
выводятся из row: Event/Projection/Lane, heads/Native, local Request либо peer receipt,
связанные Auth/Seen/Author; Admission добавляет S1 capture path. Непривязанные refs
недоступны; union обязан покрывать все положительные charges. Total refs≤24 на action,
actual old/new plan≤16MiB. P128KiB,
temp/head/repair/per-C isolation не ослабляются. Readable сохраняет R2 matching
receipt C+ordinal/existence; signature/current-policy всё ещё adapter. ACK/delivery
не двигает readable heads. Unfunded/oversized plan — отказ, не скрытая ordinary/pool subsidy.

Для retry private common commit engine принимает вычисленный внутри purpose digest:
history domain `history-commit-request/v1` покрывает context/view, expected generation,
operation tag и canonical bounded typed inputs: source/Core/text/claims, ticket IDs,
Auth inputs, Native before/after и ordered frame_ids. Raw vectors публично отсутствуют.
Exact last commit retry с тем же digest проверяется **до**
fresh append predicates: advanced heads не превращают успешный retry в hole/conflict.
Иной digest при том же commit_id — Conflict. Last retry возвращает прежний результат,
не повторяет consumes/Native; старый row нельзя выдать за новый через иной commit_id.
Retry не пропускает root/scope integrity; Init, включая exact last retry, всегда
проверяет persisted identity/heads coherence §1.1 перед успешным ответом.
Public API не принимает готовый digest; generic S1 digest прежний. Older unknown
commit outcome требует reopen + permanent Event/Request lookup, без повторного Native
trial/publication и без применения старого checkpoint поверх новых commits.

## 3. Capture, cursor и page

`history_first(context,limit)` в одной короткой read transaction проверяет current
authenticated root/identity/heads и захватывает exclusive F. `history_page(context,
opaque_cursor,limit)` открывает новую read transaction текущего root; generation
не входит в cursor, source/projections immutable. Нет MVCC pin, process session,
expiry или полного order cache. Локальной readable истории peer/lease не требуется.

Exact cursor codec,275B; `u64` BE, никаких optional fields:

```text
header8 = 4b 47 48 32 01 02 00 00    // KGH2, codec1, view semantics2, reserved0
body243 = header8 || incarnation32 || view_id32 || C32 || D32 || E32 || SID43
          || F_A64 || F_B64 || next_A64 || next_B64
cursor275 = body243 || MAC32
K = BLAKE3 derive_key("kilogram a3 v5 cursor", existing_vault_master32)
MAC = BLAKE3 keyed_hash(K, "kg:history2:cursor\0" || body243)
```

K — private Zeroizing field Crypto, заново выводится при open, не getter/export.
Full32B tag сравнивается constant-time через blake3::Hash equality. Base64URL no-pad
ровно367 chars; bounds до decode/allocation, strict alphabet/length/re-encode equality.
MAC/codec/domain/profile/C mismatch → InvalidCursor. Scope берётся из persisted
identity, а не caller-supplied D/E/SID. Проверить1≤next_R≤F_R≤current_heads_R;
valid-MAC F выше current heads → IntegrityBlocked/SnapshotUnavailable, не пустой конец.
Append/restart сохраняют старый suffix; новая first захватывает новое F. Whole-profile
rollback не получает новых anti-rollback гарантий.

Алгоритм буквально A3 §5, next_A=next_B=1 при first:

```text
load at most two candidates: exact Lane(role,next_role) below frozen F
validate typed Projection/source/Core hashes, previous slot and parent role/n/ID links
while emitted < limit and remaining:
    eligible := all signed parent ordinals < next[parent.role]
    if no eligible candidate: IntegrityBlocked
    emit eligible candidate with lexicographically smallest EventId
    advance only its lane; replace only that candidate if still below F
finished := next_A==F_A && next_B==F_B
seal updated cursor; encode bounded response; return
```

Implicit previous Text уже покрыт sequential next. Parent с n≥frozen F, missing expected
Lane/Projection/source, bad binding/hash/proof — IntegrityBlocked; не skip/SyncComplete.
Candidate loader: Lane+Projection+Event, previous Lane, до двух parent
Projection+Lane checks =≤8 point gets. ≤k+2 candidate loads, plus identity/heads2:
**≤8(k+2)+2≤274 gets**,≤2 resident candidates; no predecessor-prefix/DAG traversal.
Все reads используют один read transaction/root; bounded proof cache освобождается
между candidate loads, чтобы не копить274×256 paths. Read transaction закрывается
до ответа/await; хранится только page output/cursor.

`history_point(context,id)` возвращает тот же Text row с проверкой metadata/Lane/source;
None означает «нет readable Text», не утверждение об отсутствии accepted Pending.
Bound view с частично отсутствующими expected records — integrity error, unbound raw
S1 context — HistoryUnavailable. Poisoned store — ReopenRequired. Finished page
пустая лишь при next==F; finished cursor можно повторять после append без новых rows.

## 4. Response codec и verification plan

Один reusable local DTO/encoder; CLI/IPC/network wiring отсутствует. Limit1..32.
CBOR unsigned/bstr/array profile, deterministic shortest heads:
`Page=[2,view_id32,F_A,F_B,finished0|1,cursor275,items]`,
`Row=[EventId32,role,n,sequence,request16,UTF8_bstr]`.
JSON ровно `{version,view,F,finished,cursor,items}`, item keys
`{id,role,ordinal,sequence,request,text}`; IDs/request lowercase hex, cursor Base64URL,
u64 decimal numbers, serde_json UTF8 escaping. Delivery status отдельно.

Max Row CBOR4170B, full32-row Page133775B≤256KiB. Worst4096 NUL text bytes →24576
escaped chars, JSON row24776B и full response793405B≤1MiB. Реальный полный encoding
проверяется перед выдачей cursor; overflow не публикует advanced cursor. Future
IPC envelope обязан проверить свой полный bound отдельно, не считать DTO за весь frame.

Модули: `indexed_dialog/history.rs` (read/write APIs), узкие `history_types.rs`,
`history_cursor.rs`, `history_codec.rs` и tests; common engine в indexed_dialog.rs.
Добавление private cursor KDF не меняет master provider/Root format. Предлагается
одна direct dependency `base64.workspace=true` (workspace0.22 уже есть): strict
canonical cursor text, без DIY Base64. CBOR — три private bounded emit primitives,
поскольку существующие push_* protocol crate-private; без подключения protocol/native
dependency ради encoder. JSON/blake3/AEAD прежние. Эти edits — только после review.

| Будущая проверка | Конкретный ожидаемый результат |
|---|---|
| Малый causal reference | Empty/genesis/one/two lanes, n1 vs genesis previous, implicit edge, cross-parent blocking, EventId tie-break; same order bounded topo reference |
| Writer integrity | holes/wrong previous/parent mapping/scope/Core span/unknown binding/version/cycle/conflicting IDs reject; pending не двигает F; generic bound-C writer bypass reject |
| H1 correspondence | Review trace old A1/R1 + new q2/R2 + empty Materialize; extra consume при valid row; один ticket/два rows, два tickets/один row; wrong request/role/n/ID/source; existing identical row как new — reject |
| H1 purpose/scope | Native-only/empty TextBatch; extra raw effect/ref; C1 operation с C2 effect/credit/ref/global Author owner; generic credit-only bound-C bypass — reject; exact last retry no-op |
| H2 init/repeat | Все строки state table §1.1, отдельно queued-only, pending-only+checkpoint, native-only, Seen/Request/owner-C Author; identity-only/head-only, bad codec/proof/count/tail/source, B0-only, foreign scope reject; active coherent repeat сохраняет ledger |
| Atomicity/credits | ≤4 mixed frames, R1 repeated native updates/refs, R2 receipt mismatch, quota+I/O ambiguity: old or new rows+heads+ledger вместе; last-id retry/reopen без повторной materialization |
| Cursor | page sizes1..32, every small boundary, append/reopen old exact suffix/no gaps/duplicates, new first видит append; altered/domain/C/incarnation/view mismatch reject; no hidden MVCC pin |
| Encoding | exact max UTF8/NUL/control chars/u64, strict Base64 and275B MAC codec, actual CBOR/JSON caps, malformed proofs не empty success |
| History100k | Новая fixture с двумя lanes/Text projections/Core-source claims/parents; тот же typed validator+commit, bounded test bulk within16MiB, не runtime importer;0 native E2E |
| Resources |1k/10k/100k real pages+point+ordinary append/resume/reopen; candidate/get/plan counters, real memory/cache/I/O; обе warm-фазы96/32,384/192/cache64, ratios≤4,total1200s |

Для всех validation rejects H1/H2 ожидаются byte-identical root/Queue/Pending/heads/
Native/ledger и неизменный C2; это будущие vectors, сейчас выполнялся только source review.

S1 raw100k не содержит typed projections/lanes/scope/parents; превращать его в ready
history нельзя. Он сохраняется для storage regression. Нужен один отдельный persistent
history100k build после реализации/малых timings; fixture reuse для следующих проверок,
без повторной генерации по кругу. Test bulk вызывает тот же per-row validator/atomic
plan, H1 bijection/single-C census и настоящие synthetic Queue/Materialize либо Pending/Readable ledger transitions,
с bounded count/actual bytes; native runtime API остаётся≤4. S1 Event-only timing
не доказывает timing трёх связанных history indices: это остаётся будущим измерением.
При ceiling/bound
failure — stop/analysis, не автоматическое повышение limits. Windows tests только
no-run+stable runner, CargoJobs2/BelowNormal/locked/offline; сейчас тесты не запускались.

Открытая граница — будущая реализация verified adapter и его native/protocol truth;
S2 не подменяет её MAC/metadata flag. Противоречия Root schema5/bounds не выявлены;
на review вынесены именно codec, managed writer fence, trusted DTO и direct base64 dep.
Wire2/Root2/migration/live runtime/GUI и secret getter не входят. A2 OPEN7 warnings /
strict advisory FAIL, deferred reproducer untouched. Frozen S1 artifacts, baseline
EXE d9b7ff… и foreign development-environment.md SHA330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798
сохраняются. После callback — остановка до review; commit/push не выполняются.
