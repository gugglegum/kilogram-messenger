# A3-S3: Codec2, bounded Guard2 и closed storage seam

2026-10-25, Asia/Yekaterinburg. **План принят: [root review2](long-lived-dialog-a3-protocol-s3-plan-review-2.md). Реализация не начата.**

Основание: [S3 task](long-lived-dialog-a3-protocol-s3-task.md),
[принятый A3 contract](long-lived-dialog-a3-contract.md),
[contract review2](long-lived-dialog-a3-contract-review-2.md),
[S2 implementation review4](long-lived-dialog-a3-history-s2-implementation-review-4.md).
Текущая доработка отвечает [S3 plan review1](long-lived-dialog-a3-protocol-s3-plan-review-1.md)
(SHA256 `f46662b070a138a9e90a12f5bad9395ebaa54afa9ce5e6b4c3556a4b5a3bdbc4`).
Review1 и остальные документы сохраняются; обратный callback не повторяется.
Ниже уточняется один следующий component slice; согласованные wire2, единый Q,
сохранение native session и будущие A3 gates не меняются.

## 1. Baseline и рекомендуемая граница

Baseline HEAD/master/origin/master: `e136c59b829c6ecc5eb501273f25091e9b9e4789`,
parent `61524df3a77203d73673f47d38decf6b817615e3`. Это опубликованный S2.
Числа S2 относятся к indexed storage и trusted synthetic DTO; native E2E readiness
из них не следует. В этой сдаче Rust/helper/Cargo/dependency files не изменяются,
тесты и benchmarks не запускаются, commit/push не выполняются.

Рекомендуется реализовать после review **library component**:

1. Typed Codec2 со статической криптографической проверкой событий, authority,
   selected proofs/controls и receipts; явные parsed/verified состояния.
2. Pure Guard2/preflight и bounded checkpoint над прежними byte-oriented native
   primitives; authenticated point lookup seen keys вместо checkpoint set.
3. Узкие typed операции S2 writer для format2 source/seen/receipt correspondence.
   Component driver в CLI unit tests соединяет настоящий codec/native/guard/store.

Production actor пока не получает новый маршрут вызова. Ни CLI command/profile2,
ни transport ALPN2 listener, ни consent ceremony, ни offline-send policy, ни live
FreshHandshake→Active2/P-A-K handoff, ни file migration в этот slice не входят.
Types этих controls нужны для проверки bytes/Q; наличие verified proof не означает
live activation. Guard1/Engine и обычные runtime defaults продолжают прежнюю работу.

Whole A3 сохраняет будущие gates:640 Text **каждой стороны**, offline256, два процесса,
две C, upgrade/fault/direct/relay/restart. Component evidence ниже их не заменяет.
100k fixtures не открывать и не повторять; absolute UX latency и I/O amplification
остаются вопросами интеграции и измерений будущего runtime.

### Проверенные исходные seams

