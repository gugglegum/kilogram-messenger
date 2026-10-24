# A3-S3: повторное review и приёмка плана

2026-10-25, Asia/Yekaterinburg. Baseline `e136c59b829c6ecc5eb501273f25091e9b9e4789`.
Проверен исправленный [S3 plan](long-lived-dialog-a3-protocol-s3-plan.md): входная
версия 85514 bytes, SHA256
`3b149b5cc1f5716525945de48fb4a8c936fc470b66371e1d78bdcd8c0da7c836`.
После проверки root меняет только статус/ссылку на это review; окончательные bytes
плана и остальных документов фиксирует отдельный accepted-files manifest.

**План принят. R1–R3 из [review1](long-lived-dialog-a3-protocol-s3-plan-review-1.md)
закрыты на уровне контракта.** Сначала публикуется точный набор документации, затем
root отдельно поручает реализацию принятого component slice. Код и тесты S3 ещё не
написаны; приёмка документа не является доказательством работы протокола или MVP.

## Проверка исправлений

| Замечание | Принятое решение и независимая сверка |
|---|---|
| R1, genesis request | Wire/Core/ID остаются exact. Только внутренний HPB1 genesis request нормализуется в прежний нулевой sentinel после проверки Offer/Grant prefix. Text request не меняется. Bound2 reader повторяет semantic correspondence, в том числе после reopen и для parent/first/tail reads. Текущие Binding::check/encode/decode и unbound S2 fixtures остаются совместимыми. В matrix добавлены ненулевые genesis prefixes, correctly signed wrong-prefix rejection и повреждённая projection binding. |
| R2, authority evidence | Один resolver проверяет supplied/stored raw hashes, exact opaque certificate hash и role, missing/IO/Integrity, duplicates/unused inputs. O/A/F/R различаются; current-policy F/A остаются явно trusted adapter inputs. Новый Pending атомарно сохраняет O/A, старые pending/capture paths для bound2 закрыты. F получает immutable per-event Control link, R — exact receipt link. Renewal с тем же certificate допускается, old evidence не заменяется. ACK требует сохранённый/resolvable opposite credential, exact duplicates не дописывают revisions. |
| R3, differential set | Четыре bounded entrypoints: Event2, Receipt2, fresh proof и credential bundle. 19 field comparisons +18 подходящих malformed comparisons =37. Frame/Inner/existing/legacy проверяются в codec и не изображаются поддерживаемыми state parser формами. |

Root повторно сопоставил решение с `history_types.rs` Binding/request, `history.rs`
candidate/projection checks, `history_writer.rs` purpose/row/credit allowlists и
`credits.rs` Pending→Admission persistence/admission_paid. Дополнительные Control
records должны попасть в точный managed Plan/retention, а не в generic writer.
Таблица O/A/F/R устраняет ошибочное сравнение авторского AuthRef с credential receiver.

Сохраняется принятый A3 wire/Q/native-counter контракт. Native adapter и pinned
vodozemac не модифицируются, private pickle/debug parsing не вводится. Guards,
codecs, typed storage и test-only component driver имеют ясные границы доверия.
Предложенное отсутствие новой Cargo dependency edge допустимо.

## Обязательства реализации

1. Реализовать именно §1–§6 принятого плана: Codec2/static verification, bounded
   Guard2/preflight/checkpoint, closed storage correspondence и test-only CLI driver.
   Production activation, transport, migration, current-policy owner и UI остаются
   последующим частям A3; нельзя подключать обходной маршрут ради прохождения теста.
2. Parsed/structural/verified/native/current-policy состояния не смешивать. State
   resolver не удостоверяет Root signatures или revocation, caller DTO не является
   proof. Сохраняются H1/H2, immutable conflicts, shared Author fence и точные retries.
3. Проверки S07/A01–A04/D01–D09 входят в acceptance наряду с остальной matrix.
   N01/N02 используют настоящий native и authenticated storage, N03 — bounded pure
   import fixture. Их ожидаемые counts пока не выполнены и не заменяют whole-A3 gates.
4. Новый bound2 reader добавит correspondence/evidence reads. Старые S2 throughput
   цифры нельзя автоматически приписывать этому пути; heavy 100k run сейчас не нужен.
   Actual component timing/counts и failures фиксировать честно, без снижения bounds.
5. Windows tests: compile-only Cargo и stable runner, Jobs12/BelowNormal. Relevant
   targeted checks, Clippy/fmt/whitespace; никаких сетевых hash-named harness launches.

## Публикация документации

Принимаются только README, chat-coordination, S3 task/plan/review1/review2 из
`long-lived-dialog-a3-protocol-s3-plan-accepted-files.json` и сам manifest. Один
обычный commit с единственным parent `e136c59b829c6ecc5eb501273f25091e9b9e4789`,
затем non-force push origin/master. Frozen S1/S2 evidence/manifests/fixtures и чистый
development-environment.md не входят в diff. Root сверяет опубликованные paths/blobs
и remote, после чего передаёт implementation task.

Новые Rust harnesses для документации не запускались. Проверка локальных ссылок,
whitespace, exact input hashes и Git manifest выполняется перед dispatch.
