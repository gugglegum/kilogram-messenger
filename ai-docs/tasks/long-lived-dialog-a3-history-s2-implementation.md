# A3-S2 implementation — partial, performance precheck blocked

2026-10-24. **Код сдан на review как partial result, S2 не принят.** Small correctness
и S1 regression прошли. На1k/10k выявлено препятствие для1200s ceiling, поэтому
100k build не запускался по правилу остановки из S2 поручения. Limits/profile/cache
не повышались и не менялись ради результата. Product commit/push не выполнялись.

Baseline: `61524df3a77203d73673f47d38decf6b817615e3` (опубликованный S2 contract).
Основание: [task](long-lived-dialog-a3-history-s2-task.md),
[принятый plan](long-lived-dialog-a3-history-s2-integration-plan.md),
[review2](long-lived-dialog-a3-history-s2-integration-review-2.md),
[memory amendment1](long-lived-dialog-a3-memory-budget-amendment-1.md).
Исторические accepted contract/reviews/manifests/evidence не переписаны.

## Реализованный слой

Новые `history*.rs` рядом с `vault/indexed_dialog` разделяют typed DTO/codecs,
writer lowering, cursor, read API, tests и private scale fixture. Изменения S1 engine
ограничены общим commit purpose, generic fence, внутренним cursor KDF, private ticket/
pristine helpers, освобождением read proofs и buffer counters. Root schema5 и namespace
discriminants сохранены. Transport/CLI/IPC/GUI/wire2/native adapter не подключались.

- `initialize_history` и `commit_history`: закрытые Init/Genesis/TextBatch/QueueAccept/
  AcceptPending/CapturePending/RepairCredit/Acknowledge. Публичных raw effects/credits/
  refs/digest нет. Private validator проверяет обе стороны new-row↔consume, уникальные
  ticket/slot/EventId, source/Projection hashes, request/body/ticket, role и C. Старый
  Event не становится новым row. Request HRQ1 постоянен, FIFO по authenticated min Queue;
  enqueue q=expected generation, один QueueAccept на commit.
- Все generated keys/refs одного C, Author value/leaf owner совпадают с C. Generic
  fence учитывает ordinary mutations, credit-only paths, refs и действительный owner
  global Author. Unbound S1 raw API сохраняется; reserved history keys закрыты всегда.
- H2 init требует canonical-empty24 local summaries, отсутствие owner-C Author charge,
  пустые unused global roots и pristine ledger. Existing domain state не усыновляется.
  Repeat проверяет scope/view, heads, genesis/tails/source/counts/suffix и Native presence;
  active queue/pending ledger при этом не обнуляется. Это boundary validation, не full scrub.
- Event/Projection/Lane/heads/Native/request/receipt и credit переходы проходят общий
  S1 atomic engine. R1 committed-baseline/spent и R2 receipt-ordinal checks сохранены.
  Exact last-id retry не повторяет Native/consume; Init retry также проверяет coherence.
  Для older outcome доступны permanent `history_request`/`history_point`.
- `history_first/page/point`: два next candidates, signed parents +implicit lane order,
  minimum eligible EventId, frozen F. Нет prefix/DAG scan, order array, expiry, server
  session или MVCC pin между pages. Read proofs сбрасываются между candidate loads.
  Missing/corrupt expected remaining records дают error, не пустой успешный конец.
- Cursor MAC: private Zeroizing vault-derived key, full32B constant-time blake3::Hash
  equality, scoped incarnation/view/C/D/E/SID/F/next, strict Base64URL no-pad.
  Raw master/native/device secret getter не добавлен.
- Actual local CBOR/JSON encoders проверяются до возврата advanced cursor. Receipt
  payload —32B EventId marker, C/role/n заданы authenticated key/AAD; это storage
  evidence owner claim, не wire receipt/signature verification.

