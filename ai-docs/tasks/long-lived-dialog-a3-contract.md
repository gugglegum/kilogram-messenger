# A3: долговечный личный диалог и индексируемая история

2026-10-24, Asia/Yekaterinburg. **Контракт принят после исправлений R1–R3:
[root review2](long-lived-dialog-a3-contract-review-2.md). Реализации пока нет.**
Основание — [task](long-lived-dialog-a3-task.md), принятые
[OD1](lossless-sync-a1-bidirectional-contract.md) и
[R1–R4](lossless-sync-a1-bidirectional-implementation-review-2.md).
Текущий baseline `860b351fb747562e1fe12d469b1ad053217a7c0f` включает supporting A2,
Iroh1.1.0/redb4.2.0/vodozemac0.10.0; зависимости этим контрактом не меняются.
Проверены actual turn `gpt-6.1-sol/max` и global `service_tier=default`; tier отдельно
в turn metadata не сериализован, противоречащего override не обнаружено. Human
поручение `01a16bf9-89cd-7cb3-8661-36eef244e0fc` подтверждено через read_thread.

## 1. Решение и измеримый исходный предел

**Рекомендация:** непрерывные application ordinals и та же native session/epoch,
индексируемый authenticated store schema5, cursor history из двух lanes,
rolling storage credits вместо оплаты всего lifetime. Один явный переход L1→L2
меняет формат/правила, **не** Olm account, E/D/SID, предыдущие ordinal или историю.
Периодических rollover/новых genesis каждые512 сообщений нет.

Существенные альтернативы:

| Вариант | Решение |
|---|---|
| Поднять LIFETIME | Не решает prepaid reservation, growing seen-set, полный index/manifest и history rebuild |
| Новая Olm epoch каждые512 | Возможна только с отдельным retained-old-epochs протоколом; добавляет accounts/OTKs, concurrent cutovers и старые native states без необходимости: native index уже u64 |
| Непрерывная session + индекс | Выбрано: сохраняет проверенный predecessor/ClosedTail порядок, требует одного format upgrade и нового storage API |

Baseline проверен чтением кода и принятых измерений, без повторного большого corpus:

| Связь в текущем коде | Наблюдение → требуемое изменение |
|---|---|
| [Core::parse](../../crates/kilogram-protocol/src/ordered_dialog_v1.rs#L302), [Guard](../../apps/kilogram-cli/src/ordered_dialog/ratchet.rs#L25) | n/g/seen привязаны к512; seen сериализуется в каждый private checkpoint → u64 и durable seen-key index |
| [queue/materialize](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L106) | lifetime учитывает queue;128 queued/128 unacked; materialization требует lease → L2 disk queues, bounded scheduling и отдельная local authorization |
| [repair](../../apps/kilogram-cli/src/ordered_dialog/repair.rs#L127), runtime hints | end/send/receive≤512, но receiver-driven prefix уже подходящий → u64 ranges, bounded32/bytes, funded window |
| [OrderedVault](../../crates/kilogram-state/src/vault/ordered.rs#L48) | RAM BTreeMap всего index; paths фильтрует весь map, commit клонирует/сериализует old/new index и MAC → authenticated point/range operations |
| [history](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L345) | каждый page собирает paths; любой append инвалидирует cache; rebuild читает все events и выполняет topo sort, worst O(N²); offset требует prefix traversal → стабильный cursor без полного rebuild |
| [vault integrity](../../crates/kilogram-state/src/vault.rs#L1949), ordered open/recover | весь manifest index MAC; startup verify/load_records/shadow scan → фиксированный root, authenticated paths и отдельный scrub |
| L1 quotas |128MiB/C оплачивают 2×512×128KiB навсегда; index8MiB/delta32MiB/logical384MiB/physical2GiB и generic snapshot512MiB не допускают A3 простым изменением n |

Принятый [maximal-index test](lossless-sync-a1-bidirectional-evidence/final-storage-after-history-cache.log):
18,958 records, index2,712,820B, old/new plan6,740,456B. Это L1 bound, не100k storage test.
[Текущий A2 mixed run](transport-security-a2-root-review-evidence/mixed-result.json):
64 Text/317 commits/62.30s; [reopen](transport-security-a2-root-review-evidence/mixed-reopen-result.json)
272 immutable/native hashes/4.47s. EXE на fixed field path сейчас действительно
`d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`.
Перед первой product edit этот executable сохранить по постоянному пути
`target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe`, проверить bytes hash.

Небольшое read-only измерение: public outer/Core metadata258 signed Event файлов из
прежнего synthetic duplex256 (`.tmp/ordered-dialog-process/0aafb6b6f4cc488fa08c4998e8073cc9/one.state/events/od1`).
232,954B,256 Text=128/128, у всех n>1 собственный предыдущий Text есть в parents.
Слияние §5 дало **тот же** canonical order:16337 eligibility checks старого алгоритма
против504 candidate checks/878 parent point lookups. SHA256 списка emitted IDs
`e5d63461cb6bdd4e6cad212cc9767e20058b215dd0bd37aeba45c1e9838a3905`.
Одноразовый Node/std::fs reader/CBOR subset,221ms включая чтение и оба алгоритма;
без native secrets, записи файлов, запуска runtime или генерации новых fixtures.
Это проверка алгоритма на принятом историческом input, **не** новый storage/E2E benchmark.

## 2. Непрерывность, формат L2 и authority

Числа canonical unsigned u64, checked arithmetic; byte strings/arrays и deterministic
CBOR как OD1. n, native index, g, author_sequence и storage generation различны.
Все encryptions имеют один contiguous n/lane; n0 — только genesis, n≥1 — Text.
Никаких hidden encrypted controls/key pings. Parent/previous/ClosedTail/DH-cause,
реальный native header, inner==outer и trial rollback сохраняются.

ALPN2=`kilogram/m1/ordered-dialog/cbor/2`, crypto suite1, limits2. Frame2 —
`[2,kind,B,request16,body,signature64]`; kinds1–17 наследуют назначения OD1,
18–21 определены в §3. Domains меняют `kg:od1:` на `kg:od2:` для signatures,
IDs, transcript/consent/descriptor и exporter binding. Exporter label
`kilogram ordered-dialog v2`, context `[ALPN2,dialer_endpoint,listener_endpoint]`;
на wire только hash exporter. Version1 decoder/limits **не ослабляются**.

```text
Core2 = [2,C,D,E,SID43,role,n,previous?,author_sequence,kind,request16,
         sorted_user_parents<=2,expected_native,author_auth_ref,mode_ref]
Event2 = [2,Core2,NativeHeader,olm_bytes,device_signature64]
Inner2 = [2,Core2,payload]
DataItems2 = [[format=1|2, exact_Event_bytes], ...]
```

NativeHeader/expected_native/ClosedTail имеют прежние формы, но без512 predicates.
Event2 signature/ID охватывают canonical Event2 с новыми domains. В existing C
D/E/SID прежние; `Core2.mode_ref=Q` привязывает новый формат к ModeProof2 (§3).
Получатель определяет inner format по уже проверенному outer, не пробует decoders
до успеха. Exact old Event1/ID/Inner1/signature остаются прежними.

Новое согласие двух Account Roots:
`ConsentBody2=[2,C,certA,certB,1,2,origin,offline_encrypt=1,not_before,expires]`,
`origin=[0]` для fresh либо `[1,D,E,SID]` для existing. Domain `kg:od2:consent\0`;
Consent2=`[body,rootA_sig,rootB_sig]`,≤4KiB. Scope/membership/capabilities/монотонные
locally pinned authority и revocation как OD1; Root secrets только в admin ceremony.

Hello2 расширяет Hello1 последним `origin`; proof/admitted выбирают suite1/limits2
и exact certs/consent. Existing origin обязан совпасть с локально selected session.
Stage/mode_ref peer сообщает в Resume2; сам hint не разрешает data или смену mode.
Канал с peer на другой стадии допускает только необходимые controls/старые bytes,
пока нужный certificate не проверен и не установлен. Текущие consent refs должны
совпасть; исторический selected pair/certificate проверяется с исходными подписями,
но **никогда не заменяет current authorization**. R1 действует и при renewal.

**Нормативное изменение относительно L1:** Active2 разрешает *локальную* durable
encryption при offline peer. Требуются current local Root2 consent/authority обеих
сторон, durable activation по своему origin (§3), ModeProof2 и generation fence;
live TLS lease не нужен.
Это явно разрешено `offline_encrypt=1` обоими владельцами. Новых revocation oracle
нет: используются последние локально известные signed revisions. Истечение consent,
известный revoke или clock/read failure запрещают новое local encryption.
Dispatch/receive/ACK/upgrade по сети по-прежнему требуют свежего admitted TLS lease
и повторной current policy проверки перед commit. Публикации вне admitted channel нет.
L1-context до завершения upgrade сохраняет прежнее требование lease.

Queue idempotency — authenticated lookup `(C,request16)` до проверок новой capacity:
same bytes→прежний Queued/EventId без writes, иначе RequestIdConflict. Для *нового*
request нужны policy и funded storage credit. Queue/undelivered теперь могут быть
большими на диске, window128 относится к планировщику/сети, не числу долговечных записей.
Materialization atomic: FIFO queue→ciphertext/source/indices/projection/native/request
mapping/undelivered+credit transition. До commit ничего не передаётся; после commit
retry только exact bytes. Volatile failed trial не является published encryption.

Guard2 держит только текущие two-lane cursors/key/index/g/first/parent/tail и rotation
flags. Seen keys — authenticated `(C,role,key32)→g/first_id` index, проверяемый перед
новым DH и изменяемый с native commit. Никакого растущего RAM set/Bloom decision.
Native vodozemac session config1/public pickle APIs не меняются. Локальный source
подтверждает u64 chain indices, receiving chains≤5 и skipped keys≤40/chain; ordered
ingress не создаёт skipped gaps. Native+guard cap128KiB проверяется на serialized output.
До native call выполняется предикат §2.1, включая внутренние *следующие* counters;
CounterExhausted не меняет accepted state, без wrap/reset/нового lifetime budget.
`AuthRef=H(raw_credential_bundle)` остаётся прежним; changing wire domains не меняет
идентификатор уже сохранённого credential. Optional fields используют прежние []/[x].

### 2.1. Native counter preflight (R3)

Pinned vodozemac0.10.0: `olm/session/chain_key.rs:81–124` выдаёт key с index j,
затем делает j+1; `double_ratchet.rs:205–213,260–273,299–325` увеличивает Known
RatchetCount при receive нового DH и при activate следующего собственного DH.
Поэтому representability одного wire index/g недостаточно. Для каждого шага:

- Все n+1, author_sequence+1, generation+1 и новые g — checked. Для consume key j
  требуется **checked(j+1)**; continuation j=checked(last_index+1), новый chain j=0.
  Native post-index ровно j+1; index u64::MAX никогда не передаётся native API.
- `T(role,g)=checked(checked(2*g)+role)`, A=0/B=1, — native DH turn этой chain.
  До new-DH call требуется checked(T_pre+1)=T_post; continuation не меняет T.
  Исключение — исходный A0: outbound constructor уже Active(T=0), без DH increment.
  Pre-inbound A0 создаёт Inactive(T=0) и remote post-index1. B0 activate даёт T=1.
- T_pre выводится из authenticated Guard: если rotate_due, это T последнего нового
  peer chain (native Inactive); иначе T own chain (Active); до A0 — исходный0.
  Own/peer first, used_own_first, parent/ClosedTail обязаны доказать эту фазу до call.
  Новый peer DH в Inactive запрещён: иначе native мог бы выполнить два advance.
  Same-key late tails используют существующий remote chain, T sender не меняют.

Здесь A0/B0/A1/B1 — **DH chains**, не обязательно Text ordinals:

| Первый key chain | g / native T / next symmetric index после consume | Native действие |
|---|---|---|
| A0 Init |0 /0 /1 | A уже Active0; B inbound Inactive0 |
| B0 Confirm |0 /1 /1 | B activate0→1; A new receive Active0→Inactive1 |
| A1 |1 /2 /1 | A activate1→2; B new receive Active1→Inactive2 |
| B1 |1 /3 /1 | B activate2→3; A new receive Active2→Inactive3 |

Индукция: Ak имеет T=2k, Bk — T=2k+1; каждый следующий DH требует предыдущий peer
first и новый own first. Continuations меняют только symmetric index. После native
trial public Packet.header подтверждает exact key/j; guard+native сохраняются одним
commit. RatchetCount не читается через private pickle/debug: public API его не
экспортирует; вывод опирается на проверенные constructors/переходы и authenticated
checkpoint provenance. Fresh2 начинает с этих constructors; imported L1 — только
принятый OD1 Guard1/native checkpoint с проверкой source/first/parent/tails/SID/config,
g≤511 (T≤1023), включая pre-session optional phases. Миграция выводит фазу и T из
того же guard, без native calls/reset. Произвольные libolm/unknown-provenance pickles
не являются supported L1 import и не принимаются этим путём.

Future pure checked-arithmetic vectors: j=M−1→post M разрешён, j=M отклонён;
continuation last=M−2 разрешён, last=M−1 отклонён; T=M−1→M разрешён, T=M→next
отклонён; g=(M−1)/2 даёт A:T=M−1/B:T=M, g+1 переполняет T. M=2^64−1.
При T=M same-chain receive/send разрешён при остальных bounds; следующий DH нет.
Vectors проверяют preflight, не создают huge native states и не заменяют >512 test.

## 3. Однократный L1→L2 handoff существующей session

Сначала storage upgrade §7, затем отдельное согласие/activation для каждого C.
Для handoff нужны complete L1 Ready и оба valid Consent2 с existing origin. При
incomplete initial handshake новый binary сначала заканчивает его в L1 compatibility
mode; миграция хранения сама по себе не создаёт session/OTK и не обходит этот gate.

A остаётся единственным dialer/coordinator. После fresh admission ALPN2 A атомарно
замораживает **только local send materialization** на `cutA=own.next`, записывает P.
B при приёме P аналогично фиксирует cutB. Receive/old prefix/ACK продолжаются.
Долги/queue/pending не обязаны быть пустыми; полный native blob hash в P не включается,
потому что receiving cursor законно меняется во время handoff.

Signed controls2 — `Control2=[2,kind,body,device_signature64]`, signature по
`kg:od2:control-sign\0 || enc([2,kind,body])`, ID по
`kg:od2:control-id\0 || enc(Control2)`; independent of TLS B:

```text
kind18 P = [C,D,E,SID,consent2_ref,nonce32,cutA,prevA] // signed A
kind19 A = [C,P_id,cutB,prevB]                       // signed B
kind20 K = [C,P_id,A_id]                         // signed A
Certificate2 = [2,1,P_raw,A_raw,K_raw,selected_Consent2_raw] // existing ModeProof2
Q = H(kg:od2:mode-certificate\0 || enc(ModeProof2))
kind21 Committed = [C,Q]                        // signed B, durable
```

Wire2 kind18 body=`[P_raw,selected_Consent2_raw]` (оба bstr), kinds19–21 body=`[control_raw]`.
A сохраняет raw consent вместе с P. Даже если до первого B Prepared consent renewed,
B может проверить старое выбранное P: exact consent_ref, обе Root signatures и тот же
existing scope/permissions плюс **current** valid Consent2/authority. Произвольного
старого hash/peer signature недостаточно. Renew не заставляет A менять уже frozen cut.

P/A содержат prev собственного source на cut−1;1≤cutA,cutB≤512. Каждый signer
проверяет свой exact send cursor/prev. Receiver проверяет cut>всех уже принятых V1
ordinals этой lane, включая pending. Не требуется уже иметь весь старый prefix:
peer prev — signed обещание, которое обязательно совпадёт с last_id при достижении cut.
Несовпадение — signed fork/block C; перед ним native cursor не перескакивает.
Кроме accepted slots V5 durably хранит monotonic peer source watermark из проверенных
Resume/Prefix headers (compact local authenticated observation). Cut не ниже уже
наблюдённого source_next; при равенстве prev совпадает с его last_id. Старые delayed
headers не понижают watermark. При импорте V4 известны только реально сохранённые
свидетельства/accepted slots; sender всё равно фиксирует свой durable source head.
Таким образом новый формат не заменяет уже объявленный старый prefix меньшим числом.

| Durable state | Разрешено / restart |
|---|---|
| L1Active, intent only | Обычные L1 rules; отмена intent разрешена; old binary peer ещё совместим |
| A Offering2(P) | Send frozen; повторяет exact P, принимает old data/ACK. После durable P rollback в L1 не выполняется: B мог сохранить A при потерянном ответе |
| B Prepared2(P,A) | Send frozen; повторяет exact A. Old receive/repair разрешены, новые Event2 до K не принимает |
| A Selected2(Q,K) | Атомарно сохраняет certificate/cuts/Guard2, повторяет K; принимает Event2 от B, но local new send ждёт Committed |
| B Active2(Q,Committed) | Commit certificate/Guard2 и durable Committed до ответа; новые send/receive разрешены |
| A Active2(Q,Committed) | После проверки/commit B proof разрешает local send; оба возобновляют сохранённую очередь |

Control bytes immutable, duplicate возвращает сохранённое; conflicting P/A/K/Q
создаёт bounded signed-fork evidence и блокирует только C. B не выбирает competing
P; simultaneous operator intents обслуживает A. Crash/обе стороны offline сохраняют
stage и cuts; после fresh reauth повторяются controls, не genesis/OTK. Renewal меняет
current consent, не selected Q. После P/Prepared допустима pause, не reset/rollback.

После установки Q: `format1 iff n<cut[role]`, `format2 iff n>=cut[role]`.
Старые источник/receipts/pending/EventIds/request IDs не переписываются. Exact old data
идёт через DataItems2 с format1. Новая chain может прийти раньше старого tail, но
native trial всё равно ждёт exact next/prev и старый ClosedTail. Receipt1 допустим
только для legacy event ниже cut; Receipt2 связывает format/EventId/C/D/E/SID/role/n/Q.
Already durable Receipt1 сохраняется byte-exact; ни один wrapper не превращает
possession/claim readable в Delivered.
AckItems2 также tagged `[receipt_format,exact_receipt_bytes]`: новый body не переопределяет
старые подписи/IDs. Исторические author/admission/final/receipt snapshots — те же
четыре capture roles из OD1 §11; последующие policy refresh остаются bounded current
pins, не создают ещё одну permanent копию на каждый duplicate ACK.
`ReceiptBody2=[2,C,D,E,SID,Q,event_format,lane,n,EventId,receiver_cert_ref,receiver_auth_ref]`;
Receipt2=`[body,receiver_signature64]`, domains `kg:od2:receipt-sign\0`/`receipt-id\0`.
Signer — exact opposite-lane Device; source сравнивает каждое поле со своим immutable
event и current policy. До известного Q при handoff old data получает Receipt1.
Однажды сохранённый receipt не заменяется на другой format при duplicate/query.

**Единый reference (R2):** Q — 32-byte hash ModeProof2 по формуле выше, для обоих
origin. Proof immutable,≤16KiB; известный Q в Core2/Receipt2/Mode2/Resume2 обязан
совпадать с локально verified proof. Peer hint его не выбирает. Renewal сохраняет
selected proof/Consent2 bytes и Q, но current authorization проверяется отдельно.

| Origin | Exact ModeProof2 / activation |
|---|---|
| Fresh | `[2,0,C,selected_Consent2_raw,Offer_raw,Grant_raw]`; проверяются обе Root signatures, origin fresh, device signatures, exact selected initial Hello pair/transcript/certs/suite1/limits2 как R1. Q известен **до Init**, не зависит от SID/genesis/Ready, циклического hash нет |
| Existing | Certificate2 `[2,1,P_raw,A_raw,K_raw,selected_Consent2_raw]`; проверка §3 и старых Ready1, cuts из P/A; activation — таблица Selected/Active выше |

Fresh L2 использует версию2 Offer/Grant/Init/Confirm с suite1/limits2, одним fresh
account/C и единственным reserved OTK B; cuts=[0,0], format1 никогда не принимается.
Init/Confirm уже имеют Core2.mode_ref=Q. n0 request16 и пустые user parents проверяются
**до native decrypt** (R4). Предвыборные phases сохраняют реальные optional D/E/SID,
без fake IDs (C1). Fresh Ready2 — Control2 type3 с body
`[D,E,SID,A0_id,B0_id,role,1,2,Q]`, подпись и ID по domains Control2. Ready2 создаётся
и сохраняется с точными genesis/native bindings до publication. Локальный Fresh
переходит из FreshHandshake в Active2 **одним commit**, в котором доказаны оба
readable genesis и оба durable Ready2 с этим Q; обычно это import последнего peer
Ready2 при уже сохранённом own Ready2. Нужны fresh lease/current policy/generation.
До commit Text/local offline encryption запрещены. После commit offline encryption
использует тот же Q и local Root2 policy; никакого P/A/K для fresh нет. Active peer
повторяет own Ready2 при reconnect, если другой ещё FreshHandshake: до activation
тот отвечает NeedHandshake на Text без possession/ACK. Existing Ready1 не переписывается.

Для fresh2:
`E=H(kg:od2:epoch\0 || enc([2,C,certA,certB,Offer_raw,Grant_raw]))`,
`D=H(kg:od2:domain\0 || enc([2,1,2,C,certA,certB,E]))`.
Для existing origin эти формулы **не** пересчитываются: его исходные D/E сохраняются.

Два нормативных restart traces (future tests сравнивают exact Q во всех полях):

- Fresh: Proof(Q)→A0/B0→own Ready2 durable→kill до peer Ready2 commit: reopen остаётся
  FreshHandshake, offline send отказан. Retry Ready2→atomic Active2(Q)→kill/reopen→
  первый Text/Receipt2(Q)→peer offline→следующий durable Text(Q). Никакой reselection;
  mismatched Q в genesis/Text/Ready отклоняется до native/state commit, в receipt —
  до снятия undelivered. Уже accepted IDs/bytes сохраняются.
- Existing: A Selected(Q)/B Prepared→K→B Active(Q), Committed lost→kill обоих→reopen
  сохраняет Q; A может принять Event2(Q), но не materialize своё. Exact K retry→старый
  durable Committed(Q)→A Active(Q)→offline send(Q). Renewal не меняет Q, конфликтный
  reference не заменяет proof/accepted bytes; новая current policy обязательна.

## 4. Schema5: point/range storage с incremental integrity

Один redb4.2 database/OS owner lease, прежний protected root и key protection.
Новый `IndexedDialogVault` не использует whole `VaultManifestIndex`/paths scan и
per-message filesystem shadow. Generic legacy/OrderedVault mutable APIs schema5
не обслуживают. Fixed namespace set≤64; contexts≤2, global device-sequence indices
только для pinned local+remote signers (≤3). Старый legacy inventory4096 не меняется.

| Namespace / logical key | Value / invariant |
|---|---|
| Event(C,EventId256) | Immutable exact format+event bytes, либо verified legacy reference |
| Lane(C,role,n64) | Immutable EventId; contiguous committed prefix, no alternative ID/slot |
| Projection(C,EventId) | Readable immutable sealed UTF8/request/Core binding; metadata parents→role/n |
| Request(C,request128) | Queued enqueue_seq или Materialized EventId; body conflict проверяется |
| Queue(C,enqueue_seq64) | Sealed body + retained credit; FIFO, хранится до materialization |
| Undelivered(C,n64) | Indexed obligation + future receipt credit; min/range≤128 без всего backlog |
| Receipt(C,role,n64) | Immutable verified receipt/provenance; старые ACK query работают |
| Pending(C,n64) | Exact event/original+admission auth refs/status/credit;≤64+head |
| Seen(C,role,key256) | Immutable g/first ID; replay/reuse test через index, не pickle set |
| Auth(C,hash256) | Exact immutable captured bundles;≤4 retained refs/encrypted frame |
| Author(signer namespace,sequence64) | C/EventId, глобальная cross-C fork проверка; allocator общий |
| Context/admin/control | Constant current heads/native/policy/selected controls/repair grant/ACK cursors |

Event/Projection/Receipt/Auth и единственный источник ciphertext не GC. Queue удаляется
только с committed materialization. Pending — только с readable commit/явным исходом
без потери accepted bytes. Undelivered — только с verified receipt. ACK intents
агрегируются cursor/ranges поверх immutable receipts: Sent не означает peer possession;
lost ACK восстанавливается query, поэтому per-message Sent tasks не хранятся вечно.

**Authenticated map:** binary compressed radix trie над fixed logical keys≤256bits,
значения и nodes зашифрованы XChaCha20Poly1305; BLAKE3 keyed hashing/KDF как vault.
Новых crypto primitives нет. Namespace/C/type/keys входят в domains и AEAD AAD.
Separate BLAKE3 derive-key contexts имеют ASCII prefix `kilogram a3 v5 ` плюс exact
suffix `node encryption`, `value encryption`, `integrity`, `lookup`, `cursor`;
ключи выводятся из прежнего vault master key. Envelopes — version+random24-byte
nonce+XChaCha ciphertext/tag; metadata
AAD включает schema5/profile incarnation/namespace/record address. Native/Root keys
не используются вместо vault keys и не экспортируются в index/cursor.
Node address — keyed hash namespace+entrance prefix; compressed node хранит actual
prefix. Child address выводится из branch prefix+edge, child digest хранится в parent.
При split/merge переносится только root child node, не весь subtree. Leaf связывает
logical key, value hash/length и credit charge. Branch — prefix, two child hashes,
count и used/reserved counters по двум C; envelope≤256B. Отсутствие — проверенный
empty edge/prefix mismatch, не произвольный redb NotFound. Missing expected node/value
или неверный proof = IntegrityBlocked, **не** повод создать повторный request/event.

Leaves/value objects и изменённые пути обновляются в одном redb WriteTransaction с
root anchor. Immutable namespaces допускают только put-if-absent-or-byte-identical.
RootAnchor≤32KiB: schema/claim/generation,≤64 roots, quota/counter summaries,
migration state refs; ни paths, ни список всей истории туда не входят. Старые COW
node версии освобождает redb после коротких read transactions; ненужные radix nodes
удаляются в той же mutation. Персистентных cursor snapshots, удерживающих старые roots,
нет. Integrity проверяется на access; whole-snapshot rollback не получает новой
гарантии anti-rollback без внешнего trusted counter, как и прежний local vault.

`lookup/prove_absent`, `insert_unique`, `replace_mutable`, `first/range(limit)` и
atomic `commit(expected_generation,plan)` — единственные data APIs. Source prefix
читает exact n, не unauthenticated range с возможностью пропустить leaf. Ordered
queue/undelivered iterator идёт по authenticated tree; min undelivered определяет
contiguous delivered frontier без O(backlog) scan. Предварительная сборка/валидация
plan включает все data/nodes/credit изменения; SQL/redb error ambiguous → invalidate
workspaces, reopen и resolve commit_id/request mapping до retry/публикации.

Application clean reopen читает anchor/heads/current native и их proof paths, не всю
историю. Инициализация allocator redb и unclean repair могут зависеть от числа file
pages; их стоимость измеряется отдельно, clean DB-open тоже не объявляется O(1).
Полный integrity scrub/rebuild/compact — explicit
streamed maintenance с cursor/cancel и disk preflight, никогда часть каждого append/page.

## 5. History API: bounded canonical cursor

Два author prefixes дают только два кандидата. History2 добавляет implicit edge от
предыдущего Text той же lane; сохраняет все signed parent edges. Для conforming OD1
sender это уже существующий parents edge, поэтому порядок совпадает с прежним topo.
Для ранее принятого noncanonical V1 без такого parent добавляется только lane order;
event bytes/содержимое не меняются. Это явная version2 view semantics, не скрытая
перепись истории. Invalid/cyclic legacy metadata до upgrade даёт диагностический
отказ и доступ к прежней read-only history, не silent data repair.

Snapshot=`F=(committed_A_next,committed_B_next)` захватывается атомарно. Own authored
projection и peer readable prefix оба causal-closed: каждый wire parent уже committed
в этом C/E; unknown/cross-C parent invalid до native commit. Cursor содержит profile
incarnation/C/D/E/SID,F,next_A,next_B и persistent device-local MAC;≤384B binary,
Base64≤512chars. В нём нет native keys, нет server-side session/cache/expiry requirement.

```text
first: next_A=next_B=1, freeze F
page(cursor,k):
  get at most one next Text from each lane below F
  eligible iff every parent ordinal < next[parent.role]
  emit smallest eligible EventId; advance only that lane
  repeat at most k times; return updated authenticated cursor
```

Parent role/n приходит из authenticated metadata, не из загрузки всех events. Implicit
previous уже покрывается последовательным next lane. Если eligible нет при remaining
rows — integrity error, не пропуск/пустой SyncComplete. Snapshot F causal-closed;
будущие append не входят в него. Source rows неизменны, поэтому cursor после restart
и concurrent append возвращает тот же suffix на текущем authenticated root без MVCC pin.
Следующая новая first открывает более свежий view. Limits1..32, CBOR response≤256KiB,
CLI JSON≤1MiB с escaping; Delivery status читается отдельно, не меняет immutable page.
Numeric offset для L2 не предлагается; point lookup EventId и cursor resume доступны.
L1 compatibility offset остаётся ограничен своим L1 dataset, не всей L2 историей.

Пусть d≤256 — radix path, h — redb tree height, k≤32, b≤4 batch:

| Операция | Затраты; без total-history arrays |
|---|---|
| Queue/materialize/receive/receipt | O(b·d·h + bounded bytes); d обычно O(log N), то есть возможен O(log²N), не O(N) |
| Duplicate/request/seen/fork lookup | O(d·h + одного bounded value) |
| Prefix/history page | O(k·d·h + output bytes),≤2 candidates и≤2 wire-parent point checks/row |
| Clean reopen | redb_open(P) + O(fixed heads·d·h + native/control bytes); allocator metadata отдельно, cache фиксирован |
| Full traversal/scrub/import | O(N·d·h + bytes), streamed; не каждый page/append |

## 6. Rolling credit, catch-up и честное исчерпание места

Default owner policy:16GiB retained/logical quota/C, physical profile ceiling48GiB;
меняется явно владельцем, не согласуется произвольно peer. Existing used bytes и
promised credits нельзя забрать shrinking/reassignment. Это bytes quota, не lifetime
counter; после добавления места тот же C/E/SID продолжает работу. Per-C counters/root
charges отделены; один C не тратит защищённые credits другого. Общий disk/DB I/O fault
может остановить оба, это явно общий ресурс.

| Объект | L2 hard bound |
|---|---|
| Text/Inner/raw Olm/Event/Auth |4/8/12/16/16KiB; Core2≤2KiB, membership32KiB |
| Consent/locator/selected pair/upgrade certificate |4/8/192/16KiB; control value≤512KiB, control budget4MiB/C |
| Wire/count |1MiB полный frame включая4-byte prefix;≤32 events; exact encoded-byte fit, не count-only |
| Pending |64×64KiB +1×64KiB head, all components charged; incoming ahead≤1024 ordinals как flow control, не lifetime |
| Scheduling |128 queue/undelivered refs в RAM; durable counts ограничены bytes quota, не128 |
| Permanent frame credit P |128KiB: event16+projection8+receipt1+до4 auth×16+indices/node growth/encoding запас |
| Native/checkpoint |128KiB/C; seen history отдельно в indexed store |
| Plan / native actor batch |16MiB old+new affected records/nodes;≤4 frames OR one admin/control operation |
| Physical writer policy |≤128MiB explicit backend write bytes/обычный commit; allocator set_len отдельно по фактической длине и profile capacity; headroom256MiB (§6.1) |
| Cache/buffers |redb cache64MiB явно (default library1GiB не использовать); A3-owned live buffers≤192MiB |
| Runtime admission |2 contexts/connections,≤4 bidi streams/connection, bounded IPC/network tasks и fair turns |

Net radix nodes≤2/leaf; charge leaf value плюс два256B nodes, поэтому permanent
per-frame128KiB включает index retention, а не каждый COW rewrite. До24 point mutations
на frame;≤260 affected nodes/mutation. Worst old/new nodes четырех frames:
`4×24×260×256×2=12,779,520B`;≤1MiB data old/new,≤256KiB current native old/new и
остальной fixed metadata укладываются в16MiB plan. Encode-bound test обязателен до
activation. При actual plan overflow batch уменьшается до одного до publication;
если один не fit — StorageBlocked/maintenance, не silent cap change/eviction.

Queue admission резервирует P+sealed body≤8KiB. Materialization превращает часть P
в actual retained bytes, оставляет debt credit для receipt/auth; unacked source
сохраняет debt до verified receipt. Pending admission отдельно оплачивает temporary
64KiB **и P будущего readable commit**. Already accepted IDs всегда остаются в ledger.
Queue, materialized debt и pending могут сосуществовать; ledger держит ownership и
reserved/used суммы, не считает одни bytes свободными в двух состояниях.

Защищённый repair pool64MiB/C не продаётся ordinary queue/admin writers. Receiver
выдаёт один durable `RepairGrant=[id,start,end,unissued_bitmap,credits]`, window≤256.
Каждый отсутствующий ordinal получает P; уже принятые pending имеют свой P. Для256
missing нужно32MiB, не lifetime128MiB. Remote source_head произвольной величины не
создаёт пропорциональной allocation. Head-only имеет отдельный temp64KiB slot и token
из grant; ordinary future64 могут быть заполнены. После head commit temp освобождается,
но retained P **потрачен**: следующий head берёт следующий prepaid token, не эти64KiB.
После окна pool replenishment только из реально оставшейся quota. Без пополнения
новый grant не обещается: StorageBlocked со всеми сохранёнными bytes. Нет обещания
бесконечного прогресса на конечном диске. Cancel освобождает лишь unissued tokens;
accepted pending/debt остаются с принадлежащим им credit, без eviction.

Передача grant token в pending credit меняет владельца reservation, не резервирует
P второй раз. При readable commit actual retained bytes переходят в used, неиспользованный
остаток возвращается origin pool. Уже принятый ordinary pending сохраняет свой P и
не получает второй token при включении его ordinal в новый repair window.

Точные body layouts для changed kinds (остальные наследуют OD1 с version/suite/limits2):

```text
Cursor2=[next_n,last_id?]; Status2=[reason,position?]
Mode2=[stage(0=FreshHandshake,1=L1,2=Offering,3=Prepared,4=Selected,5=Active2),Q?,[cutA?,cutB?]]
GrantHint2=[grant_id32,start,end]
Credit2=[ordinary_slots,ordinary_bytes,head_temp_bytes,max_events,max_wire_bytes,
         max_ahead_ordinal,grant_hint?]
9 Resume=[C,D,E,SID,handshake_ids,send_cursor,receive_cursor,Mode2,Status2,Credit2]
7 EpochCipher=[C,D,E,event_format,exact_event_raw]
10 Deliver=[C,D,E,SID,lane,AuthRecords,DataItems2]
11 PrefixReq=[C,D,E,SID,lane,start,end,expected_prev?,Credit2]
12 PrefixResp=[C,D,E,SID,lane,start,actual_end,source_cursor,AuthRecords,DataItems2,more]
13 Complete=[C,D,E,SID,related_request16,exact_all_requested_EventIds]
15 AckBatch=[C,D,E,SID,lane,AuthRecords,AckItems2,not_readable_slot_pairs]
16 AckQuery=[C,D,E,SID,lane,slot_id_pairs]
17 Head=[C,D,E,SID,send_cursor,receive_cursor,Mode2,Status2,Credit2]
```

Все lists count≤32 кроме фиксированных tuples; AuthRecords sorted unique, unused
entries запрещены. Existing stages до known Q допускают только format1 data; Selected
принимает format2 с проверенным Q. FreshHandshake допускает только необходимые
format2 genesis/Ready, не Text; Q? пуст до selected Grant, затем Some(Q), cuts Some(0).
Head-only: slots0, max_events1, end=start+1,
head_temp_bytes64KiB, max_wire_bytes68KiB, ordinal внутри funded grant. Token bitmap
локален и не передаётся peer. Credit — snapshot, receiver перепроверяет его в commit.

Resume2 сообщает source/readable cursors+last IDs, mode/cuts, bounded credit/window,
а не весь inventory. Prefix2 ограничивает end-source_head и count32, проверяет prev,
возвращает actual_end/more. Источник читает exact sequential slots; legacy Event1 ниже
cut также доступен. Partial durable possession → RoundError/Partial с exact saved IDs;
Complete только всей заявленной пачке. Readable/causal/ACK frontiers различаются.
Новый packet далеко впереди credit/ahead window получает Backpressure/NotPossessed,
а не false ACK. Auto drain продолжает head после reopen без нового входящего packet.

### 6.1. Production redb growth, headroom и recovery (R1)

**Нет отдельного hard cap на прирост файла/transaction.** Fresh production redb4.2.0:
page p=4096, usable region R=4GiB; `set_region_size` в `db.rs:1475` только cfg(test,
fuzzing), не используется. `page_manager.rs:1546–1593` для requested block r=p·2^order
до первого полного региона выбирает U'=max(2U,U+2r); затем дополняет trailing T и
добавляет регион: ΔU=R−T либо2R−T; без trailing ΔU=R. При U=128MiB/r=4KiB
обычный single append может потребовать ещё128MiB, и он **разрешён**.

`layout.rs:154–199,222–239`: L(U)=p+U+ceil(U/R)·H, H — bytes регионального header;
у новых production v3 H=0, initial p=4096. При import p/R/H берутся из сохранённой
geometry самой redb; backend не парсит/переписывает header. Для buddy block r≤R один
grow имеет ΔU≤2R, добавляет≤2 regions, ΔL≤2R+2H; при v3 общий консервативный
bound одного grow **8GiB**, не обещание, что обычный append выделяет giant block.
Точный пример v3:134221824→268439552B, Δ134217728B. Это bound одного allocator
request, не новый per-transaction ceiling; все actual resize учитываются отдельно.

Policy — тонкий file-backed StorageBackend, без собственного allocator: перед каждым
**фактически запрошенным** set_len(L') проверяет checked `L'+F+S≤physical_quota`,
где F — retained файлы профиля вне этой DB, S=256MiB commit/recovery headroom.
OS free preflight при росте требует max(0,L'−L)+S; это наблюдение, не OS reservation.
Нет требования заранее держать8GiB свободными на каждый append и нет отказа только
из-за ΔL>64MiB. При достаточной capacity backend исполняет exact set_len с нулевыми
новыми позициями и sync, без большой RAM zero-buffer; zero-fill/extension I/O и
latency учитываются **отдельно** от explicit write budget128MiB и cache64MiB.
Суммарные actual length/growth/write/sync измеряются, app buffer bounds прежние;
расширение может стоить O(ΔL) работы OS, не выдаётся за маленькую encoded запись.

Нет whole-history backup. Physical ceiling48GiB включает DB/F/S; отказ лишь если
точный запрос не помещается в configured capacity/headroom, OS free/IO failure или
нарушен другой реальный bound. Результат различает ConfiguredCapacity и IOFailure,
с exact requested length; batch1 не считается решением allocator boundary.
Продолжение: owner увеличивает physical quota/доступное место, reopen/resolve commit_id
и повторяет тот же request в том же C, без reset/eviction. Logical repair credits не
гарантируют физическую allocation, progress условен успешным physical commit.

grow сначала resize+sync, затем публикует layout; при kill/error возможен durable
лишний suffix. Backend не обрезает его и не возвращает фиктивный success. После
ошибки workspace закрывается; native/request/root outcome разрешается только reopen
через штатные redb recovery/layout reconciliation (`header.rs:208–295`), затем lookup
commit_id/request. ACK/publication ждут доказанного commit. Старые bytes/debt сохраняются;
truncated/invalid layout остаётся corruption, а не поводом fallback/re-encrypt.

Actor уступает между bounded chunks/commits; C1 gap/quarantine не останавливает C2.
Нельзя держать native workspace/DB read transaction через network await. Current
authority/consent/time и generation перепроверяются в commit purpose: LocalEncrypt
либо PeerReceive/Dispatch(lease). I1–I4 сохраняются: operational rollback всего trial,
durable prefix при invalid следующего, immutable accepted bytes, gating до repositories.

## 7. Existing profiles: explicit storage upgrade и fences

Новый binary по умолчанию не мигрирует mode4. Owner вызывает explicit storage upgrade
под тем же lifetime StateDirectoryLock. Сначала old-v2 recovery, verify legacy index
MAC и preflight quotas/source inventory; active old writer → Busy. Generic/legacy
profiles не принимаются этим путём. Новый NativeAccount/DeviceIdentity не генерируется.
Fresh `device-enroll --profile-mode ordered-dialog-v2` отдельно создаёт schema5:
account fence до публикации, claim+saved Device keys в первом atomic root. Fence-only
ambiguous creation → CreationConflict; resume только собственного durable claim с
теми же saved keys. Повтор не усыновляет root и не перевыпускает identity (R2).

Migration одной DB, не две независимо активируемые копии:

1. **До fence:** source read-only, normal cancel оставляет mode4. Проверить legacy
   schema4/claim/keys, descriptors не публиковать заново. Неразрешённый prepared v2
   journal запрещает начало. Сохранить source manifest4 header/index-blob hash/generation.
2. **Atomic Fence5+Intent:** в том же redb commit заменить old `manifest` на sentinel
   со schema_version5 и установить authenticated Root5/MigrationIntent. В этой же
   transaction old encrypted `runtime/od1/profile-mode` заменяется mode5 fence;
   его исходные bytes/hash сохраняются в Intent как единственное explicit source
   override. Остальные old index/rows пока не меняются. Простое relabel header→4
   теперь тоже не пройдёт old full verify: mode record не совпадёт с old index MAC.
   Old validate_manifest отвергает5 до mutable writer; old standalone
   ratchet по-прежнему отвергает ASCII account fence. После fence нет downgrade в4.
   Sentinel использует old manifest encoding с schema5, count/bytes0 и root digest;
   это fence, не счётчик всей новой DB. Real counts/quota — только Root5. Новый loader
   dispatch schema5 выполняет до old validator; generic writer5 по-прежнему запрещён.
   Mode fence content=`kilogram:ordered-dialog:v2\0`; source decoder для этого одного
   retired metadata entry использует saved original из authenticated Intent.
3. **Copy/index batches:** source index≤8MiB читается один раз/resume; immutable event/
   auth/projection bytes могут храниться verified V4Ref (`record_key,cipher_hash,
   content_hash,length,type/scope`) на прежние encrypted rows. Все ref reads проверяют
   эти hashes и expected path/type/AAD, не whole legacy manifest каждый раз. Typed
   lane/request/author/parent/receipt indices строятся streamed, затем cross-checked.
   Cursor+processed digest commit с batch; повтор byte-identical, иначе conflict.
4. **Atomic Ready5:** перенести authoritative mutable heads/native/queue/pending/debt,
   пересчитать credits, установить versioned current pointers. Native/account/session
   state переносится public pickle APIs с теми же curve/SID/SessionKeys, без создания
   второго live domain. Old native current pointer retirement и новый pointer — один
   commit; old native records/shadows не являются loader fallback. Удалять derived
   duplicates можно лишь после durable replacement и отсутствия sole references.
   Immutable EventId/request/identity/history bytes не удаляются и не переписываются.
5. Каждый C сначала **L1 compatibility** на indexed storage: прежний wire1/limits1,
   Guard1 bounds/lease, same keys и old peer interoperable. OTK/phase/restart rules
   C1 сохраняются. Успешный handoff §3 меняет только этот C; другой C остаётся рабочим.

L1 contexts сохраняют также неистраченную legacy lifetime reservation; её нельзя
продать новым writers до handoff. После выбора L2 освобождается лишь неиспользованный
остаток, после покрытия existing queue/pending/receipt debts и нового repair pool.
Снижение quota ниже этих обязательств отвергается. Увеличение числа queued/unacked
и offline local encryption действуют только Active2; L1 compatibility сохраняет R3.

Pre-session phases также индексируются с реальными optional IDs. Compatibility
mutations поддерживают indexed seen-key mirror; handoff проверяет его совпадение с
bounded Guard1 seen и заменяет serialized growing set на Guard2 references атомарно.
Заранее принятый pending, exhausted own.next512 и old queued requests не исчезают:
после handoff старые n<cut догоняются, очередь шифруется один раз в новый format2.

Cancel/kill после Fence5 оставляет MigrationPaused/Copying, требует new binary resume;
old data доступны новым read-only legacy history path. Это forward-only upgrade,
не обещание, что старый EXE снова сможет писать. Partial/moved native rows не открывают
второй native workspace. Root5 Ready не устанавливается при неполном copy/verification.
Повреждение/недостаток места — явный blocked state, без очистки fence/state/history.

V5 writes только primary DB, per-message v2 filesystem journal/shadow не создаются.
Lock recovery dispatch различает schemas **до** old ordered recovery. До Fence5
terminal old journals убраны; старый binary может убрать terminal bookkeeping, но
не изменить native/trust/history. Existing exact L1 offset history/compat control
не сканируют data другого L2 C. Unknown mode/version никогда не fallback в legacy.

| Комбинация | Нормативный исход |
|---|---|
| mode4/new binary без explicit upgrade | Прежняя L1 работа, включая лимит; не auto conversion |
| schema5/L1 C ↔ old remote L1 | Доказанная wire1 compatibility; local old EXE schema5 отклоняет |
| Active2 C ↔ wire1-only peer | Explicit UpgradeRequired/unsupported, без новых writes/ACK/downgrade |
| Active2 ↔ Active2, late Event1/Receipt1 | Только ниже cut, exact old bytes, current auth и общий ordered ingress |

## 8. Direct/relay и порядок реализации

Сейчас OD1 `ordered::bind_loopback` отключает relay; supporting A2 доказал ordinary
relay adapter, **не** OD1 relay acceptance. L2 добавляет explicit route policy и
signed Locator2 с endpoint/IP и≤1 relay URL в пределах8KiB. Использует normal Iroh
builder/await_route_policy; direct и relay сходятся в один ingress/actor/ledger.
Для tests — свой loopback relay по принятому `local_relay_transport` примеру,
нормальные packets/TLS, exporter и identity gates; внешние relays не нужны. Смена
route/reconnect меняет B/current auth, не Event bytes/session/ordinal. L1 compat
сохраняет нынешний direct-only transport, пока не принят mode2.

CLI entrypoints: `device-enroll --profile-mode ordered-dialog-v2`, offline
`ordered storage-upgrade --state-dir ... --expect-account ...`, Root-admin
`ordered consent2-create/import` и runtime `ordered upgrade/upgrade-status`,
`ordered history --cursor ...`. Root-admin создаёт подписанный artifact вне state;
живой owner импортирует его через authenticated IPC, Root secrets не загружает.
Profile2 выбирает indexed storage; per-C mode берётся из authenticated store, не
из редактируемого profile JSON. Новый profile публикуется create-new после Ready5;
crash до его публикации исправляется resume/profile-create на прежних keys, без
rollback DB. Ordinary defaults и старый profile1 не превращаются в L2 автоматически.

Реализация после review, одним согласованным этапом с логическими частями:

1. `kilogram-state/src/vault/indexed_dialog.rs` + `authenticated_index.rs`:
   typed point/range API, Root5/AEAD trie, credit ledger, bounded backend/stats,
   fault seams; mode-aware lock/generic-writer fences в `lib.rs`/`vault.rs`.
2. `kilogram-protocol/ordered_dialog_v2.rs`, ratchet Guard2 + indexed seen API,
   bounded serializers/checked arithmetic. Codec1/primitive adapter unchanged.
3. Offline storage migration/admin и V5 L1 compatibility adapter; current `Engine`
   выводится на typed repository, L2 не получает generic paths()/full-history cache.
4. Consent2/fresh2/handoff state machine; update `engine/handshake/state/outbox/
   ingress/repair/runtime`, purpose-specific commit policy. B-only liveness,
   current lease/revocation/locator lifecycle сохраняются.
5. Cursor history/IPC/CLI, route2 path и targeted process/storage tests. `main.rs`
   только dispatch; historical evidence/accepted manifests не переписываются.

Существенная новая поверхность — authenticated index/migration, поэтому negative
membership/absence, key-space isolation, atomic root и deletion/fork proofs являются
gates, не только throughput tests. Добавления dependencies не требуется.

## 9. Измеримая приёмка будущей реализации

До edit сохранить baseline EXE/hash/source snapshot. Каждый gate пишет exact source/
EXE hash, команды, phase counters/bytes/latency и scope; fixture/IO injection/kill/
graceful reopen различаются. Все network harnesses только stable runner, fixed EXE,
CargoJobs2/BelowNormal/locked; Fast запрещён. Не повторять весь старый corpus по кругу.

| Gate | Конкретный результат / evidence |
|---|---|
| A3-1 real dialogue | Fresh L2 и resumed L1,≥640 Text **каждой стороны**, receiving DH≥16/сторону; exact IDs/requests/UTF8 и cursor history после обоих restart. E/D/SID постоянны; crossing511 не создаёт epoch/genesis |
| A3-2 genuine offline | После real setup peer process остановлен; sender durable materializes≥256 encrypted Text при valid local Root2 policy **без live lease**. На disk source/undelivered>128, RAM scheduling≤128. Возврат peer:≥8 pages, restart между выбранными pages, reorder/full64+head, late old chain/lost ACK/exact retry; C2 одновременно duplex≥32 Text/сторону |
| A3-3 long native chain | Actual native/engine one-way>512; pure checked preflight vectors §2.1 для symmetric post-index и общего T двух DH directions, A0/B0/A1/B1, continuation/rotation/imported L1. Reject до native call; никаких huge native states/traffic. Реальные production limits отдельно от уменьшенных test windows |
| A3-4 persistent100k |100000 rows одной C через real indexed storage/AEAD/projection/range/page API; synthetic import явно **не**100000 E2E. Числа1k/10k/100k, actual bytes read/write/grow/sync, commits/plan bytes, PrivateBytes/cache and cold reopen. Capture/resume cursors в разных местах после append/restart, exact suffix/no duplicates, cross-C isolation |
| A3-5 complexity | Instrumented обычные append/page не вызывают full index/record/directory traversal; candidate heads≤2, record lookups/plan bounded. At100k/1k p95 page/append ratio≤4 на одном verification setup; sizes/output сопоставимы. Каждый sampled operation без O(N) работы, total full traversal O(N) допускается. Profile PrivateBytes≤384MiB, рост после cache warm1k→100k≤64MiB |
| A3-6 resources | Малые ordinary quotas и full pending; funded256-head repair window расходует retained tokens и затем честно требует refill. Hard file-backed StorageBackend capacity реально достигается, плюс injected ENOSPC/sync/ambiguous-commit paths. Old bytes/queue/debt сохраняются, false Delivered/holes нет. Не заполнять диск разработчика; backend quota и injected OS errors не выдавать за full-host-volume test |
| A3-6a allocator boundary | Реальный production Builder/cache64MiB, без test/fuzz region/page knobs. Детерминированные indexed writes до первого grow ΔL≥128MiB (usable около128→256MiB); trigger — normal single append, capacity≥1GiB. Exact set_len/write/sync/latency, успешные reopen и последующие append того же C/requests. Отдельно capacity=L'+F+S−1 отвергает реальный growth, после увеличения capacity/reopen exact retry продолжает. Формула §6.1 покрывает верхние регионы без48GiB прогона |
| A3-7 new crashes | Real parent kill до/после Fence5, copy batch/Ready5; P before publication, B Prepared before response, A Selected, B Active before Committed reply, A before first format2 publication. Плюс selected native/receive/receipt IO windows. Existing14 windows повторять только где writer/recovery изменился |
| A3-8 migration/version | Baseline d9b creates real L1: active/exhausted511, accepted queue, pending64+gap/late DH, old receipts; upgrade сохраняет identities/ciphertext/IDs/history. Mixed old remote compat PASS до handoff, затем отказ; local/live old writer не обходит schema5. Оба restart traces §3 с exact Q в Core/Ready/Resume/Receipt и pre-native mismatch rejection; renewal/revoke/expiry, bad cut below pending, conflicting controls, partial old genesis, R1–R4/I1–I4 equivalents |
| A3-9 routes/legacy | Normal own loopback direct+relay/reconnect меняют путь, не decode order/bytes; real exporter freshness. Relevant ordinary transport/legacy history smoke, strict Clippy/fmt/locked checks; никаких crafted security packets |

Storage benchmark использует тот же persistent index/commit и history reader, batches
ограничены actual16MiB plan; bulk maintenance может иметь больше point mutations,
но не обходить integrity/quotas. Synthetic marker недопустим в runtime profile.
Обязательно измерять отдельные normal single-append после bulk import, не только bulk rate.
Storage page samples должны реально читать данные с backend после reopen/cache clear.

Бюджет на этом Windows verification setup: read/codec/component checks≤10min;
один final dialogue+offline/2C acceptance≤40min; persistent100k≤20min; выбранные
upgrade/fault/version gates≤20min. Это ceilings для остановки/анализа bottleneck,
не разрешение skip gate/подмены его counters. Нет sleep до expiry: targeted clock
явно отмечается. При стабильном проходе не повторять heavy gate без затронувшей правки.

## 10. Решения для root и граница остановки

На review вынесена **одна** схема: continuous native epoch, schema5 authenticated
radix store, rolling256-head grants, offline-encrypt Root2 permission, forward-only
storage fence/per-C handoff и causal cursor. [Review1](long-lived-dialog-a3-contract-review-1.md)
принял эти решения по смыслу, включая local authorization и cancel после fence/P;
целый контракт после исправлений принят [повторным review](long-lived-dialog-a3-contract-review-2.md).
Реализация сдаётся логическими частями; принятие контракта не заменяет review кода.

Disposition review1: **R1** — §6.1 actual allocator-driven growth/capacity/headroom/
recovery вместо64MiB ceiling, source-backed layout bound и A3-6a. **R2** — единый Q
из origin-tagged ModeProof2, fresh Ready2→Active2 и два restart traces (§3/A3-8).
**R3** — post-message-index и общий DH T preflight, genesis/import proof и boundary
vectors (§2.1/A3-3). Проверены чтением pinned sources и чистой BigInt арифметикой:
9 counter vectors/54 layout cases PASS; это не runtime/storage evidence реализации.
Остальные правила/числа остаются нормативными; несвязанный scope не расширялся.

Границы: personal two-device/two-C, operator locators, local direct/own relay.
Нет GUI/onboarding, groups/multi-device/mailbox lifecycle/legacy4096 fix/M1.2.63.
[A2](transport-security-a2-implementation-review-1.md) остаётся открыт:7 warnings,
strict advisory gate FAIL, DEFERRED runtime security reproducer не разрабатывался,
не запускался и не переносился сюда. External/daily-MVP readiness не заявляется.

В этом этапе изменён только этот contract. Исторические evidence untouched;
чужой `development-environment.md` сохраняет SHA256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`.
После документа — один substantive callback root и остановка. Product code,
dependencies/schema/fixtures, commit/push до приёмки не запускать.
