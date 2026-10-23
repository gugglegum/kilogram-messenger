# A1: контракт двустороннего ordered dialog v1

2026-10-23. **Контракт принят для реализации согласованного opt-in этапа.**
[Решение повторного review](lossless-sync-a1-bidirectional-contract-review-2.md).
Runtime/crypto/process acceptance реализации ещё не выполнен.
Baseline: `1fce03ee2d2886f871e0ce44a641e3eeba9cdd0f`, код первого slice —
`dcc1300b759d45b0dfbb77e08b93d2cac0957b80`.
Основание: [задание](lossless-sync-a1-bidirectional-task.md),
[приёмка первого slice](lossless-sync-a1-integration-review-2.md),
[его отчёт](lossless-sync-a1-integration.md),
[proposal §5](lossless-sync-a1-proposal.md#5-f1f2-feasibility-spike--уточнения-после-review),
[R1–R4](lossless-sync-a1-review.md), [S1–S3](lossless-sync-a1-spike-review.md),
[RFC-0193](../../docs/RFC-0193-user-sovereignty-and-contextual-identity.md).
Прямое разрешение человека проверено через `read_thread`: userMessage
`01a1661b-1561-7ec0-9380-b2835fd35738`, чат оркестратора
`01a164ec-589c-7b41-bc34-3ba158d1f22b`, host `local`.

Iteration 2: адресная правка C1–C3 по [review iteration 1](lossless-sync-a1-bidirectional-contract-review-1.md).
[Исходный контракт](lossless-sync-a1-bidirectional-contract-iteration1.md) сохранён побайтно,
SHA-256 `77c0c69babc7363353bcc422d144c057c374382a993560192415bdfca37686ab`.
Решения §15 и исправления C1–C3 приняты повторным review. Нормативные разделы
iteration2 не менялись при обновлении этого статуса; §16 фиксирует проверки до приёмки.

## 1. Решение и граница гарантии

Новый явно выбранный режим **OD1**: два авторизованных устройства, новый профиль,
до двух независимых личных contexts, по одной выбранной Olm session/epoch на context.
У session две направленные application lanes, но **один совместно сериализуемый
native Session state** на устройстве. Direct и prefix sync используют один ingress.
Свежие Olm accounts каждого context/epoch независимы; legacy pickles не импортируются.
Смена receiving DH chain обязательна и остаётся внутри этой же epoch.

OD1 даёт один committed результат на логический запрос отправки, byte-exact retry,
сохранность принятого ciphertext и последовательный decrypt от handshake genesis.
Progress требует доступного корректного prefix у авторизованного peer, fair scheduling,
действующей авторизации и успешных writes в обозначенном logical/physical budget.
Permanent gap, недоступный peer, invalid head, исчерпание или потеря ключей явно блокируют
соответствующий progress; новые epoch/reset/eviction не используются для обхода.

Это не migration старых профилей и не весь A1. Native unknown-tail gate первого slice
остаётся. Группы, multi-device fan-out/replacement, GUI, relay/mailbox delivery, A3,
M1.2.63 и внешнее использование до отдельного A2 dependency gate не входят в этап.
Первичное создание device keys нового профиля — обычный enrollment; переиздание или
ротация существующих device/root keys не являются способом включения OD1.

## 2. Проверенные исходные ограничения

| Факт baseline | Следствие |
|---|---|
| `kilogram-ratchet` использует `vodozemac 0.10.0`; SessionId доступен до encryption, `SessionKeys` содержит identity/base/OTK public keys | Выбранную session можно связать с transcript без trusted selection. |
| Native header даёт key/index, но не previous-chain length; receive-frontier getter отсутствует | Guard строится от нового genesis по собственному порядку; private pickle fields/Debug не читаются как importer. |
| `Session::encrypt` может создать новый sending DH key внутри вызова | Inner frame не может включать ещё неизвестный key или собственный EventId. Он связывает ожидаемый переход, outer signature — фактический header/ciphertext. |
| `iroh 1.0.3::Connection::export_keying_material` публичен | Новый channel binding выводится из реального TLS connection, не из строки endpoint. |
| Legacy `authorize_with_listener` использует `SyncSessionBinding::from_transport_label`; default ALPN — `kilogram/m0/sync/8`, payload Postcard v6 | Этот binding/codec не объявляется новым handshake. OD1 получает отдельный ALPN и admission. |
| Vault schema 3, cap snapshot 512 MiB, record 64 MiB; legacy wire cap 8 MiB | Новый opt-in schema gate обязателен; caps ниже ограничены полным encoded frame/commit. |
| `StateDirectoryLock::acquire` делает transaction recovery **до** vault open; старый manifest version=1 | Одного schema gate недостаточно: нужен versioned journal, свежий root и lifetime ownership. |

Код: [ratchet](../../crates/kilogram-ratchet/src/lib.rs),
[transport](../../crates/kilogram-transport-iroh/src/lib.rs),
[legacy authorization](../../crates/kilogram-protocol/src/wire.rs),
[canonical primitives](../../crates/kilogram-protocol/src/canonical_v1.rs),
[state/journal](../../crates/kilogram-state/src/lib.rs),
[vault](../../crates/kilogram-state/src/vault.rs).
Dependency sources прочитаны из закреплённых Cargo.lock registry copies, не из новой версии.

## 3. Имена, credentials и единственный выбор session

- `C`: 32-byte random context identifier, создаётся owner ceremony и привязан к
  подписанному membership двух accounts. Пользовательский display label не является C.
- Roles `A=0`, `B=1`: меньший/больший device signing public key по unsigned byte order.
  Оба обязаны вычислить одинаково после admission; одинаковый key/self-dialog отвергается.
  Roles вычисляются по pinned credentials до dial, затем подтверждаются admission;
  случайный порядок IPC open или прихода соединений их не меняет.
- `CertRef = H(cert_bytes)`, `AuthRef = H(auth_bundle_bytes)`. Bundle содержит exact
  существующие signed certificate + authority snapshot; максимум 16 KiB целиком.
  Membership максимум 32 KiB. Device cert bytes обеих ролей фиксируются в epoch.
- `EpochId E = H(epoch-tag || enc([1,C,certA,certB,offerA,grantB]))`, где `offerA/grantB`
  — bstr полного enc(signed durable record). `D = H(domain-tag || enc([1,suite,C,certA,certB,E]))`.
- `SID`: exact base64 string native `session_id()` (43 ASCII bytes для этой suite).
  Сравнивается с public prekey session ID, `SessionKeys` и выбранным session state.
- `Slot = (D,E,sender_role,n)`; `n` — u64 application ordinal, для L1 `0..511`.
  EventId — другой тип, 32-byte hash подписанного immutable event; SID, D и E также
  разные типы. Ни один hash/author_sequence/causal parent не заменяет ordinal.

Здесь `H=BLAKE3-256`, `enc` определено ниже. Никакого нового account-wide public ID
или счётчика нет. Существующие AccountId/device certificates пока видны и связываемы,
как и device author_sequence: это precursor RFC-0193, не готовая contextual anonymity.
Будущая замена credential mapping не должна менять смысл context-local lanes.

Admission требует локально принятого контакта с точными peer account/device и C,
валидного membership, capabilities SignEvents/SyncHistory, certificate-to-key binding,
монотонного locally pinned authority и отсутствия revocation. Peer-provided новый root
не становится доверенным из-за корректной self-signature. Root подписывает enrollment/
membership; online transcript/events подписывают Device keys.
AccountA и AccountB должны отличаться; same-account multi-device здесь UnsupportedPath.

Signed local/peer context consent задаёт C, две exact cert refs, OD1/L1 и validity
interval; default validity 30 дней, проверка времени с допуском 120 s. Consent renewal
только явно владельцем, для тех же C/devices, без смены E/SID/сброса frontiers.
Current signed authority snapshot baseline не имеет expiry: не выдумывать online
revocation oracle. Проверяются последние **локально известные** signed revisions,
а также срок consent/transport descriptor и connection authorization lease.
Истечение/revocation даёт AuthBlocked до новых writes; committed history не стирается.

## 4. Canonical codec и immutable event

Новый ALPN: `kilogram/m1/ordered-dialog/cbor/1`, suite `1`, limits profile `L1=1`.
Он регистрируется только OD1 runtime; legacy/cbor-v2 policy и default не меняются.
Нет payload sniffing, fallback после decoder/ALPN/admission failure или implicit upgrade.

Codec использует существующие deterministic CBOR primitives: только unsigned integers,
byte strings и definite arrays, минимальные integer/length encodings, фиксированная arity,
нет maps/tags/floats/indefinite/trailing bytes. UTF-8 text — проверяемый bstr, не CBOR text.
`Option<T>` = `[]` либо `[T]`; sorted sets без duplicates. Unknown version/kind запрещён.
Depth ≤8, items ≤8192 на отдельный объект, до allocation проверяются length/count caps.
Embedded encoded objects — bstr, каждый декодируется своим schema/limit.

Все tags — указанные ASCII bytes с завершающим NUL:

```text
kg:od1:domain\0                 kg:od1:epoch\0
kg:od1:event-sign\0             kg:od1:event-id\0
kg:od1:control-sign\0           kg:od1:control-id\0
kg:od1:auth-hello\0             kg:od1:auth-proof\0
kg:od1:auth-pair\0              kg:od1:channel-binding\0
kg:od1:consent\0                kg:od1:descriptor\0
kg:od1:receipt-sign\0           kg:od1:receipt-id\0
kg:od1:native-state\0           kg:od1:local-projection\0
kg:od1:private-offered-a\0       kg:od1:private-granted-b\0
kg:od1:private-selected\0
```

Durable control record = `[1,control_kind,body,device_signature64]`;
signature input = `control-sign-tag || enc([1,control_kind,body])`,
ControlId = `H(control-id-tag || enc(record))`. Epoch Offer/Grant/Ready используют это.

Все ключи/hash/ID имеют 32 bytes, Device/Root signatures — 64 bytes, времена — u64
Unix seconds, `revision` — u64≥1. `certA/certB` в схемах означают CertRef, не raw cert.
AuthBundle = `enc([1,raw_device_certificate,raw_account_authority_snapshot])`, оба raw
— bstr из существующего verified `encode()` Postcard; требуются decode→encode equality
и отсутствие trailing bytes. Membership — так же exact verified bytes существующего
`ConversationMembershipSnapshot`, scope=C, owner=AccountA, members ровно два accounts.
ConsentBody = `[1,C,certA,certB,1,L1,not_before,expires]`;
Consent = `[ConsentBody,root_signature_A,root_signature_B]`, обе signatures по
`consent-tag || enc(ConsentBody)`, ConsentRef=`H(consent-tag || enc(Consent))`.
Explicit account owner A создаёт C/membership и подписывает, B импортирует и подписывает
тот же consent; Root secrets не загружаются runtime. Consent≤4 KiB.

DescriptorBody = `[1,C,local_cert_ref,peer_cert_ref,endpoint_id,addresses[],revision,
not_before,expires]`, address=`[family,ip_bytes,port]`, family=4/6, ip=4/16 bytes,
port=1..65535; ≤16 addresses, sorted lexicographically по `(family,ip,port)`.
Descriptor=`[DescriptorBody,device_signature]`, signature по descriptor-tag+enc(body),
DescriptorRef=`H(descriptor-tag || enc(Descriptor))`; cap8 KiB. Проверяются peer pin,
монотонная revision и expiry≤not_before+3600. Новый runtime после bind атомарно
выдаёт новую revision; импорт хранит только текущую и предыдущую locator version.

`olm_kind`: 0=PreKey,1=Normal (native enum), `olm_bytes` — binary `to_bytes()` данного
variant, не base64/JSON envelope. `SessionKeys=[identity_key_A,base_key_A,one_time_key_B]`.
Suite1 использует только `SessionConfig::version_1()`, без experimental crypto changes.
Durable control kinds: 1=Offer,2=Grant,3=Ready; их body arity задана §6.

```text
Core = [1, C, D, E, SID, sender_role, n, previous_event_id?, author_sequence,
        cipher_kind, request_id16, causal_parents[], expected_native, author_auth_ref]
ExpectedNative = [g, expected_index, dh_parent_event_id?, closed_tail?]
ClosedTail = [previous_g, previous_ratchet_key32, previous_last_index, previous_last_event_id]
NativeHeader = [olm_kind, actual_ratchet_key32, actual_index]
Event = [1, Core, NativeHeader, olm_bytes, device_signature64]
Inner = [1, Core, payload]
```

Event signature input = `event-sign-tag || enc([1,Core,NativeHeader,olm_bytes])`;
EventId = `H(event-id-tag || enc(Event))`. Actual header должен точно совпасть с
разбором olm_bytes; для prekey сверяются также identity/base/OTK/SID.
Inner.Core byte-exact равен outer.Core. NativeHeader.key не входит в Inner:
он может возникнуть внутри encrypt. Inner.expected_native сверяется с actual header
и guard; собственный EventId нигде не включается в его plaintext/подпись как self-reference.

Cipher kinds: `1=Init`, `2=Confirm`, `3=Text`. Других encrypted controls в v1 нет.
Init/Confirm имеют n=0 соответствующей lane и payload bindings из §6; Text n≥1,
payload `[utf8_body]`, body≤4096 bytes. Все Olm encryptions расходуют ровно один n.
ACK, Ready, auth, Resume, range/control messages **не encrypt через Olm** и n не расходуют.
Автоматических encrypted key pings/ACK-of-ACK нет. DH происходит при обычном duplex traffic.

`request_id16` Text — IPC logical queue ID; у genesis первые 16 bytes соответствующего
Offer/Grant ControlId. `author_sequence` берётся из существующего device allocator
в той же transaction; другие contexts создают в нём gaps. Это не crypto cursor.
Parents — максимум два известных committed user-event causal heads этого C/E,
sorted by EventId; pending/control events туда не включаются. В этом two-author,
single-epoch scope parents должны быть среди уже committed peer prefix или собственных
authored events. Unknown/cross-context parent — invalid metadata, не причина native skip.

## 5. Реальный transport admission

На каждом новом Iroh connection сначала DirectOnly route gate и OD1 ALPN, затем auth.
Для локального acceptance endpoint имеет настоящие loopback IP transports, relay/lookup
отключены; `clear_ip_transports`/trusted session labels не заменяют сетевую проверку.
OD1 v1 требует directly reachable endpoints; relay/NAT-wide acceptance — другой scope.
Legacy ordinary runtime сохраняет свои route policies.

Для OD1 **A — единственный TLS dialer, B — listener**; открытие с B сначала ставит
local OpenIntent и ждёт A. После admission B может отправить OpenIntent по этому же
connection. Таким образом simultaneous IPC open создаёт один outbound attempt у A,
а не два конкурирующих transcript/epoch. На C допускается один admitted connection;
новый dial A сначала закрывает/инвалидирует старый, а stale callbacks получают новое
connection generation. Peer и C выбираются из pinned descriptor до dial и проверяются
снова в Hello; фактический accept от B или второй live connection получает Busy/Denied.
Два contexts могут иметь по своему connection. Другие relay/discovery роли не вводятся.

`X32 = connection.export_keying_material(label="kilogram ordered-dialog v1",
context=enc([ALPN,dialer_endpoint_id,listener_endpoint_id]))`.
Transport roles берутся из фактического dial/accept path. В wire/persistence идёт
`B=H(channel-binding-tag || X32)`, не X. Endpoint IDs сверяются с connection.remote_id
и подписанным locator/текущим Device proof; статическая строка endpoint не является B.

Порядок: оба Hello → оба Proof → оба AuthAdmitted. Hello содержит
`[C,transport_role,local_endpoint,remote_endpoint,nonce32,[suite=1],[L1],auth_bundle,
membership,consent_ref,descriptor_ref]`; raw bundle/membership — bstr, transport_role
0=dialer,1=listener. Hello и Proof — полные signed Wire records §10 с соответствующим
auth tag вместо control-sign-tag; signature охватывает все первые пять fields Wire.
`T = H(auth-pair-tag || enc([B,hello_dialer,hello_listener]))`;
оба hello — bstr full Wire. Proof body=`[T,C,selected_suite=1,L1,certA,certB]`.
Несовпадение выбора/ролей/nonce/credentials/ALPN отклоняется до создания native accounts.
Connection authorization lease ≤300 s и не дольше consent/descriptor. Reauth только
через закрытие старого и новый connection по §5.1, без competing in-place exchange.
Перед каждым mutation current authorization проверяется заново, clock/read failure —
operational; Queue без crypto materialization и чтение committed history не требуют
живого connection. Новая Text materialization требует current admitted lease.

При initial epoch admission каждая сторона сохраняет exact pair Hello/Proof и T
**до AuthAdmitted**. AuthAdmitted подписывает `[T,C,1,L1]`; control/data разрешены только
после обеих подтверждений. Этот initial pair входит в durable Offer/Grant и переживает
restart; reconnect устанавливает новый B/T и аутентифицирует повтор старых durable records.
Epoch identity не меняется из-за нового connection/exporter. Active reconnect не создаёт
новые native accounts/OTKs; новые auth records bounded, не бесконечный append-only log.

InitialPair=`[1,T,hello_dialer_raw,hello_listener_raw,proof_dialer_raw,proof_listener_raw]`,
≤192 KiB. Только A выбирает initial T первым durable Offer; B до Grant имеет лишь
candidate pair. Offer wrapper включает exact InitialPair: если у B после crash его нет,
B проверяет обе подписи/own Device proof, C/keys и current authorization и сохраняет
его вместе с Grant. Старый подписанный B подтверждает только прошлый transcript,
не заменяет fresh current-channel admission. До Grant неподобранный candidate можно
заменить выбранным A pair; после Grant T неизменен. Хранятся максимум selected pair и
один current-connection pair. Crash до публикации Offer не оставляет двух epochs.

Initial consensus требует одинакового L1, не произвольного минимума чисел. Недостаточный
version/limit/capability — UpgradeRequired; local storage credit согласуется отдельно.
Старые/отозванные credentials или истёкший consent не вызывают downgrade/new epoch.

### 5.1. Maintained connection, reauth и locators — C2

Source startup intent — durable OD1 runtime profile с `enabled_contexts[]`, максимум
два C с уже принятым consent/pins. `runtime-profile-create` задаёт этот explicit список;
runtime/relaunch читает его независимо от Queue/outbox/IPC open. Для **каждого** enabled
C роль A обязана запустить maintain task сразу после bind: dial/auth/reconcile даже
при пустой собственной очереди. B поднимает listener, при admission публикует свой
OpenIntent/phase и ждёт A; отдельный dial task у B запрещён. Context не включается
автоматически только потому, что кто-то прислал bytes.

Явная пауза в этом этапе — остановленный runtime либо исключение C из enabled_contexts
operator profile с его последующим relaunch; online silent pause не вводится. Пока C
disabled, network writers/auto materialization закрыты, Queue и history сохраняются.
Повторное включение использует прежние Head/Account/E/SID. Invalid/expired consent,
revoked credential, missing/expired locator дают явный blocked reason, не Paused/reset.

На enabled C только один maintain/auth coordinator, generation отменяет старый dial
и callbacks. Один общий deadline15 s охватывает dial+Hello+Proof+Admitted; не более
одного attempt одновременно. После ошибки backoff1/2/4/8/16/32/60 s, cap60; success
сбрасывает backoff. Обнаруженный close сразу запускает retry по этому расписанию.
На idle Active connection A делает Resume round каждые30 s с timeout15 s: отсутствие
response инвалидирует connection не позднее45 s, затем retry. C2 fair scheduler
не ждёт queue activity другого C. Valid locator/consent import будит уже существующую
task; при неизменном AuthBlocked повторная проверка не чаще60 s или explicit event.

A — **единственный инициатор auth state**: после dial он создаёт Hello request/nonce,
B создаёт ровно один response Hello/nonce на этом accepted connection, затем отвечает
на Proof и Admitted. Registry по `(C,connection_generation)` не допускает второго
creator из listener/outbound worker/IPC: duplicate request возвращает тот же response,
conflicting transcript закрывает connection. B не начинает самостоятельную reauth.
После закрытия новый generation создаёт новые nonces/B/T; выбранный epoch InitialPair
и immutable handshake records не меняются. Current pair заменяется bounded записью.

Lease deadline — минимум auth-established+300 s и consent/обоих imported descriptors
expiry. A планирует replacement на auth-established+240 s, **если это раньше deadline**;
он закрывает старый connection, отменяет его data writers и запускает один новый
auth attempt. Если short consent/locator deadline наступает раньше240 s, работает
именно deadline timer: после него AuthBlocked/LocatorRefreshRequired. Каждая сторона
самостоятельно отказывает expired/revoked lease до mutation; close не ждёт сетевого
подтверждения. Epoch keys/frontiers/outbox при reauth не сбрасываются.

После admission оба отправляют kind4 OpenIntent как current phase advertisement.
A автоматически приводит handshake к согласованному состоянию: Authorized→единственный
Offer; Offered→exact Offer (B возвращает свой exact Grant); InitDurable→exact A0
(B возвращает/создаёт B0 по §6); ReadyLocal/ActiveLocal→exact own Ready и Resume.
Если peer phase отстаёт, сначала повторяются нужные Offer/Grant/A0/B0, затем Ready,
только потом Text. Own ledger никогда не откатывается к peer phase; конфликт уже
выбранных refs/SID — blocked evidence, не создание новой session. OpenIntent после
reconnect обязателен и без своего IPC ordered-open, а NeedHandshake повторяет этот
repair plan. Единственный initiator **epoch** остаётся A, phase advertisement не election.

Descriptor lifecycle сознательно operator-assisted: при bind/relaunch endpoint
публикует signed locator revision+1, expiry=now+3600 s; отдельный explicit locator
refresh/export может подписать следующую revision тех же endpoint/addresses.
Это не enrollment/смена Device keys. Новые raw bytes доставляются peer существующим
CLI import/operator files. Hello несёт **только ref**, не передаёт новый locator;
неизвестный ref/новый endpoint требует import, самоподписанного Hello недостаточно.
Ни refresh по таймеру без доставки peer, ни автоматический discovery не обещаются.
Перед истечением operator может refresh/export/import; при expiry connection закрыт,
locally runtime status=LocatorRefreshRequired (wire reason2 AuthExpired; неизвестный
ref —11 Denied). Import валидной higher revision будит maintain/reconcile, без нового
open/send у A. Без актуальных bytes обеих сторон automatic progress не обещается.
Import/own publication новой descriptor revision инвалидирует lease, привязанный
к прежнему ref, и закрывает connection; A coordinator затем делает один fresh attempt.
Reader не продлевает старый lease молча и не выводит новые raw bytes из одного hash.

При relaunch A новый endpoint/descriptor может потребовать import **на B** до успешного
admission; у A уже должен быть действующий B descriptor. В B-only acceptance этот
operator шаг и interval LocatorRefreshRequired указываются отдельно. Как только pins/
locators валидны с обеих сторон, idle A обязан восстановить связь автоматически,
без своего IPC open/send: backoff≤60 s + attempt≤15 s при доступном loopback peer.

## 6. Epoch handshake и restart

Только A создаёт outbound Olm session. B при simultaneous open отправляет OpenIntent;
A при нескольких connections возвращает один durable Offer. Нет двух candidate sessions
и election по случайно первым пришедшим prekey messages.

| Запись / шаг | Обязательные bindings и atomic local action |
|---|---|
| ContextAuthorized | Explicit owner consent, C, membership, exact two certs, OD1/L1; allocation reserve. Data writers закрыты. |
| Offer A | `[C,certA,certB,T,nonceA32,fresh_A_curve_key,1,L1]`; A сохраняет fresh Account + Offer одним commit перед отправкой. |
| Grant B | `[C,OfferId,T,nonceB32,fresh_B_curve_key,one_time_key_B,1,L1]`; fresh B Account генерирует ровно один OTK, durable GrantedB/Reservation `[OfferId,GrantId,one_time_key_B,0]` по §12.1 до отправки Grant. |
| Init A0 | A создаёт одну outbound Session по B curve/OTK, знает SID и SessionKeys до encrypt; Core n=0, prev=None, g=0/index=0/parent=None/tail=None. Payload `[OfferId,GrantId,T,session_keys]`. Trial + A0 + native pickle + send cursor/outbox атомарны. |
| Confirm B0 | B проверяет prekey public tuple и SID, trial inbound Account/Session, MAC/Inner. Успех одной transaction потребляет reserved OTK, коммитит A0 и создаёт B0 на этой же session. B0 n=0, prev=None, g=0/index=0, dh_parent=A0.id, tail=None; payload `[OfferId,GrantId,SID,A0.id]`. |
| Ready | После проверки обоих genesis role создаёт durable signed `[D,E,SID,A0.id,B0.id,role,1,L1]`. B может записать свой Ready в transaction B0. A сохраняет B0/native cursor и свой Ready согласованно. |
| ActiveLocal | Только own Ready durable **и** exact peer Ready durable. Тогда Text materialization разрешена. Нет требования недостижимого одновременного общего знания activation. |

Native A account не публикует OTK: он только initiator; B OTK используется только A.
OTK не копируется из legacy pool, не резервируется повторно под другой Offer и не
возвращается в pool после consumption. Invalid trial его не потребляет; operational
failure откатывает Account/Session. Exact duplicate Init обслуживается из committed
handshake ledger, не повторным create_inbound_session. Другой Init/SID в той же epoch — fork.
PreKey разрешён только у A0, B0 и все Text — Normal. Public prekey identity/base/OTK
должны совпасть с SessionKeys, A curve из Offer и B OTK из Grant; native Account B
использует свой reserved curve/OTK, не переданные peer private material.

Если Text пришёл в ещё не ActiveLocal peer, ответ NeedHandshake без admission/decrypt.
Отправитель сохраняет Text и приоритетно повторяет exact Ready/genesis; это не failure
доставки, который можно исправить новой encryption/epoch. После reconnect Resume
содержит hashes сохранённых handshake records, стороны досылают недостающее.

Restart каждого шага даёт либо отсутствие committed record, либо тот же record/account/
reservation/SID; byte-exact **committed** records retransmit под новым channel wrapper.
До первого durable A0 SID ещё не выбран: crash uncommitted outbound trial может создать
другой SID, не заменяя существующую session. Peer не обязан
помнить volatile old TLS session: initial signed admission pair уже durable с обеих сторон.
Unpublished trial после rollback можно повторить; опубликованная reservation/selection
не заменяется. Exact same record idempotent; signed conflicting record блокирует context,
сохраняет оба доказательства в bounded control slot, не выбирает «победителя» молча.

В этом этапе нет epoch replacement/abort/reset после Offer. Один context имеет одну
pending/active epoch; timeout только backoff/reconnect/explicit diagnosis. Даже owner
consent не делает новую epoch средством обхода старого gap. Bad handshake может навсегда
блокировать этот context; другая независимая context остаётся отдельной. Политика
удаления/архивирования/переоткрытия таких contexts не включена.

## 7. Guard через DH transitions

Для каждого направления хранятся `next_n, last_event_id, g, native_key, last_index,
chain_first_event_id, seen_keys`. Совместный session state также хранит current local
sending-chain first EventId, `send_rotate_due`, последний received-new-chain first ID
и какой local send chain уже использован как основание peer DH transition.
State восстанавливается из authenticated OD1 checkpoint, не из native Debug/private fields.

**До trial receive:** подписи/authorization/context/epoch/SID, exact slot/ID/bytes,
`n==next_n`, `prev==last_event_id`; разбор actual native header соответствует outer.

1. Тот же peer native key: g прежний, actual index=last_index+1, expected_index тот же,
   dh_parent прежний, tail отсутствует. Index gap/underflow/overflow не разрешает decrypt.
2. Новый key: actual index=0, g=previous_g+1, key отсутствует в seen_keys;
   ClosedTail точно равен локальному `(g,key,last_index,last_event_id)` old peer chain;
   dh_parent — first EventId **текущей собственной sending chain**, ещё не использованной
   для другого peer transition. Иначе Invalid/blocked до native call.
3. Genesis — только два специальных случая §6. После selection никакого нового inbound
   session candidate или PreKey с другим SID. Нельзя иметь два новых peer keys подряд
   без intervening собственной sending chain, которую peer мог обработать.

После успешного trial native decrypt сравниваются Inner.Core и outer.Core, payload,
SID и все bindings. Только затем trial становится кандидатом commit. Native state
не сохраняется внутри decrypt helper. Bad inner/MAC отбрасывает failing trial.

**Send:** при `send_rotate_due=false` ожидаются прежний key/index+1/g. При true —
fresh key, index=0, g+1, cause=last received-new-chain first ID и ClosedTail прежней
собственной chain. Core с ожидаемым переходом известен до encryption; actual key
читается после неё и подписывается. Несовпадение собственного output с guard —
OperationalFailure, никакой публикации. Получение known-key continuation не меняет
send_rotate_due; получение нового peer key устанавливает true, первый send снимает его.
Decrypt и encrypt одной session никогда не выполняются параллельно.

Почему old tail закрыт: до new-chain event n проверены все `0..n-1`, prev связывает
последний old-chain event и explicit ClosedTail подтверждает его header. Sender
обязан присваивать ordinal **каждой** encryption в durable transaction. Поэтому
легитимного неучтённого K1 после signed successor L0 нет. Поздний exact old event —
duplicate без native decrypt; другой EventId в том же slot — fork. Автор, намеренно
скрывший/форкнувший собственные encryptions, может остановить свою lane; контракт
не восстанавливает тайные/никем не сохранённые данные Byzantine автора.

Пример: A шлёт K0,K1; B прочёл K0 и ответил B0; A получил B0 и создал L0.
L0 имеет next application ordinal после K1, prev=K1.id и ClosedTail=K1 header,
dh_parent=B0.id. B не decrypt L0 до K1, даже если native L0.index=0. Это и есть
недостающее authenticated ordering evidence; legacy L0 без него всё ещё блокируется.
Vodozemac может удалить старые receiving chains после обработки закрытого prefix:
replay обрабатывается по immutable ledger, а не повторным использованием message keys.

## 8. Durable send и сетевой retry

IPC queue request содержит stable request_id16, C, peer, body. Сначала durable Queue
без ordinal/ciphertext; повтор ID с теми же данными возвращает прежний queue/EventId,
с другими — RequestIdConflict. До ActiveLocal очередь ждёт handshake.
Ключ idempotency — `(C,request_id16)`; authored plaintext фиксируется как exact UTF-8
bytes, causal parents и author_sequence выбираются только при materialization.

Materialize — одна transaction под owner actor: current auth/mode/generation/credit,
queue всё ещё Queued, fresh current native state → trial encrypt → guard checks →
immutable event/Auth bytes → authored sealed projection → ordinal/author_sequence,
native/account state, source ledger и materialized outbox. Всё либо durable, либо нет.
Network получает immutable bytes **только после** successful commit/recovery visibility.
Ключи не расходуются отдельным `next_message_key` и не публикуются из volatile trial.

Все transport retries/resume отправляют те же Event bytes/EventId и Auth records.
Channel B/request id/wrapper signature на новом connection могут быть новыми — это
не повторная encryption события. Outbox acked/dequeued не удаляет source ciphertext:
он хранится весь lifetime epoch и обслуживает prefix repair.

Точная crash-гарантия: одна **committed/materialized encryption** на request_id.
Если процесс погиб после вызова encrypt, но до commit и публикации, volatile trial
может вычисляться снова; старый EventId не был durable/видим и ordinal hole не возник.
Запрет самого повторного CPU-вызова encrypt в этом окне без сохранённого результата
не обеспечивается transaction и не обещается (вопрос review §15). После materialized
commit новое encrypt для этого request_id запрещено даже при lost ACK/restart.

## 9. Receive, frontiers и ACK

Один ingress получает direct DeliverBatch, PrefixResponse и повтор после restart.
Admission сохраняет exact encoded Event + нужные exact Auth bytes/decision refs
в encrypted vault pending, отдельно от readable EventStore и causal parents.
Есть четыре разных состояния: possession IDs; crypto committed prefix; readable text
projections; causal heads. Acked source outbox/frontier — ещё одна независимая величина.
Control genesis расходует crypto ordinal, но не является пользовательским сообщением.

Drain обрабатывает максимум 8 contiguous records в одной transaction. Guard/trial,
native state, committed immutable event/authorization, projection, removal pending,
frontier/causal update и durable ACK intent коммитятся вместе. Cache/overlay обновляются
только после commit, при operational error инвалидируются/reload перед следующей работой.

| Outcome | State и реакция |
|---|---|
| Pending | Exact bytes durable, possession есть; нет decrypt/readable/message ACK. |
| Readable | Trial и bindings успешны, projection/native/event/frontier/ACK intent durable. Exact duplicate возвращает stored receipt, не decrypt. |
| Invalid до admission | Нет новых bytes/frontier/ACK; bounded diagnostic. |
| Invalid ready pending | Durable quarantine exact offending bytes/reason, failing trial discarded; valid prefix того же batch может коммититься вместе. Ordinal не пропускается. |
| OperationalFailure | I/O/local state/pickle/decode своего checkpoint/maintenance/unknown error: rollback всего текущего batch, не quarantine. Ранее durable batches не отменяются. |
| Blocked | AuthExpired/Revoked, missing predecessor, StorageBlocked, UpgradeRequired, invalid head; accepted bytes и старый readable prefix сохранены. |

Pure input rejection отделено от чтения local authority/membership/state. Unknown local
error не объявляется атакой peer. Unexpected missing native key — local/state failure,
а не разрешение skip/reset. Auth policy change после admission даёт AuthBlocked,
не делает исторический readable текст удалённым. Повтор quarantine не вызывает decrypt
или busy loop; другие contexts продолжают работу.
Quarantine продолжает занимать тот же pending slot/bytes; accepted bytes не вытесняются
новым ingress. Для fork сохраняются первые два конфликтующих signed records в отдельном
bounded control slot; последующие conflicting bytes отвергаются **до** admission.

ReceiptBody = `[1,D,E,lane,n,event_id,receiver_cert_ref,receiver_auth_ref]`;
Receipt=`[ReceiptBody,device_signature64]`, signature по receipt-sign-tag+enc(ReceiptBody);
AckId=`H(receipt-id-tag || enc(Receipt))`. Он создаётся и хранится
один раз для Text event, не расходует Olm ordinal/author_sequence. Никакой ACK для
Pending/invalid. Receipt доступен dispatch/queries только после readable commit.
Sender сверяет exact own ledger slot/id, signer/context и current admission, затем
атомарно помечает тот же request/outbox Delivered. Другой id при том же n не принимается.

AckIntent Pending/Attempted хранит exact receipt bytes. После network write он может
стать Sent, что не доказывает peer delivery. Receipt всё равно сохраняется; duplicate
Text или AckQuery возвращает **тот же** receipt. После restart Pending intents досылаются,
sender replays/query ACKs для своего unacked window. Resume readable cursor — hint для
AckQuery, не substitute ACK. ACK-of-ACK не создаёт native/message events.

## 10. Wire control, prefix repair и partial rounds

Один bi-stream содержит один length-prefixed request и не более одного response,
затем FIN. Terminal kinds 13/14/15/17 не требуют ответного application frame.
Wire = `u32be(length) || enc([1,frame_kind,B,request_id16,body,channel_signature64])`.
Signature по control-sign-tag + enc первых пяти fields; initial Hello/Proof используют
свои tags. Length включает весь CBOR, 4-byte prefix также входит в cap1 MiB.
Response повторяет request_id16. Не более двух in-flight streams на peer/context;
timeout 15 s, backoff
1/2/4/8/16/32/60 s, без немедленного повтора неизменного rejected batch.

Kinds: 1 Hello, 2 Proof, 3 AuthAdmitted, 4 OpenIntent, 5 Offer, 6 Grant,
7 EpochCipher (Init/Confirm), 8 Ready, 9 Resume, 10 DeliverBatch, 11 PrefixRequest,
12 PrefixResponse, 13 BatchComplete, 14 RoundError, 15 AckBatch, 16 AckQuery,
17 HeadAdvertisement. Схемы body фиксированы (имена raw означают encoded bstr):

```text
1 Hello       [C,transport_role,local_endpoint,remote_endpoint,nonce32,[1],[L1],
               auth_bundle_raw,membership_raw,consent_ref,descriptor_ref]
2 Proof       [T,C,1,L1,certA,certB]
3 Admitted    [T,C,1,L1]
4 OpenIntent  [C,local_phase,offer_id?,grant_id?,init_id?,confirm_id?]
5 Offer       [C,offer_raw,initial_pair_raw]
6 Grant       [C,offer_id,grant_raw]
7 EpochCipher [C,D,E,event_raw]
8 Ready       [C,D,E,ready_raw]
9 Resume      [C,D,E,SID,handshake_ids,send_cursor,receive_cursor,status,credit]
10 Deliver    [C,D,E,lane,auth_records,events_raw[]]
11 PrefixReq  [C,D,E,lane,start,end_exclusive,expected_previous_id?,
               max_events,max_wire_bytes,ordinary_slots,ordinary_stored_bytes,head_stored_bytes]
12 PrefixResp [C,D,E,lane,start,end_exclusive,materialized_head,
               auth_records,events_raw[],more]
13 Complete   [C,D,E,related_request_id16,event_ids[]]
14 Error      [C,D?,E?,related_request_id16,outcome,reason,position?,retry_after_s]
15 AckBatch   [C,D,E,lane,auth_records,receipts_raw[],not_readable_pairs[]]
16 AckQuery   [C,D,E,lane,slot_id_pairs[]]
17 Head       [C,D,E,send_cursor,receive_cursor,status,credit]

Cursor = [next_n,last_event_id?]
Credit = [ordinary_slots,ordinary_stored_bytes,head_stored_bytes,max_events,max_wire_bytes]
Status = [reason,position?]
SlotIdPair = [n,event_id]
AuthRecords = [[auth_ref,auth_bundle_raw]...]  // ≤32, sorted by ref, no duplicates
HandshakeIds = [offer_id?,grant_id?,init_id?,confirm_id?,ready_A_id?,ready_B_id?]
```

Credit — snapshot usable admission capacity, не удалённая lease и не possession set:
ordinary_stored_bytes=min(free ordinary byte budget,ordinary_slots×64 KiB);
head_stored_bytes=64 KiB только при свободном head slot, иначе0. После interleaved
admission snapshot может устареть; receiver всегда проверяет actual encoded charge
в своей transaction. Именно поэтому partial quota outcome возможен и не означает
protocol completion. Неиспользуемая физическая quota при slots=0 не рекламируется.

`more`=0/1; ranges `0≤start<end_exclusive≤512`; response end — actual end of contiguous
returned records, не выше requested end. Prefix materialized_head — source Cursor;
при short response more=1, иначе0. Все count/credit bounds ограничены §11;
неиспользуемых auth rows нет. `local_phase`: 0=Authorized,1=Offered,2=Granted,
3=InitDurable,4=Confirmed,5=ReadyLocal,6=ActiveLocal,7=Blocked. Missing phase records
повторяются exact посредством kinds5–8, до SID Resume заменяется OpenIntent.
Reason codes: 0=None,1=WaitingPrefix,2=AuthExpired,3=Revoked,4=StorageBlocked,
5=UpgradeRequired,6=InvalidPrefix,7=OperationalFailure,8=NeedHandshake,9=Busy,
10=MissingPrefix,11=Denied,12=InvalidInput,13=RequestIdConflict. Error outcome:
1=Invalid,2=OperationalFailure,3=Blocked; unknown code — decoder rejection, не fallback.
Error bounded≤1 KiB; D/E оба отсутствуют только до selected epoch. Position — ordinal.

Обмены: Hello→Hello, Proof→Proof, Admitted→Admitted, Offer→Grant,
Init→Confirm через kind7, Ready→Ready, Resume→Resume, Deliver→Complete/Error,
PrefixReq→PrefixResp/Error, AckQuery→AckBatch/Error. OpenIntent не требует response;
обе стороны публикуют phase по §5.1, A начинает/повторяет свой handshake record. После
PrefixResp receiver отправляет Complete/Error отдельным stream с original related ID.
Terminal response допустим в request stream; повторно ответ на него не создаётся.
Concurrent duplicate immutable controls idempotent; response не создаёт новую epoch.
Durable controls/events внутри wrapper имеют свои signatures/IDs, не пересоздаваемые
на retry. Current B/signature/request correlation не входят в EventId/ControlId.

- HeadAdvertisement/Resume содержат materialized send `(next_n,last_id)` и receive
  `(next_n,last_id)`, blocked reason и exact storage credit; Resume также handshake IDs.
  Они подписаны текущим connection owner; advertised head — committed, не queued.
- PrefixRequest body: `[C,D,E,lane,start,end_exclusive,expected_previous_id?,
  max_events,max_wire_bytes,ordinary_slots,ordinary_stored_bytes,head_stored_bytes]`.
  Request nonce — wrapper request_id. Только выбранному участнику этой epoch.
- PrefixResponse: `[C,D,E,lane,start,end_exclusive,materialized_head,
  auth_records[[hash,raw]...],events[raw...],more]`. Contiguous increasing ordinals,
  первый prev равен expected_previous_id; exact ID/signatures/chain для каждого.
  Source отвечает из собственного immutable materialized ledger, не re-encrypt.
- Отсутствующий requested committed record → signed MissingPrefix/Blocked с точной
  позицией; никакого обхода другого hash page, выдуманного completion или reset.
- DeliverBatch/PrefixResponse максимум 32 events, но всегда ограничены encoded bytes
  всего frame. Все необходимые Auth records включаются один раз по hash; неизвестный
  auth ref без bytes не допускается в durable pending.
- BatchComplete содержит `[C,D,E,request_id,exact_event_ids_in_response_order]` только
  если **весь** batch durable. Possession completion не означает readable/ACK.
  При partial quota/IO/invalid outcome — RoundError, не subset completion. Durable
  subset остаётся известен только receiver из его local pending index; Resume не
  сообщает possession IDs. Source сохраняет все bytes и выполняет repair по §10.1.
- AckQuery запрашивает до 32 own materialized `(n,id)`; AckBatch возвращает stored
  receipt bytes + auth records, либо NotReadable для точных отсутствующих ACKs.

При gap запрашивается именно `start=receive_next`, не первые missing hashes.
Если ordinary pending заполнен, head-only request имеет max_events=1 и отдельный
head credit; sender обязан начать с requested head, не с предпочитаемых late events.
После head commit drain до 8 и повторяется, затем запрашивается следующий gap.
Known duplicates не занимают новое место. Существующий pending head не запрашивается
бесконечно: он либо drain-eligible, либо публикует явный blocked reason.
Fair scheduler чередует два contexts; не держит transaction или network wait ради gap.

### 10.1. Repair без remote possession inventory — C3

Нового inventory/query kind нет. `possession IDs` §9 — **локальное** множество receiver;
из Resume/Head нельзя выводить, какие future events pending. Source знает собственный
immutable ledger/outbox, signed committed receive cursor peer, status/credit и exact
receipts. Даже full BatchComplete — знание possession только указанного batch, без
права удалить source history или заменить readable ACK. Для progress не требуется
помнить это знание после restart.

Нормативный алгоритм на каждой стороне после reopen/reconnect:

1. Vault recovery загружает exact pending/local index и committed receive_next.
   Как только admission/consent/mode разрешают mutation, автоматически планируется
   drain существующего head, не ожидая Text/IPC sync. Если нужен fresh connection
   lease, task ждёт maintain/admission §5.1 и просыпается после него. По8 contiguous
   records на transaction, yield между batches для другого C; до gap/quarantine/
   AuthBlocked/StorageBlocked. Получение каждого нового pending head тоже будит drain.
2. После handshake/Ready каждый обмен Resume сообщает **актуальный** send Cursor,
   receive Cursor, status и Credit. До первого такого snapshot после reconnect
   source не повторяет future DeliverBatch. Неизвестный remote subset остаётся
   неизвестным; равенство credits не считается равенством pending IDs.
3. Receiver использует peer.send_cursor.next_n как authenticated materialized upper
   bound. Если локального pending[receive_next] нет и upper bound больше next, он
   сам посылает PrefixRequest(start=next, expected_previous_id=last committed ID).
   При ordinary_slots=0 **либо** ordinary_stored_bytes недостаточны для maximal item:
   end=next+1,max_events=1,max_wire_bytes≤68 KiB,head_stored_bytes=64 KiB, из отдельного
   reserve. При свободной ordinary quota end=min(upper,next+32), response дополнительно
   byte-capped; меньший frame разрешён. Уже имеющийся head сначала drain/явный Blocked,
   его не загружают снова. Actual blocked head не превращается в запрос successor.
4. Source приоритетно обслуживает authenticated PrefixRequest из **того же** ledger,
   включая уже possession-completed/Delivered events. Параметры запрошенного range/
   prev/credit проверяются, head приходит первым; это не новый encrypt. После response
   receiver сохраняет новые exact bytes, duplicates не charge; запускает drain и
   следующий нужный range. Complete отправляется только для **всех IDs этого нового
   response** после admission, либо Error/обрыв. Он не достраивает старый partial batch.
5. Для своих unacked Text с n<peer.receive_cursor.next_n source делает AckQuery
   (до32 exact slot/id pairs). AckBatch выдаёт сохранённые receipts; только их проверка
   делает Delivered. NotReadable не означает possession absence. После drain receiver
   также dispatch Pending ACK intents; потеря ACK покрывается query/duplicate Text.
   Для n≥peer cursor сначала нужен receiver-driven prefix; plain cursor не ACK.

Scheduling rule: максимум один unconfirmed future DeliverBatch на direction; timeout,
partial Error или обнаруженный gap переводит её в RepairWait. В RepairWait control/
PrefixRequest/PrefixResponse/AckQuery и drain имеют приоритет над повтором future push;
source не шлёт снова тот же заведомо переполненный future batch. Условия выхода — новый
Resume/Head с продвинувшимся receive cursor **и** подходящим ordinary credit; без
изменения действует backoff/periodic Resume, не busy loop. Future outbox не удерживает
transaction/stream бесконечно: timeout15 s освобождает место для repair. При restart
RepairWait может быть volatile: правило «сначала Resume» восстанавливает его без знания
remote subset. Remote requests обслуживаются независимо от собственной unacked window;
queued Text/неполученный Complete не блокируют выдачу source prefix.

Точный partial/lost-Complete trace (соответствует 32-event wire cap): receiver уже
прочитал A0/A1, next=2; pending A3..A49 (47 slots). DeliverBatch A50..A81 содержит32
подписанных immutable events, но до разрыва durable только A50..A66 (17). Теперь
pending A3..A66 занимает64 ordinary slots; ни Complete для32, ни Text ACK этим future
17 не выдавался. Source хранит весь исходный ledger, но не знает эти17 IDs как accepted.
После reopen next=2 сохраняется; Resume показывает receive Cursor(2,A1.id), slots=0,
ordinary_bytes=0,head_bytes=64 KiB; source.send Cursor(82,A81.id). Receiver запрашивает
lane A,[2,3),prev=A1.id,max_events=1,head credit. Exact A2 входит через reserve; drain
до8, затем следующие автоматические batches доходят до A66, next=67. Повторы любых
A3..A66 не charge/decrypt повторно. Затем PrefixRequest [67,82), byte-capped на pages,
доставляет прежние A67..A81, после commit next=82. AckQuery/dispatch дают exact receipts
для32 A50..A81, источник помечает их Delivered без знания старого subset. Старый
request так и не получает ложного subset Complete; новый одноэлементный head response
или поздние полные pages могут получить Complete только для собственного exact batch.

## 11. L1 budgets: frame, pending, полный commit

Это предложенные нормативные bounds нового формата, не перенос старых чисел proposal
на legacy. Реальные serialized sizes проверяются до admission/materialization и commit;
при невозможности выдержать bounds — StorageBlocked/UpgradeRequired, не byte rewriting.

| Объект / ресурс | L1 |
|---|---|
| Contexts / epochs | 2 contexts/profile; 1 pending/active epoch/context, без replacement |
| Lifetime | 512 encrypted frames/lane, включая n=0; после n=511 — UpgradeRequired, не reset |
| Text / Inner / raw Olm / signed Event | 4 / 8 / 12 / 16 KiB; Outer.Core ≤2 KiB |
| Auth bundle / membership / Hello | 16 / 32 / 80 KiB |
| Consent / descriptor / InitialPair | 4 / 8 / 192 KiB; selected и один current pair включены в control cap |
| Полный stored admission item | 64 KiB: Event, original author Auth, current pinned peer Auth и metadata; refs ведут только к durable exact blobs |
| Pending каждой receiving lane | 64 ordinary slots ×64 KiB + **1 head slot ×64 KiB**, hard cap 4160 KiB |
| Unacked sender window / queued texts | 128 materialized Text/lane; 128 unmaterialized/context, sealed item≤8 KiB; genesis учитывается control ledger |
| Native + sealed guard record | ≤128 KiB/context включая sealing/codec overhead, opaque pickle + bounded seen-keys; no refill/unbounded skipped-key dump |
| Permanent charge / encrypted frame | 128 KiB, включая event, projection≤8 KiB, receipt≤1 KiB, indices/outbox и до четырёх auth blobs по16 KiB |
| Initial/control records | 4 MiB/context; initial admission, credentials, guards, Ready/conflict diagnostics bounded |
| Drain batch / full encoded transaction delta | ≤8 records / ≤32 MiB, включая old/new copies full manifest/index≤8 MiB каждый, native/control changes и deletes/indices |
| Root commit reserve | 32 MiB не расходуются ordinary writers; одна mutation transaction/profile |
| Root ordinary/admin base / emergency credit | 16 / 4 MiB; current revocation gate не требует места нового сообщения |
| Logical profile bound / underlying vault | ≤384 MiB / existing 512 MiB; record cap 64 MiB не увеличивается |
| Physical/staging policy | ≤2 GiB profile files, journal/shadow/redb staging; OS write/sync могут всё равно отказать |
| Wire / response count | ≤1 MiB **полный** frame, ≤32 events, decoder bound до allocation |

При activation резервируется **весь lifetime permanent budget** context:
`2 lanes ×512×128 KiB =128 MiB`. Он не продаётся другим writers и учитывает будущий
рост readable history. У обоих contexts всего 256 MiB. Head consumes свой reserved
slot/bytes; следующий head финансируется оставшейся заранее оплаченной lifetime
capacity, а не предположением «lease текущего head освободится целиком» (S3).
Даже консервативное двойное учётное покрытие pending даёт
`16+256+16.25+8+2+4+32 =334.25 MiB <384 MiB <512 MiB`.
Новые contexts/administrative writes не могут потребить эти reservations.

128 KiB включает worst-case 16 KiB event +8 projection +1 receipt +8 indices/outbox
+64 auth и запас под record encoding; actual affected record sizes/index-path overhead
суммируются в transaction plan. Четыре refs покрывают event author, authority при
admission, authority при final decrypt/delivery decision и receipt author. Между
admission и final commit промежуточные refresh не создают per-event auth history;
current/previous pins ограничены admin/control budget. Subsequent duplicate ACK не
добавляет decision snapshot к уже Delivered событию. Sharing лишь экономит fit.
Whole wire builder учитывает raw auth bytes и подписи: `32×64 KiB` уже превышают 1 MiB.
При worst-case item64 KiB и wrapper allowance4 KiB, включая length prefix,
помещаются **15**, не32 items.
Head-only ≤68 KiB wire. Сначала encoded-byte fit, только затем count.
Полный manifest/index charge входит в permanent per-frame bound; один commit может
переписать его целиком, поэтому 32 MiB transaction reserve не подменён суммой восьми
новых текстов. Recovery/staging planner дополнительно считает **actual** old/new
file copies и redb allocation; logical delta не является верхней границей disk writes.

Logical head/progress гарантия относится к compliant L1 frames, зарезервированному
lifetime и успешным physical writes. Staging не является OS reservation: проверяются
фактические file lengths и next-transaction backup/replace footprint до transaction;
во время ошибки нет false ACK и accepted bytes не удаляются. Redb allocation/full disk
может дать StorageBlocked даже при final-history fit; это отдельный честный outcome,
не доказательство moving reserve. Точные большие metadata/state bounds подтверждаются
новыми encode-bound tests перед activation, не объявляются уже измеренными здесь.

## 12. Хранение, ownership, совместимость

Новые profiles создаются только явным `device-enroll --profile-mode ordered-dialog-v1`
в **новом** state directory. Не принимать существующий initialized/legacy/single-chain
root даже если сейчас нет messages. Повтор возможен только для собственного interrupted
Initializing claim; если до захвата lock возникло чужое state — CreationConflict.
Schema4, mode marker и fence становятся durable под тем же OS lock **до** публикации
credentials/descriptor/готовности. Пока initialization не завершена, writers закрыты.
Инициализация сначала durable устанавливает account fence, затем v2 journal + schema4
DB-primary/mode; только после обоих допускаются protected domain/authority records.
Crash до первого fence/manifest не создаёт accepted OD1 state: если old binary успел
инициализировать такой ещё пустой root, OD1 resume получает CreationConflict. Это
не migration и не обещание отказа old binary на ещё не созданном profile. После
готового schema4 ни один legacy writer не получает mutable repository. Между фазами
creation runtime удерживает lifetime OS lease; точные seams входят в T10.

Schema4 authenticated manifest имеет required feature OD1 и immutable mode. New loader
знает v3 legacy и v4 OD1 явно; старый schema3 binary отвергает v4 до mutable repositories.
OD1 transaction journal version2: recovery не допускает применения неизвестного prepared
manifest старым writer. Старый `recover` может удалить terminal/incomplete journal
metadata до schema check: layout не должен зависеть от него после committed-primary;
новый recovery восстанавливает shadow из vault. Это требует crash tests, а не обещания
«old binary совсем не делает filesystem calls». Lock/безопасный terminal cleanup не
равнозначны разрешению менять native/trust/history/outbox.

Root `ratchet/account.pickle` имеет постоянный non-legacy OD1 fence, format выбирается
из загруженного mode, не best-effort чтения. Legacy scaffolding directories и dummy
32-byte compatibility pickle-secret существуют с initialization; это не используемые
domain secrets. Старый standalone ratchet loader отказывает на account format до
создания session/prekeys. Реальные domain accounts/prekeys/pickles живут только в
OD1 namespace и никогда не копируются в legacy root account.
Fence bytes — ASCII `kilogram:ordered-dialog:account:v1\n`; dummy secret — 32 zero
bytes, не ключ domain. Claim связывает CSPRNG nonce32, schema4, expected AccountId и
созданный DeviceId; собственный interrupted claim разрешено resume только с теми же
входами/сохранёнными keys. После ready claim immutable; чужое содержимое не усыновляется.

Runtime держит version-independent StateDirectoryLock **всю жизнь**, не только на commit.
IPC requests входят в owner actor и не reacquire OS lock. Offline commands берут тот же
lease; при активном runtime идут через authenticated IPC либо Busy. Fresh-only creation
исключает уже загруженный legacy writer; попытка включения OD1 в его существующий root
или при живом one-shot owner отвергается. Никакой автоматической quiesce/migration.
После stop/crash старые public entrypoints дополнительно останавливает schema/journal/
account fence; это проверяется исполняемым old-binary test, не только чтением loader.

### 12.1. Private state до session и после selection — C1

Единственный namespace private state — `ratchet/od1-contexts/<C-lowerhex>/native`:
он не меняется при появлении D/E/SID. Authoritative `ContextHead` лежит в vault row
`runtime/od1/contexts/<C-lowerhex>/head`. Оба пути относятся к существующим Ratchet/
Runtime kinds и меняются registered transaction receipts. Shadow — производная
копия sealed bytes, не второй источник выбора phase или способ fallback.

```text
ContextHead = [1,C,local_cert_ref,role,context_generation,private_phase,native_generation,
               offer_id?,grant_id?,D?,E?,SID?,native_blob_hash?]
NativeBlob = [1,private_phase,native_generation,sealed_payload]
SealedPayload = [encapsulated_key32,ciphertext_bstr]
AAD = enc([1,profile_claim_nonce32,C,local_cert_ref,role,private_phase,native_generation,
           offer_id?,grant_id?,D?,E?,SID?])

OfferedA payload = [1,1,pickle_secret32,account_pickle_raw]
GrantedB payload = [1,2,pickle_secret32,account_pickle_raw,reservation]
Selected payload = [1,3,pickle_secret32,account_pickle_raw,session_pickle_raw,guard_raw,
                    consumed_otk?]
Reservation = [offer_id,grant_id,one_time_key_B,0]   // reserved
ConsumedOtk = [offer_id,grant_id,one_time_key_B,1]   // consumed tombstone, only B
```

`private_phase`: 0=Absent,1=OfferedA,2=GrantedB,3=Selected; это отдельный enum от
wire `local_phase`: Ready/Active не являются новым private format. `native_blob_hash`
=`H(native-state-tag || enc(NativeBlob))`. Для phase1/2/3 HPKE info — соответственно
private-offered-a/private-granted-b/private-selected tag; используется существующий
Device-local `EncryptionPublicKey::seal`/`DeviceEncryptionIdentity::open`, затем vault
encryption. SealedPayload кодирует public fields существующего SealedMessage, не
новый crypto primitive; ciphertext включает AEAD tag. В phases1–3 обязательны
NativeBlob и непустой native_blob_hash, в Absent их нет. AAD строится
из authenticated Head и immutable root claim, не из untrusted blob header. Unknown
phase/tag/version отказывает; отсутствующий ID кодируется `[]`, не zero placeholder.

| Phase | Head bindings, payload и authoritative local action |
|---|---|
| Absent, любая роль | native_generation=0, все optional IDs/hash пусты, NativeBlob отсутствует; ContextAuthorized ещё не создаёт Olm Account. |
| OfferedA, только A | OfferId присутствует, GrantId/D/E/SID отсутствуют; native_generation=1. Fresh Account и fresh C-local pickle secret создаются вместе с signed Offer, Head и blob одной transaction **до** публикации Offer. |
| GrantedB, только B | OfferId/GrantId/D/E присутствуют, SID отсутствует; native_generation=1. E/D уже вычислены из full signed Grant. Account, единственный reserved OTK, exact Grant/InitialPair, Head и blob коммитятся вместе **до** публикации Grant. |
| Selected A | OfferedA→Selected, native_generation=2: exact Grant проверен; trial outbound session + A0/guard/source outbox, прежний Account/secret и Head/blob меняются одной transaction. D/E/SID теперь все присутствуют, consumed_otk пуст. Нет отдельного durable промежутка A без Account между Grant и A0. |
| Selected B | GrantedB→Selected, native_generation=2: public tuple/MAC/Inner A0 проверены trial; Account после consumption + Session + B0/guard, consumed tombstone, A0/B0 ledger, own Ready и Head/blob одной transaction. После commit OTK нельзя резервировать повторно. |

Последующие encrypt/decrypt меняют Selected blob и native_generation на +1; Head
меняется в той же transaction. Любая context mutation увеличивает context_generation;
Ready/ACK/queue без native changes оставляют native_generation/blob hash прежними.
Generation не обнуляется и не wrap: overflow — UpgradeRequired. Все public control refs
ссылаются на exact durable signed bytes той же transaction либо более раннего commit.

`pickle_secret32` создаётся один раз на local C и хранится **внутри** sealed payload.
`account_pickle_raw/session_pickle_raw` — UTF-8 bytes результатов публичного native
`pickle().encrypt(&pickle_secret32)`; decode — публичный `from_encrypted/from_pickle`.
Pickles остаются opaque: private JSON fields/Debug не используются для selection/
frontiers. Guard — собственный versioned state §7. Legacy dummy secret не используется.

Reader начинает с current vault Head: проверяет C/path, local cert/root claim/role,
phase availability matrix, public Offer/Grant signatures и вычисленные E/D; затем
hash/blob phase/generation, HPKE info/AAD и payload version. После opaque native load
проверяет public Account curve с Offer/Grant, SessionKeys/SID с выбранным transcript,
reservation/consumed status и guard. Swapped C/role/phase или stale blob не пробуются
как другая session. Missing/corrupt local Head/blob — OperationalFailure с rollback,
без генерации replacement Account. Проверка stale относится к current authenticated
Head; новой защиты от согласованного отката всего vault здесь не объявляется.

На cutover commit-point определяет **единственный** current Head/blob: до DB-primary
commit recovery оставляет OfferedA/GrantedB, после — Selected. Даже если shadow старый,
читатель не выбирает его account; recovery обновляет shadow из current vault. Оба
phase blobs/old-new journal copies encrypted; опубликованный Account не пересоздаётся.
Cap128 KiB включает весь NativeBlob с secret, sealing/codec overhead; Head, pre-session
controls и transient old/new copies входят в те же control/transaction/staging budgets
§11, без отдельной неучтённой provisional quota. Pending/index rows отделены от native
record, history map не переписывается одним checkpoint. Event/projection/runtime
records получают typed OD1 paths/codec и те же transaction receipts.

### 12.2. Readers и сериализация

Не reinterpret OD1 EventId как legacy signed event. Новые store/read adapters используют
typed OrderedEventKey, LocalTextProjection variant с C/D/E/event binding и format-tagged
history DTO. Existing history API получает branch по mandatory profile mode, поддерживает
pagination и точные IDs; CLI/history/IPC читают тот же repository. Legacy profile/history
и прежний single-chain fixture остаются в своих readers, без migration/переподписи.

Send и receive сериализуются одним owner executor, по context не более одного trial.
Network await держит immutable bytes/IDs/request state, **не** mutable ratchet workspace.
После await actor вновь проверяет mode/epoch/credentials и current domain generation;
устаревший plan пересчитывается. ACK reconcile по stable IDs можно rebase на текущее
state; нельзя записать старый pickle поверх interleaved receive/send/другого context.
Два contexts имеют независимые accounts/cursors/credit; общий root mutex короткий,
fair, без ожидания gap/network. Общие disk/IO/auth failures описываются отдельно.

## 13. Entry points и минимальный implementation diff

Новая логика — обозримые `ordered_dialog/{codec,handshake,ratchet,ingress,outbox,repair}`
modules; не новая копия monolithic main.rs и не refactor recovery/всего store.

| Существующий участок | Изменение для OD1 |
|---|---|
| `async_main`, `run_command`, `Command::state_directory`, `runtime_options_from_profile`, `runtime` | Mandatory mode/lease dispatch после разрешения root/recovery, **до** generic preparation, включая RuntimeFromProfile. |
| `prepare_runtime_listener`, `install_own_authority_primary`, `install_enrollment_primary`, `install_membership_primary` | Initial OD1 enrollment/consent и bounded authority updates через mode-aware transaction; legacy pool preparation не вызывается для OD1. |
| `queue_runtime_message_with_id`, `handle_runtime_ipc_work`, `run_runtime_outbound_cycle_inner`, materialized/delivered/retry records | Idempotent Queue→Materialized atomic send и stable outbox; новый actor lease вместо nested root locks. |
| `send_runtime_delivery_with_failover/to_candidate`, `persist_runtime_delivery/retry` | OD1 ALPN/admission/direct only, immutable dispatch/reconcile after await; no legacy/relay/mailbox fallback. |
| `handle_runtime_application_connection`, `handle_authorized_application_connection`, `handle_delivery_request` | Отдельный OD1 handler, credential admission и единый ingress; generic handler не получает OD1 domain handle. |
| `sync`, `handle_sync_request`, `attempt_runtime_contact_sync`, `DecryptingSessionStore` | OD1 mode отправляется в prefix session adapter; legacy/canonical materialization не импортирует OD1 carriers. |
| `connect`, `listen`, `encrypt_ratchet_fanout`, ordinary `RatchetState::load_or_create/decrypt/encrypt` | Explicit UnsupportedProfile/Path для OD1 до native/event writes; не частичный bridge. |
| `commit_runtime_mailbox_text/acknowledgement/payload`, uploads/dispatch/reverse ACK, volunteer/mailbox configuration | OD1 отказ до ledger/event/native/delete/receipt mutation. Legacy mailbox на других profiles остаётся. |
| `materialize_canonical_sync_v2_legacy_carriers`, history rewrap/import/recovery/link/reenrollment/device replacement | OD1 writer path запрещён до изменения domain; dormant mechanisms не подключаются. |
| State-vault migrate/key import/restore, enrollment/authority CLI и scheduler workers | Mode-aware allowlist до writers; unsupported operations отказывают. Old key import уже verify database до install key; restore только в новый root. Explicit malicious local file overwrite не является защитной границей. |
| `collect_runtime_history_page`, history CLI/runtime IPC | Typed OD1 reader/projection branch; exact content/IDs after reopen, existing ordinary read smoke сохраняется. |
| `kilogram-state` transaction/vault, `kilogram-store` | v4/profile gate, v2 journal, scoped mutable/append receipts, exact byte reservation/index plans и typed OD1 repositories; не A3 redesign. |
| `kilogram-ratchet` | Новый byte-oriented pure trial adapter поверх pinned vodozemac. Старый string/device-pair wrapper не умеет binary Inner и не переиспользуется. I1 classifier/legacy unknown-tail fence сохраняются. |
| `kilogram-protocol`, `kilogram-session`, `kilogram-transport-iroh` | Новые schema/golden encoders, handshake/prefix session state, ALPN/exporter/byte-capped IO; legacy public protocol не активируется заново. |

Сначала совместимые loader/mode/journal primitives, затем codec/handshake, затем atomic
send/ingress/ACK/prefix, затем runtime wiring и процессный acceptance — части одного
implementation change-set с review, а не новые самостоятельные общие design milestones.
Никакого whole-vault verify/directory scan на каждое сообщение: startup verifies once,
transaction plan использует authenticated manifest index/credit ledger и batch≤8.
После IO/recovery cache invalidated; это не разрешение обходить validation ради скорости.

## 14. Реальный запуск и конечные проверки реализации

Предлагаемый CLI интерфейс — контракт будущей реализации, команд сейчас ещё нет:

1. Existing account-create + mode-aware device-enroll в два новых root A/B.
2. `ordered-context-create/import` под explicit owner consent создают C/membership
   и peer pins; display labels отдельно. Для второго context другая random C.
3. Existing runtime-profile-create/runtime-from-profile с explicit OD1 mode поднимают
   **два процесса**; они публикуют реальные Device-signed OD1 descriptors после bind.
   Contact import может использовать текущие CLI/operator files. Descriptor содержит
   C, expected peer/device, endpoint ID/direct addresses и expiry≤1 h, обновляется после
   restart, не является prekey grant. Новый endpoint reauthorizes тот же epoch.
   Runtime profile хранит explicit `enabled_contexts` (CLI repeatable
   `--ordered-context <C-hex>`, ≤2); maintain §5.1 создаётся из него при startup.
   Proposed `ordered-locator-export --refresh`/`ordered-locator-import` проходят
   authenticated runtime IPC при живом owner, либо offline mode-aware lock; прямой
   импорт поверх lifetime lease не разрешён. Это operator file path для descriptors.
4. IPC `ordered-open`, QueueMessage, SyncContext, HistoryPage, Status, Shutdown идут
   в реальные actors. Смена locator после restart выполняется exact signed descriptor
   import/refresh, не подстановкой auth/session fixtures. Stable executable paths,
   content SHA-256, process PID, handshake record hashes и actual endpoint IDs в evidence.
   `ordered-open` — explicit запрос текущего handshake/status, не единственный trigger
   connection; B-only trace запрещает его и QueueMessage на A.

| Gate | Обязательное наблюдение |
|---|---|
| T1 handshake/duplex | Настоящие Iroh auth/exporter/OTK/Init/Confirm/Ready, один SID; ≥256 **Text** total (по128 каждой стороны), точные request IDs/EventIds/plaintexts после reopen через history API. Handshake не входит в256. |
| T2 DH | По каждой receiving lane g достигает≥8 после genesis, native keys действительно менялись, index resets0; минимум восемь **committed** receiving transitions/сторону. SID/E не менялись. |
| T3 gap + late old chain | После A0/B0 A1(L0) доходит до B; B1(S0) задержан. A2..A65 ещё L-chain; A2 задержан, A3..A65 доставлены. B1 приходит A, A66(M0) доставляется B раньше A2. Pending64 заполнен; M0 не decrypt до A2..A65. Head-only prefix repair приносит A2, drain восстанавливает tail и M0. Gap>40, ordinal и encryption order не равны arrival/hash order. |
| T4 process restart | Stop/relaunch обоих actors после Offer без D/E/SID, Grant без SID, до/после Selected cutover, Confirm/Ready, Queue, materialized send, pending admission, readable commit и ACK. Account/OTK и committed refs/bytes те же; один логический результат. |
| T5 precise crashes | Child-process kill после materialization commit до dispatch, после pending commit, trial до commit, readable DB-primary commit до network ACK; отдельно существующие fault seams native/account/trust IO. Ясно различать kill, injected error и graceful reopen; не заявлять все power-loss windows. |
| T6 duplicates/partial | Lost ACK, duplicate direct/sync/range/restart: те же ciphertext/id/receipt; ordinal fork/other EventId и missing Auth record не дают completion/ACK. C3 full64 partial/lost-Complete trace §10.1: source не знает remote subset; auto drain/head repair после reopen без subset Complete. |
| T7 hostile metadata | Valid Device signature + forged index/g/closure/cause, cross-context ciphertext, inner/outer mismatch, MAC failure; invalid trial не меняет native/readable. I1 real IO rolls back prefix, I2 format fence сохранён. |
| T8 independent contexts | A gap/invalid head в C1, duplex в C2 через свои native accounts продолжает progress; ciphertext/keys не общие. Interleaved author_sequence не используется как lane ordinal. |
| T9 budgets | Encoded maximal text/credentials/OfferedA/GrantedB/Selected state/frame/old-new transaction plans; reduced test windows с full futures, head reserve, overflow и restart. Logical prepaid capacity и отдельно physical/full-history StorageBlocked. I3/S3 counterexamples не скрываются за final-history fit. |
| T10 negotiation/fences | Unsupported/mixed ALPN/limits, revoked/expired authority/consent, conflicting/duplicate handshake; C1 swapped/stale phase blobs; C2 B-only send при idle/relaunch A, lease/locator expiry/import без IPC open/send у A. Old binary до/после mode creation, live old owner, prepared-v2/committed-primary crash; unsupported paths не меняют protected state. I4 RuntimeFromProfile проверяется. |
| T11 ordinary compatibility | Existing ordinary runtime IPC smoke, legacy history и принятые I1–I4 regressions. Не повторять старый 18-minute acceptance при каждой мелкой правке; новые финальные tests на замороженном diff. |
| T12 time/resource evidence | Отдельно setup/auth, queue/materialize, direct, repair, reopen/history, count/size vault commits, p50/p95/total и причина bottleneck. Не считать прежние1095s приемлемостью живой переписки. |

Scheduler может детерминированно задерживать **настоящие serialized network records**
после durable send, разрывать connections и инициировать настоящий PrefixRequest.
Он не создаёт accounts/session/credentials/handshake/decoded bodies вместо actors,
не вызывает receiver put напрямую и не заменяет TLS exporter строкой.
Runtime statuses: NetworkPaused, ConnectionBackoff, LocatorRefreshRequired,
Negotiating, Active, WaitingPrefix(n), PendingStored,
ReadableCommitted, WaitingAck, Delivered, AuthBlocked, StorageBlocked,
UpgradeRequired, InvalidPrefix, OperationalFailure. Pending не отображается Delivered.

Targeted unit/component tests/golden vectors идут перед process acceptance, затем один
финальный frozen-diff run и applicable fmt/locked-offline Clippy/readiness gates.
Windows harnesses только stable runner, builds jobs=2/BelowNormal, no direct cargo test
execution. Local loopback restriction sandbox при необходимости снимается для реального
IPC/network test с объяснением, а не подменой protocol fixture. A2 до внешнего запуска.

## 15. Принятые решения review и оставшиеся gates

Механизмы выше — одно конкретное предложение. Не оставлены выбором реализации роли,
тип binding, подсчёт ordinal, DH parent/closure, full/partial completion, outcome ошибки,
Root ownership или retry semantics. Значения ALPN/schema/caps тоже предложены явно.

В review iteration1 приняты оба содержательных решения §15:

1. «Без повторной encryption» означает **один committed, опубликованный результат**
   на logical request. Literal запрет повторного volatile
   CPU-вызова после crash до persistence не доказуем данным atomic-commit механизмом;
   он потребовал бы иного durable crypto execution/RNG interface. Transport retry после
   commit всегда byte-exact. Это граница, которую нельзя скрыть формулировкой теста.
2. Принят консервативный L1: 512/lane,128 MiB reservation/context,2 contexts/profile,
   fresh-only schema4 и отсутствие epoch replacement. Более широкие quotas/activation
   требуют совместного пересчёта, а не ослабления head/full-commit invariants.
   512 включает genesis: максимум511 Text/lane. Этот предел относится к opt-in этапу,
   не означает пригодный daily-MVP lifetime limit и не закрывает A3.

Неопределённость native/record overhead должна быть снята encode-bound tests до
активации implementation. Пока это требуемые bounds, не выполненные benchmarks.
Любое противоречие, мешающее T1–T12, возвращается в review как конкретный blocker,
а не обходится migration/reset/новым общим feasibility этапом.
Содержательных развилок, которые должен тайно выбрать implementer, не оставлено.
Повторный review C1–C3 завершён; actual serialization/staging и остальные T1–T12
остаются обязательными проверками реализации, не результатами этого документа.

## 16. Проверки этой части

Прочитаны задача/AGENTS, приёмка и предыдущие R/S/I уточнения, RFC-0193, конкретные
session/ratchet/transport/store/vault/runtime entrypoints и pinned dependency APIs.
Реальный TLS exporter и доступность SessionKeys/SID подтверждены чтением local source.
Адресные document checks iteration2: 17 разделов, 17 wire kinds, 15 local links
существуют, code fences сбалансированы, trailing whitespace/лишней пустой строки EOF нет.
Арифметика:128 MiB/context;4160 KiB/lane;334.25 MiB conservative profile bound;
15 worst-case wire items;97 KiB/frame до sealing/codec overhead,31 KiB margin. Это
проверка формул, не runtime/encode evidence: фактические layouts обязаны пройти T9.
`git diff HEAD -- apps crates scripts Cargo.toml Cargo.lock` пуст, индекс пуст;
HEAD=`1fce03ee2d2886f871e0ce44a641e3eeba9cdd0f`. Rust tests/builds/spikes не запускались.
Чужой `ai-docs/development-environment.md` сохранил исходный SHA-256
`330ad3c4d1941b213e7e5bef958416a6847d3defb8e368f79d6d21c944efe798`. После сдачи
контракта — содержательный callback и остановка до review; commit/push только после
приёмки контракта и отдельного поручения оркестратора.

## 17. Закрытие C1–C3 и обязательные traces iteration2

Это описание проверок **будущей реализации**, не новые результаты runtime tests.

| Review | Исправление | Проверяемый результат |
|---|---|---|
| C1 P1 | §12.1: стабильные C paths, phase-specific HPKE info/AAD и current ContextHead; OfferedA/GrantedB→Selected атомарны | Public Account/OTK/committed records переживают restart без несуществующих IDs; чужой/stale blob отказывает, shadow не выбирает account |
| C2 P1 | §5.1: startup enabled-context intent у A, bounded maintain/idle probe/reauth; explicit locator import boundary | B-only Text достигает Delivered после разрыва/relaunch idle A без своего open/send; expiry даёт явный Blocked, import возвращает progress |
| C3 P2 | §10.1: local possession, receiver-driven prefix + exact ACK query; RepairWait и auto drain | Unknown accepted subset не нужен source; full64 head repair продвигается без subset Complete/ложного ACK |

**C1 restart/cutover.** A публикует Offer после commit OfferedA/native_generation1,
погибает до Grant; reopen читает C-based Head/blob с D/E/SID=`[]`, тот же Account key
и Offer bytes. B публикует Grant после GrantedB/generation1, погибает до Init; reopen
читает тот же Account/OTK reservation, E/D присутствуют, SID=`[]`, возвращает exact Grant.
A→Selected kill до primary commit оставляет OfferedA; после primary commit — Selected
с exact A0/SID, даже при старом shadow. B cutover аналогично: до commit OTK reserved/
нет B0, после — consumed+exact B0/Session/Ready. Duplicate A0 возвращает B0, не создаёт
inbound session повторно. Подстановка OfferedA в Selected, blob из C2/другой роли или
старого generation не проходит current Head/hash/AAD; protected state/ACK не меняются,
операция OperationalFailure. Все local IO fault seams также rollback, не quarantine.

**C2 B-only/relaunch.** C enabled у обеих сторон, valid consent/pins/locators, existing
selected epoch; очередь/outbox A пусты. Разрывается connection, на B ставится единственный
QueueMessage. B не dial. Maintain A обнаруживает close/idle timeout и независимо от
своего queue делает auth+phase/Ready/Resume repair; B materializes/dispatches Text,
A readable-commits/ACK, B→Delivered. Повтор: A завершается, B queue сохраняется; A
relaunch сам читает enabled C. Если endpoint поменялся, новый signed A descriptor
operator импортирует **на B**; это prerequisite, не IPC open/send у A. С момента valid
pins/locators retry A продолжается автоматически; E/SID/Account прежние, Text один.
Happy-path test с ordered-open на A не заменяет этот trace.

**C2 lease/expiry.** При auth lease300 s, действующих ещё>300 s descriptors и consent
A в t=240 s закрывает connection и один раз запускает fresh Hello/Proof/Admitted;
у B нет auth initiator. Nonces/B/T новые, selected InitialPair/E/SID прежние. До нового
Admitted Text остаётся Queued/materialized immutable outbox, старый generation не
может commit. Затем deadline locator наступает раньше следующего240 s timer: обе
стороны закрывают expired lease, local status LocatorRefreshRequired/AuthBlocked,
Queue/pending/history и ciphertext сохраняются, retries не выдают Deliver/ACK. Само
истечение не публикует новый locator и не меняет keys. Explicit export/import current
signed higher revision (при relaunch — actual endpoint) будит A и восстанавливает
admission/repair без open/send на A; без import trace заканчивается честным Blocked.
Consent expiry/revocation аналогично блокирует writers; locator import не обновляет
consent/authority и не снимает revocation. Timer/expiry targeted tests могут управлять
clock; evidence обязано отличать их от реального process restart/network acceptance.

**C3 partial/lost-Complete.** Нормативный полный trace §10.1 использует next=2,
47+17=64 ordinary pending, unknown subset A50..A66, source head82, PrefixRequest [2,3)
через64 KiB head reserve, auto drain до67, затем [67,82) и exact receipts. Проверить
source без сохранённого knowledge о17 accepted IDs, kill/reopen receiver, duplicates
и отсутствие старого subset Complete. Ни current Cursor(2,A1.id), ни credit0 сами
по себе не описывают possession set; они только запускают описанный receiver repair.