**Trust boundary:** PreparedReadable/Native/Auth — trusted local owner/adapter claims.
Storage проверяет hashes/span/typed links/atomicity, но не доказывает canonical Core
extraction, signature, native MAC/header/inner, plaintext или current policy. Это
задача будущего verified adapter. Synthetic fixture Core-shaped bytes и256B native
placeholder не являются реальными OD1/wire2/Olm frames: native E2E=0 во всех S2 runs.

## Dependency и actual codecs

Direct `base64.workspace=true` требует служебную lock edge. Первичный locked check
FAIL101 сохранён в [check-initial.log](long-lived-dialog-a3-history-s2-evidence/check-initial.log).
Root принял [точное уточнение](long-lived-dialog-a3-history-s2-lock-clarification-1.md).
Добавлена только `"base64 0.22.1"` в dependency list kilogram-state. Все736 package
entries/name/version/source/checksum и другие edges неизменны; unlocked Cargo не
запускался. [Машинная сверка](long-lived-dialog-a3-history-s2-evidence/lock-edge-verification.json).

| Encoding | Выполненный результат |
|---|---|
| Identity / heads / max binding / Request |177 /118 /255 /94B |
| Cursor binary / Base64URL |275B /367 chars, malformed/MAC/scope checks |
| CBOR max row / page32 |4170 /133775B; independent canonical max-row byte vector |
| JSON max row / page32 |24776 /793405B; actual JSON parse,4096 NUL bytes, u64 maxima |

`CommitReport.app_buffer_bound_bytes` и page stats дают консервативную верхнюю оценку
app buffers по live overlay lengths/capacities, запасу для sparse BTree nodes и
bounded input/plan/encoding. Это **не** измерение allocator attribution. Process
PrivateBytes/peak и redb cache data измерены отдельно без вычитания неучтённой памяти.
History path отвергает oversized app plan; production cache остаётся64MiB.

## Выполненные проверки

Windows: verification profile, debug assertions, Jobs2/BelowNormal, locked/offline,
`redb/cache_metrics`. Harnesses компилировались без исполнения и запускались только
через `scripts/run-cargo-tests-stable.ps1`, fixed EXE paths, test-threads1.
Команды/исходники/EXE hashes: [provenance](long-lived-dialog-a3-history-s2-evidence/build-provenance-final.json),
[commands](long-lived-dialog-a3-history-s2-evidence/commands.json).

| Проверка | Факт / raw evidence |
|---|---|
| State library locked/offline check | PASS, check-library-1.log |
| S2 small |14/14 PASS, small-tests-3.log: causal reference/all limits, snapshot append/reopen, point, scopes/MAC, actual encodings, H1/H2 public/structural/private negatives, own_role1/capture, generic global-owner fence, FIFO/request/retry/ACK, injected IO ambiguity |
| S1 components |16/16 PASS, s1-component-regression.log; включает growth, quota/IO и3 owned-child kill points общего engine, не новый S2/native kill corpus |
| S1 independent R1/R2 review tests |5/5 PASS, s1-root-regression.log; repeated/shared checkpoint conservation и matching receipt |
| Strict Clippy all state targets/cache_metrics | PASS, clippy-4.log |
| Workspace fmt / working diff whitespace | PASS, quality-final.json |

Сохранены исходные неуспехи: small-tests-1.log — две ошибочные `?` в тестах (6 compiler
diagnostics, harness не запускался); clippy-1.log — recursion depth большого diagnostic
json! и2 style findings; clippy-2.log — один test loop finding. JSON был разбит на два
объекта, compiler recursion limit не повышался. small-tests-2.log —11 промежуточных
PASS до расширения coverage; итоговые14 — в small-tests-3.log. Логи не переписывались.

## Persistent1k/10k и причина остановки

Одна новая `.tmp/a3-s2/history-small`, сохранена с10016 user Text rows (+genesis).
Обе Text lanes, source/text1024B, до2 signed parents, каждый16-й row local. Local
QueueAccept остаётся отдельным durable commit; fixture bulk≤128 использует тот же
private validator/atomic ledger и настоящие Queue/Materialize либо Pending/Readable.
Bulk entry point существует только cfg(test), runtime TextBatch≤4. Нет N-sized
expected vector: domain проверяется потоково, oracle держит только2 tails/frontiers.

