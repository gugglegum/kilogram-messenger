# A3-S5.2 — indexed L1 continuation implementation and verification proposal

2026-10-26. **Plan for root review; no implementation or run authorization.**
Baseline `73c499818ba9b699c331aa842e64e8432e90c59a` on master. Authority:
[S5.2 task](long-lived-dialog-a3-migration-s5-l1-task.md), SHA256
`ba8425e12954cfa9ce178512cb5d293b37e76fd9d684344175ce844813cb7eb6`.
The [accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md) remains
unchanged (`f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`).
This document maps its cut2 onto the published S5.1 code; it does not reopen the
architecture. No product/script edits, builds, tests, DB opens, EXE staging or
publication have occurred in this planning dispatch.

Proposed outcome: one Ready Root7 owner continues the same config1 L1 dialog with
the real frozen d9b peer, then with another indexed owner. IDs, Device keys,
original nonce, existing Account/Session/OTK, queue, pending, receipts and history
survive. Only Locator1/ALPN1 is enabled for this cut. Existing Consent2/control
handoff, Selected/mixed writes and whole P5 remain separately gated.

## 1. Concrete starting gaps and implementation scope

| Existing seam at this baseline | Required change; retained boundary |
|---|---|
| [main dispatch](../../apps/kilogram-cli/src/main.rs#L4118) calls `storage_migration::reject_indexed_runtime` before Profile2/Profile1 selection | Add explicit Profile3 dispatch to a new indexed L1 runtime. Unknown/malformed Profile3 must fail, never fall through to V4 Engine or fresh Runtime6. Profile1 and Profile2 retain their own dispatch. |
| [published Profile3](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs#L420) already contains pins, source/migration IDs, config revision, enabled contexts and direct-only route | Add a strict matching loader and resolve those pins/config against Root7. Public JSON does not establish current mode, Native readiness or authority. No root secrets loaded. |
| [Root purpose gate](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L893) currently allows only `CommitInput::Migration7` on Migrated7 | Add a closed `CommitInput::Legacy1` arm and a dedicated facade, not generic/component writes and not a relaxed Runtime2 arm. Check purpose/stage/mode before retry shortcuts. |
| [MigrationRepository](../../crates/kilogram-state/src/vault/indexed_dialog/migration7.rs#L32) owns `Inventory` and implements `LegacyRead` | Keep it for migration/audit. A new live facade loads authenticated Ready/profile/current records without constructing a V4 inventory. Live owner has no `entries()`/`paths()` API. |
| [Head7/current Native checks](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/records.rs#L683) and funding checks currently serve the storage slice | Separate immutable migration audit checks from current live correspondence. Ready/Progress/source refs remain immutable; current inner Head1/native pointer may advance through closed L1 operations. Preserve FundedOnce and budget equations, not an assumption that live counters never change. The existing output-digest comparison already excludes Ready (`records.rs:863`); do not compare its frozen audit digest with later live roots. |
| [copy lowering](../../crates/kilogram-state/src/vault/indexed_dialog/migration7/copy.rs#L507) provides hot Event/lane/projection/receipt/Seen/request rows, kind5 snapshots and kind13 imported tickets | Add explicit readers for these existing forms and post-Ready full L1 captures. Imported request value32 and imported queue/pending tickets must not be parsed as fresh Runtime6 records. |
| [Owner2](../../apps/kilogram-cli/src/ordered_dialog/runtime2/owner.rs#L166), [policy2](../../apps/kilogram-cli/src/ordered_dialog/runtime2/policy.rs#L19), [fresh history binding](../../crates/kilogram-state/src/vault/indexed_dialog/history_source_v2.rs#L384) require fresh origin/selection | Keep these checks. Add a sibling Owner7/L1 policy/source reader; no removal of `Origin::Fresh`, fake Q, dummy `StaticSelection2`, Native2 relabel or error-driven verifier fallback. |

Proposed source scope (names below are new APIs, not claims they already exist):

- State: `indexed_dialog/legacy1_runtime{.rs,/read.rs,/ops.rs,/codec.rs}` and
  `history_source_v7.rs` / a version7 history adapter. Touch the common commit gate,
  migrated record/budget checks and bounded history candidate dispatch only as
  needed. State keeps no protocol/ratchet dependency.
- CLI owner: `ordered_dialog/runtime7/{config,owner,legacy1,policy,network,ipc,runtime}.rs`.
  Reuse the wire1 parsers/cryptography and Guard1 algorithms from
  [handshake](../../apps/kilogram-cli/src/ordered_dialog/handshake.rs#L77),
  [Engine bootstrap](../../apps/kilogram-cli/src/ordered_dialog/engine.rs#L508),
  [outbox](../../apps/kilogram-cli/src/ordered_dialog/outbox.rs#L121),
  [ingress](../../apps/kilogram-cli/src/ordered_dialog/ingress.rs#L66) and
  [PrivateState](../../apps/kilogram-cli/src/ordered_dialog/state.rs#L174).
  Translate algorithms to typed repository methods; do not instantiate an Engine
  over Root7 or translate a generic `Changes`/path mutation batch.
- Share only transport-neutral bounded infrastructure with S4: mailbox capacity16,
  pre-spawn connection permits2, scheduling/ref permits, poison/reopen discipline
  and measurement conventions. Extract small resource/window primitives if useful;
  retain typed Job2/Owner2 and Job7/Owner7 protocol paths. S4 admission, selected
  verification, Native2 envelopes and fresh codecs remain separate.
- Expose Profile3/IPC3 in `main.rs` and `ordered2_cli.rs`; extend the existing
  `migration_s5_process` stable integration target with named L1 cases. Add a new
  S5.2 case/controller runner writing a separate ledger/evidence directory. Do not
  append new consumption to the published S5.1 ledger.

## 2. Repository/owner seam and cold startup

`IndexedL1Repository7::open_ready(lock, ExpectedOwner7, config)` owns the one
lifetime `StateDirectoryLock` and `IndexedDialogVault`. It checks layout7, matching
account/Device/claim/source/migration/incarnation, stored effective config and
storage Ready. Copying/Paused/Blocked migration opens remain admin-only. Root6,
Component5 and schema4 cannot be adopted by this facade.

Expose typed `owner_material`, `context_snapshot`, `native1`, `event_at`,
`request`, `queue_page`, `pending_at/page`, `receipt_at`, `ack_page`, `seen`,
`history_first/page/point` and closed commit methods. Pages use explicit C/namespace,
limit<=32 and a byte cap; point getters return bounded typed records. No raw core,
arbitrary destination key, public import flag or whole-source scanner escapes.

At startup validate the small authenticated migration/Ready witness, then current
per-C Head7 and Native1 once. Original Head/Native source refs remain audit facts;
they are not the current pointer after a live commit. `PrivateState::open` restores
the current **Native1** with the original nonce/Device/self-HPKE info/AAD and real
inner Head1; metadata is in Head7, not an enlarged private blob. Native1 remains
<=128KiB. A workspace fork/restore is not a new E2EE Account/Session. Do not reseal
or advance Native merely because the owner booted.

Two small derived-index preparations may be necessary before enabling a C:

1. Imported committed requests already point to Event IDs; imported queued
   requests need a reverse request->ticket index. Build it from bounded **indexed
   Queue** pages (max128 entries/C), using a resumable closed `PrepareL1Indexes7`
   purpose. Derive request/order from authenticated kind13 tickets; do not scan V4
   filenames or decrypt/reseal every queue body. Uniqueness/charge is checked by
   state. No endpoint publication for that C until preparation commits.
2. Initialize HVI1/HistoryBinding7/heads from real scope and indexed lane frontiers
   when complete genesis/Ready1 exists. Imported candidates can read existing
   hot Event/projection + kind5 membership directly; no full H2 backfill or new
   LegacySnapshot creation is needed. Incomplete session reports NeedHandshake.

Preparation has independent per-C checkpoints, paid metadata and exact retries;
it does not reopen MigrationCopy/Verify or modify the immutable migration audit.
Known AuthBlocked/signed-fork C1 is a per-C state, not failure to load C2. A corrupt
root/MAC or poisoned storage handle is a distinct storage-wide failure.

## 3. Native phase continuation, not reconstruction

All rows below require current policy and authenticated live lease before Native
mutation, and an identical final close gate before durable commit/dispatch.

| Actual saved phase | First legal next work | Legitimate constructor boundary / retry |
|---|---|---|
| Enrollment, zero C | Offline status/authority administration only; no enabled runtime | No synthetic C/session or endpoint for an empty enabled set. Creating a replacement dialog is outside this cut. |
| Registered phase0, no Native | ALPN1 initial Hello/Proof/current Consent1; A prepares Offer, B handles Offer/Grant | First A Offer may call `Account::fresh(false)` once. First B Grant may call `Account::fresh(true)`/reserve its OTK once. Device/C/claim stay original. Missing persisted Native here is real absence, not permission to recreate a later phase. |
| A Offered1 | Resend exact saved Offer; consume matching Grant and produce real A0 | Restore existing Account/key. First outbound Session constructor once; no fresh Account/OTK. Commit Grant/D/E/SID, Native/Guard1/Seen/A0/global author sequence together. Duplicate Grant returns saved A0 without Native work. |
| B Granted2 | Resend exact saved Grant; consume matching A0 and produce B0/own Ready1 | Restore existing Account/key/unused reservation. Inbound Session/OTK consumption occurs once; actual ciphertext/inner/transcript/public keys must match. Commit consumed reservation, Native/Guard1/Seen/A0/B0/Ready together. |
| phase3 A with A0 but missing B0 | Receive exact peer Confirm; persist B0/Ready | Existing Session only; no constructor. Duplicate Confirm returns cached Ready, no second decrypt. |
| phase3 B with both genesis, incomplete Ready | Exchange/resend the remaining exact Ready1 controls | Metadata-only after validation. No genesis regeneration or Native reseal. |
| complete phase3 | Queue, materialize, receive/drain, repair/ACK; lifetime remains512 including genesis | Preserve Account/Session keys, SID/D/E, counters and Guard1. Existing `own.next512` refuses only a new accepted request; old exact request retry/status/history/receive/ACK remain valid. |

Native phase and LocalMode are separate. In this cut LocalMode=L1, wire projection
is stage1/no-Q/no-cuts, including partial phases; wire0/fresh Runtime6 is never used.
Partial status fields remain optional: no fake D/E/SID/frontier/view. Reserving the
one future genesis slot for queue lifetime accounting does not invent a Guard.next.
Text materialization requires both genuine readable genesis and Ready1; bounded
queue before completion follows old R3 reservation rules.

Saved quarantine and signed-fork witnesses are not cleared by boot, policy refresh
or migration. C1 history/status remain available; its forbidden operations remain
blocked. Existing R4 conflicting proofs stay evidence, never readable sources.

## 4. Closed purposes and atomic effects

Propose `Legacy1Operation7` behind named facade methods. Each operation binds root,
C, current Head/Native generation/hash, policy generation and relevant immutable
refs. State derives keys, counts, digest and U/R effects; owner verifies signatures,
current permission, plaintext and actual Native transitions. Neither accepts caller
totals or a trusted/imported boolean. No IPC exposes the operation DTO or raw Native.

| Closed purpose | One durable write set / observable rule |
|---|---|
| `PrepareL1Indexes7` / `InitializeHistory7` | Only derived request indexes and real-scope view metadata from existing authenticated rows; resumable bounded pages. No Native, source mutation or snapshot creation. |
| `PinAuthority1` / `RenewConsent1` | Root-signed monotone authority/account pins or same-C/certs jointly Root-signed Consent1; policy generation/high-water update. Invalidate affected C leases before next dispatch. Own Root changes may affect both C; a peer/C-specific change does not. |
| `PublishLocator1` | Fresh endpoint/family1/revision strictly above imported/current saved value, signed canonical descriptor + policy fence. Commit revision/raw before file/IPC publication; no deletion of an unowned prior artifact. Same-live retry is exact; crash may waste a revision, never reuse it. |
| `L1Bootstrap` subvariants Offer/Grant/Init/Confirm/Ready | Exact phase-specific selected controls, current/selected admission refs, original inner Head1 and actual Native transition, genesis/lane/Seen/global Author sequence/history where applicable. Existing durable result is read before any duplicate crypto. |
| `L1Queue` | Request16/body identity, sealed queue/ticket/order and reserved capacity atomically. Same request/body is exact retry; changed body is Conflict. No Native or Event ID for merely queued work. |
| `L1Materialize` | <=4 eligible queue items -> signed Event1, lane, original self projection, request result, full applicable captures, own debt ticket, Native1/Guard1/Seen/Head/global Author sequence. Retire queue reservation only in this commit; send only after committed outcome. |
| `L1Pending` | Valid signed source + original O, actual admission A, ordinary/head origin/charge, exact request/ordinal, quarantine evidence if already present. Possession alone changes no readable frontier/Native/Delivered. Duplicate conflicting source durably blocks C, not overwrites old bytes. |
| `L1Readable` | <=4 eligible pending -> verify Guard/header/parents before Native, decrypt/inner, preserve O/A, capture actual F, sign/persist exact R and ACK intent, projection/lane/Seen, Native/Head and ticket retirement in one commit. A quarantined item is not silently retried/cleared as healthy. |
| `L1Acknowledge` | Exact saved own Event + real opposite Receipt1/credential -> immutable receipt/delivery overlay and exact debt release once. Duplicate exact receipt is no-op; a hint/Complete/projection is not a receipt. No Native call. |
| `L1AckProgress` / `ObserveHead1` | Exact ACK emission progress or authenticated monotone peer source tuple/last ID; no cursor jump/readable promotion/debt release from hints. Bounds are format1-specific, not a future global Root7 ordinal512 restriction. |
| `L1Block` | Immutable known-reason signed conflict/quarantine evidence + blocked ref; invalidate C lease. Preserve old readable/Native and C2 progress. Genesis-conflict capture retains the supported old boundary, including R4's distinct evidence role. |
| `IncreaseCapacity7` | Reuse accepted same-owner capacity CAS/publication path through the held core, with stored identity/paths. No second writer, implicit increase, extra endowment or import. |

Root layout/purpose matrix: Component5 keeps component purposes; Runtime6 keeps
fresh Runtime2; non-Ready Root7 keeps migration/admin only; Ready Root7 permits the
new closed L1 purposes and accepted capacity administration. Migration Copy/Verify/
LegacySnapshot remains LifecycleClosed after Ready **before exact-retry shortcuts**.
Offering/Prepared/Selected/Active2 writers are unsupported in this cut.

Native trial is private, synchronous and bounded; no workspace crosses network
await. On precommit failure discard the trial and publish nothing. Ambiguous IO
poisons the handle and invalidates Native/cache/leases; reopen under the same lock
and resolve commit ID/request outcome before another Native call. A committed
request is not re-encrypted after a kill-before-dispatch.

### Capture and credit correspondence

Imported readable rows retain immutable LegacySnapshot tags. New bootstrap/Text
readable transitions use a distinct full-L1 version7 proof: applicable O/A/F/R are
the exact bundles actually used, captured at admission/finalization, not guessed
from a catalog. Imported pending keeps its exact O/A; its later real drain creates
F/R. Genesis has no user-message receipt debt; queued data has no Event captures.
Late receipt adds a separate current delivery record without rewriting old tags.

Global Author/next-sequence ownership remains Device-wide across C, not reset per
context. Update Guard1 and the indexed Seen mirror in the same native commit;
maintain it incrementally, without replay/folding all history on every turn.

Keep B4=147980288 held per occupied C until the later selection boundary. Imported
and new L1 queue/pending/head/own-awaiting tickets consume their actual U/R and
recomputed `covered`/LegacyHold; reservation is not a second scalar charge.
Preserve queue128KiB, pending128+temporary64KiB minus already-paid captures,
separate head capacity and opposite receipt debt>=48KiB rules. Auth/shared refs are
paid once with explicit ownership. ALPN1's bounded legacy work uses Held capacity;
do not invent format2 repair grants or repair-origin outstanding for imported work.
The once-paid64MiB Ready endowment is conserved, not recreated on boot/drain/ACK.
No refill API is introduced. Retained source backing/global metadata stays charged
to owner0; per-C ticket release cannot spend another C's hold. Physical copies,
nodes, retained backing and shadows remain real capacity obligations.

## 5. Owner, admission and public surface

One `Owner7` owns the facade and at most one authenticated lease per C. Derive its
Native1 cache from persisted current generation/hash; invalidate on uncertainty.
Reuse the [S4 scheduling pattern](../../apps/kilogram-cli/src/ordered_dialog/runtime2/runtime.rs#L51):
mailbox16, round-robin C, send/drain alternation and <=4 Native packets/turn.
[Window shares](../../apps/kilogram-cli/src/ordered_dialog/runtime2/window.rs#L21)
remain128 total, hard64/64 with two enabled C, including when one is blocked.
[Resources](../../apps/kilogram-cli/src/ordered_dialog/runtime2/resources.rs) must
hold two **global** pre-TLS/pre-spawn permits, bounded streams/frame buffers and
cleanup. They are not two permits per C or per ALPN.

Use [transport ordered1](../../crates/kilogram-transport-iroh/src/ordered.rs#L35),
exact wire1 Hello/Proof/Consent1/Offer/Grant/Ready, exporter and Descriptor1 parsing.
Root7 advertises ALPN1 only in this cut; fresh Profile2 retains its ALPN2 runtime.
`Lease1` checks connection close/validity, unique generation, monotonic deadline,
current policy generation/time, exact current own/peer Locator1 refs, remote endpoint
and actual selected IP path at dispatch and final commit. Do not reuse Lease2's
Locator2/Q assumptions. Descriptor expiry/revocation that was permissible as an
archive witness is not current admission.

Every boot has a fresh endpoint. A permitted C publishes a new monotone Locator1
only after the metadata commit and owned-file checks. AuthBlocked C has no false
publication-ready status; another permitted C can use the endpoint. Root7 L1
requires explicit DirectOnly: RelayOnly/Auto-with-relay fails `L1RequiresDirectOnly`,
with no hidden relay discovery/fallback. Current own authority, per-peer-account
pins, Consent1 and wall high-water are checked; Root7 cannot adopt S4's single
fresh peer pin as a substitute for its registered per-C peer roster.

Map wire1 Resume/Head, Deliver, MissingPrefix/PrefixBatch and AckBatch/ack recovery
onto bounded typed readers and the closed purposes above. Retransmit the exact
committed source/receipt, with zero Native work; hints and prefix possession never
release debt or advance readable state. Keep actual format1 range/frame limits
and the separate reserved head path. Do not funnel old repair through Runtime2's
Q/format2 repair-grant admission.

Keep the future eligibility seam explicit: real complete session/genesis/Ready and
current L1 permission are separate predicates from future current Consent2/ALPN2
control admission. No requirement for an already-present Q/Selected state may be
embedded in common Root7 load/scope validation. Cut3 must be able to admit genuine
pre-Q L1/Offering/Prepared control pairs. At its first durable P (A) / Prepared
(B), ALPN1 admission ends and materialization freezes; Selected is not that first
boundary. These transitions/ALPN2 are unsupported now, with no compatibility
fallback or placeholder success.

Proposed IPC3 requests are canonical nested tuples inside the existing authenticated
OrderedDialog IPC command. Fix these exact arities during root review:

| Request | Exact proposed tuple |
|---|---|
| Status | `[3,1]` |
| Queue | `[3,2,slot,request16,text]` |
| Request status | `[3,3,slot,request16]` |
| Indexed history | `[3,4,slot,cursor_or_null,limit]` |
| Locator1 import/export | `[3,5,slot,1,action,raw_or_null]`; action0 import requires actual Descriptor1 bytes, action1 export requires null |
| Signed authority import | `[3,6,raw_authority]` |
| Consent1 renewal, proposed new tag11 | `[3,11,slot,expected_current32_or_null,raw_consent1]`; exact current-ref CAS, same C/parties/keys, current verification, no historical-admission flag |
| Capacity CAS | `[3,35,expected_root_generation,expected_config_revision,expected_config_hash32,candidate_config]` |

Slot must be occupied/enabled where required; each field keeps its normative
record cap and the outer IPC byte cap. Null expected consent means genuinely
absent, not a wildcard. Tags7–10 and family2 locator/upgrade requests return
UnsupportedInCut2. Unknown tags, bad versions, extra fields and invalid optional
shapes fail rather than retry another codec. Tags32–34 remain MigrationAdmin-only;
the live runtime must reject them rather than start a second admin/writer. Stopped
admin uses the exclusive lock; capacity tag35 is available to the authenticated
same-owner admin/runtime through the held core.

Proposed new CLI names in `ordered2_cli.rs`: `ordered status3`, `send3`,
`request-status3`, `history3`, `locator3-import/export`, `authority3-import` and
`consent1-import` select IPC3 explicitly, leaving existing IPC2/legacy commands
unchanged. `run` loads only the strict already-published Profile3. The storage
capacity CLI needs an explicit exclusive choice of stopped state access versus
authenticated `--ipc-file`; it must never open a second mutable repository while
the owner runs. No new profile/account/context creation ceremony is part of this cut.

Status exposes storage stage, real native phase/options, L1-ready, per-C policy/
block, queue/pending/head/debt/credits, current locator revision and proof kind.
It reports handoff unsupported/NeedHandshake/NeedConsent2 truthfully, not a fake Q.
Known C1 policy/block failure leaves C2's independently allowed work running.

## 6. History and request semantics

[HVI1 identity](../../crates/kilogram-state/src/vault/indexed_dialog/history_types.rs#L304)
already contains incarnation+C/D/E/SID/role and **no Q**. Reuse that codec/view ID
and frozen-frontier cursor mechanics. Add HistoryBinding7 and a closed candidate
dispatch for `LegacySnapshot7` versus post-Ready full L1 captures. Do not weaken
fresh H2 binding or fabricate a selection certificate to make its parser pass.

Initialize a live view only with real complete session/genesis/Ready; partial
bootstrap returns NeedHandshake instead of a synthetic SID/view. A page reads only
the requested C: indexed lanes/Event/projection/binding, small authenticated origin
metadata and referenced capture records. No V4 table lookup or Native128KiB restore
per row, full source verification or full-history cache. Imported proof membership
is explicit; malformed full proof never falls back to the snapshot variant.

Page<=32, bounded point/range buffers; IPC counts actual escaped JSON<=48KiB before
advancing the candidate cursor. Old cursor frontiers/suffix remain exact after
append/restart and future handoff. Leave future format2 ordinals/cursors able to
cross512; the L1 limit belongs to L1 source/materialization, not the generic view.

Request status is point-based Queued/Committed/Delivered using queue reverse index,
Event mapping and current receipt overlay. Same ID/body retries consume no Native;
changed text conflicts, including imported rows whose actual projection supplies
the original body. New history includes imported+new L1 rows and current delivery
overlay; immutable snapshot tags do not change. `storage-history` remains the
distinct frozen **V4 membership** dataset<=1024 source rows/C, never includes new
post-Ready rows or claims an opaque frozen cursor. Its delivery output must clearly
separate original snapshot delivery from any current exact receipt overlay.

## 7. Minimal verification matrix — proposed names, not existing PASS

Use new scoped selectors `indexed_l1_` (C) and `migration_s5_l1_` (F) on prepared
stable harnesses. No new empirical feasibility is part of this planning dispatch.

| Named check / production entrypoint | Actual acceptance assertions | Estimated whole contributing seconds |
|---|---|---:|
| C-L1-purpose: common gate + `open_ready`/closed op lowering | Layout5/6/7 positives; wrong purpose before retry, Copy/Verify/new snapshot after Ready closed; wrong owner/mode/config/fence; ambiguity poison/reopen. | Included in C210–340 below |
| C-L1-bootstrap/captures: Owner7 phase dispatch + typed state | Phase0/1/2/3 options, no fake view, no recreate existing Account/reservation; old/new proof discrimination, O/A/F/R application and bad-source-before-Native; imported fork/quarantine stays blocked. | same |
| C-L1-ledger: each commit + real summaries/tickets | Paid auth once, LegacyHold per C, no cross-funding/refill, old/new debt exactly once; boundary quota need/need-1; queue lifetime includes genesis. | same |
| C-L1-history/request: bounded candidate/read/index prep | Imported and full-L1 proof variants, actual byte cap/escaping, stable suffix after append/reopen, bad/cross-C cursor, no C2 or Native/source-scan read, queued/committed retry/body conflict. | same |
| C-L1-admission/resources + changed shared smoke | Lease/route/locator/policy close gates, mailbox16/permits2/window64/64, unsupported IPC3 tags; targeted fresh Root6/Native2 and Component5 positives where common gate/resource reader changed. No bulk allocator/100k repeat. | same |
| F-L1-small-interop: Profile3 -> Owner7 -> ALPN1/data/ACK | New small genuine d9b origin; migrate A only, actual indexed A↔frozen d9b B duplex/retry/offline/restart; stop/migrate B and both indexed continue same IDs/keys/SID/D/E/history, real receipts. | 80–120 |
| F-L1-partials: actual control-peer holds + Bootstrap | Recreate five genuine partial boundaries using the accepted public control-peer technique; finish once, queued requests preserved, constructor counters only at legitimate missing-object boundaries; no fake Ready/view. | 80–130 |
| F-L1-obligations: wire pending/drain/ACK + C2 | Real gap reaches64 ordinary+reserved head; late/gap drain, actual renewed O/A and own-unacked debt, delayed exact R releases once; quarantine/fork C1 keeps history/block while real C2 progresses within64/64. | 100–160 |
| F-L1-policy: live IPC/Locator1/admission | Current expiry/revocation/renewal, malformed input, stale/closed lease, direct-only refusal; historical witness alone cannot admit. Root/account scope and C-specific isolation. | 45–75 |
| F-L1-atomic: typed Native commit and public dispatch | Parent alive/kill/wait at Native staged, durable-before-dispatch and readable/receipt outcome; reopen exact request/commit ID resolves old-or-new, no double constructor/encrypt/decrypt, exact history/Seen/credit/debt. | 60–100 |
| F-L1-511-origin: new controller -> **frozen d9b only** | One new real elected A511/B16 Text origin, complete ACKs, A own.next512/B own.next17; old d9b new-request refusal and exact old retry leave Native unchanged. Exact keys/Native/IDs/request/cipher/public provenance. Freeze originals after stopped validation. No manufactured Head/Event or raised product lifetime. | 500–850 |
| F-L1-512-gate: copied frozen origin -> migration -> live facade | Migrate same origin, new A request rejects UpgradeRequired before Native before/after reopen; old retry/status/history/receive/ACK remain correct and opposite lane can continue when permitted. Freeze resulting indexed checkpoint for later handoff. No Event2 claim. | 80–140 |

C is five logical groups, not five repeated whole suites: prepare once per affected
package/features and group selectors to avoid repeated cache/staging overhead.
Estimate C base210–340 seconds including fixture-independent targeted checks and
whole runners; proposed C reserve160 gives a **new C tranche500 seconds**.

F row ranges include each row's setup, copied branches, process/lease waits, kills,
recovery, final hashes and whole runner (roughly15–25 seconds wrapper allowance per
group, not free setup). Sum **945–1575 seconds**; add225 seconds contingency to the
upper estimate for a **new F tranche1800 seconds**. Failed attempts remain charged;
contingency is not permission for quiet reruns or a second full511 generation.

Existing cost anchors, all read-only:

- [S5.1 small d9b setup](long-lived-dialog-a3-migration-s5-storage-evidence/F-feasibility-01.json):
  13.8950435 seconds; later copied-seed check24.3522197.
- [S5.1 owner](long-lived-dialog-a3-migration-s5-storage-evidence/F-storage-final-r2-02.json):
  76.1118936 runner; [process3+2](long-lived-dialog-a3-migration-s5-storage-evidence/F-process-controls-final-r2-02.json)
  controls94.023144 runner /78.29 Rust. These are scaling anchors, not new L1 PASS.
- [actual old mixed runtime](transport-security-a2-root-review-evidence/mixed-result.json):
  setup4.9882851 / exchange53.4841043 / total62.2957725. Its `text_count=64` means
  **32 Text per side** (`send_next33`), not64 per side.
- [old full511 component](lossless-sync-a1-bidirectional-implementation.md):
  816.71 seconds in unoptimized verification; not a per-message network latency.
  These justify the accepted500–850-second slow-origin range and rule out a cheap
  fresh-S4-throughput promise.

### One-time exhausted origin and later cut512 acceptance

Recommend **one exhausted lane A511 with real B16**, not an unbudgeted double511
origin. Contract M02 requires real own511/next512; it does not require both cuts512.
This origin also exercises multiple real opposite/DH exchanges. The current
`test-ordered-dialog-process.ps1:8` caps MessagesPerLane128 and has symmetric counts:
`-MessagesPerLane511` is **not** an existing working command. Add a new bounded
S5.2 driver supporting asymmetric counts through actual d9b IPC, unique requests,
bounded batches/backpressure and genuine ACK completion. Product LIFETIME512 stays
unchanged. No private positive Native/Head/Event factory can replace the origin.

Run small infrastructure/interop/fault gates first. Before the long generation,
recompute remaining tranche and require the remaining seed upper850 + gate140 +
contingency still to fit. Propose a900-second whole seed-attempt stop bound with
cleanup reserved inside it; any failure/overrun is retained and triggers root
replanning before another long attempt. Estimates are not permission to exceed.

Freeze the stopped V4 originals with exact producer EXE/source/script hashes,
owner/C/role/D/E/SID/SessionKeys, source IDs/generations and logical Event/request/
Native hashes. Never open a frozen original. Work only on an isolated active copy
of that same Native domain; preserve a stopped indexed-L1 exhausted checkpoint
after this cut's gate. Serially reuse a new copy of that checkpoint in cut3 for
actual handoff with cutA512 (cutB is its actual value, initially17), then first
Event2 n512 with the same Account/Session/D/E/SID. No generator cost is charged
twice, but every copy/reopen/handshake is charged when actually executed.
Evidence exports contain public SessionKeys/fingerprints and hashes, not Native
plaintext/pickles, private keys or IPC bearer secrets; protected DB copies stay local.

S5.2 can accept exhausted L1 refusal/retries/history. Only cut3 can accept actual
cut512/Q/Selected/Committed/format2 continuation. The final whole P5 remains one
independent continuous run; neither these branches nor prefix/suffix fragments
replace it. If root instead requires duplex511/511 now, revise the seed forecast
upward before execution (1022 Text versus527); recommend a separate F2200 tranche
decision, not silently using the single-lane850 estimate.

## 8. Proposed budget accounting and reserved later work

Published S5.1 ledger remains immutable: C279.3257298, F1236.2886628, P0. Its C600 /
F1320 ceilings, including the R4-only +120 amendment, do not authorize this cut.
Unused old balances are not free S5.2 setup. Future runner records a separate
S5.2 ledger with the accepted plan/review hash and immutable S5.1 baseline totals.

| Set | Proposed new S5.2 tranche | Cumulative maximum if the whole tranche is used |
|---|---:|---:|
| C | 500 s | 779.3257298 s |
| F | 1800 s | 3036.2886628 s |
| P | 0 s | 0 spent; later whole-P ceiling2400 remains unconsumed/unallocated |

These are explicit new limits for root approval, not a reset or reinterpretation
of the accepted contract's original600/1200 budgets. A future amendment records
both the new tranche and cumulative ceiling before its first run. Root may choose
the larger full-duplex-origin option; that changes proposed F cumulative maximum
to3436.2886628, not the product protocol.

Reserve planning room for **later separately authorized** cut3/final work:
C350–550 and F650–1100 for existing controls/current-policy/Selected/mixed/fault
checks using the preserved exhausted checkpoint; then the one P<=2400 with its
own retained controller/runner accounting. Indicative all-stage upper envelopes
are C1329.3257298, F4136.2886628, P2400 under the recommended option. They are not
budgets granted now or a forecast that whole P certainly passes. Root must make
that later decision after source mapping/cost evidence; do not consume P as spare F.

## 9. Execution/reuse controls and review decisions

After acceptance only, proposed command surface:

```powershell
# Existing wrapper, compile only; charged preparation after budget acceptance.
$s52PreviousOffline = $env:CARGO_NET_OFFLINE
try {
    $env:CARGO_NET_OFFLINE = 'true'
    & scripts/run-cargo-tests-stable.ps1 -Package @('kilogram-state','kilogram-cli') -CargoJobs 12 -Profile verification
} finally {
    $env:CARGO_NET_OFFLINE = $s52PreviousOffline
}
# Proposed new S5.2 runner/selector names; these do not exist yet.
& scripts/invoke-migration-s5-l1-case.ps1 -Case C-l1-final-01 -Set C -Package @('kilogram-state','kilogram-cli') -Filter indexed_l1_
# $s52StoppedOrigin must name a new stopped original with recorded provenance;
# the future runner copies it and never opens that original.
& scripts/invoke-migration-s5-l1-case.ps1 -Case F-l1-small-01 -Set F -Package kilogram-cli -Filter migration_s5_l1_small_ -FixtureDirectory $s52StoppedOrigin
& scripts/invoke-migration-s5-l1-case.ps1 -Case F-l1-511-origin-01 -Set F -Scenario ExhaustedOrigin -MessagesA 511 -MessagesB 16
```

Commands above are proposed selectors/interfaces, not claims that the new scripts
or tests exist. Both packages currently require no extra Cargo feature for these
targets (`CargoFeatures=@()`); changing that requires recording the exact feature
set. Always supply the fixture parameter to a seed-consuming process group.
The new runner must time compile-only preparation, field-image build/staging and
every contributing attempt including cached compilation, inventory and cleanup.
Record preparation once under the appropriate accepted C/F set; it is not free
work outside the ledger. Locked/offline verification, Jobs12/BelowNormal; no direct
network `cargo test`. Static compile/Clippy/fmt also retain exact argv and elapsed
time, with their classification explicit rather than silently omitted.

Keep current S5 fixed app/control-peer and `migration_s5_process` harness paths;
record content hashes, never revision/hash filenames. Preserve frozen d9b/S3/exact
S4/relay images and accepted historical DB/evidence. If an S5.1 binary is required
as a predecessor control, preserve it at a separate stable predecessor path before
authorized staging; do not overwrite its sole accepted copy without that plan.
No EXE recopy while its exact path is live. Source freeze/build/staging and Native
domain ownership finish before cases; record before/after source sets and copied
image hashes as in S5.1.

CLI wrapper compile/copy/run is one serial lane. Independent cases may overlap
only with distinct identities/fixtures/Native domains and prepared images, never
two active copies of the same role/session/counter domain. Native paired peers are
one real protocol pair, not counterfactual branches. Parallel elapsed times are
still charged per contributing attempt; contention is not a controlled throughput
benchmark. Do not rely on parallel execution to make the1800 ceiling fit.

Reuse S5.1 migration/provenance/credit/fork cases by exact source/function diff;
rerun affected common-purpose/current-ledger/history readers and fresh Root6/S4
positives where changed. S1/S2 normal append/page/allocator and ratchet/transport
wire libraries are intended unchanged; no100k repeat without changing their
relevant branch and a new root plan. S5.1 storage Ready or fresh S4 evidence never
substitutes for the new live Root7 branch's assertions.

No contradiction in the accepted architecture was found by source inspection.
The closed current implementation seams above are expected cut2 work, not reasons
to weaken fresh-only gates or edit the contract. Root decisions needed before
implementation: approve this module/purpose boundary and the derived queue-index /
history-variant preparation; approve the exact new Consent1 IPC3 tag/tuple
(suggest tag11, leaving7–10 unsupported); accept the one-lane511/16 origin or fund
the larger alternative; explicitly grant the chosen C/F tranche and later reserve
strategy. No run is authorized merely by these estimates.

Only this plan is the executor's changed deliverable. The four root-owned dirty
task/navigation files are preserved. Callback with this file's exact hash, then
stop for review; product implementation requires a separate root assignment.
