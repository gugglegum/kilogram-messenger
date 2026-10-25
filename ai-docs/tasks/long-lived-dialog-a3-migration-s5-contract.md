# A3-S5: explicit V4 migration и handoff существующего L1

2026-10-25. **Контракт принят [root review2](long-lived-dialog-a3-migration-s5-contract-review-2.md).
Публикация — по exact accepted manifest; реализация начинается отдельным поручением root.**
Baseline — принятый S4 `0c1baa887008f784a86fd7ebabdcd5145fb4d1c0`, parent
`9547ed171c64bf5ea529b4ba487acf6aa1a61b5b`. Создаётся только этот документ.
[Задача](long-lived-dialog-a3-migration-s5-task.md),
[S4 acceptance](long-lived-dialog-a3-runtime-s4-implementation-review-5.md).
Нормативны [A3 §3](long-lived-dialog-a3-contract.md#L180),
[§7–9](long-lived-dialog-a3-contract.md#L551),
[A3 review2](long-lived-dialog-a3-contract-review-2.md),
[S4 §10](long-lived-dialog-a3-runtime-s4-contract.md#L727) и
[root provenance decision1](long-lived-dialog-a3-migration-s5-provenance-decision-1.md).
Исправления этой редакции отвечают на
[contract review1](long-lived-dialog-a3-migration-s5-contract-review-1.md); финальная
приёмка контракта ещё требуется. D1/D2/D4 направления приняты, D3 — в принципе;
R1–R4 disposition в конце документа. C600/F1200/P2400 остаются без исключений.
Уточняются оставленные storage/runtime seams; wire2, Native config1,
P/A/K/Committed, cuts и Q не переопределяются.

## 1. Рекомендуемое решение и вопросы review

Один forward-only upgrade **той же redb DB**, затем indexed L1 compatibility,
затем независимый existing handoff каждого C. Device secrets, Account/SessionKeys/
OTK reservation, original profile nonce, C/D/E/SID, Event/Receipt/request bytes и
долги сохраняются. Нового genesis при переносе или handoff нет.

| Решение | Предложение / статус |
|---|---|
| D1, discriminator | Новый authenticated MigratedRuntime7 внутри schema5, отдельный purpose/marker. ComponentRoot5 и fresh RuntimeRoot6 не переименовываются. Требуется full contract review. |
| D2, historical captures | Root decision1 принял migration-only LegacySnapshot с Recorded/NotApplicable/UnrecordedLegacy. Таблица применимости и запреты ниже — explicit amendment предпосылки A3/S3, а не реконструкция утраченных A/F. Конкретный encoding ещё предмет review. |
| D3, credits | Unused prepaid legacy capacity остаётся escrow; rolling pool64MiB явно финансируется один раз из quota. Release только unused escrow при Selected/Active после покрытия долгов. |
| D4, C2 | Storage copy DB-wide/offline: до Ready writable C2 также не запускается. После Ready per-C Offering/Prepared/Selected/blocked C1 не останавливают C2. Это граница A3 §7, не online copy одной части DB. |
| D5, стоимость | P<=2400s, F<=1200s, components<=600s. Real d9b511 seed угрожает F ceiling; до длинного run требуется короткая feasibility-проверка, при непомещении — решение root, не исключение setup из суммы. |

## 2. Source inventory, owner и preflight

Принимается authenticated schema4/ordered-dialog-v1, созданный поддержанным OD1
writer с config1. Источник — primary DB, не filesystem shadow. Только load-existing
master key; missing key/claim или wrong expected owner не создают identity.
Revoked/expired current permission не мешает сохранению допустимой старой истории:
static migration provenance не выдаёт live permission. После Ready C остаётся
AuthBlocked до допустимой current policy.

| V4 origin/phase | Перенос / продолжение |
|---|---|
| Enrolled,0 C | Сохранить enrollment/claim/keys, ReadyNoContexts; runtime без enabled C не стартует. Native отсутствует. |
| Registered C, Head phase0 Absent | Реальные отсутствующие D/E/SID/Offer/Grant/native остаются optional. Initial L1 handshake выполняется позднее. |
| A phase1 Offered | Exact Offer/account snapshot/key; нет Grant/D/E/SID/session/guard. Не вызывать fresh account повторно. |
| B phase2 Granted | Exact Offer/Grant/D/E, OTK reservation used=0; нет SID/session/guard. |
| phase3, incomplete genesis/Ready | Только coherent saved genesis/Native/Ready. Закончить initial L1 handshake; handoff=NeedHandshake до обоих readable genesis и Ready1. |
| phase3 Active, включая own.next512 | Полный перенос. L1 limit511 Text/сторону сохраняется; handoff с cut512 допустим. |
| Valid quarantine/signed-fork blocked C | Сохранить evidence/pending/block. Upgrade не лечит block; handoff не очищает его. C2 после Ready продолжает работу. |
| Fence-only / claim-less / orphan genesis / inconsistent phase | CreationConflict/UnsupportedSource/IntegrityBlocked до fence; не угадывать keys/phase/IDs. |
| Generic schema1–3, Component5, Runtime6, future schema | Не вход migration API. Никакого adoption/relabel/fallback. Runtime6 продолжает отдельный fresh S4 путь. |

Preflight под одним lifetime StateDirectoryLock:

1. OS lock до recovery/DB open: живой old/current owner => Busy. Проверить root,
   metadata/master key/path без symlink и без replacement identity.
2. Recovery dispatch различает schema/anchor **до** old journal handler. Для V4
   terminal v2 journal завершить по authoritative primary; unresolved prepared
   delta сначала требует однозначного recovery. Schema5/Root7 не запускает V4
   restore/rollback. Не fence незавершённый journal.
3. После recovery source read-only. Проверить old manifest/index AEAD+MAC,
   generation binding, mode/claim/cert/keys и inventory. Recovery может чинить
   derived shadows: cancel гарантирует schema4 и прежний protected logical
   primary, но не byte-identical filesystem bookkeeping.
4. Новый bounded streaming verifier вместо old open()->verify/load_records/
   restore_shadow с whole bodies в RAM. Index<=8MiB читается один раз/process или
   resume, row ограничен OD1 cap, total logical source<=384MiB. Нужный shadow
   recovery также streamed; после Ready runtime не получает paths()/full index.
5. Рассчитать target obligations, retained source/shadow bytes, configured
   capacity и actual redb growth/headroom по A3 §6.1. Недостаток => pre-fence refusal;
   quota не повышается автоматически. Status выводит pins/counts/hashes/capacity,
   а не ключи/pickles. Один lock/snapshot связывает inventory и Fence.

## 3. Проверяемые и сохраняемые факты

Каждая entry authenticated source index получает ровно одну классификацию.
Unknown path/type, чужой C/Device или unexplained extra row — ошибка, не пропуск.

| Источник | Проверка и destination |
|---|---|
| mode/claim/Device secrets/certificate/own и peer authority | Exact bytes; secret→public/cert/account/claim. Те же secrets в new owner material. Root revisions не понижаются. |
| specs/Consent1/initial-pair/Offer/Grant/Ready1/locators | Canonical/signature/roles/membership/transcript/D/E; immutable archive refs и typed specs/current records. Old locator — witness, не живой endpoint. |
| Head/Native1 | Phase/native_generation/blob hash, original nonce/AEAD/info tag, public account curve/config1/SID/SessionKeys/OTK по selected controls. Нет import arbitrary pickle. |
| Event1/lane | Exact EventId/signature/C/D/E/SID/role/n/author sequence/Core/header/parents/ClosedTail; bijection readable lane0..next−1 и source rows. |
| projection/request | Original self-HPKE info/AAD C/D/E/ID; UTF8/request соответствуют Core. Exact sealed projection сохраняется; derived display DTO не называется старым ciphertext. |
| queue | Sealed old bytes/request16/text/original generation/order, отсутствие source mapping до materialize; exact request/body retry. |
| pending/pending-state | Source/O/A/ordinary-head/quarantine/charge;64 ordinary+reserved head, no duplicate readable/pending ordinal. |
| outbox/receipts/ack-intent | ID/lane/n/status/signature/opposite Device/auth; Delivered только actual peer receipt. Недоставленный debt не исчезает, доставленный не создаётся заново. |
| authored/seen/next-sequence | Signer→sequence→C/ID, cross-C fork refusal; global next sequence exact. Guard1 seen сверяется с readable sources, pending не добавляет Native seen. |

### 3.1. V4Ref и completeness

Old records остаются в vault-records-v1; old encrypted index/generation metadata
retained. Intent связывает original manifest bytes, index cipher hash, generation/
generation-envelope hash, claim/nonce и inventory digest.

`V4Ref1=[1,source_id,record_key32,path,type,context?,cipher_hash32,content_hash32,length]`:
canonical bounded encoding, path<=256 и конкретный OD1 record cap. Ref находится
в новом authenticated value. Read проверяет key/cipher hash/length/old AEAD AAD,
decoded path/type/scope и index content hash/length. Произвольный external key,
shadow и whole-manifest verify на каждый ref access запрещены.

Inventory digest — ordered fold path/key/cipher/content hashes/type/scope.
Copy повторяет fold; digest/counts/cross-index cardinalities в конце совпадают.
Checkpoint содержит phase/last source key/position/processed digest/counts/output
summaries/source_id. Resume проверяет тот же index/source один раз; changed source
=> BlockedIntegrity, partial output не Ready.

Горячие Event1 raw и normalized history projection/binding копируются в обычные
indexed slots: normal page не добавляет legacy-table lookup. Cold auth/control/
original sealed projection могут иметь V4Ref. Старый backing immutable, sole refs
не удаляются. Retained/duplicated bytes/nodes оплачиваются явно, ссылка не делает
backing бесплатным.

### 3.2. Native provenance без replay

Migration witness создаёт только private verifier §2–3: exact source Head/native
hash/nonce/selected controls/genesis/Ready/inventory. S3 test OriginFact и pure
L1CheckpointFacts DTO не удостоверяют происхождение; live API не принимает blob.
Public restore/snapshot проверяют config1/SID/SessionKeys. Copy/handoff не вызывают
Native encrypt/decrypt, Account::fresh, OTK или outbound/inbound constructors.
Private pickle fields не парсятся. Новая local envelope не меняет inner session;
original raw native остаётся witness.

Fold readable prefix<=1024 events/C использует compact_l1_checkpoint/import_l1.
Порядок по двум frontiers/previous/parent/ClosedTail/Guard preflight, не hash-pages
и не timestamps. При конкуренции own continuation и нового peer DH first own
continuation должен идти раньше first: иначе утрачивается реальная Active/Inactive
provenance. Known-chain late tails могут идти после own new first. Нужен bounded
linear/topological fold, без exponential search/Native replay. Нет coherent fold —
отказ. Итоговые compact summary и оба seen sets совпадают с Guard1, включая
rotate_due/received_new_first/used_own_first.

L1 mutations сохраняют indexed seen mirror одним commit. На Selected проверяется
**текущий** Guard1/mirror, не blob из P: receive после cut законно меняется.
Guard1 growing set заменяется Guard2 refs атомарно.

#### Детерминированный two-frontier fold (R4)

Вход L[0]/L[1] — authenticated **readable** lane arrays, ровно ordinals0..next−1
из Head/Guard1, всего<=1024. Pending/quarantined possession в них не включаются.
Каждая meta уже связана с exact signed source/header/ID/scope и допустимыми
user parents. Словарь ID→(role,n,kind) строится только из этих arrays; отсутствующий
parent/cause/ClosedTail ID не достраивается из pending, timestamps или догадки.
Phase без session/Guard1 проверяется своей Native1 формой без изготовления empty
Guard2; для native_phase3 с session/Guard1 допустимы и неполные genesis frontiers.

```text
expected_checkpoint := compact_l1_checkpoint(authenticated_Guard1_facts)
g := Guard2::new(local_role)
pos := [0,0]; seen := {}; emitted := {}; ordered := []
while pos != [len(L[0]),len(L[1])]:
    ready := []
    for r in [0,1]:                        # ровно два кандидата
        if pos[r] == len(L[r]): continue
        (m,id) := L[r][pos[r]]
        lane := g.own if r == local_role else g.peer
        if m.n != lane.next or m.previous != lane.last: reject(source-prefix)
        deps := m.user_parents + optional(m.expected.parent)
                + optional(m.expected.tail.last)
        if any dep not in emitted: continue
        obs := exact seen[(r,m.key)] or Absent
        t := g.clone()                      # bounded Guard2, не Native
        step := t.preflight_send(m) if r == local_role
                else t.preflight_receive(m,obs)
        if step is invalid: continue
        if t.apply(step,m,id,obs) is invalid: continue
        cont := lane.next>0 and Some(m.key)==lane.key
                and m.expected.g==lane.g and m.expected.tail is absent
        ready.push((r,m,id,t,cont))
    if ready is empty: reject(no-progress/dependency/native-phase)
    continuations := ready where cont
    if continuations not empty:
        take := own candidate from continuations if present
                else peer candidate from continuations
    else:
        if len(ready) != 1: reject(inconsistent-two-native-firsts)
        take := ready[0]
    g := take.t; pos[take.r] += 1
    seen.entry((take.r,take.m.key)).or_insert(
        generation=take.m.expected.g, first=take.id)
    emitted.insert(take.id); ordered.push(take.m,take.id)
require pos == [len(L[0]),len(L[1])]
require g.encode() == expected_checkpoint
require keys(seen[r]) == authenticated_Guard1.lane[r].seen for r=0,1
require exact generation/first for every seen key from these source rows
require import_l1(local_role,ordered,expected_checkpoint) gives same g/seen
```

В pseudocode preflight/apply — summary-only adapter существующего Guard2; root/native
persistence-generation fences проверяются отдельно на настоящих Head/commit. Это
не обход final gate и не вызов native encrypt/decrypt. Expected.parent и ClosedTail
проверяются самим preflight сверх наличия deps; user parent должен быть допустимым
Text того же C/E, genesis user parents пусты. Одного hash membership недостаточно.

Почему tie безопасен: own continuation при доступном новом peer first обязана идти
раньше first — получение first делает rotate_due и такая continuation уже не могла
бы быть создана. Если continuation требует ещё не emitted parent, она не eligible:
правило приоритета не перескакивает dependency. Peer known-chain continuation меняет
только peer index/last/sequence, не first/g/rotate/cause; её можно поставить перед
eligible own new first, даже если физически она пришла позже. Это commuting summary
steps, не утверждение о chronology Native. Две continuations при выполненных deps
коммутируют; фиксированный own-first tie делает результат воспроизводимым. Два
non-continuation candidates одновременно не eligible в valid Active/Inactive turn:
новый peer DH в Inactive запрещён, own new DH требует предыдущий peer first,
Confirm требует Init. Нарушение — integrity rejection, не выбор через DFS.

Каждая итерация потребляет один source, максимум два bounded trials, O(N) work и
O(N) metadata<=1024 без Native replay. No-progress и несовпадение **всех** финальных
key/g/index/first/last/parent/sequence/rotate/cause/seen fields дают отказ.
Малые C cases: несколько own continuations до нового peer first; late peer
continuation после own new first (в том числе с user parent на него); excluded
pending64; A0-only/both-genesis/no-Ready prefixes; одинаковые next/g/index при
invalid cause/ClosedTail/user dependency. Ни один не требует full511 run.

### 3.3. Explicit amendment: LegacySnapshot provenance

Root [decision1](long-lived-dialog-a3-migration-s5-provenance-decision-1.md) подтверждает:
[pending](../../apps/kilogram-cli/src/ordered_dialog/ingress.rs#L204) имеет O/A,
[readable](../../apps/kilogram-cli/src/ordered_dialog/ingress.rs#L325) удаляет
pending-state без per-event A/F links, а [projection](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L86)
их не содержит. Catalog доказывает bytes, не роль bundle в данном событии.
Нельзя выбирать A/F по current pins, времени, одному оставшемуся bundle или cert.

Capture tag различает **Recorded(ref)** (реальная exact ссылка и проверенный bundle),
**NotApplicable** (роль не применяется к данному типу/фазе) и
**UnrecordedLegacy** (роль применима, но old writer не сохранил связь).
Delivery state отдельно различает awaiting opposite receipt и Delivered.

| Frozen source row | O | A | F | R / обязательная доставка |
|---|---|---|---|---|
| Own Text, undelivered | Recorded | NotApplicable | Recorded при exact persisted link, иначе UnrecordedLegacy | NotApplicable на snapshot; отдельный AwaitingReceipt debt обязателен. R не потерян, его ещё не было |
| Own Text, Delivered | Recorded | NotApplicable | Recorded либо UnrecordedLegacy | Recorded opposite Receipt1+bundle обязательны; missing => Blocked |
| Peer already-readable Text | Recorded | Recorded при exact witness, иначе UnrecordedLegacy | Recorded либо UnrecordedLegacy | Recorded own durable Receipt1+R bundle/ACK intent по source state; missing обязательный receipt => Blocked |
| Own genesis | Recorded source/initial-pair bundle | NotApplicable | Recorded либо UnrecordedLegacy | NotApplicable: genesis не user-message ACK |
| Peer readable genesis | Recorded source/initial-pair bundle | Recorded при real link, иначе UnrecordedLegacy | Recorded либо UnrecordedLegacy | NotApplicable; Ready1/genesis gates проверяются отдельно |
| Peer pending Text | Recorded | Recorded exact pending-state A | NotApplicable: readable transition ещё не было | NotApplicable; possession не readable/Delivered |
| Accepted queue без event | NotApplicable для ещё не созданного event | NotApplicable | NotApplicable | NotApplicable, request остаётся Queued |

Все реально сохранённые links валидируются и сохраняются, их нельзя заменять Unknown.
Core O разрешается из exact persisted bundle, включая cryptographically bound
initial-pair bundle для genesis, если hash/ref совпадает; не из guessed current.
Missing обязательный O/source/signature/bundle, projection/Head/Native/Seen mismatch,
corrupt AEAD/index/fork или missing обязательный receipt остаются Blocked.

`LegacySnapshot1` связывает migration incarnation/source manifest hash+generation,
source_id и exact membership pre-fence inventory: C/role/n/ID/request плюс verified
old Head/Native/readable-prefix/source/projection facts и capture tags. Создаётся
только typed MigrationCopy/Verify lifecycle, **закрытым после Ready**. Нельзя создать
его generic/history/live/IPC/JSON/peer путём, через null refs или corruption fallback
full H2 row. Это local migration evidence, не historical A/F certificate.

History/point/cursor reader имеет явный bounded variant для LegacySnapshot, без
per-read V4 scan; proof kind доступен internal audit/status. Text/IDs/requests/cursor
semantics те же. Old pending не превращается в readable snapshot: O/A переносятся,
последующий actual drain после Ready пишет реальные F/R. Все новые indexed L1/mixed
transitions после Ready записывают полный **применимый** capture set.

Late ACK может добавить отдельный exact receipt/delivery record к imported own
source, но не переписывает immutable snapshot tags и не повышает его исторический
A/F proof. Existing durable receipt не заменяется. Marker не даёт current policy/
lease/cut/Native restore permission, не создаёт ACK из projection, не синтезирует
Delivered и не запускает повторный decrypt/re-encrypt readable Text.

## 4. Layout7, namespaces и version fences

Предлагаемый root plaintext:

```text
0x07 || ASCII("KGOD2MG\0") || u16be(1)
|| exact Root6 common-field order:
   incarnation,account,claim,signing32,encryption32,context_slots[2],
   generation,commit_id,request_digest,roots[64],ledger[2]
```

Это MigratedRuntime7, **не schema7**: outer manifest schema5, root AEAD derivation/
envelope/sentinel digest и table names прежние. Component5/Runtime6 bytes/markers,
fresh Native2 payload/AAD не меняются. Prefix7 отвергается exact S3/S4 Root::decode
после правильного AEAD, до mutable repository. Новый binary распознаёт каждый
layout явно; failed decode не вызывает попытку другого codec.

- Новый Namespace::Migration=23: global slot51, context0 only, account-scoped AAD,
  Intent/CopyCheckpoint/Ready/source inventory/ref witnesses. Mapping explicit,
  не формула Author slots. Runtime6 требует slot51 пустым.
- Runtime slots21/45: Head7/Mode7/current pointers; profile slot22:
  Profile7 с required migrated-origin marker/source_id/occupied slots/saved nonce.
  Marker существует уже в первом committed root, не добавляется потом.
- Namespace::LegacyBudget=24: explicit per-C slots23/47, singleton key bytes0,
  owner=c. Это не формула c*24+24, которая столкнулась бы с Event другого C.
  Runtime6 по-прежнему требует23/47 пустыми; Root7 требует один budget leaf на
  occupied C. Global Migration23 имеет slot51, это другой namespace/purpose.
- Existing Event/Lane/Projection/Request/Queue/Pending/Receipt/Seen/Auth/Native/
  Author discriminants сохраняются. Migration/mixed bindings имеют explicit tag7.

Root.claim равен original profile claim nonce; incarnation — новый storage ID,
создаётся один раз при fence и не меняется на resume. Source C сортируются по C
в occupied slots. При0/1 C свободный **storage partition ID** выводится domain hash
из source_id/slot, отмечен unoccupied; это не новый C/D/E/SID/native domain.
Операции требуют occupied=true до Native/commit. Profile7 допускает<=2 peer pins:
V4 два C могут иметь разных peers; own+до двух peer signers укладываются в прежние
<=3 Author slots. Current account revocation инвалидирует все затронутые C leases,
а per-C consent — только этот C.

Root7::check проверяет purpose/marker/Intent/source_id, phase-dependent cardinalities,
occupied slots/namespace/credit correspondence. Generic/component/history facade
и retry shortcuts отклоняют purpose **до** lowering. Только closed MigrationOperation7
и RuntimeOperation7 меняют Root7. S5 binary открывает Runtime6 по unchanged fresh
пути; implicit6→7 conversion отсутствует.

| DB / binary / путь | Live owner | Cold path |
|---|---|---|
| V4 + d9b/S4/S5 profile1 | Busy второму | Обычный L1; explicit storage upgrade отдельно |
| V4 + fresh enroll2/runtime2 | Busy | ExistingStateRequiresMigration/CreationConflict до mutation |
| Copying/Ready Root7 + d9b local/standalone | Busy | schema5/account fence refusal до Native/trust/history write |
| Root7 + exact S3 schema5 helper | Busy | Correct owner/config, AEAD и затем prefix7 refusal до writer |
| Root7 + exact S4 runtime helper | Busy | Correct owner/config, AEAD и prefix7 refusal; не отказ из-за неверного profile JSON |
| Root7 + S5 generic/component/fresh-only facade | Busy/purpose refusal | RuntimeWriterRequired/UnsupportedRuntimeProfile до commit/fast retry |
| Copying Root7 + S5 | Busy второму | Только MigrationAdmin resume/status/old read-only history; не network runtime |
| Ready Root7 + S5 Profile3 | Busy второму | По authoritative per-C L1/Offering/Prepared/Selected/Active |
| Component5/Runtime6 + migration command | Busy | UnsupportedSource/AlreadyFreshRuntime; не adoption |
| Missing/corrupt marker/magic/revision/suffix/future root | Busy либо fail closed | Никакого mutable fallback/relabel/Native; reader тоже не угадывает layout |

Fence заменяет **только** encrypted runtime/od1/profile-mode на
`kilogram:ordered-dialog:v2\0` в той же DB transaction. Original mode plaintext+
encrypted envelope/key/hash сохраняются в Intent как единственный source override.
Original index MAC остаётся прежним: восстановление только old manifest/header до4
обнаруживает changed mode row. ASCII account fence сохраняется. Profile/header
relabel не является supported downgrade. Original mutable source rows после fence
не пишутся, поэтому других override нет.

### 4.1. Exact local record domains и lifecycle

Новые local payloads используют тот же canonical unsigned/bstr/array subset,
shortest encoding, optional []/[x], no text/map/null/trailing; это private state
codec, не новая wire версия и не protocol dependency. Wrapper:
`ASCII("L7R\0") || u16be(1) || kind:u8 || payload_len:u32be || enc(payload)`.
Body<=512KiB, checkpoint<=8KiB, Intent<=24KiB, LegacySnapshot<=4KiB; caps проверяются
до allocation/decode. Существующий authenticated leaf overhead/used/reserved
accounting оплачивает wrapper и prepaid capacity, а не только payload.

```text
SourceIdentity1 = [1,manifest4_raw,index_cipher_hash32,generation4,
                  generation_envelope_hash32,claim4_raw,account32,device32,
                  inventory_digest32]
source_id = BLAKE3("kg:od2:migration-source\0" || enc(SourceIdentity1))
migration_id = BLAKE3("kg:od2:migration-instance\0" || enc([source_id,incarnation]))
unused_slot_id = BLAKE3("kg:od2:unused-storage-slot\0" || enc([source_id,slot]))
Migration record key = BLAKE3("kg:od2:migration-record\0" ||
                             enc([migration_id,kind,logical_key_bstr]))

kind1 Intent7 = [7,SourceIdentity1,source_id,migration_id,
                [mode_record_key32,original_mode_envelope,original_mode_plain],
                initial_config_digest32,identity_config_digest32,occupied_mask]
kind2 Progress7 = [7,migration_id,stage,work_phase,last_index_position,
                  last_path?,processed_digest32,counts,output_effects_digest32,
                  reason_bstr]
counts = [total_entries,[[readableA,readableB,pending,head,queue,receipts,refs],
                        [readableA,readableB,pending,head,queue,receipts,refs]]]
kind3 Ready7 = [7,migration_id,inventory_digest32,verified_output_digest32,
               current_pointer_set_digest32,retired_source_set_digest32,
               profile_publication_spec_digest32]
kind4 Ref7 = V4Ref1
Capture = [0,exact_ref32] | [1] | [2]  // Recorded / NotApplicable / UnrecordedLegacy
kind5 LegacySnapshot7 = [7,migration_id,source_ref_id,C,role,n,EventId,request16,
                        head_witness_ref,native_witness_ref,projection_ref?,
                        [O,A,F,R],snapshot_delivery]
snapshot_delivery = 0 genesis | 1 own-awaiting | 2 own-delivered | 3 peer-readable
```

Stage:0 Copying,1 Verifying,2 Paused,3 Blocked,4 Ready. Work_phase:0 refs/hot rows,
1 cross-indices,2 Native/Seen,3 final-ready; pause/block сохраняет work_phase и
checkpoint. reason<=1024 bytes. Source refs/classification и witness membership
immutable; mutable Progress допускается только migration purpose до Ready.
Ready idempotent outcome можно прочесть после IO/reopen, но Copy/Verify/новый
LegacySnapshot после Ready=>LifecycleClosed **до retry shortcut**.
Snapshot capture tags принимаются только по таблице §3.3/source state; arbitrary
tag2 для O/R или заменённый delivered flag отвергаются.

Head7 разделяет native_phase0..3 и LocalMode7. UpgradeIntent7 — отдельный per-C
record/flag с request/selected-current-consent ref; **Intent не wire stage**.
Полная projection LocalMode7→accepted Mode2 строится named conversion, не raw cast:

| LocalMode7 / intent | Mode2.stage | Q / cuts |
|---|---:|---|
| L1, intent absent |1=L1| Q absent, оба cuts absent |
| L1, UpgradeIntent7 present |1=L1| Q absent, оба cuts absent |
| Offering, только A |2| Q absent, [cutA,absent] |
| Prepared, только B |3| Q absent, [cutA,cutB] |
| Selected, только A |4| Q present, оба cuts exact certificate |
| Active2, A либо B |5| Q present, оба cuts exact certificate |

LocalMode7 хранит named variants и exact optional control/proof/Committed refs,
cuts/monotonic peer-source tuple; numerical encodings1..5 проверяются closed codec.
Wire0=FreshHandshake остаётся **только** fresh Runtime6. В MigratedRuntime7 ни
отсутствие intent, ни incomplete Native phase0–3 не проецируются в wire0.
Для incomplete L1 bootstrap local/status projection всё равно stage1/no-Q/no-cuts,
но реальный ALPN2 Existing Hello/Resume/Head запрещён до полного Ready1/genesis/
D/E/SID: продолжаются ALPN1 OpenIntent/initial handshake и прежняя bounded queue.
P/A/K/Committed и Text native materialization до L1 readiness недоступны.
Unoccupied slot/Copying DB не имеют сетевой projection вообще.
Send freeze derived из Offering/Prepared/Selected, не caller boolean; cancellation
удаляет только UpgradeIntent7 в L1 до P. Role-stage/Q/cut shape проверяется до wire
serialization и при peer parse, поскольку ModeView2::parse проверяет лишь числовой cap.

HistoryBinding7 — отдельный version7 closed record: immutable HVI1 scope/view,
source_id, cert refs/role→Author mapping, migration inventory digest; exact mode
certificate/Q устанавливается отдельным immutable control record. View не зависит
от Q. Post-Ready rows используют full captures и explicit format, snapshot rows —
только kind5 membership. Reader не дополняет unknown поля full H2 записью.
Global migration metadata/backing charges относятся к slot0 quota; per-C hot rows/
tickets — к своему C, global shared Author/auth ownership дедуплицируется явно.
LegacyBudget7 kind6/EffectiveConfig7 kind7/SourceBacking7 kind8 и summary enforcement
определены в §6.1–6.2. Они не меняют common Ledger/Summary codecs Root6/Component5.

Digests не самоссылочны: output_effects_digest — fold typed produced key/value
hashes/used/reserved **без Progress/Ready**, не root hash содержащего их дерева.
Ready verified_output/current_pointer/retired-set digests также не включают Ready.
Profile publication spec содержит pins/config/paths/source_id/migration_id, но не
hash root, который сам включает этот spec digest. Actual full root/sentinel digest
после вставки metadata удостоверяется штатным anchor, отдельного hash cycle нет.
History reader проверяет небольшой authenticated Head/Native-origin witness и
membership refs; не восстанавливает Native и не читает128KiB pickle на каждую row.

## 5. Atomic migration write sets и recovery

Один DB/lock, Durability::Immediate, plans<=16MiB, lastCommitId/requestDigest/outcome
resolution как S4. Нет second OrderedVault/Native workspace. Source index читается
один раз на process/resume; ordinary append/page не делают global scan.

| Переход | Один redb commit | Kill/IO/retry |
|---|---|---|
| Fence7 | schema5 sentinel+AEAD Root7/required marker+immutable Intent/source witness+mode5 source override+reserved migration/legacy obligations; generation=checked source_generation+1 | До commit recovered V4; после Copying/forward-only. Ambiguous IO: reopen под тем же lock, inspect source_id/stage/commit_id, не повторять random incarnation/fence |
| CopyBatch | <=32 source entries,<=1MiB input; refs/hot copies/derived indices/charge/retention effects+cursor/digest/counts/output summaries/exact operation ID | Exact position/digest/output retry=>AlreadyCommitted; changed bytes=>Conflict. Нет current Native publication. IO poison/reopen, не trim/delete suffix |
| Verify/Ready7 | Final digest/cardinalities/completeness/Guard1-seen/projection/receipt checks, current indexed Head1/Native1/queue/pending/outbox/credits pointers, retirement old current pointers, Ready/source_id/profile publication spec | Stream verify до final transaction; root/generation/witness recheck внутри. Неполный copy никогда Ready. Source только witness, не loader fallback |
| ProfilePublish | Только после Ready: external create-new profile, writable temp+sync+atomic publication | Crash=>ReadyNeedsProfile. Resume/profile-create сверяет owner/device/source_id/exact desired bytes, не генерирует keys и не откатывает DB |

Completeness может иметь несколько maintenance verification batches с checkpoint;
Ready проверяет их exact root digests/source generation, а не создаёт huge transaction.
До Ready source immutable и Native writer не существует. Output digest/cardinalities
не соответствуют preflight inventory => Ready недоступен.

Retirement — атомарная смена authoritative namespace/current-purpose registry на
indexed pointers. Original Head/native/queue/outbox bytes могут остаться backing,
но не current fallback. L1 continuation меняет только новые indexed slots; V4
per-message journal/shadows не создаются. Удаление derived duplicates/backing —
будущий retention с проверкой sole refs, не скрытая часть S5.

До fence pause/cancel=>PreflightCancelled, schema4. После fence pause завершает
лишь текущий atomic batch=>MigrationPaused, forward-only resume. Corruption/quota/
capacity=>MigrationBlocked с phase/checkpoint/reason. Old read-only history идёт
через original authenticated index/source override, не incomplete derived output:
под lock после остановки copy либо через того же MigrationAdmin owner. Broken
requested row=>HistoryUnavailable/IntegrityBlocked, без shadow fallback.

## 6. Credit reconciliation без искусственного refill

V4 не имел rolling ledger. Его [validate_reservations](../../crates/kilogram-state/src/vault/ordered.rs#L562)
prepaid на C:128MiB lifetime+4MiB controls+1MiB queue+2×4160KiB pending/commit
workspace=141.125MiB. Global/retained source/shadows учитываются отдельно.

Ready calculator строит из inventory U_liveLegacy, actual queue/pending/head/
undelivered/receipt tickets, auth/ref ownership, remaining reservation и retained
bytes. Один calculator для preflight/copy/Ready; caller не задаёт ledger totals.
Existing queue оплачивает128KiB/frame; pending128KiB+64KiB temporary minus already
paid capture; opposite receipt debt не меньше48KiB; отдельный head64KiB. Full64+head
не заменяется64 всего. Потребление actual source не имитируется grant/refill.

D3 реализуется через budget leaf (§6.1), с отдельной формулой для каждого c:

```text
LegacyHold_c = max(0,B4-covered_used_c-covered_reserved_c) while Held; else 0
need_c = SUM all64 used[c] + SUM all64 reserved[c]
         + repair_free_c + unissued_grant_c + unused_head_temp_c
# reserved[c] уже включает LegacyHold_c и pre-Ready endowment E_c
need_c <= effective_logical_quota_c
repair_free_c + unissued_grant_c + outstanding_repair_c <= 64MiB
```

B4=141.125MiB — imported reservation policy. Attribution каждой строки/ticket
фиксирует legacy/current origin, charged owner C/shared refs: одна оплата не
считается дважды. Суммы U/R включают retained source/index attribution, новые
copies/nodes/controls; shadows учитываются отдельно в physical capacity.
Native/control prepaid capacity входит R, не бесплатный
буфер. Освобождённый до handoff legacy debt возвращает свободное внутри B4 в
LegacyHold, не продаёт его new writer/другому C.

Repair64MiB endowment выделяется явно **один раз при Ready** из quota сверх остальных
обязательств. Это новая оплаченная reservation, не refill расходованного pool и не
Ledger::new при reopen. Imported pending не получает fictitious repair grant:
completion оплачена source obligations, ordinary/head counts/origin сохранены.

L1 сохраняет own.next+accepted_queue<=512, queue<=128/unacked<=128 и lease для
native mutation. Exhausted511/own.next512: новый request UpgradeRequired; старый
request retry/history/receive/repair/ACK работают. Во время Offering/Prepared/Selected
freeze только materialization; queue может приниматься по прежним L1/R3 bounds
с current допустимой policy. Расширенные limits/offline encryption — лишь Active2.

При A Selected/B Active сначала покрываются existing queue/pending/head/old receipt
debt, Native/metadata/new repair obligations; освобождается **только LegacyHold**,
то есть unused часть. U/R не списываются как unused. A Selected оставляет очередь
оплаченной, но не materialize до Committed. Late Receipt1 снимает exact debt один
раз; Complete/possession этого не делают. Pool уменьшается на retained costs;
refill — отдельная оплаченная операция, не эффект reconnect/K/profile resume.

Quota ниже need=>отказ без удаления rows. После increase physical/logical capacity
reopen/resume/exact retry того же source_id продолжается. Paused reader/status
использует stored valid root, не требует writer-open с недостаточной новой config.
Physical accounting включает full old DB, retained shadows и requested redb growth:
логическая ссылка не отменяет physical liability/production headroom.

### 6.1. Per-C LegacyBudget7 — реальная leaf reservation (R3)

Чтобы не менять S1 Summary/ledger encoding, escrow хранится **в reserved поля leaf**,
а не только payload. New private legacy_budget_put не использует Runtime6
runtime_put_key с fixed reserved total. Для occupied c единственный
`Key(c,LegacyBudget24,zero32)` лежит в root slot23+24*c, имеет owner=c и kind6:

```text
LegacyBudget7 = [7,migration_id,c,B4,hold_state,covered_used,covered_reserved,
                 legacy_hold,repair_funding,repair_start_reserve,
                 released_at_generation?,release_Q?]
hold_state = 0 Held | 1 Released
repair_funding = 0 ReservedBeforeReady | 1 FundedOnce
B4 = 147980288 bytes                         # 141.125MiB
M = 4096                                   # fixed metadata capacity
q = encoded L7R(kind6) length, q<=M
leaf.charge = q + 41 + 32 + 2*NODE_CAP       # существующий charge(), NODE_CAP=256
leaf.reserved = (M-q) + legacy_hold + repair_start_reserve
budget_summary.used[c] = leaf.charge
budget_summary.reserved[c] = leaf.reserved
budget_summary.count = 1; other-owner used/reserved = 0
```

Namespace24 mutable только в closed budget lowering; arbitrary runtime_put или
generic mutation не получают этот ключ. Namespace23/global51 требует отдельного
account-scoped lowering: существующий global Author-only branch с value64B нельзя
распространить на Migration простым изменением global(). Namespace/owner/cap checks
явно различают эти варианты; Component5/Runtime6 сохраняют прежние dispatch rules.

Таким образом budget metadata всегда M+585=4681 bytes, независимо от размера CBOR
integer; **legacy_hold уже входит в Root::totals().reserved[c]**. Его нельзя снова
прибавлять поверх R_c в Ledger::check. Точная source attribution:

```text
D = {Event,LaneA,LaneB,Projection,Request,Queue,Undelivered,
     ReceiptA,ReceiptB,Pending,SeenA,SeenB,Auth,Native,Admission}
covered_used_c = SUM root[c*24+namespace].used[c] over D
covered_reserved_c = SUM root[c*24+namespace].reserved[c] over D
L_c = max(0,B4-covered_used_c-covered_reserved_c) if Held else 0
U_c = SUM all64 summaries.used[c]
R_c = SUM all64 summaries.reserved[c]          # ВКЛЮЧАЕТ budget leaf M-q+L_c+E_c
P_c = repair_free_c + unissued_grant_bytes_c + unused_head_temp_c
need_c = U_c + R_c + P_c <= effective_quota_c
repair_free_c + unissued_grant_bytes_c + outstanding_repair_c <=64MiB
```

До release все source/data в D — format1: новый format2 в Held state запрещён
closed purpose. Оплаченный data leaf/ticket учитывается в covered ровно один раз
через свой namespace summary; отдельно суммировать его source/ref/receipt counters
запрещено. Сам LegacyBudget, Runtime/Control/Current/Profile/Credit/global Author
и global Migration51 **исключены** из covered. Это сознательно консервативный B4:
их настоящие U/R оплачены сверх escrow, не маскируются как свободная legacy capacity.
После Selected/Active формат смешанный и covered перестаёт определять L: оно0;
сохранённые release covered numbers служат audit, не разрешением убрать actual U/R.

Charged owners: hot row/ticket/source copy c — owner=c; global source index/original
backing liability/small migration metadata — owner0; global shared Author/auth row
имеет одного persisted owner/ref accounting. Physical file/shadow liability входит
configured physical capacity **один раз на файл**, отдельно от logical leaf charge.
Global source backing в slot51 не уменьшает ни covered_0, ни тем более covered_1.
Reference у c1 оплачивает лишь свой actual leaf; global body у0 не заменяет escrow c1.
Quota1 нельзя обойти низкой quota0 или общей суммой двух quotas. Metadata часть4681
входит control budget, L/E — logical liabilities, не огромный in-memory control buffer.

Kind8 SourceBacking7 — один global accounting leaf в Migration51, owner0, key по
§4.1(kind8,empty logical key), raw<=4KiB:
`[7,migration_id,source_id,retained_entries_digest,entry_count,retained_logical_bytes]`.
retained_logical_bytes — сумма actual key+value lengths retained legacy table entries
в той же DB (source rows, old index/generation bindings и replaced mode-fence entry),
каждая entry ровно один раз. Original manifest/mode bytes, уже перенесённые только в
Intent payload, оплачены его actual leaf и вторично в backing sum не входят.
q8=encoded L7R(kind8) length; leaf.charge=q8+585,
leaf.reserved=(4096−q8)+retained_logical_bytes. Остальные V4Ref leaf оплачивают
только свои actual metadata/prepaid capacity, без второй reservation того же backing.
Fence устанавливает этот неизменный inventory-derived digest/sum; copy/Ready сверяют
membership, каждый Root7 load/commit сверяет singleton/owner/charge/reserved с payload.
Sole backing refs не удаляются/не GC этим S5. Physical redb overhead/growth/shadows
не вычитаются из L и остаются отдельной actual-file/headroom проверкой §2/§6.

Каждый Root7 load (включая clean reopen и capacity retry), до получения writable
handle, проверяет authenticated required budget singleton/owner/kind/migration_id,
точные charge/reserved/meta capacity, state↔Mode7/funding↔storage-stage, derived
covered totals и L для Held. Root::check/Ledger::check используют все summaries;
bounded budget leaf read повторяет correspondence с summary. Every staged commit
считает новые data summaries, производит budget adjustment, затем проверяет final
root до backend commit. Generic caller не задаёт budget payload/reserved. Missing/
wrong owner/сниженный L/искажённая summary=>IntegrityBlocked, не budget rebuild0.
Unoccupied slot не имеет LegacyBudget/endowment: free/grant/outstanding/pending=0,
head absent. Root7 проверяет эту форму отдельно от Runtime6 pristine ledger с pool64.
Общие unused-head/metadata quota bounds остаются; global owner0 charges допустимы
и при0 occupied C, но не создают фиктивный per-C budget или session.

| Atomic purpose | Budget / pool effect в том же commit |
|---|---|
| Fence/copy | Held, L из produced D summaries; E_c=64MiB как repair_start_reserve в budget leaf. До Ready ledger.free/grant/outstanding=0; нельзя вызвать обычный Ledger::new и получить ещё64MiB. Copy charge уменьшает L ровно по covered delta |
| Ready | Проверить все imported queue/pending/head/receipt tickets; сохранить их origin/reservations. Перенести E_c=64MiB из leaf reserved в ledger.repair_free ровно один раз, funding=FundedOnce/E=0. Общая need не получает бесплатные64MiB |
| L1 queue/pending/materialize/readable | Actual row/ticket U/R и recomputed covered/L меняются атомарно. Queue128KiB/pending128+64KiB не добавляются вторым scalar. Head occupied temp уже в ticket; unused temp лишь в P, никогда оба |
| Late Receipt1 / ACK progress | Снять exact debt reservation/ref effect once; если covered уменьшается, Held L возрастает на освободившуюся часть. Другой C не может купить эту ёмкость |
| Select A / Activate B | Сначала проверить все existing/new funded obligations и Q/cuts/policy. Сохранить release covered snapshot/generation/Q, Held→Released/L=0 в том же selection commit. Ledger.free/pool не обнулять/восстанавливать |
| Reopen/retry/C2 | Не менять funding/hold/pool. C2 меняет только свои summaries/budget плюс явно owned shared metadata; debt/escrow C1 не расходуются |

Занятый ordinary/head ticket уже оплачен в U/R и отражён pending/head; repair-origin
outstanding — pool conservation counter, не дополнительный quota charge. Unissued
grant и free pool лишь P; их нельзя вычесть из LegacyHold как второй captured ticket.
Legacy-origin imported pending не превращается в repair-origin outstanding.
Израсходованный pool после FundedOnce не получает E заново; только explicit funded
Refill с прежними ledger limits. Один accepted old slot не получает второй grant.

### 6.2. Config identity и capacity-increase CAS (R3)

Intent.initial_config_digest — immutable audit исходного полного config, **не**
требование byte equality всех будущих quota values. IdentityConfig7 отдельно pin-ит
account/claim/device/source_id/migration_id/incarnation, occupied contexts/slots,
canonical state-dir и output-profile/control-IPC/locator publication paths,
layout/reservation policy/options. В capacity retry они неизменны. До Ready также
не меняются route/startup options; после Ready обычная explicit route операция —
отдельный runtime purpose с L1 eligibility checks, не capacity retry.
Canonical IdentityConfig7 bstr/uint/array bytes сохраняются в private Profile7;
identity_digest=BLAKE3("kg:od2:migration-config-identity\0"||enc(identity)).
Initial full-config digest вычисляется один раз с отдельным domain
`kg:od2:migration-config-initial\0`, включая initial capacities/options. После retry
он остаётся audit исходного запроса; current identity сверяется со stored pins.

Kind7 EffectiveConfig7 — authenticated singleton в Migration51, capacity<=4KiB:
`[7,migration_id,revision,identity_digest,initial_config_digest,
[logical_quota0,logical_quota1],physical_capacity,previous_config_hash?]`.
Key — Migration record key из §4.1 с kind7 и empty logical_key_bstr; revision1
имеет absent previous hash, дальнейшая revision ссылается на exact replaced raw.
Hash вычисляется от raw с domain kg:od2:effective-migration-config\0, не входит
сам в своё payload. Initial record revision1 соответствует Intent initial digest;
Root7 load проверяет identity/digest/migration_id, ledger quotas=effective quotas.

IncreaseCapacity7 принимает expected root generation+config revision/hash и
candidate config. Разрешены лишь componentwise nondecreasing logical quotas и
physical capacity, с checked bounds/actual retained/growth headroom. Identity/
пути/contexts/options должны совпасть; lower/retarget=>ConfigConflict до write.
Один commit обновляет EffectiveConfig7 revision/hash-chain link, ledger quotas и
current profile-publication plan; LegacyBudget/funding/source_id/Native/Q не меняет.
План публикации в Profile7 содержит effective-config revision+desired bytes digest;
Ready7 digest сохраняет initial-at-Ready plan как audit, не принуждает игнорировать
последующий явный capacity CAS. Pending final publication использует **последний
committed effective config/plan**, не исходные quotas из Intent. Публикация и plan
связаны CAS; crash/retry сверяет revision и exact bytes. Уже опубликованный профиль
не переписывается silent resume: обновление требует explicit expected-file-hash CAS
по тем же pinned paths, иначе ReadyNeedsProfileUpdate.

Capacity CAS — разрешённый owner purpose во время paused/copying/blocked и после
Ready; создать LegacySnapshot/второй repair endowment он не может. Если для metadata
commit/recovery нужен больший physical backend limit, candidate повышение применяется
только к этой явно запрошенной CAS/recovery под lock; обычный writer не стартует,
пока outcome конфигурации не разрешён. Ambiguous commit reopen читает old/new
revision/hash/commit_id и повторяет тот же request; не меняет migration incarnation.
Read-only status проверяет stored root/config, не подменяет их меньшей caller quota;
необходимое recovery при недостаточной physical capacity явно требует capacity retry.

Адресные C/F assertions: quota_c=need_c−1 refusal до write; need_c проходит;
обычный L1 append и clean reopen не теряют L; delayed R1 возвращает freed debt в L;
C2 не расходует escrow1; source backing owner0 не покрывает hold1; occupied/head
reservation не двойная; tampered budget payload/reserved/owner/missing leaf blocked;
increase после ConfiguredCapacity продолжает exact source/checkpoint и публикует
новые quotas при прежних IDs/paths; CAS stale/lower/retarget refusal, kill до/после
CAS/Ready не создаёт второй E/free pool. Всё внутри C600/F1200, без нового long run.

## 7. Indexed L1 и existing control machine

### 7.1. Один owner, два wire paths

Один dedicated owner/mailbox16/lock; old Engine не остаётся parallel writer.
L1 algorithms используют typed point/range repository, не Runtime7 paths()/Changes
или full-history cache. Private lowering знает L1 bootstrap/queue/materialize/
pending/readable/receipt purposes и атомарность Native/Guard1/seen/credits.
Native batch<=4, alternation C и128 actual refs с64/64 сохраняются.

Direct endpoint явно может advertising ALPN1+2. Decoder/exporter label выбирается
ровно по negotiated ALPN. У C один admitted lease; переход на ALPN2 закрывает старую
ALPN1 lease до новой admission. Global permits2 действуют **до** TLS/spawn, не по2
на ALPN. Endpoint/actual route/locator/current policy/final closed lease обязательны.

L1Active без handoff использует exact Locator1/ALPN1/Hello1/Proof1/Consent1/limits1,
direct-only. Owner выдаёт fresh Locator1 для нового endpoint, не reused old endpoint
из shadow. Locator2 публикуется отдельно для explicit Consent2/ALPN2; decoder одного
семейства не принимает другое. Старый удалённый d9b реально interoperates с migrated
L1. Transport eligibility задаётся отдельно от send permission: ALPN1 разрешён
только LocalMode7=L1 с/без UpgradeIntent7, включая incomplete L1 bootstrap. Durable
P у A (Offering) или P/A у B (Prepared) — первая необратимая граница отказа ALPN1
для этого C: в transition инвалидируется любая прежняя ALPN1 lease, до следующего
dispatch. Далее reconnect/old data/repair/ACK только ALPN2, в том числе при pause,
expiry или absent Q. Precommit abort оставляет прежний L1 transport state. Другой C
может сохранять ALPN1 на endpoint. Storage fence запретил старый local EXE раньше;
Selected не является первой transport boundary. Freeze materialization начинается
с тех же durable P/Prepared, но не запрещает old receive/repair/ACK и bounded queue.
B до своего Prepared может ещё materialize L1; его eventual cut включает эти rows.

До local Q есть закрытый typed LegacyBeforeSelection dispatch; после local Selected/
Active — S3 mixed verifier с установленным proof. Нельзя вызывать требующие
StaticSelection2 Event **или Receipt** verifiers с dummy/null/peer-selected Q, а при
ошибке mixed verification нельзя пробовать pre-Q путь. Полные правила ниже.

Locator1 revision после каждого boot больше imported/current saved revision;
endpoint новый, commit revision предшествует export. Locator2 revision также
монотонна в своём explicit family. Same-live retry exact, previous owned public
artifacts проверяются до удаления/публикации. AuthBlocked C не получает ложный
publication-ready; current permission проверяется до подписи/выдачи locator.

Mixed L1/L2 profile принимает DirectOnly. RelayOnly/Auto-with-relay пока enabled C
требует L1 direct=>L1RequiresDirectOnly. После обоих handoff применимы S4 routes и
URL-only RelayOnly. Так сохраняется A3 L1 direct-only без silent fallback или нового
per-C route redesign; основной P явно завершает C2 handoff до RelayOnly.

#### 7.1.1. Admission, pre-Q dispatch и asymmetric reconnect (R2)

Новый VerifiedExistingScope1 получается только из production-verified existing
Ready1/обоих readable genesis, historical account/Device membership и точных
C/D/E/SID/roles/selected Offer1/Grant1. Это не ModeProof и не permission. Fresh
ALPN2 admission связывает его с **current** Root2/Consent2(existing с тем же scope,
cert roles и полномочиями), interval/provider/revocation high-water, настоящими
Locator2/endpoint/route, TLS exporter и подписанными Hello2/Proof2/current lease.
Итог — отдельный AdmittedExisting2 с scope и stage compatibility, даже без Q.
Все current checks повторяются final gate. Admission даёт control/допустимый old
receive/ACK, а materialize определяется только local durable state. Неполные
Ready1/genesis/SID не получают такой admission: wire L1 status (§4) не становится
фиктивным Existing Hello2; bootstrap продолжается через настоящий ALPN1.

Таблица для **fresh** admission перечисляет достижимые пары; применяется симметрично
по роли local A/B. Intent — flag при L1. Цифры — accepted Mode2, не LocalMode enum.

| Durable A / wire | Durable B / wire | Проверка hints и разрешённый recovery |
|---|---|---|
| L1±intent /1 | L1±intent /1 | Q/cuts absent; explicit current Consent2 и complete scope позволяют control admission; до P доступна также обычная ALPN1 compatibility |
| Offering /2 | L1±intent /1 | P потерян: Q absent, cutA лишь у A; fresh ALPN2/control resend exact P, B не обязан уже знать cutA/Q |
| Offering /2 | Prepared /3 | A потерян: Q absent, cutA у B равен durable P, cutB bounded и согласован с B source head/watermark. У A cutB устанавливается лишь signed A, не hint |
| Selected /4 | Prepared /3 | K потерян: у A local Q, у B Q absent; both cuts exact у обоих. Admit/control/exact K retry не требуют check_selected на B hint. B принимает old1/control; A принимает допустимые old1 и Event2 с local proof, собственный Text frozen |
| Selected /4 | Active2 /5 | Committed потерян: оба Q/cuts exact; B повторяет saved Committed на exact K. A receive2 разрешён, materialize2 только после signed Committed commit |
| Active2 /5 | Active2 /5 | Exact Q/cuts обоих и обычные mixed operations |

Role-incompatible Offering/Selected у B или Prepared у A, stage0,
неверные optional shapes, Q у L1/Offering/Prepared, cut mismatch с уже проверенным
local control/proof и любое отличие **известного** Q отвергаются до watermarks/
debt/Native. Остальные fresh пары (например A Active/B Prepared или A L1/B Prepared)
невозможны без потери durable transition и отвергаются. Prepared B при A Selected
видит some-Q лишь как authenticated **opaque hint** до проверки exact K: cuts уже
известны и сравниваются сразу; Q нельзя установить или использовать для Event2.
После K-derived Q сравнить ранее принятый hint; несовпадение — bounded conflict,
не adoption. Selected A не вызывает strict check_selected для absent-Q Prepared.

На уже admitted lease delayed более ранний **совместимый** hint не откатывает local
или cached durable stage/control, cuts/Q или watermark; ограничения формы и известные
conflicts сохраняются. Hint не является полномочием data: B мог commit Active и
послать Event2 раньше нового Head; A Selected проверяет такой Event2 своим proof,
даже если последний cached B hint ещё Prepared. На fresh lease используется таблица,
а не произвольная терпимость к stale states. Hello/Resume/Head builder формирует
actual local projection; signed old hints не переподписываются от нового stage.

Все handlers сначала проверяют canonical Frame2, exact C/D/E/SID, negotiated ALPN2,
sender signature/actual admitted lease и current gate. Frame2 signature не заменяет
immutable source/receiver signature. Typed варианты при local L1/Offering/Prepared:

| Frame / typed input | Проверки и разрешённый closed commit до Q |
|---|---|
| Resume / Head → LegacyHead1 | Existing handshake IDs exact; Mode shape/пара выше до изменения watermark. Source cursor1..512 с mandatory last ID; receive cursor1..512<=actual own source_next с exact local immutable ID at next−1. Source next монотонен; equal next/different ID — conflict. Lower compatible hint не снижает bound. Frozen sender source cursor=cut/prev, hint peer cut не заменяет signed P/A. Receive cursor/Complete не снимает ACK debt |
| PrefixReq / PrefixResp → LegacyPrefix1 | Legacy ordinal/source/cut bounds, contiguous start/end/previous/actual source cursor, canonical sorted exact-used AuthRecords, credit/frame/actor caps и bounded count (runtime<=16, wire<=32). Нет allocation по произвольному source head. Приём отдельных source только через LegacyData1; possession не readable. Verified source cursor даёт durable watermark, не cursor jump или receipt |
| Deliver/EpochCipher tagged1 → LegacyData1 | Exact Event1/Core/header/ID/old signature/auth role/Ready1/scope; n<512 и n<known cut[role], previous/parents/ClosedTail/Guard1, immutable duplicate и paid pending/head. Readable commit Native1/Guard1/indexed seen/captures (§3.3)/receipt/debt/budget атомарен. Tagged2 до local selection отвергается **до** pending/possession/Native; не хранится для будущего принятия |
| AckBatch tagged1 → VerifiedPreQReceipt1 | Actual immutable Event1 загружается из own source repository; Text, n>0/<512, exact ID/role/C/D/E/SID. Проверка Receipt1 описана ниже; сохранить exact immutable receipt и снять только соответствующий debt once. Missing/unknown source или bundle/не тот lane/receiver не дают Delivered |
| AckQuery / Complete | Query bound/ID pairs связываются с actual source/readable. Вернуть уже saved Receipt1 raw; новый receipt лишь за actual readable. Complete подтверждает exact possession запрошенных IDs, не plaintext/ACK и не Delivered |

Для Receipt1 новый closed protocol verifier принимает VerifiedExistingScope1,
authenticated exact Event1 source и RootVerifiedCredential **opposite Device**, не
StaticSelection2. Он проверяет canonical exact body
`[1,D,E,source.role,source.n,source.id,receiver_cert_ref,receiver_auth_ref]`,
соответствие AuthRecords bundle hash/certificate/account/role и old RECEIPT_SIGN
signature. Legacy raw/id остаются прежними. ParsedReceipt1::parse/raw могут быть
reused, но его нынешний verify требует StaticSelection2: нужен отдельный typed
verify_pre_q, без ослабления existing verify и без fabrication selection.
Сертификат/bundle удостоверяют receipt provenance; current Root2/Consent2/lease
отдельно разрешают сегодняшний ACK commit. Genesis, wrong source/kind/role/ID,
unknown bundle и conflicting immutable receipt отвергаются до debt mutation;
duplicate exact receipt не получает новый credit и не меняет Native.

New pre-Q readable пишет Receipt1 и реальные применимые O/A/F/R; imported
LegacySnapshot не заменяет receipt. После local Q новый legacy readable без
receipt получает Receipt2(event_format1,Q), уже существующий Receipt1 неизменен.
Prepared peer не проверяет Receipt2 через pre-Q seam: sender сохраняет ACK debt и
повторяет controls до K. Затем обычный S3 mixed receipt verifier с local proof.
Selected/Active Head также dispatch отдельно от frozen S4 head_body/accept_head:
source/readable bounds учитывают cuts **по format**, receive cursor может уже быть
>512 за Event2 при frozen own source=cut. Нельзя наложить общий L1 cap на mixed
cursor. Только когда оба hints selected/active, strict selected Q/cut check применим;
local proof всё время обязателен для Event2/Receipt2 независимо от peer hint.

Так P-lost A Offering может открыть ALPN2 к ещё L1 B, а K-lost A Selected — к
Prepared B без Q; ни ALPN1 fallback, ни симметричный selected hint не требуются.
Существующие S4 selected-only require/check_scope/head entrypoints должны получить
закрытый existing/pre-Q branch, а не отключённую Q проверку на общем fresh пути.

### 7.2. Durable stages и write sets

Native phase1/2/3 и wire mode — разные enums. Existing не подставляется в fresh
S4 active==both Ready2; old Ready1 не переписывается в Ready2. Current Consent2
existing C/D/E/SID устанавливается отдельно от immutable historical proof.
A — единственный coordinator; simultaneous B intent лишь просит A продолжить.

| Переход / purpose | В одном closed commit | После commit / restart |
|---|---|---|
| L1 + UpgradeIntent7 | Request/body/current Consent2 existing ref/intent; LocalMode остаётся L1/wire1, нет cuts/Q | Cancellable; ordinary L1 и queue/R3/receive/ACK |
| A→Offering(P) | Both Ready1/genesis; current Root2/Consent2+fresh ALPN2 lease; exact own.next/last=cutA/prevA; selected raw consent+P raw; send frozen+control debt до publication | Только materialization frozen, old receive/repair/receipts и legacy-bounded queue. Exact P retry. Rollback после commit запрещён |
| B→Prepared(P,A) | P+selected raw consent/cut/watermarks; собственные cutB/prevB; A raw/control debt/freeze | Durable A до ответа. Event2 до K не принимается в pending/possession/Native |
| A→Selected(Q,K) | Exact A/K/certificate/Q/cuts; current Guard1+mirror→Guard2/native envelope; mixed history binding/escrow reconciliation/K debt/receive-only Selected | K лишь после commit. Принимает format1 ниже cuts и Event2(Q); local encryption ждёт Committed |
| B→Active(Q,Committed) | Exact K/certificate/Q/cuts/Guard2/native/current pointers; mixed binding/credits; immutable signed Committed raw+response debt | Commit до ответа. Queue может materialize2; old receive/ACK продолжаются |
| A Selected→Active | Exact signed B Committed(Q), durable activation/control outcome; selection неизменна | Local materialize/offline2, без нового P/genesis/keys |

Каждый stage использует CAS captured root/context/policy generation и exact typed
operation digest/control IDs. Duplicate возвращает **сохранённые bytes**, не re-sign
или Native. Conflicting P/A/K/cuts/Q=>bounded signed-fork evidence/block C, не
replacement/reset. Clean final-policy abort reusable без Head/Native/debt changes;
IO/abort ambiguity требует poison/reopen/resolve до retry. Hooks before/after durable
commit и before publication обозначают разные реальные kill windows.

Own cut — committed own.next, включая genesis; queue в него не входит.
1<=cut<=512; own prev — exact source at cut−1. Peer cut > всех readable/pending
format1 ordinals, включая quarantine/head; cut>=durable observed source_next из
verified Resume/Prefix/Head. При равенстве last_id exact. При неполном old prefix
signed prev — обещание, проверяемое при достижении cut без cursor jump. Mismatch
=>fork/block C; новый format Native trial до нужного prefix не вызывается.
Lower delayed hints не снижают watermark. V4 import не придумывает несохранённый
historical watermark: реальные accepted slots дают lower bound.

Current control/pending/readable/ACK commit проверяет Root/Consent interval/provider/
high-water/actual lease снова после staging. При renewal selected consent остаётся
старым: P carries exact raw, обе Root signatures/scope/permissions проверяются
отдельно от valid current consent; Q не меняется. Expiry/revoke/clock/provider
паузят transitions, не reset/rollback. ALPN1 L1 требует current Consent1/Root/lease.
ALPN2 после explicit existing Consent2 требует current Root2/Consent2/lease;
old proof остаётся историческим свидетельством, не current permission.
Начиная с durable P/Prepared ALPN1 fallback запрещён (§7.1); old policy не заменяет
current Consent2. Local native1 materialization
до P требует L1 permission/lease; Offering/Prepared/Selected не materialize ни
format1, ни format2 независимо от доступной consent версии.

### 7.3. Mixed bytes, receipts и lost Committed

Existing ModeProof/verifier/TaggedItem/MixedReceipt S3 не меняют wire:
format1 iff n<cut[role], format2 iff n>=cut[role]. После Q Core2/Receipt2/Mode2/
Resume2 refs равны local verified Q. Old source/receipt bytes/IDs не переписываются.
Data1 после Q tagged внутри wire2; failed v2 не fallback в v1. Единый ordered ingress
проверяет next/prev/ClosedTail; смена format не сбрасывает Native session.

До Q новый old readable получает Receipt1. Already durable Receipt1 exact на retry/
query. Новый readable legacy packet после Q получает Receipt2(event_format1,Q),
если immutable receipt ещё отсутствовал. Если другой Prepared, ACK остаётся durable
debt до K, не заменяется Receipt1. Receipt1 для Event2, format2 ниже cut или wrong Q
отвергаются до debt mutation. Late old tail после own new first не меняет own
g/index/first/rotation; новый peer DH first затем идёт через обычный guard.

Обязательный trace: A Selected/B Prepared → K → B Active+Committed commit → ответ
теряется → kill обоих. Reopen сохраняет Q/cuts. A принимает B Event2, но не шифрует
своё. Exact K после fresh reauth возвращает прежний Committed raw; A Active commit,
тот же queued request materialize ровно раз. Renewal не меняет selected bytes/Q.
Отдельно P до publication и B Prepared до response; frozen send не размораживать.

## 8. Typed seams, history и operator API

Предлагаются будущие private modules vault/migration7.rs,
indexed_dialog/migration7.rs, runtime2/legacy1.rs и runtime2/existing.rs.
State остаётся без protocol/ratchet dependency. Owner проверяет signatures/Native/
inner/current permission; state выводит structural fields/digests и проверяет
witnesses/credits/purpose/before-after correspondence. Нет generic imported writes.

| Typed API / action | Граница |
|---|---|
| MigrationRepository::preflight/fence/copy_batch/verify_ready/pause/resume | Held lock/verified V4 source, не caller records/totals/native |
| MigratedRepository::snapshot/history/source_ref | Version7/marker/owner/stage; Copying только source read-only |
| L1Bootstrap/Queue/Materialize/Pending/Readable/Acknowledge | Native1/current indexed pointers/Guard1+seen/credits одним commit; native1 требует lease |
| Propose/Prepare/Select/CommitB/ActivateA | Exact controls/Q/cuts/watermarks/native/policy CAS, не set-Head/active flags |
| MixedPending/Readable/Acknowledge | Format-specific structural reader; LegacySnapshot membership либо full captures; history/credit/native atomicity |
| IncreaseCapacity7/RepairRefill | Capacity CAS (§6.2) сохраняет identity/pool; refill — другой оплаченный purpose, не эффект recovery |

Head7 содержит source_id, actual phase/role/native_generation, original nonce,
exact inner Head1 и discriminator Native1/Mixed7. При Ready Native slot хранит
**неизменный raw Native1**: его прежний self-HPKE info/AAD/Head/nonce проверяются
как есть; additional metadata находится в Head7, а не расширяет128KiB blob.
Outer indexed AEAD/Head7 bind связывает это с incarnation/C/cert/source_id; caller
не может подставить произвольный inner Head/Native. Последующие L1 operations
продолжают Native1 seal с тем же key/nonce и обновлённым real inner Head1.

Selected меняет slot на Mixed7 self-HPKE с info `kg:od2:migrated-private\0`, AAD
`[7,incarnation,claim,source_id,C,cert_ref,role,native_phase,native_generation,
Offer1_ref,Grant1_ref,D,E,SID,Q]`, прежними public-restored account/session/key/
reservation и Guard2. Combined sealed blob<=128KiB. Conversion checked-bumps
native_generation один раз как изменение private-state encoding, при0 Native
packet calls; own/peer g/index/next/first/T и SessionKeys/SID неизменны. Original
source Head/native_generation/raw остаются immutable witness. Duplicate controls
сначала читают durable stage, не reseal. Frozen Root6/Native2 payload/AAD остаются
отдельным вариантом: S5 binary открывает fresh S4 без reseal/format relabel.

History view immutable по incarnation+C/D/E/SID/role; Q/cut/stage не меняют view ID.
Indexed cursor сохраняет frozen frontiers через handoff/restart; append не вставляет
новые rows в старый snapshot. Root7 reader различает LegacySnapshot/post-Ready L1/
Mixed bindings, source/receipt format/cut и direct parents. Page<=32; IPC считает
48KiB actual escaped JSON **до** consumption candidate cursor. Shared Author
ownership сохраняется; запрос C1 не сканирует C2.

V4 не имел opaque cursor. Offset API остаётся только для bounded Event1 dataset
<=1024 source rows/C, а не всего растущего Event2. Нельзя объявлять old offset
frozen cursor. Приёмка сравнивает exact pre/post migration readable IDs и отдельно
новый indexed cursor до handoff/его exact old suffix после append. Incomplete
initial session без real SID не получает fake view: HistoryUnavailable/NeedHandshake.

Отдельные operator commands:

```text
ordered storage-upgrade --state-dir ... --expect-account ... --expect-device ...
  --config-file ... --check
ordered storage-upgrade --state-dir ... --expect-account ... --expect-device ...
  --config-file ... --resume --control-ipc-file ... --output-profile ...
ordered storage-upgrade-status (--state-dir ... | --ipc-file ...)
ordered storage-upgrade-pause --ipc-file ...
ordered storage-history (--state-dir ... | --ipc-file ...) --context ... --offset ...
ordered storage-increase-capacity (--state-dir ... | --ipc-file ...) --config-file ...
  --expected-root-generation ... --expected-config-revision ... --expected-config-hash ...
ordered consent2-create --origin existing --existing-origin-file ... [Root admin args]
ordered consent2-import --ipc-file ... --slot ... --expected-current (absent|hash) ...
ordered upgrade --ipc-file ... --slot ... --request-id ...
ordered upgrade-status --ipc-file ... --slot ...
```

MigrationAdmin может иметь authenticated status/pause/old-history IPC под тем же
owner/lock, но не network endpoint. Pause освобождает lock после whole batch outcome.
Stopped status/history/capacity CAS берут exclusive lock; Busy не обходится second
DB writer. Check не fence; ordinary resume после Ready лишь завершает publication
последнего committed plan. Повышение config — явная отдельная команда/CAS, не
молчаливое принятие отличающегося --config-file обычным resume.
Profile3 с явным label ordered-dialog-indexed описывает pins/slots/enabled/routes/
quotas/public paths; authoritative per-C mode/Q/cuts только в Root7. Old profile1 и
fresh Profile2 не редактируются для adoption. Public existing-origin export содержит
Ready/genesis/scope, проверяемые offline Root-admin; Root secrets не загружаются
ни в live migration, ни в runtime owner.

IPC — explicit new typed version3 envelope внутри existing OrderedDialog command,
disjoint от S4 envelope2, bounded tags/args/errors. Old clients=>UnsupportedVersion.
Нет set-mode/set-Q/set-native/set-cut. Status показывает storage stage/source_id/
checkpoint, occupied C/native phase/mode/cuts/Q/frozen reason/credits/debt/policy
block/profile publication и provenance kind; не secret/private Native payload.

Предлагаемые IPC3 tags:1 status,2 queue(slot,request16,text),3 request-status,
4 history(slot,cursor?,limit),5 locator(slot,version1|2,import|export,bytes?),
6 authority-import,7 consent2-import(slot,expected-ref?,raw,historical),
8 upgrade(slot,request16),9 upgrade-status,10 cancel-intent(slot,request16),
32 migration-status,33 migration-pause,34 migration-history(C,offset,limit),
35 increase-capacity(expected-root-generation,config-revision/hash,candidate-config).
Каждый tag имеет exact fixed tuple arity/caps; extra arguments/unknown tags fail.
Tags32–34 доступны только MigrationAdmin; cancel-intent только до P. IPC2 для
Runtime6 остаётся unchanged; local CLI выбирает явный endpoint version, не codec
fallback после ошибки. Authority/mode mutation всегда проходит repository purpose.
Tag35 доступен authenticated same-owner MigrationAdmin/Runtime7, с explicit
IncreaseCapacity7 purpose; он не импортирует source или live authority.

## 9. Actual source map baseline0c1baa8

Links указывают на существующий код; proposed modules выше ещё отсутствуют.

| Entry point | S5 change/seam |
|---|---|
| [StateDirectoryLock](../../crates/kilogram-state/src/lib.rs#L382), [recover](../../crates/kilogram-state/src/lib.rs#L950) | Header-aware dispatch до old journal; lifetime lock migration/owner |
| [OrderedVault open/get](../../crates/kilogram-state/src/vault/ordered.rs#L166), [recover/shadow](../../crates/kilogram-state/src/vault/ordered.rs#L694) | Bounded source verifier, не full old writer поверх fence |
| [old types/caps/reservations](../../crates/kilogram-state/src/vault/ordered.rs#L500) | Explicit inventory, escrow B4, no silent dropped path |
| [manifest/index MAC](../../crates/kilogram-state/src/vault.rs#L1951), [record AEAD](../../crates/kilogram-state/src/vault.rs#L2007) | Same-DB fence/source override/V4Ref and completeness |
| [Root codec](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L218), [loader](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L554) | Layout7, byte-compatible5/6 и actual predecessor refusal |
| [purpose/commit](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L823), [namespaces](../../crates/kilogram-state/src/vault/indexed_dialog/types.rs#L116) | Migration global51, purpose before retry, closed lowering |
| [runtime records](../../crates/kilogram-state/src/vault/indexed_dialog/runtime2.rs#L88) | Не выдавать existing за fresh Ready2, occupied slots/<=2 peer pins |
| [ledger](../../crates/kilogram-state/src/vault/indexed_dialog/credits.rs#L113) | Source-attributed escrow/tickets, no reopen refill |
| [H2 binding](../../crates/kilogram-state/src/vault/indexed_dialog/history_protocol2.rs#L172), [fresh-only source](../../crates/kilogram-state/src/vault/indexed_dialog/history_source_v2.rs#L384) | Mixed/import structural variant, не ослабление old encoding |
| [view ID](../../crates/kilogram-state/src/vault/indexed_dialog/history_types.rs#L305), [history candidate](../../crates/kilogram-state/src/vault/indexed_dialog/history.rs#L78) | Stable view/cursor через cut, isolated Event1 offset |
| [claim/device](../../apps/kilogram-cli/src/ordered_dialog/admin.rs#L43), [Head](../../apps/kilogram-cli/src/ordered_dialog/state.rs#L11), [Native open](../../apps/kilogram-cli/src/ordered_dialog/state.rs#L255) | Primary/nonce/phase/real optional IDs, Native provenance |
| [Engine native](../../apps/kilogram-cli/src/ordered_dialog/engine.rs#L288), [save](../../apps/kilogram-cli/src/ordered_dialog/engine.rs#L356), [sequence/source](../../apps/kilogram-cli/src/ordered_dialog/engine.rs#L395) | Typed L1 adapter, exact keys/author sequence; no generic Changes |
| [queue/materialize](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L106), [receipt](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L267), [history](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L345) | Request/debt/R3 и bounded indexed history |
| [ingress](../../apps/kilogram-cli/src/ordered_dialog/ingress.rs#L140), [Guard1](../../apps/kilogram-cli/src/ordered_dialog/ratchet.rs#L71) | Full64+head, source/guard/seen, historical capture gap |
| [Guard2 import](../../crates/kilogram-ratchet/src/ordered_v2.rs#L505), [Native persistence](../../crates/kilogram-ratchet/src/ordered.rs#L172) | Trusted production source witness вокруг pure DTO, zero constructors |
| [existing proof](../../crates/kilogram-protocol/src/ordered_dialog_v2/mode.rs#L317), [Committed](../../crates/kilogram-protocol/src/ordered_dialog_v2/mode.rs#L397) | Static verifier есть; отсутствует durable/current/cut machine |
| [MixedEvent](../../crates/kilogram-protocol/src/ordered_dialog_v2/frame.rs#L469), [receipts](../../crates/kilogram-protocol/src/ordered_dialog_v2/authority.rs#L410) | Reuse exact wire, closed mixed writer новый |
| [fresh-only policy](../../apps/kilogram-cli/src/ordered_dialog/runtime2/policy.rs#L19), [owner load](../../apps/kilogram-cli/src/ordered_dialog/runtime2/owner.rs#L166) | Existing current/static branches, fresh fences сохраняются |
| [data](../../apps/kilogram-cli/src/ordered_dialog/runtime2/data.rs#L298), [watermarks](../../apps/kilogram-cli/src/ordered_dialog/runtime2/network.rs#L307) | Freeze/Selected receive-only/mixed ACK/durable source head |
| [Native2](../../apps/kilogram-cli/src/ordered_dialog/runtime2/native.rs#L59), [L1 runtime](../../apps/kilogram-cli/src/ordered_dialog/runtime.rs#L1127) | One owner, Native1/Mixed variant, no parallel Engine |
| [ALPN1](../../crates/kilogram-transport-iroh/src/ordered.rs#L14), [ALPN2](../../crates/kilogram-transport-iroh/src/ordered2.rs#L1) | Explicit dual ALPN/direct, global2 permits and per-mode refusal |
| [CLI stubs](../../apps/kilogram-cli/src/ordered2_cli.rs#L491), [fresh fence](../../apps/kilogram-cli/src/ordered_dialog/runtime2/admin.rs#L20) | Explicit storage-admin/handoff/existing consent, не enroll2 |
| [pause hook](../../apps/kilogram-cli/src/ordered_dialog/test_support.rs#L123), [primary pause](../../crates/kilogram-state/src/vault/ordered.rs#L425) | Writable-handle marker controls до long runs |

## 10. Acceptance: реальные L1 fixtures и whole P5

Все fixtures новые. Frozen S1/S2 DB/старые P/F vaults не открываются и не меняются.
Old fixed image target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe:
61531648B, SHA256
`d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`.
Primary P создают два real old CLI, затем продолжают те же два state dirs current
binary. Никаких synthetic Native checkpoints/rows/constructor-origin booleans.

### 10.1. Короткие infrastructure checks сначала

1. Compile-only harness/helpers; stage fixed EXE лишь без живых children.
   Exact source/feature closure/hash фиксируются до run. На этом contract этапе
   ни build, ни stage, ни migration не выполняются.
2. Windows hook controls: writable temp/sync/atomic marker, live PID и реально
   заблокированный owner IPC, затем release/kill и whole commit outcome. Два legacy
   File::open(hit).sync_all требуют отдельного reviewed infrastructure patch до
   их использования. Не терять sync/atomicity и не принимать early exit за pause.
3. Frozen d9b **не** получает эти defective pause flags. Partial Offered/Granted/
   selected-not-Ready создаёт real ALPN1 control peer, удерживающий следующий ответ
   после signed Offer/Grant/Init/Ready. Parent подтверждает live PID/phase через
   IPC и actual signed exchange, затем kill. Peer использует real public Native;
   не пишет private rows. Это явно fixture-origin scope, не primary whole P.
4. Короткий fresh d9b→copy/Ready→old-remote smoke, несколько Text/phase counters;
   затем handoff/lost Committed/real own-relay B-only. Accepted S4 relay helper
   используется с verified hash/liveness/bounded stderr. Smoke не suffix full P.
5. До511 оценить реальные old-process costs на коротком range и remaining F budget.
   Новый helper failure/непомещение=>root decision, не quiet long repeat. Source
   freeze до финальных contributing sets.

### 10.2. Matrix

| Gate / set | Origin / production entrypoint | Assertions / actual fault |
|---|---|---|
| M01 / C+F | Real d9b active/queue → migration API; bounded fold §3.2 | Same secrets/public keys/session/nonce/IDs/ciphertext/projection/request/receipt; copy Native calls/constructors0; primary против stale shadow. Small fold cases: multiple own continuations before peer first; late peer continuation after own first; pending excluded; incomplete genesis; same-looking summaries/invalid dependency reject |
| M02 / F | Real d9b own511/next512 | Old/new indexed L1 new request отказ до/после reopen, old retries сохранены; handoff cut512, первый new Event2 n512 с прежними D/E/SID; no genesis/OTK/account/session |
| M03 / F | Real d9b pending64+gap/head/late DH/old receipt | Exact pending flags/O/A/bytes, no eviction при Ready/Selected; funded lowest head; late tail не меняет own first/T, old ACK снимает exact debt once |
| M04 / C+F | d9b partial via actual control peer | Real optional IDs/reservation, finish L1 once/Ready gate; local stage1 без fake D/E/SID/ALPN2 admission; inconsistent partial отказ до fence |
| M05 / F | Migration process / isolated stopped-seed branches | Parent kill before/after Fence, middle copy, staged-before-Ready, durable Ready-before-profile; exact schema/stage/source_id/retry, immutable source |
| M06 / C+F | Mode projection/dispatch + existing two-process owner | Positive encodings L1±intent=1/Offering=2/Prepared=3/Selected=4/Active=5; wrong role/stage0/Q/cut shape reject. Lost P/A/K/Committed at actual durable-before-publication windows + fresh TLS reconnect, pre-Q Head/Resume/Prefix and nonempty Receipt1 debt. Selected↔Prepared absent-Q admits exact K retry; hint не снимает debt/не разрешает Event2 у B, controls Native calls0; A Active before first publication kill не rematerialize |
| M07 / F | Current Root/consent/time/provider/lease + controls | Renewal keeps selected Q; revoke/expiry/failure after staging/at close refuses cleanly; conflicting controls/bad cut below pending/header pre-Native blocked; C2 progresses |
| M08 / C+F | Copy/Ready/control/new mixed native/readable/receipt IO + budget/config §6.1–6.2 | prewrite/aftersync/abort ambiguity, poison/reopen/lastCommitId; quota need−1 refusal/need pass; append/reopen/delayed receipt/C2 preserve per-C hold; slot0 backing не покрывает hold1, reservation не двойная. Tampered/missing budget reject; capacity CAS stale/lower/retarget reject, kill/resume сохраняет ID/checkpoint, effective published quotas и one endowment |
| M09 / F | Actual live/cold d9b/S3/exact S4 helpers | Busy/purpose/version refusal with correct owner/config and positive controls; protected logical state unchanged; header/profile relabel/future/missing marker |
| M10 / C+P | Actual history/IPC | Exact IDs/requests/UTF8/parents/old-new history/cursor suffix; byte/invalid/cross-C cursor/absence/completeness negatives |
| M11 / C+F | Real V4 credential renewal/readable+pending/undelivered sources | O/R проверены, missing A/F ровно UnrecordedLegacy; pending exact A; new drain реальный F/R. Missing O/bundle/обязательный receipt, wrong inventory/generation/C/ID/projection/ref отказ; full-proof corruption не legacy fallback |
| M12 / C+F | Root7 purposes/live IPC/peer/current policy | Fake readable membership/marker/null-ref import, copy после Ready и generic/runtime creation отказ; unknown legacy refs не удовлетворяют lease/cut/revoke. Own unacked ждёт real opposite receipt; restart preserves marker без double import/decrypt |
| P5 / P | Two real state dirs d9b→current | Full resumed trace ниже: offline256/C2/direct+own relay/end counts/Native/ref/debt/history/memory |

Новые R1/R3/R4 component assertions малы и входят в существующие C600 gates, не
отдельный corpus/511 replay. M06 использует короткие isolated stopped-seed branches
для lost P/A/K; lost Committed/Selected receive выполняет также whole P5. Каждый
network case действительно закрывает старую TLS lease и делает fresh admission,
а не только повторяет кадр на прежнем connection. Где receipt held, Head receive
cursor уже может подтверждать possession/readable, но debt остаётся до exact
Receipt1. Ни один дополнительный длинный suite не предлагается; оценки §11 должны
включать эти короткие cases и setup.

### 10.3. Контрольный whole resumed-L1 P5

| Шаг | C1 unique Text A/B | C2 A/B | Проверяемая граница |
|---|---:|---:|---|
| d9b setup,8 duplex rounds A2/B2 |16/16|0/0| Два real C/genesis; hold B16 late known-chain tail после readable B15; saved source/Native/IDs |
| B stopped, A queue17/18 accepted; migrate только A |16/16|0/0| Две sealed queue записи, zero copy Native calls; lock/fence/source witness |
| A indexed L1 ↔ real d9b B: queue17/18+A19..24, B17..24 |24/24|0/0| Actual old remote compatibility; B16 held, A peer prefix15/higher pending; observed B source_next25. Hold actual saved B Receipt1 за A24: nonempty old ACK debt |
| Stop/migrate B; existing consent/P/A/K; lost Committed+оба restart |24/25|0/0| Fresh pre-Q ALPN2 Head/Resume при Offering↔Prepared; cursor не снимает A24 debt, exact Receipt1 снимает once до Q. cuts[25,25], same E/D/SID; A Selected не send. B Event2 n25 pending; release B16 до17..24/readable B25, затем exact K/Committed retry |
|16 duplex rounds A2/B32 после activation |56/536|0/0| Уже B25 idempotent retry включён в первый range, не новый Text; receiving DH>=16/сторону, Q exact |
| A57..384, B537..640; B640 held |384/640|0/0| B-only materialized debt, пока не Delivered |
| B реально stopped, A385..640 offline, restart A без B |640/640|0/0|256 encrypted Text без lease, disk debt256>128, same Native/session reopen |
| B возвращён, missing A385/64 ahead; C1 holds64 refs, C2 duplex1..32 |640/640|32/32| C2 indexed L1/ALPN1, C1 format2/ALPN2; real simultaneous modes/64-64, no discarded debt |
| Funded head/pages>=8; B kills pages2/5; lost nonempty ACK/A restart |640/640|32/32| Exact catch-up кроме held B640, no duplicate/re-encryption |
| C2 handoff без новых Text; RelayOnly/rebind/latest import |640/640|32/32| C2 cuts[33,33], control Native calls0; B640 delivered без A new send/open |
| Graceful restart обоих, full histories/cursors/status |640/640|32/32| C1 history1280/C2 history64 на каждом peer, queued/pending/debt0, receipts640/640 и32/32 |

After-reopen trace удерживает Committed responses до actual readable B25 у A
Selected; exact K retries продолжаются. P/A/K/Committed transitions сами дают0
Native calls; созданный/прочитанный B25 — отдельные2 data operations.

Итог: **1344 Text+4 genesis,2696 successful encrypt/decrypt packet operations** на
всей истории двух state dirs. Frame retries/control/seal/restore не packet calls.
Frozen d9b не печатает S4 numeric packet counters: baseline expected operations
выводятся из verified durable sources/readable prefixes, actual новые S5 calls
пишутся отдельно. При held B16 первоначально ожидается71 (8 genesis operations,
32 Text encrypt+31 Text decrypt), после old compat95;9 delayed old decrypt даёт104;
последующие format2 C1 и L1 C2 дают2592. Это assertions по actual commit effects,
не заранее подставленные fixture counters. Copy/handoff constructors0; initial
по одному account/session/C/сторона (4/4) повторно не создаются. Для incomplete
origins отдельно считаются ещё законно не выполненные initial operations.

Save indexed cursor до handoff внутри Ready snapshot: A после old compat readable39
Text (own24/peer15), например после16 rows suffix=**23**. После append/restart frozen
suffix всё ещё23; fresh final snapshot1280. V4 offset отдельно сравнивает bounded
original dataset до/после storage, не притворяется suffix1280−offset. Первое сравнение
после миграции A делается до новых compat sends, на тех же readable31 Text.
Saved Native/public keys/D/E/SID/selected controls/proof сравниваются по каждой фазе;
local sealed-envelope hash может измениться при легальном rewrap, это не reset session.

## 11. Бюджеты, reuse и Windows execution

A3 ceilings: C<=600s, один final P<=2400s, selected migration/fault/version F<=1200s;
compile/static отдельно. Task явно задаёт verification/locked/offline/Jobs12/
BelowNormal вместо старого A3 Jobs2. Network harness compile --no-run, execute только
stable wrapper. Предлагается P controller2350s, runner overhead внутри2400;
TLS15/admission30/round15/lease<=300 не меняются. S4 exception3000 не переносится в S5.

Оценка, **не результаты и не разрешение превышения**:

| Set | Оценка с fixture/setup/runner |
|---|---|
| C | Infrastructure/smoke80–140 +typed negatives/native fold/credit/history180–300 +overhead30–60 =>290–500s, ceiling600 |
| P5 | S4 P2003.5355583s/3288 calls; P5 2696 calls, но плюс d9b/copy/handoff. Planning1640s data scale +150–350 setup/migration/control +40–100 overhead/history =>1830–2090s ориентир, ceiling2400; linear scale не benchmark |
| F | Real d9b511 seed500–850s +остальные seeds/gates250–350 +overhead40–60 =>790–1260s. Верхний range не помещается в1200: stop/replan/root decision до длинного run |

Основание: [S4 final P](long-lived-dialog-a3-runtime-s4-implementation.md),
[d9b real Duplex64](transport-security-a2-root-review-evidence/mixed-result.json)
62.2957725s/setup4.9882851/exchange53.4841043;
[OD1 full511 component](lossless-sync-a1-bidirectional-implementation.md#L112)
816.71s — другая нагрузка, не network latency. Нельзя обещать дешёвый exhausted
fixture по fresh S4 throughput. Feasibility имеет named contributing command в C/F;
fixture generation не выносится в бесплатный setup. Если прогноз не помещается,
root выбирает bounded организацию или **явную поправку бюджета до run**.

Seed генерируется один раз в новом temp приписанным suite command. Суммы final
runner elapsed включают generation/copy branches/setup/kills/reopen/overhead.
Reuse учитывается один раз с source/hash; compile/static отдельно. Для fault
counterfactual допустимы sealed stopped-seed copies: original не writable после
freeze, в момент активна лишь одна branch этого Native domain; branches не
составляют whole P. Параллельно только independent identities/fixtures и уже
подготовленные разные EXE, без shared writer/domain.

Постоянные paths предлагаются такие:

- S5 CLI: target/compatibility/long-lived-dialog-migration/kilogram-cli.exe;
- real ALPN1 control peer: target/compatibility/long-lived-dialog-migration/kilogram-l1-control-peer.exe;
- exact S4 probe: target/compatibility/long-lived-dialog-predecessor-s4/kilogram-runtime-predecessor.exe,
  isolated git archive0c1baa8/Cargo.lock/features, не current library;
- d9b, S3 predecessor, S4 CLI d5d8c8 и relay b8723b8 остаются на принятых fixed paths,
  hashes сверяются; relay можно reuse без rebuild/overwrite;
- proposed integration target migration_s5_process даёт stable
  target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe.

Wrapper копирует selected stable harness EXE заново **даже при Run**. Полный CLI
compile/copy/run/child-cleanup — одна serial lane: второй CLI/S4 runner не стартует,
пока жив первый harness/owned CLI/helper. Source freeze/build-staging заканчиваются
до P/F. Fixed app меняется только после zero native-CIM exact-path inventory.
State/protocol units параллельны лишь при отсутствии shared package-copy/live-image
collision; конкурентный measurement не называется controlled throughput.

| Source-scoped reuse | Новая проверка S5 |
|---|---|
| S1/S2 trie/absence/range/100k/allocator | Migration namespace/refs/backing/Ready root/mixed reader.100k не повторять без изменения соответствующего normal append/page/allocator; при таком изменении нужен отдельный root plan |
| S3 Native config1/Guard2 checked arithmetic/mixed/static proof | Production origin witness/current Guard1+seen/cut/source bounds и real Native continuation. Pure facts не file migration |
| S4 Root6/Native2/fresh policy/route bounds | Exact untouched bytes/branches и адресный fresh reopen smoke; actual new Root7/dual ALPN/Selected/current-purpose guards проверить заново |
| S4 offline/full64/C2/own relay | P5 повторяет применимые свойства с existing origin/mixed bytes; fresh trace не заменяет S5 |

Reuse требует exact file/function/branch diff и final EXE hashes. Если общий reader/
commit/gate изменился, old PASS не приписывается новой ветви: адресный contributing
check и его время входят в set. Strict Clippy all-targets затронутых packages,
fmt/locked build/helper syntax/diff/link отдельно. Failure/ceiling=>сохранить original
result/log/source и callback root. Никаких quiet repeats/reset clocks/PASS из prefix+suffix.

## 12. Disposition review1, implementation cuts и stop этой сдачи

| Finding | Изменение контракта / проверка будущей реализации |
|---|---|
| R1 | §4.1: intent отдельный, total named projection в accepted stages1..5, incomplete stage1 без фиктивного ALPN2 scope; M04/M06 encoding/shape negatives |
| R2 | §7.1.1: pre-Q scope/current admission и все typed handlers, отдельный Receipt1 verifier; reachable asymmetric states и K retry без symmetric Q; ALPN1 refusal уже P/Prepared (§7.1–7.2). M06/P5 реальные reconnect/Head/nonempty old receipt debt без добавления long suite |
| R3 | §6.1: authenticated LegacyBudget leaf23/47, exact owner/used/reserved/covered equations, load/every-commit checks и atomic hold/funding/release. §6.2: immutable identity/initial audit vs capacity-only authenticated CAS, effective publication plan. M08 boundary/reopen/C2/delayed-ACK/tamper/config-fault cases |
| R4 | §3.2: two-frontier executable fold, eligible continuation/own tie, no-progress rejection, causal checks и exact final Guard/Seen; M01 small cases, zero Native replay/constructor |

Это disposition документа, **не PASS реализации**. Contract-only local checks
подтверждают links/anchors, отсутствие whitespace errors и сохранность protected
inputs/исторического evidence. C/F/P и новый код на этом этапе не выполняются.

После contract acceptance root может поручить review/publish отдельных exact пакетов:

1. Infrastructure controls, typed Root7/source verifier/fence/copy/Ready/read-only
   migration API, integrity/version/quota/kill evidence. Это ещё не успешный S5 без
   actual indexed L1 remote.
2. Indexed L1 owner/transport/history/request/receipt/escrow; real d9b compatibility,
   incomplete handshake/exhausted512; явная LegacySnapshot amendment, закрытая после Ready.
3. Existing Consent2/control stages/Selected receive-only/mixed closed writer/
   Native fold/cuts/old ACK/current-policy/fault gates.
4. Один whole P5+final F/C/static, complete source/evidence correspondence, failures
   ledger и exact implementation allowlist → root final review.

Это логические cuts, не сокращение итоговой приёмки. A3-1 resumed L1, A3-7 migration/
existing-control crashes и A3-8 preservation/compatibility должны быть закрыты.
Whole A3/MVP, public rollout, GUI/groups/multi-device/mailbox/M1.2.63 и crafted A2
relay DoS этим документом не закрываются.

На contract-only этапе Rust/tests/Cargo/DB/EXE не менялись, fixtures/migration/
acceptance/heavy tests не запускались. Root-owned task/README/next-stage/coordination/
provenance decision и historical manifests/evidence/development-environment сохраняются.
После local-link/diff проверки — substantive callback root с size/SHA256 и R1–R4
disposition; принятые направления D1/D2/D4 и D3 in principle сохранены, D5 неизменён;
**stop before implementation/commit/push** до явной приёмки конкретного пакета.