| Метрика |1k|10k|
|---|---:|---:|
| Fill seconds |14.726|161.564 для8984 новых rows после1016 |
| Full warm traversal seconds |10.668|132.041 |
| Page32 p95 ms |340.885|451.749 |
| Point p95 ms |12.950|14.574 |
| Ordinary append p95 ms, acceptance+source |43.179|49.981 |
| Resume old suffix after append+reopen seconds |5.801|66.329 |
| Max candidates / point gets per page |2 /274|2 /274 |
| Max actual bulk plan bytes |986335|1496996 |
| Conservative app buffer bound bytes |25270144|27441491 |

Каждая фаза:16 ordinary appends, новая first видит новый F, старые cursors в3 местах
сохраняют прежний F после append/reopen; полный suffix от midpoint проверен без gaps/
duplicates, C2 summaries/ledger неизменны. Peak process paged bytes97,263,616B,
observed private bytes≤94,814,208B, cache data≤67,108,864B. Это наблюдения1k/10k,
**не** normative100k memory result. Реальный backend I/O сохранён:10k range читает
28,745,728B после reopen;1k query-phase reads=0, потому что7,156,041B уже прочитаны
при open и dataset помещается в cache. Эти cache hits не названы disk reads.

Полный small run399.617s. Raw [scale-small-1.log](long-lived-dialog-a3-history-s2-evidence/scale-small-1.log),
[parsed result](long-lived-dialog-a3-history-s2-evidence/scale-small-results.json),
[precheck](long-lived-dialog-a3-history-s2-evidence/performance-precheck.json).
Raw diagnostic `fill_commits` фактически означает cumulative instrumented calls,
включая ordinary appends. Точная интерпретация: fill71 и632 transactions, ordinary32
на stage; всего767, без setup. Исходный field/log сохранён, поправка явно отмечена.

**Не измерение100k:** линейная оценка только чтения =1320.4s; fill =1798.4s на100k
по последним8984 rows. Уже каждый из этих прогнозов отдельно превышает1200s, ещё
без snapshot-resume/metrics. Поэтому100k fixture не создавалась и не регенерировалась.
100k/1k ratios, warm raw96MiB/residual32MiB и итоговый100k gate — **UNAVAILABLE**.
Прошедший small test со status SMALL_TIMINGS_ONLY не подменяет full acceptance.

Узкое направление следующего repair на review: сократить повторную AEAD/proof/typed
value работу внутри одного bounded Work/read transaction, например capped verified
value cache с корректной invalidation/update на put/remove и global Author aliases.
Это предложение, не выполненная оптимизация и не доказанный CPU profile. Необходимо
сохранить H1/H2, clearing/bounds proof cache,192/384/cache64, R1/R2 и затем заново
получить small timings перед единственным100k build. Ни release switch, ни повышение
ceiling здесь не применялись.

## Сохранность и граница сдачи

[Preservation evidence](long-lived-dialog-a3-history-s2-evidence/preservation-final.json):
57 frozen S1 doc/report/evidence entries, оба manifest self и4 frozen S2 contract files
совпали. S1 source, нужный для общего engine, изменён намеренно; исторические source
blobs доступны из baseline. L1 EXE d9b7ff… и обе S1 DB2d48df…/f94e54… byte-identical.
Текущий development-environment.md clean SHA53074ff… после прямого human override;
удалённый dirty блок330ad3… не восстанавливался. Index пуст, HEAD остаётся61524df… .

Code/fixtures/evidence представлены для review; **S2 и весь A3/MVP не объявлены
готовыми**. Отдельного OS-process cursor restart test в S2 нет: проверены store
drop/open с повторным KDF и serialized cursor roundtrip; S1 child kill regressions
не подменяют этот факт. Wire/native/current-policy adapter, Root2/migration/runtime/
GUI/groups и A2 deferred reproducer не затронуты. A2 остаётся OPEN7/strict advisory
FAIL. После содержательного callback root — остановка до review, без commit/push.
