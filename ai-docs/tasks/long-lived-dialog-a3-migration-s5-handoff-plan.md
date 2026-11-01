# S5.3: existing-dialog handoff — source and verification mapping

2026-10-26. Mapping for root review, not implementation/execution authorization.
Actual published baseline: **`64c2359788ead360314793e47741adacb83b8d4f`**,
sole parent `9016b578bed8b8b5e43dd0e2e94edc376804e4f0`.
[Activation](long-lived-dialog-a3-migration-s5-handoff-activation.md) supersedes
only the old base/sequencing of the unchanged
[queued assignment](long-lived-dialog-a3-migration-s5-handoff-task.md).
Executor: the same GPT-6.1 Sol Max, no Fast.

Normative decisions remain the accepted [S5 contract](long-lived-dialog-a3-migration-s5-contract.md),
SHA256 `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`,
especially sections3,4.1,6–8,10–12; [A3 section3](long-lived-dialog-a3-contract.md#L180),
the accepted S3 proof/mixed codecs and S4 fresh/route infrastructure. The
[S5.2 acceptance](long-lived-dialog-a3-migration-s5-l1-implementation-acceptance.md),
[revision2](long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md) and completed
R-A/R-B publications provide the implementation base. No protocol replacement or
additional preparatory refactor is proposed. No source/contract contradiction was
found: the L1/fresh-only fences below are the deliberate limits of earlier cuts.

## 1. Actual seams and ownership

Paths/line numbers below refer to64c2359. New names are proposed private modules
or closed typed operations, not existing APIs.

| Existing source | Reuse and exact S5.3 extension |
|---|---|
| [Owner7/Lease1](../../apps/kilogram-cli/src/ordered_dialog/runtime7/owner.rs#L16), [actor](../../apps/kilogram-cli/src/ordered_dialog/runtime7/runtime.rs#L41) | Retain one owner, lock, mailbox16, native cache and recovery ownership. Add a lease family and Native1/Mixed7 state variant inside this owner; do not start Owner2 or the V4 Engine as another writer. |
| [typed L1 operations](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/types.rs#L134), [commit](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1.rs#L330), [core dispatch](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L478) | Retain the same Root7/core. Add `migration7/existing.rs` with `ExistingOperation`, snapshots and restricted lowering; add a distinct core input. Existing L1 purposes stay stage-restricted. No arbitrary changes/native/ledger API. The current façade may retain its name for compatibility; no repository-wide rename is needed. |
| [L1-only Head reader](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/ops.rs#L107), [Root7 load/check](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs#L624) | Add closed LocalMode7/control/native correspondence to Root7 load and every commit. Preserve old L1 shapes and migration-before-Ready closure. Add authenticated intent/control-outcome records rather than defaulting absent Q to a fresh state. |
| [budget put/check](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs#L502) | Implement the contract's Released variant at SelectA/CommitB. Current code derives Held/L for all occupied L1 contexts. Preserve FundedOnce, original backing ownership and independent C2 summaries; no new repair endowment. |
| [current policy/invalidation](../../apps/kilogram-cli/src/ordered_dialog/runtime7/owner.rs#L237), [commit/recovery](../../apps/kilogram-cli/src/ordered_dialog/runtime7/owner.rs#L579) | Add current-existing Consent2 gates and historical selected-proof checks as separate operations. Extend lease/cache invalidation to both ALPN families and all contexts referencing an advanced account pin. Retain failed-authority persistence blocking. |
| [bootstrap](../../apps/kilogram-cli/src/ordered_dialog/runtime7/bootstrap.rs), [Ready1 predicate](../../apps/kilogram-cli/src/ordered_dialog/ready1.rs), [deterministic fold](../../apps/kilogram-cli/src/ordered_dialog/storage_migration/fold.rs#L58) | Reuse real Ready1/genesis/Offer1/Grant1 validation. Feed the fold from current authenticated indexed readable rows and current Guard1/Seen, not the original V4 verifier or a historical compact DTO. Expose only the minimum internal fold access needed by `runtime7/existing.rs`. |
| [S3 existing proof](../../crates/kilogram-protocol/src/ordered_dialog_v2/mode.rs#L291), [mixed event](../../crates/kilogram-protocol/src/ordered_dialog_v2/frame.rs#L469), [mixed receipt](../../crates/kilogram-protocol/src/ordered_dialog_v2/authority.rs#L482) | Reuse exact wire/control/ModeProof2/Committed/TaggedItem verification after local selection. Add a closed verified existing-scope factory and pre-Q Receipt1 verifier; do not manufacture StaticSelection2. |
| [Owner7 network](../../apps/kilogram-cli/src/ordered_dialog/runtime7/network.rs#L239), [protocol](../../apps/kilogram-cli/src/ordered_dialog/runtime7/protocol.rs#L14) | Keep the ALPN1 branch; add `runtime7/existing_network.rs` and existing wire2 dispatch. Reuse S4 exporter/Hello2/Proof2/route mechanisms and limits, with an explicit existing admission path before Q. |
| [fresh policy](../../apps/kilogram-cli/src/ordered_dialog/runtime2/policy.rs#L17), [fresh selected Head](../../apps/kilogram-cli/src/ordered_dialog/runtime2/network.rs#L307), [Native2](../../apps/kilogram-cli/src/ordered_dialog/runtime2/native.rs#L47) | Keep fresh-only guards and Root6/Native2 bytes/AAD. Existing Selected/Prepared uses its own branch; do not relax `check_selected` or reinterpret fresh bootstrap as existing. Address shared infrastructure changes with targeted fresh regressions. |
| [L1 data](../../apps/kilogram-cli/src/ordered_dialog/runtime7/data.rs#L101), [history](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/history.rs#L33), [fresh H2 reader](../../crates/kilogram-state/src/vault/indexed_dialog/history_source_v2.rs#L384) | Add Root7 format-specific source/pending/readable/ACK lowering and history candidates for format2. Reuse HVI1 identity/cursor/causal selection and structural Event2 fields; do not install the fresh H2 binding over imported history. |
| [CLI](../../apps/kilogram-cli/src/ordered2_cli.rs#L123), [IPC3](../../apps/kilogram-cli/src/ordered_dialog/runtime7/runtime.rs#L164), [Profile3](../../apps/kilogram-cli/src/ordered_dialog/runtime7/config.rs#L24) | Implement explicit IPC3 tags7–10 and locator family2. Existing `Consent2Import` currently emits IPC2/tag9 and `Upgrade` is unsupported: add explicit existing-mode CLI selection, not codec fallback. Extend public profile route fields using the accepted S4 route shape. |

This joins the required lifecycle branches. It does not unify all L1/fresh owner,
history or admission implementations. R-A already removed the proven Ready1
predicate duplication; R-B supplies result lifetime. Further general cleanup is
outside this cut.

## 2. Public permission, intent and IPC

Add a bounded public existing-origin export from the live owner: original C,
cert roles, membership/credentials, selected Offer1/Grant1/initial pair, readable
genesis, signed Ready1 and real D/E/SID. It grants no current permission. Missing
session/Ready1 returns NeedHandshake and continues genuine ALPN1; no zero IDs,
synthetic Ready2, new context or session is created. After committed readiness,
the owner publishes an immutable public evidence container at its configured
outside-state `locator_directory/<C-hex>/existing-origin.cbor`, using existing
owned-publication checks. Republish exact bytes after restart; an IO failure is
a publication failure, not rollback of Ready. The container is canonical
`[1,spec1Raw,offer1Raw,grant1Raw,initialPair1Raw,[genesisA,genesisB],[readyA,readyB]]`,
cap512KiB with each nested existing cap. It contains only public signed evidence,
not Native/secrets. `ordered existing-origin-export --profile ... --context ...
--output-file ...` copies/verifies that public artifact without opening state;
the offline signer derives and validates C/D/E/SID/roles from its signed contents.
This adds no generic file-read IPC tag or unbounded status payload.

Extend the explicit `consent2-create --origin existing --existing-origin-file`
offline ceremony with `--step propose|cosign`; cosign also takes the bounded
public `--proposal-file`, each step its own `--account-dir` and exact expected
account/context/current and minimum-authority witnesses. Reuse the Root-signing machinery of
[context_artifact](../../apps/kilogram-cli/src/ordered_dialog/runtime2/admin.rs#L145)
and the separate-machine propose/cosign checks of
[Consent1 administration](../../apps/kilogram-cli/src/ordered_dialog/consent1_admin.rs#L136).
Support each Root signing on its own machine; require exact expected accounts,
original certs/C/D/E/SID, current signed authorities/membership/capability, minimum
revision witnesses and interval at both steps. Output remains the existing
4096-byte-capped Consent2 envelope with both Root signatures. Root secrets never
enter the live owner. Fresh `context2-create`/Consent2 behavior remains explicit.

The actor verifies imported current Consent2's origin and roles against that real
scope, CASes expected absence/reference, closes affected leases and persists the
policy generation/high-water. Historical consent import is archival only: it
cannot replace current permission or selected P consent. Expired historical
selected consent is checked for its original signatures/scope, while every live
operation needs valid current permission. Renewal does not rewrite P/proof/Q/cuts.
An own-account pin advance affects all referencing C, not just an enabled slot;
a peer pin affects every C containing that account. Independently authorized C2
with a different affected-account set remains usable.

Use the accepted Value algebra and32KiB request/48KiB response envelopes. Options
are `[]|[value]`, strings are UTF8 byte strings, booleans are closed unsigned0/1.
No text/map/null extension; reject wrong version/extra fields before mutation.

| Total arity | IPC3 request / effect |
|---|---|
|6|`[3,7,slot,optExpectedRef32,rawConsent2Bytes,historical0or1]`; raw<=4096; None means actual absence, not wildcard. Historical/current are distinct closed purposes.|
|4|`[3,8,slot,request16]`; persist/retry explicit UpgradeIntent, wake coordinator A. A B-side request never makes B coordinator.|
|3|`[3,9,slot]`; bounded public stage/control outcome/current-policy/cuts/Q/freeze/debt status.|
|4|`[3,10,slot,request16]`; cancel matching intent only while LocalMode=L1 before durable P/Prepared. Preserve an exact request outcome; no cancellation after that boundary.|
|6|Existing tag5 with family2: `[3,5,slot,2,action,optLocatorBytes]`; action0 requires raw, action1 requires absent; exact existing RuntimeLocator2 codec/cap8192, current signature/scope/endpoint/route/revision checks. Family1 remains separately encoded.|

Retain tags1–6,11,35 and migration-admin-only32–34. Add CLI `upgrade`,
`upgrade-status`, `cancel-intent` using explicit IPC3 endpoint/slot/request;
`consent2-import` requires explicit existing/IPC3 selection. IPC2 is unchanged;
failed IPC3 decoding does not retry IPC2. Status derives named local/wire stages,
native encoding, send/receive eligibility and policy errors from committed state;
it never returns placeholder success or secret/private-state bytes. The export
and ceremony additions above are local operator surfaces; they do not alter the
accepted wire2/control/Consent2 layouts or current-permission rules.

## 3. Closed durable write sets

Add `ExistingOperation` purposes for ConsentCurrent/ConsentArchive,
Intent/CancelIntent, Propose, Prepare, SelectA, CommitB, ActivateA, ObserveLegacyHead,
MixedPending/Readable/Acknowledge and control-response progress. Their private
lowering derives keys/credits/digests and verifies occupied C, Ready storage,
role/stage, expected root/context/policy generation, before-head hash and exact
control/request references. No caller totals or set-mode/set-Q/set-native/set-cut.
Format-specific queue/old data operations keep their existing paid correspondence.

| Purpose | One durable atomic write set; publication follows commit |
|---|---|
|Intent/CancelIntent|Per-C request/body/current-consent reference and intent outcome; mode stays L1/no cuts/Q. Cancel removes only the matching pre-boundary intent. Text and upgrade request namespaces are distinct.|
|Propose A|Verified complete old scope, current existing Consent2/fresh ALPN2 lease; capture actual own.next/last as cutA/prevA, exact selected consent and signed P, Offering/control debt/freeze. Invalidate the former ALPN1 token before the next dispatch.|
|Prepare B|Validate exact P/selected consent/role/observed bounds; capture B's actual own.next/last as cutB/prevB; persist P+A bytes, Prepared/control debt/freeze and ALPN1 closure. B may materialize L1 only before this commit.|
|SelectA|Validate signed A, derive exact certificate/Q/K; validate current cuts and fold current Guard1+Seen; write Mixed7/Guard2/seen mapping, current pointers, selected proof, K/control debt, mixed history binding and one-time escrow release; Selected is receive-only.|
|CommitB|Validate exact K/certificate/Q, convert B's current Native encoding and reconcile credits atomically; persist Active2 and immutable signed Committed/response debt before replying. B may now materialize Event2.|
|ActivateA|Verify signed saved B Committed against local proof/Q; persist activation/control outcome. Keep selection/Native encoding and counters; no reseal or generation bump merely for a duplicate control.|
|Mixed data/ACK|Exact immutable source/capture/request/pending/head/native/seen/history/receipt/debt/budget correspondence, current gate and root CAS in one commit. ACK possession/hints are not delivery.|

Every retry first reads durable request/stage/control outcomes and returns saved
bytes. It does not re-sign or invoke Native because another root generation
advanced. A clean final-policy rejection changes no head/native/debt and is
reusable; ambiguous IO poisons the owner until reopen/outcome resolution. Add
separate debug-only barriers before commit, after durable commit and before
publication for P/A/K/Committed. Never turn an after-commit publication error into
rollback, downgrade or an optimistic whole PASS.

LocalMode7→wire Mode2 is a named conversion: L1=1, A Offering=2, B Prepared=3,
A Selected=4, Active2=5. Intent is not a wire stage; fresh stage0 is invalid here.
The first irreversible P/Prepared also freezes materialization and prohibits
ALPN1 for that C. It still allows current-policy-bounded old receive/repair/ACK
and the original L1/R3 queue bounds. It does not freeze C2 or mint expanded queue
capacity. No format materializes in Offering/Prepared/Selected.

## 4. ALPN2 admission and closed pre-Q handlers

Build `VerifiedExistingScope1` only by verifying actual bound old controls,
Ready1/genesis/membership and exact original role/C/D/E/SID; private fields and no
unchecked facts constructor. New `AdmittedExisting2` binds that scope to current
Root/Consent2, fresh Locator2/endpoint/actual route, TLS exporter, signed Hello2/
Proof2, lease generation/expiry and current high-water. It need not contain Q.
Repeat the final policy/lease gate after staging. Keep global permits2 before
TLS/spawn, one admitted lease per C and the existing TLS/admission/round deadlines.
Endpoint advertises both ALPNs when eligible; the negotiated family exclusively
chooses decoder/exporter/signature domains. A P/Prepared transition closes ALPN1
even if it later pauses for policy expiry.

Implement exactly S5 section7.1.1's fresh reachable pairs:
L1/L1, Offering/L1, Offering/Prepared, Selected/Prepared, Selected/Active2,
Active2/Active2. Validate role/optional shapes and known cuts/Q before watermarks.
In particular, Selected A reconnects to Prepared B with B.Q absent. B treats
A's Q as an opaque authenticated hint until exact K derives it, then compares;
it never adopts Q from the hint. Impossible fresh pairs fail. Compatible delayed
hints on an existing lease do not roll back durable stages, cuts or watermarks.
Cached Prepared does not prohibit B's later legitimate Event2 at Selected A.

| Before local selection | Required closed branch |
|---|---|
|Head/Resume|Actual LegacyHead1 with source/receive cursor1..512 and exact previous immutable source ID; monotone peer watermark; equal-next/different-ID conflicts. Cuts remain derived from signed controls, not hints.|
|Prefix|Contiguous bounded source slice, previous/end/actual source cursor and exact-used sorted AuthRecords. Runtime<=16/wire<=32, paid head/ordinary slots and actor/frame caps before allocation. Possession cannot jump readable/ACK state.|
|Deliver/EpochCipher tagged1|Actual Event1 signature/scope/Core/header/Guard1/cut/source linkage; immutable duplicate or funded pending/head/readable commit. Tagged2 fails before pending, possession or Native.|
|AckBatch tagged1|Add `ParsedReceipt1::verify_pre_q` alongside its existing selection-based verifier ([current API](../../crates/kilogram-protocol/src/ordered_dialog_v2/authority.rs#L434)). It takes verified existing scope, actual own immutable Text Event1 and opposite RootVerifiedCredential; checks exact eight-field receipt body, bundle/cert/role and old signature. Unknown/genesis/wrong-lane/conflicting receipts cannot release debt.|
|AckQuery/Complete|Query actual source/readable ID pairs; return exact saved receipt or create one only for actual readable. Complete acknowledges possession, never Delivered or credit release.|

After local Q, use only the S3 mixed verifier with local verified proof:
format1 iff ordinal<that source role's cut, otherwise format2. No fallback after
mixed verification failure. Newly readable old packets get Receipt1 before Q,
Receipt2(event_format1,Q) after Q unless an immutable Receipt1 already exists.
A Prepared peer cannot consume Receipt2 through the pre-Q branch: retain ACK
debt and retry controls until it selects. Existing Receipt1 remains byte-identical.
Selected/mixed Head has its own format-aware bounds; a receive cursor may exceed
512 while an own legacy source remains frozen at its cut. Do not apply S4's
symmetric-selected Head check or a global L1 cursor cap to this state.

## 5. Same-Native conversion, credits and history

At SelectA/CommitB, load the current authenticated Head7/Native1 through the held
repository and validate the current Guard1/indexed Seen mirror. Read at most1024
actual readable legacy rows; pending/quarantine is excluded from the fold but
included in peer-cut lower bounds. Use the existing linear two-frontier fold and
`import_l1`, requiring exact compact guard/seen/g/index/first/last/cause/rotation
correspondence. No original V4 scan, private pickle inspection or Native replay.

An own cut is committed own.next including genesis, not queue length;1..512 and
prev is source at cut−1. A peer cut exceeds every readable/pending/quarantined/head
format1 ordinal and is at least the durable observed source_next; equal frontier
requires exact last ID. A signed future prev is checked when the prefix arrives,
never used to jump it. No new-format Native trial before the required old prefix.

Add `runtime7/native.rs` with a Native1/Mixed7 representation. Move/fork the actual
OrderedAccount/OrderedSession and original SessionKeys/SID/reservation from
[PrivateState](../../apps/kilogram-cli/src/ordered_dialog/state.rs#L166); replace
only the validated guard representation. Mixed7 uses exactly S5 section8's
`kg:od2:migrated-private\0` info and AAD including incarnation/claim/source/C/cert/
role/native phase/generation/Offer1/Grant1/D/E/SID/Q. Cap128KiB. Bump native_generation
once for actual encoding conversion, with zero account/outbound/inbound
constructors and zero Native packet encrypt/decrypt calls. Preserve session counters and keys;
duplicate controls do not convert again. Original source Head/native remains an
immutable witness. Do not reseal or relabel Root6/Native2.

SelectA/CommitB first cover existing queue/pending/head/old ACK and new control/
native obligations; release only unused LegacyHold once, saving release Q,
generation and covered snapshot in LegacyBudget7. Keep repair free/grant/outstanding
and FundedOnce; no refill on reopen, retry, activation or C2 work. Late old receipts
release only their exact debt once and preserve source attribution. Retained V4
backing remains paid/immutable and is not GC'd by handoff.

Keep the existing HVI1 view identity and all old Event/receipt/request/projection
bytes. Add explicit Root7 LegacySnapshot/FullL1/Mixed row readers and parent/
receipt format validation; do not rewrite incomplete legacy captures into full H2.
Extend the bounded two-candidate history loop for ordinals512+ while retaining
frozen cursor frontiers, causal order, limit32 and actual escaped JSON fit before
cursor advance. No Native restore per history row or C2 scan. Old offset access
stays a bounded legacy-data API, not a cursor for growing Event2 history.

Budget adjustment currently visits both occupied contexts. Extend that loop so
a C2 L1 commit preserves C1's Released/Q/snapshot rather than recreating Held/L;
check released-state correspondence on load and every commit. Key comparisons
stay inside the trusted future test; public results carry equality/fingerprints,
never raw SessionKeys or private pickle contents.

## 6. Two contexts, route and affected fresh gates

Reuse Resources7 and RefWindow2: global permits/mailbox limits and64/64 shares,
batch<=4, one owner turn per context. One blocked/Offering/Selected C cannot consume
C2's reserved refs or paid escrow. Queue/request-status/ACK/history dispatch must
inspect actual source format instead of always parsing Core1 (current IPC3 tag3).

Extend Profile3 with the exact accepted S4 `RouteConfig2` shape from
`kilogram_transport_iroh::ordered2`, preserving old direct-only profiles/defaults,
identity/pinned paths and capacity CAS. Profile publisher/expected-profile
comparison must understand these fields together; do not bypass the existing
publication check. Runtime eligibility is checked from all enabled durable modes:
while any needs L1 direct or has not completed handoff, relay policy returns
L1RequiresDirectOnly. After all relevant contexts are Active2, reuse the existing
DirectOnly/URL-only RelayOnly/Auto behavior, actual-route checks and locator
rotation after restart. No public relay default or silent ALPN1/relay fallback.

Shared changes needing focused regression evidence: core purpose-before-retry
dispatch; Root7 load/every-commit checks and budget adjustment; HVI1 parent/format
candidate selection; any extracted transport/route/exporter helper; IPC3 generic
caps and profile publication. Preserve fresh Runtime6 discriminators, fresh-only
policy/selection checks, Native2 AAD and IPC2. Use exact diff/function reuse for
unchanged S1/S2 allocator/100k, Guard2 arithmetic and wire vectors; old PASS does
not certify a changed shared branch. No bulk100k rerun without a relevant change
and a separate root decision.

## 7. Accepted exhausted dependency and new observations

Use only existing public evidence at mapping time:

- [boundary03 checkpoint-info](long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-boundary-03-checkpoint-info.json),
  SHA256 `2b359e4623b2e603977342df69f441e35e0276643ee420a751a3515e361771b3`;
- [accepted preservation](long-lived-dialog-a3-migration-s5-l1-preservation-review.md)
  and its [actual result](long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-preservation-01-result.json),
  SHA256 `bcd69f3c44940c269f38c3fc5fc463e30803d7f5c5e2887a9875305d62926ca8`;
- [S5.2 provenance/ownership](long-lived-dialog-a3-migration-s5-l1-implementation.md#L252).

The stopped source is
`E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb`;
A is `two/indexed.profile`, B is `one/indexed.profile`. The inactive backup is
`.tmp/migration-s5-preserved/F-exhausted-preservation-01`, manifest SHA256
`c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3`:
6956 files/45597399 bytes preserved under the documented lock exception.
Future authorized restore is only to the original pinned absolute paths, with
one active Native branch; no retargeted profiles or concurrent counterfactuals.
The frozen d9b original stays untouched. Mapping did not traverse/open either
private area or validate current private availability anew. Future preparation
also needs legitimate original-Root administration material/current authorities
for the offline Consent2 ceremony. Its usable paths/authority are a readiness
dependency to verify under the granted case, not permission to open the frozen
original or create replacement Roots. Missing authority is a root callback.

Public facts: C=`ef689629a47207362fbc870bf610d921c8e28ca0646471914bd93218fff43810`,
SID=`TflibOtoo5vbulwTkVuDsE6DrSVTFMHdGT2ZHDDx+oE`, cold frontiers[512,18],
debt/pending/queue0, receipts[511,17]. Thus planned real cuts are A512/B18,
previous A511/B17, first new A Event2 **n512**, B Event2 n18. Native generations
are A517/B88 before conversion; conversion should bump each once without packet
calls, then actual messaging changes counters normally.

An authorized future case first verifies preserved bytes/locks/pins, captures new
pre-handoff current observations and current permission, then runs this same
domain. These are new observations, not reconstruction of boundary03's lost
parent result. If availability/integrity/policy is insufficient, stop for root;
do not generate511 or invent/rewrite the seed. A has no queue room before Active
at own.next512: test the new request's genuine UpgradeRequired there, and queue
its first Event2 only after activation. Test a pre-existing queued request and
nonempty old debt/gap/head on a separate small genuine domain; do not manufacture
those conditions in the debt0 checkpoint.

## 8. Focused observable verification proposal

All entries below are future checks, not results. New component/field selectors
and exact expected matching names/counts must be frozen with source/images before
execution. Small genuine L1 domains use actual APIs and paid setup inside C/F.

| Group | Observable acceptance |
|---|---|
|Control/state components|Role/stage shapes, complete scope/intent/cancel boundary, exact P/A/K/Committed retry bytes, stable request outcomes, stale root/policy/head CAS and purpose-before-retry refusals; clean abort versus poisoned/reopen outcomes.|
|Native/conversion components|Both roles, current Guard1/Seen fold including legal late tail and invalid cut/watermark/quarantined bounds; same SessionKeys/SID/guard facts, one encoding bump/zero constructors+packet calls; duplicate no reseal.|
|Mixed/history/credit components|Both event/receipt formats around each role's cut; wrong-Q/format/source/receiver negatives before writes; actual old pending/head/receipt debt, exactly-once credits/escrow, shared Author ownership, cursor frozen suffix after append/reopen and real JSON cap.|
|Policy/IPC/purpose components|Account-referencing C set, current versus historical Consent2, expiry/revoke/renewal/clock/final stale lease, typed IPC arities/options/caps, old profile/fresh/purpose fences; no secret-output fields.|
|Real control-loss process case|Independent real durable-before-publication losses of P, A, K and Committed, stopping owned processes at named barriers. Fresh authenticated reconnect/reopen, exact raw replay. Include SelectedA/PreparedB without symmetric Q, then B Active with lost Committed: A receives B Event2 but cannot send until exact saved Committed activates it. Queued request materializes once.|
|Real pre-Q/policy process case|Create old gap/head/pending/nonempty ACK debt through real delivery/hold windows. Pre-Q repairs/Receipt1, selected late old readable/Receipt2, Prepared peer ACK deferral, no release on possession, new-format pre-Q refusal before pending/Native, cancellation/final-gate/renewal.|
|Actual exhausted continuation|Authorized preserved-domain reuse above; retain Account/Device/session/keys/C/D/E/SID, original history/requests/receipts and exact old cursor suffix; cut512/18, first A.n512; crash/reopen and new observations of exact bytes/Native call deltas. This is not whole P5.|
|C2/mixed-route process case|C1 transition/pause/fork while independently permitted C2 progresses; both64-ref shares; ALPN1 still works only for eligible C2, forbidden post-P C1. Complete both handoffs before existing own-relay/restart/locator route checks.|
|Fresh shared-branch process gate|Focused unchanged Root6 fresh bootstrap/text/receipt/reopen and selected route path, current final-policy rejection and old/new purpose fencing as affected. No broad S4 suite or throughput claim.|

Rejected data/control cases record unchanged root/head/native/seen/debt/block as
applicable; genuinely signed conflicting controls preserve bounded fork evidence
and block the affected C. Loss/final-gate hooks are debug-only external scheduler
barriers, not product permission overrides, fake signatures or Native state edits.

## 9. Budget proposal and gates

Request a new **S5.3 envelope C360/F1100/P0**, independent of closed remainders.
Propose first activation of **C180/F0/P0** for the first two component groups,
after implementation/static source review. The other C180 and all F stay inactive
until root's explicit readiness decision. No retry allowance is implicit.

| Proposed case/group | Work / whole seconds |
|---|---:|
|C-s53-controls-01|75 /90|
|C-s53-native-01|75 /90|
|C-s53-mixed-01|75 /90|
|C-s53-policy-01|75 /90|
|F-s53-control-loss-01|230 /260|
|F-s53-preq-policy-01|120 /140|
|F-s53-exhausted-01|390 /420|
|F-s53-c2-route-01|155 /180|
|F-s53-fresh-01|55 /70|

The planned whole bounds sum C360/F1070, leaving F30 unassigned under the
requested1100 envelope, not an automatic retry. Before each case require remaining
allowance to cover that whole bound and all still-planned later bounds. Exhausted
execution additionally requires small genuine control/pre-Q gates to pass, exact
reviewed restore/preservation provenance and a source-reviewed bounded controller.
If those readiness estimates no longer fit, callback before opening/restoring the
private dependency or spending another case.

Measured anchors, not timing guarantees: S5.2 small genuine origin18.7168939s,
policy52.7285073s, opaque preservation C24.1463993s and stopped raw preservation
F67.5439277s; R-A genuine component24.7713883s and public Ready dispatch148.7417507s;
R-B empty-product-fixture runner17.7590631s for0.08s kernel. These show material
wrapper/setup/finalization cost. The stopped copy's67.54s does not predict typed
open/fold/control/history time;420s is a conservative proposed bound with no
guaranteed completion. S5.2 boundary03's649.592232s includes migrations/repeated
history and a failing tail; it is neither a passing runtime estimate nor a reason
to regenerate511. No unchanged100k cost is budgeted.

Closed product-stage S5.1+S5.2 actuals remain C451.3567483/F4567.5783982/P0.
Closed R-A+R-B recorded charges are C136.0918626/F148.7417507/P0, with conservative
C60 included. Combined closed charges: **C587.4486109/F4716.3201489/P0**.
If the full new envelope is approved, cumulative maxima for these cuts including
preparation become **C947.4486109/F5816.3201489/P0**; product-only maxima would be
C811.3567483/F5667.5783982/P0. Earlier allocation amendments/unused balances are
preserved, not refunded, borrowed or silently reactivated. S5.4 whole P5<=2400
remains a separately assigned uninterrupted run; P0 here.

Use locked/offline Jobs12/BelowNormal, compile-only/stage/build/fmt/strict affected
Clippy as separately recorded static work. Compile last before a contributing
run; execute network harnesses only via the stable Windows wrapper and fixed
CLI/control/relay/harness paths from S5 section11. Preserve previously executed
image contents before replacement. One source/build/staging lane; no live image
overwrite or source edits during a bound run. Prepared independent identities
could be parallel only under an explicit root schedule; this proposal is serial.

For each future managed controller, durably save actual source/image/input/seed
inventory before execution. Pass its real path/digest/identity in the accepted
R-B explicit envelope; use `persist_before_tail` after functional assertions and
before a fallible preservation tail. Legacy `unavailable_legacy_runner` and opaque
standalone provenance cannot qualify the budgeted S5.3 run. Keep exact test-name
counts, observed wrapper/whole exits, independent work/whole watchdogs through
terminal writes, verified owned-child cleanup and late expiry checks. Retain all
failures and disclose sampled terminal-publication timing. A snapshot or kernel
PASS alone never establishes whole PASS. Failure/timeout/provenance mismatch/tool
rejection stops for root with no automatic retry or limit adjustment.

## 10. Review decisions and stop

Root is asked to accept this implementation mapping and the proposed envelope/
initial tranche, then assign exact implementation/verification readiness gates.
Protocol decisions above reuse S5. Review the concrete local public export/
ceremony and IPC shapes in section2, then freeze the actual selector/name inventory
against the implementation before tests, with no new protocol authority or wire shape.
The only exhausted-data dependency is the accepted stopped domain and original
pin/one-branch restore rule; private availability has not been rechecked here.

This deliverable changes only this plan and current navigation. Activation and
the queued task remain intact for the later S5.3 scope. No product edit, Cargo
command, EXE/test run, fixture traversal/open/restore or commit/push occurred.
Submit path/hash, decisions, dependencies and requested tranche to root, then
stop for mapping review. S5.4/whole S5/MVP, GUI/groups/multi-device/onboarding,
M1.2.63, dependency upgrades and the deferred A2 crafted relay DoS stay outside.
