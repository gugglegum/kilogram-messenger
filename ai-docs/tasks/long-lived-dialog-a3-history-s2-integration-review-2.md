# A3-S2 integration review 2

2026-10-24. **Контракт принят к реализации: H1/H2 закрыты на уровне дизайна.**
Сначала executor публикует точный комплект S2 contract docs, root сверяет committed
paths/blobs и actual remote; затем тому же executor поручается S2 implementation.
Это вторая часть уже разрешённого A3, не новое разрешение на соседний backlog.

Проверен [исправленный integration plan](long-lived-dialog-a3-history-s2-integration-plan.md)
до изменения отметки приёмки: SHA-256
`536d23e2d5682a1ddaffcf46f332ed6ef5680742f0aa24602692bac0b0d5c32f`,
Git blob `8b3bd162df842064d8584cf51dece01081cfa935`.
Baseline HEAD/local origin/master `4d1d8b191f6e84c812c10ed3a2fa17a8dfa03629`.
Root заново прочитал весь исправленный contract, S1 namespace/summary/ledger layout
и [review1](long-lived-dialog-a3-history-s2-integration-review-1.md).73 frozen S1
file hashes (кроме развивающихся README/coordination), manifest self6517db… и
foreign development-environment.md330ad3… совпали; index пуст, product drift нет.

## H1: закрытая операция и двусторонняя связь

Публичный writer больше не принимает свободные effects/credits/retention refs.
Исчерпывающая matrix Init/Genesis/TextBatch/QueueAccept/AcceptPending/CapturePending/
RepairCredit/Acknowledge задаёт permitted effects; private lowering повторно
проверяет точное соответствие новых Text и consuming transitions. Existing identical
Event не является новым row, поэтому старый A1/R1 не может погасить новый q2/R2.
Unique ticket/slot/EventId и inverse-set equality закрывают extra/double consume.

Local row связан с минимальной Queue, её request/body и Request map; peer row — с
точным Pending source и matching receipt. Все generated keys/actions/refs проходят
один-C census, включая global Author owner/value. Generic credit-only mutation
bound/partial view также закрыта. Native изменяется только вместе с новым source
в Genesis/TextBatch; before_hash и ordered frame_ids связаны с этим переходом.
Правильность opaque native post-state честно оставлена будущему verified adapter.

Permanent Request HRQ1 имеет94B=4+1+8+16+32+33, укладывается в прежние128B.
Один QueueAccept на commit использует q=expected root generation: global generation
монотонна, записи другого C могут создавать промежутки, но не меняют FIFO order.
Queued/Materialized имеют canonical event presence, repeat сравнивает exact text
через Queue/Projection. Новая map оплачивается при enqueue ordinary quota.
Last-id retry проверяется до append predicates и не исполняет Native/consume снова;
для older unknown outcome используется постоянный lookup, не replay checkpoint.

## H2: fresh init и bounded repeat

Fresh predicate требует обе known keys absent, все24 target local summaries
canonical empty, zero owner-C charges в global Author, empty unused global roots и
pristine target ledger. Это соответствует текущему S1 layout:64 roots,24 local
slots на C, Author48..50, unused51..63. Другой активный C и его Author rows разрешены
и сохраняются. Root bootstrap не даёт права усыновлять старые Queue/Pending/Native/
Seen/Request/domain records; такой переход требует отдельной будущей migration.

Repeat сначала проверяет persisted identity/view, heads, genesis/tails/source,
lane counts и bounded suffix absence, aggregate Event/Projection counts и Native
presence, затем requested identity. Numeric canonical keys +count=F +absence keys
at/after F доказывают плотность slots0..F−1; проверка не превращается в O(N) scan.
Identity-only/head-only/corrupt/foreign stored scope дают integrity error.
Нормальный живой ledger при repeat не обязан быть pristine и не сбрасывается.
Это проверка границ, а не обещание полного scrub внутренних непрочитанных records.

## Условия приёмки реализации остаются обязательными

- Contract negative vectors H1/H2 должны стать tests реального facade/private
  validator: failure сохраняет Queue/Pending/heads/Native/ledger и C2; важна также
  точная retry semantics после успешного commit и ambiguous writer outcome.
- Сохранить S1 R1 final-minus-committed-baseline/spent и R2 receipt ordinal. Private
  common engine не должен превратить history purpose в generic bypass.
- Codecs identity177/heads118/binding≤255/cursor275/Base64URL367 и реальные
  response maxima CBOR133775/JSON793405 проверить выполненными encoding tests.
  Read path:≤2 candidates,≤274 point gets/page, no prefix scan/MVCC pin.
- Малые causal/reference/cursor/fault/scope tests проходят до одного нового history
  100k build. Настоящие typed lanes/projections/source bindings и credit transitions
  обязательны; raw S1 Event fixture не заменяет историю. Bounded test bulk использует
  тот же validator/ledger и не становится публичным runtime importer.
- Memory amendment1 применяется без ослабления:96MiB raw и32MiB residual вместе
  в двух warm phases,384MiB process/192MiB app/cache64MiB, ratios≤4,1200s ceiling.
  При провале — фактический report/analysis, без автоматического повышения limits.

Новые tests не запускались: проверена спецификация, product S2 ещё не реализован.
Trusted DTO не является доказательством wire/native verification; native E2E=0.
Direct workspace base64 допустим без обновления версии/lock; новые public secret
getters и protocol/native dependency ради encoder не нужны. Wire2/Root2/migration/
runtime/UI остаются вне этой части. A2 остаётся OPEN7 warnings/strict gate FAIL;
deferred reproducer не затронут. Frozen S1 evidence/fixtures и L1 EXE сохраняются.

После этого review в plan меняется только статус со ссылкой сюда. Exact final
raw hashes/Git blobs task/plan/reviews/README/coordination фиксирует отдельный
publication manifest. Executor: тот же OD1, GPT-6.1 Sol Max/default без Fast.
После publication callback root проверяет отправленный diff; product implementation
снова сдаётся отдельно на review до своего commit/push.