| Baseline source | Что действительно делает сейчас |
|---|---|
| [Codec1](../../crates/kilogram-protocol/src/ordered_dialog_v1.rs#L142), [Core1](../../crates/kilogram-protocol/src/ordered_dialog_v1.rs#L268), [Event1](../../crates/kilogram-protocol/src/ordered_dialog_v1.rs#L347) | Restricted canonical CBOR; Core1 n<512; Event::parse уже проверяет Device signature и actual public Olm header. Это не модель parsed DTO для нового API |
| [Guard1](../../apps/kilogram-cli/src/ordered_dialog/ratchet.rs#L25) | BTreeSet seen в каждой lane, предел512, не все additions checked; остаётся только L1 |
| [Native byte adapter](../../crates/kilogram-ratchet/src/ordered.rs#L1) | Account/session fork, public encrypted pickle APIs, config1, SID, actual Packet.header; protocol/membership/inner equality и durability не его ответственность |
| [L1 private state](../../apps/kilogram-cli/src/ordered_dialog/state.rs#L232) | Head/blob hash, phase/native generation, profile/device AAD; open проверяет config/SID/Guard1. Сам ciphertext pickle не доказывает известное происхождение native counters |
| [S2 input types](../../crates/kilogram-state/src/vault/indexed_dialog/history_types.rs#L1) | Trusted adapter claims; source_id считает domain hash raw bytes, Binding хранит source/Core digest. Он не декодирует Core semantic fields и не проверяет Device signature/native MAC |
| [S2 rows/native/seen](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L511) | Закрытый lowering, exact frame_ids, native before hash; seen.key/g берутся из extras. Peer ReceiptA/B сейчас содержит EventId marker, не signed Receipt raw |
| [S2 ACK](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L453) | Сопоставляет own immutable source/debt/slot и EventId marker; Wire Receipt verification не реализован storage facade |
| [H1 lowering](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L740), [generic fence](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L157) | Row↔credit/retention correspondence; generic writes в bound history запрещены, включая shared Author ownership |
| [H2 retry/coherence](../../crates/kilogram-state/src/vault/indexed_dialog/history_writer.rs#L219), [commit](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L684) | Fresh init, coherent identity/heads, internally derived digest, same commit_id/exact digest retry, stale generation, poisoned/reopen outcomes |

Dependency direction: protocol→ratchet уже существует; ratchet→protocol был бы cycle.
State не зависит от protocol/ratchet. Предлагаемая схема сохраняет этот graph и Cargo.lock.

## 2. Codec2: bytes, limits и trust

### Canonical profile и domains

Переиспользовать только primitives `Value`, encode/decode/hash/signing_bytes из
Codec1; не вызывать Core1 parser для Core2 и не изменять LIFETIME/OD1 decoder.
Profile: definite arrays/bstr и canonical unsigned u64, shortest integer/length,
depth≤8, items≤8192, нет map/text/null/negative/tag/float/indefinite/trailing bytes.
Optional `[]/[x]`; фиксированные tuple arity проверяются до crypto/native.
SID — bstr ровно43 ASCII bytes; text — UTF-8 внутри payload bstr.

`H(tag || bytes)` = BLAKE3; подпись Device/Root по точным `tag || enc(unsigned)`.
Для Event unsigned=`[2,Core2,NativeHeader,olm_bytes]`, ID по full signed raw Event2.
Для Frame unsigned=`[2,kind,B,request16,body]`; kind1 использует auth-hello,
kind2 auth-proof, прочие control-sign. Control unsigned=`[2,kind,body]`.

| Назначение | Точный domain/label |
|---|---|
| Event signature / ID | `kg:od2:event-sign\0` / `kg:od2:event-id\0` |
| Control signature / ID | `kg:od2:control-sign\0` / `kg:od2:control-id\0` |
| Receipt signature / ID | `kg:od2:receipt-sign\0` / `kg:od2:receipt-id\0` |
| Hello / Proof / initial transcript | `kg:od2:auth-hello\0` / `kg:od2:auth-proof\0` / `kg:od2:auth-pair\0` |
| Consent / descriptor | `kg:od2:consent\0` / `kg:od2:descriptor\0` |
| Epoch / domain / Q | `kg:od2:epoch\0` / `kg:od2:domain\0` / `kg:od2:mode-certificate\0` |
| TLS binding hash / exporter label | `kg:od2:channel-binding\0` / ASCII `kilogram ordered-dialog v2` |

ALPN2=`kilogram/m1/ordered-dialog/cbor/2`, suite1, limits2.
Exporter context=`enc([bstr(ALPN2),dialer_endpoint32,listener_endpoint32])`, output32;
wire B — только domain hash output. В этом slice проверяются функция context/hash
и Frame binding, реальный TLS exporter остаётся будущему transport gate.

**AuthRef и cert_ref сохраняются:** BLAKE3 exact credential/certificate raw с пустым
domain, как [Credentials::reference](../../apps/kilogram-cli/src/ordered_dialog/handshake.rs#L61).
Bundle остаётся `[1,certificate_raw,authority_snapshot_raw]`; версии подписанных
DeviceCertificate/Root authority/membership не меняются из-за появления wire2.

### Основные layouts

```text
NativeHeader = [kind=0|1,key32,index]
ClosedTail = [g,key32,index,last_event_id32]
Expected = [g,index,parent_first_id?,ClosedTail?]
Core2 = [2,C32,D32,E32,SID43,role=0|1,n,previous_id?,author_sequence,
         kind=1|2|3,request16,sorted_user_parent_ids,Expected,author_auth_ref32,Q32]
Event2 = [2,Core2,NativeHeader,olm_bytes,device_signature64]
Inner2 = [2,Core2,payload]
Init payload = [Offer_id,Grant_id,initial_transcript,[session_key0,session_key1,session_key2]]
Confirm payload = [Offer_id,Grant_id,SID43,A0_id]
Text payload = [utf8_text_bstr]
```

Core≤2KiB, Event≤16KiB, Inner≤8KiB, Olm≤12KiB, text≤4KiB;
Auth bundle≤16KiB, membership≤32KiB, Consent2≤4KiB, ModeProof2≤16KiB,
descriptor≤8KiB, Receipt≤1KiB, initial-pair≤192KiB. Frame plus four-byte length≤1MiB.
Limits включают encoding overhead; maximum field length не обещает допустимость
любого их одновременного сочетания. Перед allocation проверять весь declared length.

Core parser проверяет role/kind/widths, parents≤2 строго sorted unique, n0 только
Init(A)/Confirm(B), previous=[] и parents=[]; n≥1 только Text и previous=[id].
Static selection verification до native требует genesis request16=первые16 bytes
Offer_id для Init и Grant_id для Confirm; одной проверки длины request недостаточно.
512 predicates здесь нет. Согласованность g/j/parent/tail/cursors — Guard2,
а не простая representability u64. Expected.index и outer NativeHeader.index равны.
PreKey kind0 только Init; Confirm/Text normal kind1. Actual Olm header сравнивается
с outer, PreKey SID/identity/base/OTK — с verified selection/native session keys.
Inner format выбирается по verified outer format; полный Core и payload kind/shape
проверяются после native MAC, до принятия trial.

### Authority, selected proof и controls

Role A/B определяется прежним порядком DeviceId, два разных Account; certificate
refs, messaging capability, membership C/owner/member set проверяются identity API.
Static Root signature verification не разрешает действовать по отозванной authority.

```text
Origin = [0] | [1,D,E,SID]
ConsentBody2 = [2,C,certA_ref,certB_ref,1,2,Origin,1,not_before,expires]
Consent2 = [ConsentBody2,rootA_signature64,rootB_signature64]
InitialPair2 = [2,T,HelloA_raw,HelloB_raw,ProofA_raw,ProofB_raw]
T = H(auth-pair || enc([B,HelloA_raw,HelloB_raw]))
Control2 = [2,kind,body,device_signature64]
Offer(kind1) body = [C,certA_ref,certB_ref,T,nonceA32,olmA_identity32,1,2]
Grant(kind2) body = [C,Offer_id,T,nonceB32,olmB_identity32,reserved_OTK32,1,2]
Ready(kind3) body = [D,E,SID,A0_id,B0_id,role,1,2,Q]
P(kind18) body = [C,D,E,SID,Consent2_ref,nonce32,cutA,previousA]
A(kind19) body = [C,P_id,cutB,previousB]
K(kind20) body = [C,P_id,A_id]
Committed(kind21) body = [C,Q]
ModeProof2 fresh = [2,0,C,selected_Consent2_raw,Offer_raw,Grant_raw]
ModeProof2 existing = [2,1,P_raw,A_raw,K_raw,selected_Consent2_raw]
Q = H(mode-certificate || enc(ModeProof2))
```

Все `*_raw` в proof — bstr с исходными canonical bytes, не повторно выбранная
authority. Fresh verifier дополнительно получает exact InitialPair2: обе Hello,
обе Proof signatures, exporter B в исторической паре, T, endpoints, certs, membership,
selected consent refs, origin/suite/limits совпадают. Историческая pair не выдаёт lease.
Для existing verifier получает old Ready1 pair с verified genesis/domain/SID и
проверяет P/A/K signatures A/B/A, chained IDs, selected Consent2 existing origin,
cuts∈[1,512], D/E/SID. Это static certificate validation; pending/source watermark,
заморозка send и допустимость установки cut зависят от будущего live state machine.

Fresh E=`H(epoch || enc([2,C,certA_ref,certB_ref,Offer_raw,Grant_raw]))`;
D=`H(domain || enc([2,1,2,C,certA_ref,certB_ref,E]))`. Existing D/E не пересчитывать.
Q известен до A0, не зависит от SID/genesis/Ready и не меняется от renewal.
Ready/Core/Mode2/Receipt с известным Q обязаны совпадать с проверенным selected proof.

Descriptor2: `[body,signature64]`, body=
`[2,C,local_cert_ref,peer_cert_ref,endpoint32,sorted_addresses,revision,not_before,expires]`;
address=`[4,ipv4_bytes4,port]` или `[6,ipv6_bytes16,port]`,1..16, unique, port1..65535,
revision>0, not_before<expires. Подпись/ID по descriptor domain. Его публикации нет.

### Frame bodies и mixed format

Перечень полный: значения tuple фиксированы, свободного Value body в public builders
не будет. Базовые смыслы взяты из [OD1 layouts](lossless-sync-a1-bidirectional-contract.md#L499),
изменённые — из [A3 §6](long-lived-dialog-a3-contract.md#L467).

```text
Frame2 = [2,kind=1..21,B32,request16,body,signature64]
 1 Hello = [C,transport_role,local_endpoint,remote_endpoint,nonce32,[1],[2],
            auth_bundle_raw,membership_raw,current_consent2_ref,descriptor2_ref,Origin]
 2 Proof = [T,C,1,2,certA_ref,certB_ref]
 3 Admitted = [T,C,1,2]
 4 OpenIntent = [C,local_phase,Offer_id?,Grant_id?,A0_id?,B0_id?]
 5 Offer = [C,Offer_raw,InitialPair2_raw]
 6 Grant = [C,Offer_id,Grant_raw]
 7 EpochCipher = [C,D,E,event_format,exact_event_raw]
 8 Ready = [C,D,E,Ready_raw]
 9 Resume = [C,D,E,SID,HandshakeIds,send_cursor,receive_cursor,Mode2,Status2,Credit2]
10 Deliver = [C,D,E,SID,lane,AuthRecords,DataItems2]
11 PrefixReq = [C,D,E,SID,lane,start,end_exclusive,expected_previous_id?,Credit2]
12 PrefixResp = [C,D,E,SID,lane,start,actual_end,source_cursor,AuthRecords,DataItems2,more]
13 Complete = [C,D,E,SID,related_request16,exact_all_requested_EventIds]
14 Error = [C,D?,E?,related_request16,outcome,reason,position?,retry_after_s]
15 AckBatch = [C,D,E,SID,lane,AuthRecords,AckItems2,not_readable_slot_pairs]
16 AckQuery = [C,D,E,SID,lane,slot_id_pairs]
17 Head = [C,D,E,SID,send_cursor,receive_cursor,Mode2,Status2,Credit2]
18 Propose = [P_raw,selected_Consent2_raw]
19 Accept = [A_raw]
20 Select = [K_raw]
21 Committed = [Committed_raw]

HandshakeIds = [Offer_id?,Grant_id?,A0_id?,B0_id?,ReadyA_id?,ReadyB_id?]
Cursor2 = [next_n,last_id?]
Status2 = [reason,position?]
Mode2 = [stage=0..5,Q?,[cutA?,cutB?]]
Credit2 = [ordinary_slots,ordinary_bytes,head_temp_bytes,max_events,max_wire_bytes,
           max_ahead_ordinal,grant_hint?]
GrantHint2 = [grant_id32,start,end_exclusive]
SlotIdPair = [n,EventId]
AuthRecords = [[AuthRef,exact_credential_bundle_raw],...]
DataItems2 = [[event_format=1|2,exact_Event_raw],...]
AckItems2 = [[receipt_format=1|2,exact_Receipt_raw],...]
ReceiptBody2 = [2,C,D,E,SID,Q,event_format,lane,n,EventId,receiver_cert_ref,receiver_auth_ref]
Receipt2 = [ReceiptBody2,receiver_signature64]
```

Dynamic lists≤32, кроме addresses≤16 и parents≤2; фиксированные tuples имеют указанную
arity. `more` — uint0|1. AuthRecords sorted unique по AuthRef, hash bytes совпадает;
лишние/unused entries запрещены после resolution. Error/status enums сохраняют OD1
значения; codec не превращает reason/status hint в possession или policy decision.
Credits ограничиваются production maxima: ordinary slots≤64, bytes≤4MiB,
head_temp≤64KiB, max_events≤32, max_wire≤1MiB; диапазоны start/end/ahead checked.
Head-only требует slots0/max_events1/end=start+1/max_wire≤68KiB и funded grant
на live commit; codec проверяет shape/числа, funding выполняет store.

Cursor next0→last=[], next>0→last=[id]; Resume send — immutable own source cursor,
receive — peer readable/native cursor. Это не принятый pending/ACK frontier.
Mode stages:0 FreshHandshake,1 L1,2 Offering,3 Prepared,4 Selected,5 Active2.
До selection Q отсутствует; fresh после Grant Q присутствует и cuts=[0,0].
Existing до установки Q data только format1; при Q формат1 iff n<cut[role], иначе2.
Typed `ModeView2` различает hint и locally verified selection; peer hint его не меняет.
Для fresh format1 запрещён всегда. FreshHandshake допускает лишь genesis/Ready,
Text до durable activation остаётся запрещённым будущим policy gate.

Mixed data dispatch — один явный branch по tag:1→неизменённый Event1/Inner1,
2→Event2/Inner2; несовпадающий internal version/tag — error. Никаких проб обоих decoders.
Tag относится к bytes, не разрешает mode. Receipt1 допускается только для format1
ниже cut. Receipt2 может ссылаться на event_format1 или2 при известном Q; однажды
durable Receipt1 не заменять на Receipt2. Signer receipt — Device противоположной lane;
C/D/E/SID/Q/format/n/ID и cert/auth refs сравниваются с exact immutable source.

### Минимальные typed API и уровни доверия

```rust
// Имена предлагаемые; поля verified wrappers private, raw bytes immutable.
ParsedEvent2::parse(raw) -> Result<ParsedEvent2, Codec2Error>;
ParsedEvent2::verify(&StaticSelection2, &ResolvedAuthor) -> Result<VerifiedEvent2, Verify2Error>;
ParsedFrame2::parse(raw) -> Result<ParsedFrame2, Codec2Error>;
ParsedFrame2::verify(binding, signer) -> Result<VerifiedFrame2, Verify2Error>;
ParsedModeProof2::verify(&SelectionEvidence2) -> Result<VerifiedModeProof2, Verify2Error>;
ParsedReceipt2::verify(&VerifiedSource, &ResolvedReceiver) -> Result<VerifiedReceipt2, Verify2Error>;
ParsedCredentialBundle::parse(raw) -> Result<ParsedCredentialBundle, Codec2Error>;
ParsedCredentialBundle::verify_root() -> Result<RootVerifiedCredential, Verify2Error>;
verify_inner2(&VerifiedEvent2, plaintext) -> Result<VerifiedPayload2, Verify2Error>;
```

Public façade экспортирует Core2/Expected/ClosedTail immutable accessors, typed payloads,
FrameBody2 enum, Parsed/Verified wrappers, raw/id/sign builders, limits/domains и
explicit mixed-format resolver. Credentials используют existing identity decoders
с exact re-encode equality. `VerifiedEvent2` означает canonical+signature+static scope/
selected Q+actual header; **native MAC и inner ещё не проверены**. `VerifiedPayload2`
означает inner equality/shape после успешного native decrypt на trial.
Ни один из этих типов не является current-policy permit.

Live policy позже обязан проверять Root pins/revocation/expiry/clock, local stage,
cut/source watermarks, admitted lease для network actions, offline permission только
после durable Active2, storage generation и funding ещё раз перед commit.
В этом slice crypto/component tests подают явно фиксированный static selection;
mock fresh lease не объявляется настоящим network admission.

## 3. Guard2, counters и trial

### Bounded checkpoint

Guard2 хранит version2, role, две lane summaries и rotation flags. Lane summary:
`{next_n,last_id?,g,key?,last_index,first_id?,parent_first_id?,last_author_sequence?}`.
Для пустой lane используются реальные optional fields и next0; для непустой fields
полны, next=checked(last_n+1). ClosedTail выводится из этой summary. Весь checkpoint
содержит только две lanes; никаких seen vec/set, source inventory или deferred packets.

Flags: `rotate_due`, `received_new_first?`, `used_own_first?`. Новая peer chain
устанавливает received_new_first=её first ID, used_own_first=текущий own first,
rotate_due=true. Own send с новой chain ставит rotate_due=false. Continuation,
включая same-key tail в Inactive, эти flags не меняет. Peer new first требует новый
own first, ещё не равный used_own_first, parent=current own first, точный ClosedTail
предыдущей peer chain и отсутствие key в authenticated Seen[peer].

Pre-selection native wrapper имеет phase-tagged variants: OfferedA(account,D/E/SID=[]),
GrantedB(account,reservation,D/E real,SID=[]), Selected(account,session,Guard2,D/E/SID real).
Outbound A session constructor создаёт Active0 до A0; inbound A0 trial создаёт B
Selected/Inactive0. Не сериализовать фиктивные zero IDs вместо отсутствующих значений.

Scope/checkpoint AAD привязывает profile incarnation/device,C,D/E/SID optionals,role,
phase,native_generation и Q если выбран. Пакет account encrypted pickle + session
encrypted pickle + Guard2 + reservation + envelope проверяется **вместе** ≤128KiB;
не достаточно отдельных MAX_PICKLE. Key material и inner/trials — Zeroizing.
Проверяется итоговый sealed output, включая overhead, до storage commit.

### Pure checked preflight

В `kilogram-ratchet::ordered_v2::preflight` ввести чистые функции без Session:

```text
checked_successor(value, CounterKind) -> u64 | CounterExhausted(CounterKind)
consume_index(j) -> {wire_j=j, native_post_j=checked(j+1)}
continuation_index(last_j) -> consume_index(checked(last_j+1))
turn(role,g) -> checked(checked(g*2)+role)
dh_step(Tpre,role,gpost) -> Tpost iff checked(Tpre+1)==turn(role,gpost)
preflight_send(guard,intent,storage_generation,native_generation,author_next)
preflight_receive(guard,event_meta,seen_observation,storage_generation,native_generation)
```

CounterKind различает ordinal, author sequence, root generation, native generation,
lane generation, symmetric index, native DH turn. Peer author_sequence строго больше
последнего принятого; проверяется и возможность следующего sequence, как для own.
Own allocator/global Author slot делает checked increment до native. n/g/j/sequence
не подменяют друг друга. Exhaustion сохраняет accepted bytes/queue/native и не создаёт
новый session/OTK/epoch. Return PreparedStep2 содержит immutable cursor/checkpoint
binding и все вычисленные post counters; apply не делает новых unchecked additions.

Tpre из authenticated checkpoint: Active→turn(own_role,own.g), Inactive/rotate_due→
turn(peer_role,peer.g); пустой outbound A — отдельный известный Active0 constructor.
New peer DH в Inactive запрещён до native, кроме конструктора inbound A0.

| Шаг | Известный sender ratchet до/после, preflight |
|---|---|
| A0 | A outbound Active0 остаётся0; consume j0→post1. B inbound создаёт Inactive0, remote post1 |
| B0 | B activate Inactive0→Active1; A receive Active0→Inactive1 |
| A1 | A activate1→2; B receive Active1→Inactive2 |
| B1 | B activate2→3; A receive Active2→Inactive3 |
| Continuation | T неизменен, consume exact next symmetric key; known remote chain не вызывает DH |
| Same-key late peer tail | Та же peer g/key/parent; next j, прежний sender T и flags, в Active либо Inactive |
| Новый peer DH в Inactive | Reject NativePhaseMismatch до native; никакой activate+advance |

После проверки expected fields actual native Packet.header() обязан подтвердить key/j
и kind. На new own DH generated key должен отсутствовать в Seen[own]; это post-trial
lookup, поскольку public API не даёт будущий random key до encrypt. Collision/error
отбрасывает trial. До call уже проверены все counters/phase; до commit — new key absence.
Для receive key известен до decrypt: absence/presence проверяется перед native.
Для continuation Seen обязателен с теми же g/first_id, missing/error не означает new DH.

### Последовательность одного transition

1. Parse/caps/canonical; static Device/Root/mode/scope verification; authenticated
   current snapshot/native blob, source parents и Seen point reads. Future policy gate
   располагается здесь и повторяется перед commit, не внутри parser.
2. Guard exact lane next/previous, kind/genesis request16/empty parents, sequence,
   expected g/j/parent/ClosedTail, phase/counter preflight. Out-of-order остаётся pending,
   повтор старого slot сравнивает immutable source bytes без повторного decrypt.
3. Fork account/session/guard целиком. Только trial вызывает outbound encrypt либо
   inbound constructor/normal decrypt; config1, единственный reserved OTK B. Даже если
   native primitive уже изменил trial после MAC, failure inner/sign/header discard
   касается всего trial. Live accepted object не передавать в mutating native API.
4. Сверить actual header/SID/prekey keys, verify_inner полным Core, kind payload,
   точный UTF-8, IDs/requests/parents. Apply prepared guard step на trial; post-index/T
   считаются proven индукцией по проверенным transitions, не чтением RatchetCount.
5. Serialize/seal совместный checkpoint, cap128KiB; closed operation≤4 source rows,
   actual affected plan≤16MiB. Atomically commit, затем принять trial в RAM/разрешить
   dispatch durable bytes. При failure ни source, ни receipt, ни seen не публикуются.

Здесь нет перезапуска session через512, новых key pings/hidden ciphertext или epoch budget.
Native API [ordered.rs](../../crates/kilogram-ratchet/src/ordered.rs) остаётся прежним.

### Pinned native evidence и L1 provenance

Cargo.lock [vodozemac0.10.0](../../Cargo.lock#L6808), registry checksum
`b98bf83c0992966775b8012f194b07b44928996163e5a05b741b43891571ae5b`.
Адресно прочитаны локальные sources в
`C:/Users/Paul/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vodozemac-0.10.0/src/olm/session/`:

| Source / lines | Подтверждённое поведение / SHA256 source file |
|---|---|
| [chain_key.rs](C:/Users/Paul/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vodozemac-0.10.0/src/olm/session/chain_key.rs#L81),81–124 | u64 j; create key потребляет j и увеличивает j+1. `a20b690dc1c9a1ea43cee595e85fd4433d8f01c0de601c37607a73a7d6a70b4d` |
| [double_ratchet.rs](C:/Users/Paul/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vodozemac-0.10.0/src/olm/session/double_ratchet.rs#L76),76–150,204–214,260–275,308–325 | Known0 constructors, activate/receive увеличивают T; Inactive new key может activate+advance; Known count+1 unchecked. `0b757b00caf4a281f741f54beae45f0ed0a31bdbba8df1744cb319cf40295927` |
| [mod.rs](C:/Users/Paul/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vodozemac-0.10.0/src/olm/session/mod.rs#L194),53,194–229,312–325 | Active outbound, Inactive inbound, existing receiving chain без DH, максимум5 chains. `19c7e72d7edafa25367407b91870c673da5925497d57fb799527217422adf4af` |
| [receiver_chain.rs](C:/Users/Paul/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vodozemac-0.10.0/src/olm/session/receiver_chain.rs#L186),31,50,186–249 | gap bound2000, retained skipped keys≤40/chain; consume j+1, MAC до assign. Ordered exact-index ingress не создаёт gaps. `f7e8193b32be128504d80afc9ada5f7d75964ef095542762aaae51a955556868` |

RatchetCount публичным Session API не читается. Не сериализовать private pickle в JSON,
не искать Known/Inactive через debug, не добавлять getter/fork dependency patch.

Поддерживаемый L1→Guard2 converter — pure bounded conversion **после** trusted owner
verification, не `restore(any_valid_pickle)`. Owner должен проверить прежний OD1
Head/blob hash/AAD/phase/generations, exact selected Offer/Grant/InitialPair и genesis,
known constructor origin, session config1/SID, source signatures/actual headers,
Guard1 own/peer first/previous/parent/tails/rotation flags и seen keys↔first sources.
L1≤512/lane позволяет bounded import-only fold≤1024 sources; ordinary Guard2 paths
историю не сканируют. g≤511 и T≤1023, включая optional pre-session phases.
Неизвестное/libolm происхождение или один pickle без этих bindings — UnsupportedOrigin.

Converter принимает normalized `L1CheckpointFacts` только от этого owner path;
ratchet crate сам не удостоверяет AEAD/Root/source provenance. Не именовать этот DTO
Verified. Сначала строится durable-index seed `(role,key)->(g,first_id)`, затем Guard2
теряет старый RAM set. Account/session и D/E/SID остаются прежними. В S3 проверяются
conversion и продолжение на freshly generated component fixture; чтение/миграция
реального profile и установка live mode остаются следующей части A3.

## 4. Closed atomic storage seam

### Почему требуется расширение

Простая передача `HistorySeen{key,g}` и `Acknowledge{n,id}` после подписи вне store
не проверяет source/receipt correspondence в writer. `Binding::verify_source`
сравнивает hashes, но caller выбирает metadata/Core span. Для нового production path
этого недостаточно. При этом crypto policy остаётся обязанностью адаптера; store
не получает Device/Root secrets или native Session.

Предлагается добавить private bounded **structural** reader format2 source/receipt
в state. Он заимствует slices, проверяет тот же restricted CBOR profile/widths/arity
и извлекает fields из raw; никаких signature/native-MAC/current-policy claims.
Это небольшое дублирование structural wire schema осознанно сохраняет dependency
graph. Дифференциальные vectors с Codec2 обязательны. Старый S2 trusted fixture API
не объявляется wire verifier; для bound protocol2 C он не сможет обойти новый path.

### Предлагаемый public API

```rust
struct HistorySeenRecord { generation: u64, first_event: Id } // exact stored40 bytes
struct HistoryNativeRecord { hash: Id, blob: Vec<u8> }
struct HistoryCheckpointRead { view: Id, native: Option<HistoryNativeRecord> }

fn history_checkpoint(&mut self, context: u8, expected: u64)
    -> Result<HistoryCheckpointRead>;
fn history_seen(&mut self, context: u8, expected: u64, role: u8, key: Id)
    -> Result<Option<HistorySeenRecord>>;
fn initialize_history_protocol2(&mut self, context: u8, expected: u64,
    commit_id: Id, init: HistoryInit, binding: HistoryProtocol2Binding)
    -> Result<CommitReport>;
fn commit_history_protocol2(&mut self, context: u8, expected: u64,
    commit_id: Id, operation: HistoryProtocol2Operation) -> Result<CommitReport>;

enum HistoryProtocol2Operation {
    Genesis { row: HistoryReadable2, native: HistoryNativeTransition },
    TextBatch { rows: Vec<HistoryReadable2>, native: HistoryNativeTransition }, //1..4
    AcceptPending {
        source: Vec<u8>, head: bool, grant: Option<Id>,
        admission_author_ref: Id, auth: Vec<HistoryAuth>, // <=2 distinct bundles
    },
    Acknowledge { receipt_format: u8, receipt_raw: Vec<u8>, auth: Vec<HistoryAuth> },
}
struct HistoryReadable2 {
    source: Vec<u8>, text: String, auth: Vec<HistoryAuth>,
    readable_author_ref: Id, // trusted owner current-policy context, not a wire field
    receiver_receipt: Option<Vec<u8>>, // Some only peer Text
}
```

Имена в этом разделе — предложение review, не уже существующие exports.
HistoryProtocol2Binding хранит exact immutable ModeProof2 bytes/Q, scope, role cert refs
и owner-selected Author slot map0..2. Поля извлекаются/сопоставляются structural reader;
адаптер до init проверяет proof signatures/selection. Init только на fresh empty C по H2;
в этом slice origin fresh/cuts0. Existing live C не переинициализируется. Будущий migration/
handoff получит отдельную closed операцию с собственными доказательствами и review.
Поэтому текущий checked storage path принимает только Event2/Receipt2 с event_format2;
mixed Event1/Receipt1 поддерживаются codec/component resolver, их live storage handoff
остаётся future. Наличие format tag в API не разрешает legacy bytes в fresh binding.

Authenticated native absence допустим только после coherent empty Init2 и до первой
genesis row; позже это Integrity error. Component setup создаёт/проверяет selected SID
на constructors/trial до static binding. Init2 не выдаётся за crash-safe persistence
OfferedA/GrantedB: production bootstrap/pre-selection durability проверяется вместе
с будущим owner/handshake integration, не N01/N02.

Static storage binding — соответствие source/proof, **не** признак Active2/permission.
Role→Author slot фиксируется owner при init из verified certificate mapping, не задаётся
для каждого row. Cross-C shared Author immutable ownership/fence остаётся действующим.

Для bound2 старые `HistoryOperation::{Genesis,TextBatch,Acknowledge,AcceptPending,
CapturePending}` отклоняются до lowering, в том числе на duplicate/retry paths.
Pending создаётся только новым typed AcceptPending с atomic original/admission capture:
scope/role/n/EventId выводятся из exact source, обе authority roles разрешаются по
алгоритму ниже. Отдельного нового CapturePending API этому fresh component не нужно;
uncaptured legacy pending относится к future migration. QueueAccept/RepairCredit
переиспользуют существующие typed operations без auth-аргументов. Ни `commit_history`,
ни generic `commit` не обходят эти fences. Для unbound S2 fixtures прежний trusted
storage contract, включая optional capture, остаётся без изменений.

### R1: wire genesis request и HPB1 projection metadata

Сохраняется существующий [Binding::check/HPB1](../../crates/kilogram-state/src/vault/indexed_dialog/history_types.rs#L420),
включая требование `request=[0;16]` у genesis. Выбирается **явная normalization при
lowering**, без нового Binding encoding и без изменения wire Core:

| Source row | Wire request в immutable Event/Core | Stored HPB1 Binding.request |
|---|---|---|
| A0 Init, role0/n0 | `Offer_id[0..16]`, сверенный с selected fresh proof | `[0;16]`, sentinel «genesis не имеет queue request» |
| B0 Confirm, role1/n0 | `Grant_id[0..16]`, сверенный с тем же proof | `[0;16]`, тот же sentinel |
| Text, n>=1 | Source Core.request16 | Точные source16 bytes; FIFO Request mapping использует их |

`lower_binding2(source_fields)` сначала проверяет source shape, scope/Q, genesis
kind/role/n/previous/empty parents и **точный wire prefix**, затем создаёт HPB1 с этой
normalization. Нулевой sentinel не является допустимым substitute wire request.
Owner проверяет R4 до native; state повторяет structural prefix check до lowering.
Сам request prefix теоретически может быть нулевым, если таков настоящий hash prefix;
условие — равенство выбранному Offer/Grant, а не запрет конкретных16 bytes.

Event raw, Core span и source/Core hashes всегда относятся к **исходным signed bytes**.
В native plaintext, signatures, EventId, receipts и dispatch не попадают normalized
bytes. Нельзя заново encode/sign Event с request sentinel или считать ID из projection.

Reader/reopen для bound2: после HPB1 decode/check и существующих source/Core digest
checks `history_candidate` выполняет `check_binding2(source,stored_binding,static_binding)`.
Он заново извлекает wire fields, проверяет prefix по immutable proof Offer/Grant IDs,
строит ожидаемый normalized Binding и сравнивает все поля, span и hashes с HPB1.
Тот же helper применяется к writer parent/first/tail source reads и coherence после
reopen. Required proof отсутствует/повреждён — Integrity, не fallback в trusted mode.
Binding mode определяется authenticated Init2 binding, не наличием parseable Core2.

Public history по-прежнему возвращает только Text; request в Text HistoryRow точен.
Genesis через history_point не становится user row; internal genesis candidate имеет
request sentinel, а запрос wire metadata читает exact authenticated Event raw и
структурные fields, не Binding.request. Нового обещания wire request в старом DTO нет.

Unbound HVI1/HPB1 S2 stores и fixtures читаются прежним trusted reader: их synthetic
format2 bytes не нужно объявлять настоящим Event2. Их zero-request genesis и Text
metadata не переписываются. Nonzero genesis request внутри самого HPB1 остаётся
недопустимым и для unbound, и для bound2. Новый protocol2 writer не принимает такой
HPB1 от caller. Это совместимость storage представления, не ослабление R4.

### R2: authority resolution и четыре сохраняемые роли

State выполняет structural/hash correspondence; Root signatures, certificate validity,
revocation, freshness и «это current snapshot» проверяет owner adapter до вызова.
`admission_author_ref` и `readable_author_ref` — refs из проверенного owner current-policy
context, не принимаемые из peer hints как policy proof. Прямой вызов state API не
доказывает current authorization даже при успешном результате.

`resolve_auth2(ref,expected_cert_ref,inputs,Work)` работает одинаково для всех ролей:

1. Inputs≤2 на AcceptPending/row,≤1 на ACK; строго sorted unique по id. Для **каждого**
   supplied `HistoryAuth{id,bytes}` проверить bytes≤16KiB и `id=BLAKE3(bytes)` с пустым
   domain до обращения к старому add_auth. Даже duplicate/неиспользуемый input сначала
   не получает права на вставку. Duplicate id, включая identical bytes, — Bound error.
2. По требуемому ref взять exact input либо authenticated `Auth(context,ref)` из того
   же Work snapshot. Если оба существуют, bytes должны совпасть. Saved Auth разрешён
   только после повторной проверки hash и bundle shape. Read/Integrity/IO errors
   распространяются; отсутствие input и stored leaf — MissingReferencedAuth, не None
   и не успех по одному наличию id в чужой записи.
3. Bundle — canonical `[1,certificate_raw,authority_snapshot_raw]`. State вычисляет
   `cert_ref=BLAKE3(certificate_raw)` и требует точного expected certificate из static
   role map. Это hash exact opaque certificate bytes; state не дублирует identity
   crypto decoder и не проверяет подпись/содержание Root snapshot. Owner отдельно
   проверяет exact identity encodings, Root signatures/account/capability/role.
4. После разрешения всех required refs каждый supplied input должен быть использован
   хотя бы одной требуемой ролью этой операции. Unused, conflicting stored bytes,
   wrong hash/cert или missing required ref — отказ всей операции. Равные refs разных
   ролей разрешаются одним bundle; inputs и new-charge keys дедуплицируются по hash.
5. Только после этих checks private lowering добавляет отсутствующие immutable Auth
   records под **вычисленным** hash. Existing exact bytes не пишутся и не оплачиваются
   снова. Нельзя напрямую вызвать прежний размер-only add_auth на непроверенном input.

Пусть author role=`r=Core.role`, receiver role=`1-r`. Это роли автора ciphertext,
а не dialer/listener. Таблица обязательных evidence refs:

| Credential role | Откуда ref / допустимый bundle | Certificate check | Где/когда фиксируется |
|---|---|---|---|
| Original author `O` | `Core.author_auth_ref`; exact source original bundle из inputs или Auth(C,O) | static cert[r] | Auth bytes и immutable source/Core; у peer pending ещё первый capture ref |
| Admission author `A` | `admission_author_ref` из owner current peer policy в момент first admission | static cert[r], тот же certificate что O; **не требуется A==O** | Auth(C,A), Pending ticket.captures=[O,A] и admission_paid, в том же AcceptPending commit |
| Final readable author `F` | `readable_author_ref` из owner current author policy перед readable/materialize/genesis commit | static cert[r], **не требуется F==O или F==A** | Auth(C,F) плюс immutable per-event readable-authority Control record |
| Receipt author `R` | `ReceiptBody2.receiver_auth_ref`; exact receiver bundle из inputs или Auth(C,R) | `hash(cert_raw)==Receipt.receiver_cert_ref==static cert[1-r]` | Auth(C,R) и exact signed Receipt raw в Control; на own source — при ACK |

`O` и `R` принадлежат разным Device certificates. Сравнивать source.author_auth_ref
с receipt.receiver_auth_ref на равенство запрещено. Receipt scope/Q/format/lane/n/ID
сравниваются со source; receiver certificate — с **противоположной** static role.
Renewal меняет authority bytes/AuthRef, сохраняя exact certificate и selected Q.
Это допускается для A/F/R; historical O/A и ранее durable R не заменяются новыми refs.

Точные per-operation effects и persistence:

- **Init2:** proof/certificate role map и static binding; случайных Auth inputs нет.
  **Genesis/own Text:** required refs O и F (≤2 distinct bundles), peer admission
  отсутствует, receiver_receipt=None. Сохраняются source/O, F и его event link.
  InitialPair/genesis original-author cryptographic binding остаётся owner проверкой.
- **Новый peer AcceptPending:** required refs O и A (≤2), signature/static scope уже
  проверены адаптером; writer derive n/id из source. Private Plan объединяет ровно
  Pending+CapturePending credit actions одного slot/event. Обе Auth records и ticket
  с exact [O,A] фиксируются одной транзакцией. Без capture такой bound2 pending не
  появляется. Эта операция не выполняет native decrypt и не создаёт receipt.
- **Peer Text readable:** pending exact source/ID и captures=[O,A] обязательны; O/A
  заново resolve из сохранённых Auth, caller не переопределяет их через row inputs.
  Новые inputs только F/R (≤2 distinct); R берётся из переданного signed receipt,
  F — из owner context. Все четыре refs и certificates проверяются до final commit.
  Existing credit lowering переносит capture в Admission namespace `[5,O,A]` и учитывает
  admission_paid. Наличие этого role link не заменяет наличие exact Auth bytes.
- **Own Acknowledge:** required новый/сохранённый ref только R, inputs≤1. O/F source
  evidence уже должно быть coherent; A на собственном source отсутствует. Без
  resolvable opposite certificate/Auth(C,R) debt не снимается, даже если EventId известен.
  Current receipt-signer policy проверяется owner отдельно от historical receipt bundle.

Для F writer вычисляет private Control key
`H("kg:history2:readable-authority\0" || view32 || EventId32)`; bounded value=`[1,F32]`.
Он входит в те же row/retention refs. Это дополнительный узкий record в exact allowlist,
не поле HPB1. O берётся из source, A из Admission, R из receipt raw: после reopen
четыре роли однозначно восстанавливаются без перебора Auth namespace. После readable
writer/read helper требует referenced Auth leaves и проверяет их hash/certificate;
missing/corrupt evidence — Integrity. Crypto переоценка current policy остаётся owner.

Duplicate pending/readable source не пере-capture по новой authority. Adapter сначала
читает exact durable outcome и использует старые evidence refs; в typed duplicate
operation они должны совпасть с сохранёнными. Попытка заменить A при том же source —
Conflict, а не запрет последующего readable с новым F. При duplicate ACK проверяются
сохранённые receipt raw и R; новый current policy проверяется вне permanent event
evidence. Exact retry без Auth inputs использует уже проверенный stored R и даёт
zero writes. Supplied новый неиспользуемый renewal bundle — UnusedAuth, не ещё один
per-ACK snapshot. Новый receipt raw с другим R не перезаписывает durable receipt.

Original/admission receipts snapshots не удаляются при refresh. В одном peer event
максимум4 distinct credential roles O/A/F/R; совпадающие refs делят Auth record.
Промежуточные current revisions не образуют per-event history; bounded live pins
остаются будущему owner/control path. Reader проверяет evidence по point lookups,
не пытается выбрать «самую новую» authority из всех сохранённых Auth records.

### R3: ограниченные entrypoints structural reader в state

Список исчерпывающий; Core/Consent/Offer/Grant helpers private и обслуживают только
эти формы. State layer не принимает Frame/Inner как input:

| State entrypoint | Поддерживаемый raw / извлекаемые fields | Пара Codec2 для differential |
|---|---|---|
| `read_event2(raw)` | Event2 трёх kinds; Core/span, header tuple, opaque Olm, signature width, source ID; без Olm decode/MAC | `ParsedEvent2::parse` structural fields/Core span/id, до verify |
| `read_receipt2(raw)` | Receipt2/body12, signature width; scope/Q/event tuple, receiver refs, receipt ID | `ParsedReceipt2::parse` structural fields/id |
| `read_fresh_binding2(proof_raw)` | Только fresh ModeProof2: nested Consent2 origin[0], Control2 Offer/Grant bodies kinds1/2, chained IDs/T/cert refs/C, derived Q/D/E; unsigned shape/hash checks | `ParsedModeProof2::parse` fresh branch и её derived static fields, до crypto verify |
| `read_auth_bundle(raw)` | Credential bundle version1; exact certificate/authority bstr slices, AuthRef/cert_ref hashes | `ParsedCredentialBundle::parse` exact opaque slices/hashes, до identity verification |

`read_fresh_binding2` не декодирует InitialPair/Hello/Proof frames и не устанавливает
admission. Вложенные Control kinds1/2 здесь — только выбранные Offer/Grant, не generic
Frame kinds1/2. Existing ModeProof, Event1, Receipt1, Ready/P/A/K/Committed, descriptor,
Frame kinds1–21 и Inner2 остаются в codec/component layer. Unsupported state input
может получить отказ, но этот отказ **не** считается differential comparison fields.
State не расширяется до второго полного wire codec ради количества fixtures.

Parser differential сравнивает shape acceptance/fields на подходящих этим entrypoints
raw. Crypto/static scope и durable correspondence — отдельные уровни: например,
canonical source с неверным previous может совпасть в обоих parsers, но writer его
обязан отвергнуть; bundle с bad Root signature может пройти structural/hash stage,
но owner verifier обязан его отклонить. Это намеренное разделение trust boundaries.

### Source, seen и receipt выводятся из bytes

- EventId вычисляется самим store из domain2+exact source. Core span, C/D/E/SID/Q,
  role/n/previous/sequence/request/kind/parents/g/outer key/index извлекаются из source,
  а не принимаются из HistoryReadable2. Для HPB1 применяется только явная request
  normalization §R1; source/Core hashes охватывают исходный wire request.
  Parents resolve authenticated point records
  на текущем view, либо ранее staged row той же bounded batch; role/n берутся из них.
- Previous source/header текущей lane читается через LaneA/B→Event; current key
  сравнивается с raw нового source. First/new key: expected.g0 либо checked(old.g+1),
  j0 и authenticated absence Seen. Writer сам формирует value=`g_be8||this_EventId32`.
  Continuation: exact g, checked(old.index+1), existing Seen(g,first_id), verified first
  source с теми же role/key/g; запись seen не создаётся заново. Ни caller-supplied seen,
  ни Missing/IO/Integrity→None. Actual Olm header и phase ещё обязан проверить адаптер.
- Seen namespace — `Key::hash(context,SeenA|SeenB,key32)`, immutable40-byte value;
  lookup привязан к expected root generation/view. Перед final put writer повторяет
  absence/presence в том же Work snapshot, включая earlier staged inserts batch.
  Непосредственно перед собственным new-DH trial key неизвестен; после trial lookup
  и final transaction catch collision. Cross-C/role отсутствие не разрешает чужой key.
- Native key — `Key::hash(context,Native,[0;32])`. before_hash сравнивается с actual
  stored blob; frame_ids равны derived row IDs в exact transition order. after_blob
  включает native+guard≤128KiB. Store остаётся opaque к native truth; адаптер принимает
  только собственный successful trial, не raw after_blob с wire.
- Peer Text: receipt создаётся из verified payload/current receiver credentials на
  trial, его scope/event fields совпадают со source и Q, receiver cert — с cert[1-role]
  через resolver §R2. Source author AuthRef с receiver AuthRef не сравнивается. Atomic row
  содержит Event/Projection/Lane/Native/Seen/Auth и signed receipt. Genesis receipt нет.
- Own ACK: writer извлекает n/lane/ID/format/scope/Q из receipt raw, читает immutable
  source и сравнивает event fields; receiver cert/auth refs разрешает по opposite role.
  Adapter дополнительно
  проверяет actual opposite Device signature/current policy. Нельзя caller-only
  `{ordinal,event}` снять debt без receipt/source correspondence.

ReceiptA/B по slot сохраняет существующий EventId marker, который требуют H1 и credits.
Exact signed receipt хранится отдельным immutable Control record, key вычисляет writer:
`H("kg:history2:receipt-wire\0" || view32 || role_u8 || ordinal_be8)` внутри context.
Value=`[receipt_format,exact_receipt_raw]` в bounded storage encoding. Static binding
также использует private domain-derived Control key, не caller key. Namespace IDs,
Root5 trie/AAD/accounting не меняются. Receipt query позже читает эти exact bytes.
Duplicate с другим raw receipt не перезаписывает первый; возвращает conflict/old outcome,
а runtime должен повторять ранее durable receipt. Одного receipt-id hash без raw мало.

### Одна допустимая транзакция

New typed entrypoint формирует internally derived request digest из context/view/
expected/full operation; снижает его в private Plan и проходит existing
history_validate_plan/history_execute/credit settlement/commit_with. Новый вариант
CommitInput остаётся managed History; generic fencing не отключается.

Для каждой materialized row H1 по-прежнему требует ровно соответствующий FIFO
Materialize или exact Pending→Readable. Native/Seen/receipt/Auth не могут появиться
при пустой/дублированной row, с чужим C или через unrelated purpose. H2 fresh init
расширяет exact allowlist только известными static binding records; наличие прежних
sources/native/credit effects не становится допустимой инициализацией.

В retention refs writer добавляет именно derived Seen/Control receipt/readable-authority/Auth keys,
а не произвольный список caller. Records≤24; full planned bytes≤16MiB. New actual
bytes включаются в existing spent/new_charge/baseline accounting один раз; no free
extra receipt/seen и no second credit при pending→readable. Capture original-author,
admission-author, final-readable-author и receipt-author сохраняют четыре роли:
первые две через private CreditAction::CapturePending внутри нового typed Pending,
последние две через row auth≤2 и explicit F/R links. Старый public CapturePending
для bound2 закрыт. Control F/receipt bytes также входят в actual charge/retention;
если полный bound не помещается, вся операция отвергается, а не исключает auth evidence. Renewal
pins bounded, duplicate ACK не копирует снова permanent authority. До новой capacity
проверяется exact request idempotency, как в принятом A3 contract.

| Outcome | Accepted state и retry |
|---|---|
| Parse/signature/scope/guard/seen/counter/MAC/inner/cap rejection | Native accepted fork, source, seen, credits, projection, receipt и root generation прежние; native вызов0 для всех preflight rejections |
| Stale root/native before hash | Trial discard, load authenticated latest snapshot и re-evaluate; stale trial не повторять с изменённым expected |
| Immutable seen/source/receipt conflict | Conflict/fork evidence; не overwrite, не reset, не закрывать другую C |
| Capacity/ENOSPC до root commit | Нет publication; funded queue/pending/accepted debt сохраняются, новые reservations не теряются |
| Неоднозначный write/sync outcome | ReopenRequired/poison; reopen/recovery определяет exact root/commit_id/request digest. До определения повторная encryption/dispatch запрещена |
| Same commit_id и тот же полный digest | Existing duplicate report, zero writes; не выполнять native trial заново |
| Same commit_id с другим operation/receipt/source | Conflict; expected generation сам по себе не разрешает замену |
| Successful commit | В RAM устанавливается trial и dispatch получает только exact durable bytes; receipt означает readable после atomic commit |

## 5. Файлы, exports и порядок будущей реализации

Все новые имена ниже предлагаемые. Этой сдачей ни один Rust file не создаётся.

| Слой | Новый/изменяемый файл после review | Минимальная роль |
|---|---|---|
| Protocol | `crates/kilogram-protocol/src/ordered_dialog_v2.rs`, private `ordered_dialog_v2/{codec,authority,mode,tests}.rs`; one module export в lib.rs | Typed layouts/limits/domains, Parsed/Verified APIs, source header/static selection и explicit tagged dispatch. Codec1 primitives reused, Codec1 semantics untouched |
| Ratchet | `crates/kilogram-ratchet/src/ordered_v2.rs`, private `ordered_v2/{guard,preflight,checkpoint,tests}.rs`; one export в lib.rs | Guard2, PreparedStep2, Counter errors, bounded checkpoint encode/decode, pure L1 facts conversion. Никаких protocol types в ratchet |
| State | `crates/kilogram-state/src/vault/indexed_dialog/history_protocol2.rs`, private `history_source_v2.rs`, `history_protocol2_tests.rs`; declarations/exports в indexed_dialog.rs, integration в history.rs/history_writer.rs/history_types.rs | Typed point/closed commit seams §4, четыре restricted reader entrypoints, normalized HPB1 correspondence/read/reopen, auth resolver и H1/H2/retention extension; HPB1 predicate/encoding не меняются |
| CLI component | `apps/kilogram-cli/src/ordered_dialog/protocol2_component_tests.rs`, cfg(test) module в mod.rs | Existing private Guard1/PrivateState + new library modules, real native/indexed temporary vaults, import provenance fixture и matrix. Никакого runtime dispatcher branch |
| Evidence | новый S3 implementation report/log directory после разрешения | Actual source/EXE hashes/commands/counts/outcomes; failures append-only |

Guard2 API получает normalized `EventMeta2`/`SendIntent2`, explicit SeenObservation и
snapshot binding; protocol::VerifiedEvent2 конвертируется в EventMeta2 в owner layer.
Meta DTO не становится proof само по себе. PreparedStep2 private/single-use относится
к exact guard snapshot; protocol/owner проверяет crypto, ratchet — ordering/native
arithmetic, state — durable authentication/correspondence. Общего `verify_everything`
с bool или arbitrary callback внутри writer не добавлять.

Порядок:codec+pure preflight→Guard2/checkpoint→typed storage seam→component driver.
После каждой логической части targeted checks по изменённому surface; одна согласованная
сдача implementation на root review, публикация только принятого exact allowlist.

Reused: byte adapter/config1/public pickles; identity crypto/credential representation;
S1 authenticated trie/absence proofs, credits/growth/recovery; S2 H1/H2/page/cursor.
Future: production L1 origin adapter/file migration, actor/current bilateral policy,
Active2 persistence and live handoff, scheduling/offline queues/route2/IPC/history UX.
Dependency/lock updates для этой схемы не нужны.

## 6. Regression matrix будущей реализации

**Сейчас выполнено0 тестов; ниже точные планируемые cases/vectors.** Не считать
числа expected результатами. Все negative vectors локальные, без crafted network traffic.

Fixed setup: два Account/Device signers с seeds `[0x31;32]`/`[0x52;32]`, roles сортируются
по DeviceId; фиксированный C per case, clock1700000000 в valid interval, Root/Device/
authority/membership fixtures existing identity test helpers. Native keys берутся из
настоящего OS RNG (не подменять library entropy); schedule/text/request sequences
детерминированы. Text `request16=case_u32_be||role_u32_be||n_u64_be`;
genesis request — первые16 bytes соответствующего Offer/Grant ID;
text=`S3:{case}:{role}:{n}:Привет`.
Expected EventIds считаются из **сохранённых signed raw bytes** и затем сравниваются
после delivery/reload/retry, не из trusted DTO. Один native account/C на роль,
у B ровно одна OTK; никакого refill/reset. Stateful fixtures создаются заново в temp,
не открывают frozen L1/S1/S2 DB/EXE.

### Codec / crypto / format

| ID | Фиксированный набор | Required result |
|---|---|---|
| C01 |21 Frame kinds +3 Event/Inner kinds +2 ModeProof origins +2 receipt formats =28 round-trip cases | Canonical raw equality, signatures/IDs stable; Envelope/Data/Ack tags сохранены |
| C02 |12 invalid CBOR: overlong uint/length, indefinite array/bstr, map, text, negative, tag, float, null, trailing, allocation-bomb length | Reject до allocation/native; lengths bounded |
| C03 |10 cap pairs:Core/Event/Inner/Olm/Text/Auth/Membership/Consent/ModeProof/Frame; boundary helper length=cap и cap+1 =20 vectors | Helper принимает ровно cap и отвергает cap+1 до allocation; typed-valid largest reachable tuple проверяется отдельно |
| C04 |8 version cases:2 valid tagged legacy/new,2 internal tag/version swaps,2 wrong Inner version,2 Codec1 n512 rejection/Codec2 n512 acceptance | Single decoder selection; unchanged old bytes/ID/512 policy |
| C05 |12 independent disagreements:C,D,E,SID,role,Q,auth_ref,cert_ref,outer key,outer j,native kind,inner Core | Appropriate parse/static/guard/post-MAC rejection; never accepted plaintext/receipt |
| C06 |8 crypto negatives:Event/Frame/Receipt signature tamper, wrong Device, wrong exporter B, wrong Control kind, wrong Root Consent sig, wrong selected historical Proof | No Verified wrapper; current authorization не подменяется историей |
| C07 |8 mode cases:fresh/existing Q valid; Core/Ready/Resume/Receipt wrong Q; renewal preserves Q; fresh forbidden format1 | Один exact Q; hint не устанавливает selection |
| C08 |6 collection negatives:duplicate auth,unsorted auth,unused auth,duplicate parent,unsorted parent,list33 | Fail bounded; AuthRef unchanged для existing raw credential |
| C09 |4 genesis negatives:wrong Init request,wrong Confirm request,nonempty Init parents,nonempty Confirm parents | Correctly signed outer отклонён до native, R4 сохраняется |

В C05 изменять ровно одно поле и, где нужен более поздний guard/inner test, заново
подписывать outer правильным test Device. Иначе ранняя signature failure скроет seam.
Descriptor cap/Receipt cap/InitialPair cap — дополнительно3 boundary-helper cap/cap+1
пары (6 vectors). Для fixed-arity Core/Receipt/ModeProof реальный largest canonical
tuple может быть меньше cap: padding/extra fields ради exact-cap positive запрещены.
Boundary-helper acceptance не считать успешным typed parse; C01 покрывает valid tuples.

### Pure arithmetic / phase / provenance

M=2^64−1. Все **24** vectors ниже pure, native invocations0; никаких huge pickles:

| Набор | Exact vectors / outcome |
|---|---|
| Symmetric consume (2) | j=M−1→post M PASS; j=M→CounterExhausted |
| Continuation (2) | last=M−2→wire M−1/post M PASS; last=M−1→reject |
| DH step (2) | Tpre=M−1→Tpost M PASS; Tpre=M→next reject |
| turn (4) | g=(M−1)/2: A→M−1,B→M PASS; g+1: A/B reject |
| successor (10) | ordinal,author sequence,root generation,native generation,lane generation: M−1 PASS / M reject, по2 |
| Same-chain at T=M (2) | Send/receive при last_j0→wire j1 и прочих valid bounds PASS без DH step |
| Invalid phases (2) | New peer DH при Inactive и activation с inconsistent parent/used_own_first reject |

Отдельно **8 phase vectors**: outbound A0, inbound A0, B0 activate/receive,
A1 activate/receive, B1 activate/receive. Каждая проверяет expected Tpre/Tpost и j/post;
source role/first/ClosedTail определяют phase, debug/private pickle не используется.

**8 import negatives:** missing authenticated Head/AAD, unknown/libolm origin, wrong SID,
wrong config, wrong first source, wrong ClosedTail, missing seen first, inconsistent
rotation/used_own_first. Каждая до native и до durable writes. Pre-session OfferedA/
GrantedB optional fields проверяются двумя positive shape cases (без file migration).

### Real native + authenticated index component

N01/N02 используют fresh temporary indexed vault на каждую сторону, production
Builder/cache64MiB, finite capacity1GiB/vault, два vault открыты одновременно.
Queue/Pending/credits/readable/ACK идут через closed facade; Seen/Native real point reads,
source/projection/index real AEAD writes. Batch1, никакого synthetic native marker.
Funding фиксировано перед run, assertions проверяют credit settlement. Простые payloads
не требуют100k/import benchmark. У каждого case один C и новые account/session.

| ID | Exact setup/counts | Проверяемое evidence |
|---|---|---|
| N01 | A0/B0 +640 Text A→B без B Text. Reload обоих accepted native+guard/vault после Text256 и512, продолжить до640 |642 unique source events,640 exact plaintext/requests. A1 ordinary chain j0..639, crossing511 без reset; receiving tail при B Inactive. A.g1/B.g0, seen counts A2/B1, E/D/SID неизменны. Успешных native calls1284=4 genesis encrypt/inbound/decrypt +1280 Text encrypt/decrypt |
| N02 | A0/B0 +20 cycles:16 Text A,16 Text B =320/320. Reload обоих после cycles5/10/15 и в указанном late-tail месте cycle7/8 |642 sources,640 exact Text;20 new receiving DH каждой стороны, gA=gB=20, seen21/role. Успешных native calls1284. Каждый encrypted source и полученный EventId byte-exact после restore |
| N03 | Реальный L1 fixture:A0/B0 +8 A Text +8 B Text; verify accepted checkpoint provenance, pure import cuts9/9, затем1 Event2 Text A и1 B |20 sources/18 Text суммарно, session/account/E/D/SID неизменны;40 successful native calls=4+32+4. Только in-memory conversion fixture, не profile migration/live handoff/storage import readiness |

Late-tail schedule N02:cycle7 создаёт A Text97..112, B получает только97 и создаёт
B97..112; A получает B97. Доставить A98..112 B, создать/доставить A113 (первый cycle8),
reload обоих, затем доставить B98..112 A. Они продолжают прежнюю B7 receiving chain,
пока own A8 уже Active; T/rotate flags не меняются. Затем завершить A114..128/B113..128
и остальные cycles обычным порядком. Ни одного пропущенного peer ordinal native не получает.
User parents берутся из известных source heads, previous own user event включён и sorted.

Native plaintext сравнивается по всему UTF-8 payload и Core, source IDs фиксируются до
dispatch. В каждом accepted checkpoint assert complete sealed bytes≤128KiB, fixed
Guard2 size bound и absence растущего RAM seen inventory. Для cap failure отдельный
bounded test с excess input, а не искусственное раздувание private native pickle.

### Rejection / rollback / atomicity

| ID | Exact cases | Required unchanged evidence |
|---|---|---|
| G01 |7: exact duplicate,conflicting duplicate,out-of-order gap,seen old key reuse,missing current seen,lookup Integrity,lookup IO | Exact duplicate no native; conflict/gap no consume; lookup error fail closed. Source/root/credits/native snapshots unchanged |
| G02 |6 post-trial failures:bad MAC,inner version,inner Core,wrong payload kind,own generated seen collision,combined checkpoint cap | Whole account/session/guard trial discarded; compare accepted raw checkpoint hash and next valid exact transition; no receipt/publication |
| S01 |8 canonical/shape-valid source variants:role mismatch,n not next,wrong previous,C mismatch,Q mismatch,Text request not FIFO,seen key reuse,g mismatch | Parsers agree on extracted fields; writer rejects stateful correspondence. Span берётся из raw, caller span отсутствует |
| S02 |8 receipt-correspondence negatives:C/D/E/SID as one scope vector,Q,format,lane,n,EventId,receiver cert,receiver auth | Own debt remains; no false Delivered, caller EventId claim insufficient |
| S03 |6 atomic cases:stale generation,wrong native before hash,seen immutable conflict,source immutable conflict,missing paired row/credit,duplicate row/consume | No subset new root; H1/H2 and shared Author fence preserved |
| S04 |4 retry cases:same commit exact digest,changed source same commit,changed receipt same commit,reopen ambiguous commit | Duplicate zero writes; conflicting payload blocked; reopen chooses exact accepted commit, no re-encrypt |
| S05 |4 injected storage seams:before write,after data before root,root sync ambiguous,configured capacity refusal | All old/new atomic outcomes; exact Seen/Native/source/projection/receipt/credits coherence after reopen |
| S06 |6 bypass cases:generic write bound C,old trusted TextBatch bound2,old Acknowledge bound2,old AcceptPending bound2,old CapturePending bound2,Init2 over nonempty C | Closed writer required, mandatory bound2 capture и H2 init нельзя обойти; other C unchanged |
| S07 |6 vectors:2 positive A0/B0 nonzero wire prefixes commit/read/reopen;2 correctly signed wrong prefixes (C09 R4);2 unbound HPB1 compatibility cases zero genesis accepted/nonzero Binding request rejected | Exact source/Core/raw/ID сохраняются; bound2 HPB1 request sentinel0; wrong prefix pre-native calls0 |

S07 positive фиксирует Offer/Grant nonce bytes из setup и явно asserts оба hash prefixes
не равны `[0;16]`; никаких zero-prefixed wire fixtures для прохождения HPB1. После
commit/reopen через authenticated Event/Lane/Projection reads сравниваются full raw,
Core span bytes, EventId и normalized Binding; public history не показывает genesis.
Проверка unbound HPB1 использует вновь созданную bounded trusted fixture, не frozen DB.
Дополнительный внутренний corruption vector: HPB1 имеет согласованные digest/span
для чужого Core fragment, но не expected normalized fields целого Event — reader
bound2 обязан дать Integrity; это отдельный1 vector, не часть восьми S01 raw variants.

### Authority-specific component matrix (R2)

| ID | Exact cases | Required evidence |
|---|---|---|
| A01 |2 positive traces:O=A=F с одним author bundle; renewal O=A1,A=A2,F=A3 и R=B2 при тех же A/B Device certificates | Все Root snapshots действительно подписаны; source/receipt sigs проверены adapter. Pending→readable→ACK/reopen восстанавливает exact O/A/F/R, certificate role map и прежний Q; максимум4 refs, new-charge dedup |
| A02 |8 negatives:wrong supplied raw hash,missing O,missing A,missing F,missing R,wrong opposite certificate,duplicate input id,unused input bundle | Whole operation отвергнута, old root/credits/native/receipt/debt/evidence unchanged; нельзя «resolve» по одному caller id |
| A03 |3 duplicate traces:pending exact old [O,A] zero writes;attempt replace A with renewed A' conflict;ACK exact old R при current renewal R' zero writes | Historical capture/receipt bytes не меняются; R' live context не создаёт per-event permanent record. Missing R проверяется и до duplicate ACK success |
| A04 |1 boundary vector:credential c корректными shape/hash/cert_ref, но повреждённой Root authority signature | State structural extraction совпадает с Codec2 parse; owner verify_root не выдаёт verified credential, component commit не вызывается |

A01 renewal trace — bounded generated fixture, не live policy/IPC ceremony: old signed
source с author_auth A1 принимается при verified admission A2, после renewal decrypt
и readable decision при A3 создают receipt от verified receiver B2. В каждый момент
Root authority revisions монотонны и Device certificate unchanged. Existing pending
captures A1/A2 сохраняются; новые row inputs A3/B2, ACK использует B2 bundle. Duplicate
ACK с более новым current B3 проверяет прежний B2 receipt и ничего не дописывает.
Отдельный input B3 на этом duplicate не нужен и подпадает под A02 unused rejection.
Значения F/R links и bytes проверяются после close/reopen point reads, не только
наличием четырёх Auth leaves. Между transitions нет intermediate per-event snapshots.

G02 collision и cap — explicit failure injection на post-native lookup/serializer seam:
не подмена RNG и не пример естественного роста native pickle до128KiB. Native calls
этих negatives учитываются отдельно от successful-call counts N01–N03. После discard
проверяется следующий корректный переход; заново созданный ещё не published random
DH packet не обязан иметь bytes неудачного trial.

Для S01–S07/A01–A04 использовать freshly generated A0/B0 +4 Text/role (10 sources),
bounded copies/checkpoint forks per negative, не N01/N02 rerun для каждой ошибки.
Для cross-C isolation добавить отдельную C2 с двумя genesis (12 unique sources всего
в таком fixture) и сравнивать её authenticated points/root slots до/после отказа C1.
Failure matrix snapshots включают generation/root hashes, all affected point values,
native sealed hash, pending/queue/debt/credits и delivered marker, не только plaintext.

### Differential matrix ровно по четырём entrypoints (R3)

| ID / пара §R3 | Перечисленные raw fixtures | Comparisons |
|---|---|---|
| D01 `read_event2` / ParsedEvent2 | Valid Init,Confirm,Text и Text n512; сравнить весь Core/span/header и source ID |4 success |
| D02 `read_receipt2` / ParsedReceipt2 | Valid Receipt2 для source lane0 и lane1, оба event_format2; receiver refs opposite |2 success |
| D03 `read_fresh_binding2` / ParsedModeProof2 fresh | Один valid fresh proof с nested Consent/Offer/Grant; сравнить C/certs/T/OfferId/GrantId/Q/D/E |1 success |
| D04 `read_auth_bundle` / ParsedCredentialBundle | Четыре exact bundles из renewal trace O/A/F/R; одинаковый A certificate, разные A authority bytes |4 success |
| D05 `read_event2` / ParsedEvent2 | Восемь S01 shape-valid, stateful-invalid raw variants; compare extracted fields/ID, затем отдельный writer reject |8 success parses |
| D06 event pair | Overlong root-array length,overlong Core uint,trailing bytes,wrong Core arity,wrong Header arity,Event cap+1 |6 matching rejects |
| D07 receipt pair | Overlong n,signature63,body arity11,outer arity3 |4 matching rejects |
| D08 fresh-proof pair | Outer arity5,Consent origin wrong arity при fresh tag,Offer body7,Grant OTK31 |4 matching rejects |
| D09 credential pair | Overlong bundle-array length,arity2,certificate not bstr,authority not bstr |4 matching rejects |

Итого **37** comparisons:4+2+1+4+8=19 parsed field comparisons и6+4+4+4=18 malformed
matching rejections. Это не21 Frame decoder и не прежняя сумма48. C01 Frame fixtures
остаются codec-only; initial pair/Inner/legacy/existing proof не входят в D01–D09.
Reject-unsupported не засчитывается ни в19 field comparisons, ни в18 перечисленных
canonical/shape/cap rejections. Native MAC/Root signatures/current policy/durable
correspondence одним differential parser тестом не доказываются.

### Запуск после разрешения implementation

Все новые targeted test names имеют prefix `a3_s3_` для selection через stable runner.
В будущей evidence фиксировать actual counts по rows/vector cases, preflight/native
call counters, source/EXE hash и commands. Time ceiling component/codec10min из A3;
при превышении остановиться и разобрать bottleneck, не менять counts/limits молча.
Текущий S3 task задаёт Jobs12/BelowNormal; исторический Jobs2 в frozen A3 не применяется.

Пример будущих команд, **этой сдачей не выполнялись**:

```powershell
& ./scripts/invoke-cargo-friendly.ps1 test -CargoJobs 12 -CargoArguments @(
  '--locked','--profile','verification','--no-run',
  '-p','kilogram-protocol','-p','kilogram-ratchet','-p','kilogram-state','-p','kilogram-cli'
)
& ./scripts/run-cargo-tests-stable.ps1 -Package @(
  'kilogram-protocol','kilogram-ratchet','kilogram-state','kilogram-cli'
) -Filter 'a3_s3_' -Profile verification -CargoJobs 12 -Run -NoCapture
& ./scripts/invoke-cargo-friendly.ps1 clippy -CargoJobs 12 -CargoArguments @(
  '--locked','--profile','verification','--all-targets',
  '-p','kilogram-protocol','-p','kilogram-ratchet','-p','kilogram-state','-p','kilogram-cli',
  '--','-D','warnings'
)
cargo fmt --all -- --check
```

Stable runner исполняет постоянные EXE paths; hash/revision фиксируется по содержимому,
не в имени. Existing relevant Codec1/Guard1/S1/S2 non-heavy tests отдельно по затронутым
seams; Windows harness запускается исключительно stable runner. Не запускать scale100k,
two-process legacy corpus или network route gates для одного codec/component изменения.

## 7. Решения для review и критерий остановки

Рекомендуемая единственная схема — §1/§4: libraries + closed structural storage checks,
без новой dependency edge, с test-only CLI integration driver. Требуют явного root
review следующие implementation seam уточнения:

1. Согласовать четыре restricted state reader entrypoints и37 differential comparisons:
   semantic correspondence отдельно, crypto trust по-прежнему у owner adapter.
2. Согласовать явную genesis request normalization в HPB1 при неизменном wire/R4,
   Init2 binding, checked auth resolver, mandatory pending captures и immutable
   Control records receipt-wire/readable-authority. Это новые узкие state APIs,
   а не уже выполненные S2 возможности; H1/H2 и HPB1 predicate сохраняются.
3. Подтвердить, что production owner/L1 origin verification и live current policy
   вводятся на следующем integration этапе; S3 N03 даёт только component provenance/
   conversion evidence. Опасного публичного `adopt arbitrary pickle` API нет.

Неясностей в принятом wire2/Q/native-counter решении не обнаружено. Текущий код
ещё не реализует source semantic binding, receipt raw persistence или bounded Guard2;
их необходимость нельзя скрывать результатами S2. Если root не принимает предложенный
storage seam, сначала исправить этот план, не писать обходной generic writer.

В текущей сдаче результат — только этот документ. Local links/line anchors и whitespace
проверяются без test harness. Root README/coordination/task, clean development-environment,
review1, frozen S2 evidence/manifests, L1 EXE и S1/S2 fixtures сохраняются. Bytes/SHA,
решения R1/R2/R3 и оставшиеся вопросы передаются финальным ответом в этом чате для
read-only получения root. `send_message_to_thread` callback не повторяется.
Затем **stop до архитектурного review; commit/push нет**.

Проверка draft:28 local file links/line anchors найдены; missing/out-of-range0,
trailing whitespace0, `git diff --check` PASS. Root README/coordination/task SHA
совпали с входным snapshot; development-environment SHA53074ff… и L1 EXE SHAd9b7ff…
сохранены. Review1 остаётся exact SHA f46662b…; Rust/helper/dependency edits0.
Тестовых harness invocations0; DB fixtures не открывались.
