# A3-S5.2 implementation candidate — indexed existing-L1 continuation

2026-10-26. Submitted for root implementation review against
`73c499818ba9b699c331aa842e64e8432e90c59a` on master. There are no staged changes;
the index remains HEAD. No commit/push, cut3, whole-P or product refactor is
authorized by this submission.

Existing real L1 dialogs can continue through the indexed Root7 owner: first with
an actual frozen d9b peer, then with both owners indexed. The implementation keeps
the same account/device identities, nonce, Native objects and Session/D/E/SID,
while preserving requests, readable history, pending/debt/receipt and blocked
state. Legitimate new packets advance Native; migration itself must not replace
it or create a new session. Storage Ready alone grants no live authorization.

Evidence is a collection of precisely scoped runs and root acceptances. There is
**no passing whole exhausted-boundary test**. Boundary03 remains FAIL649.592232s
at evidence preservation. Root accepted its completed functional assertions by
the bound source and170 phases, then separately accepted a real C regression and
preservation-only F PASS. Its lost original parent-result is not reconstructed.

## Submission files and authority

- Exact candidate allowlist:
  [long-lived-dialog-a3-migration-s5-l1-candidate-files.json](long-lived-dialog-a3-migration-s5-l1-candidate-files.json).
  Each ordinary entry has raw bytes/SHA256 and Git-normalized blob ID. The manifest
  omits its own digest to avoid a checksum cycle; the callback supplies that hash.
- [Final source/evidence binding](long-lived-dialog-a3-migration-s5-l1-evidence/S-final-submission-binding.json),
  169741B SHA256 `692a67f746ded2757db61a9a237489cb153dd55165646a464b3af196695b8d59`.
  It binds52 compiled inputs,33 product inputs, fixed images, every39 contributor,
  all94 static attempts with outcomes, final dependencies/features and private
  checkpoint metadata. It contains12 failed static attempts, not hidden retries.
- [Read-only reuse assessment](long-lived-dialog-a3-migration-s5-l1-evidence/S-final-reuse-assessment.json),
  45423B SHA256 `135337b1a62b5c64d665a4a24413d5dd63263e1fd7967d1c93c508a1d1fc4f6e`.
- [Protected-input/navigation comparison](long-lived-dialog-a3-migration-s5-l1-evidence/S-final-protected-inputs.json),
  5689B SHA256 `cefe2adb481476de81a06cb0f4a1b2d91336f98982c27e9bb9921b66106b9386`:
  twelve protected baseline blobs remain equal; old chronology is retained.
- Normative inputs remain the published S5 contract, accepted mapping and
  [revision2](long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md), the
  [implementation assignment](long-lived-dialog-a3-migration-s5-l1-implementation-task.md)
  and its subsequent root review/continuation/budget decisions.
- Final narrow evidence acceptance is in
  [boundary03 completion review](long-lived-dialog-a3-migration-s5-l1-boundary03-completion-review.md)
  and [preservation review](long-lived-dialog-a3-migration-s5-l1-preservation-review.md).
  The latter is5141B SHA256 `5f911cb8f48c5e6d5c1b448789b6854603562be0a7f730fa35c3449fdb14cd7e`.

Unchanged S5.2 task/plan/review/amendment/approval history and the standing direct
human test permission are included for a self-contained authority chain. Root
explicitly authorized nav3 current-status/budget/reference updates; their prior
bytes are preserved in `S-final-navigation-*.txt` with a before binding. Older
chronology is retained. The S5.3 handoff task, unrelated/protected inputs, private
state and build outputs are excluded as enumerated in the allowlist metadata.

## Product mapping and resulting behavior

| Area | Concrete implementation and invariant |
| --- | --- |
| Typed live storage | `migration7/live1.rs` plus its types/read/ops/data/history modules expose `L1Repository` over the held Root7 core. No mutable V4 Engine, reconstructed migration Inventory, generic path writes or full-history cache is used by live access. Reopen retains the lifetime lock, including failure ownership. |
| Purpose gates | `CommitInput::Legacy1` is closed to Root7; Component5/Runtime6 cannot adopt it, even through exact retry. MigrationAdmin remains separate. Copy/Verify/new imported snapshot after Ready remain closed. Root7 initialization does not require fictitious Q/Selected. |
| Paid index preparation | Closed Initialize/PrepareRequests/InitializeHistory operations prepare bounded, resumable live indexes. Immutable imported migration audit/Ready witnesses stay separate from mutable current authorization and receipt overlays. |
| Live owner | Sibling `runtime7::Owner7` holds the typed store and serializes Native/state work. Snapshot reads are bounded; existing Native is restored and retained rather than recreated. Each real partial phase continues at its first legal next operation. |
| Bootstrap | Actual Offer/Grant/Init/Confirm/Ready continuation retains genuine existing controls and queued request identities. Constructors run only at genuinely missing Account/Session boundaries; no fake ready view or synthesized positive Native/Event origin. |
| Atomic packet state | Bootstrap/Materialize/Pending/Drain/Acknowledge lower through closed operations. Native, Guard1, Seen, pointer/projection, O/A/F/R capture references, credits and debt commit together under the current policy/lease final gate. Ambiguous commits poison/reopen rather than publish speculative state. |
| Authorization | Actual ALPN1/Locator1/Hello1/Proof1/Consent1, DirectOnly, current authority and scoped lease checks. Account-pin advancement invalidates every referencing C before reuse/final commit; genuinely unrelated C keeps its token. Historical witnesses never grant current permission. |
| Resources | Mailbox16, two global pre-spawn connection permits, Native batch<=4,128 references with64/64 shares for two contexts, including blocked C1. A blocked context keeps its share; C2 does not borrow it. |
| Lifetime/debt | L1 lifetime512 includes genesis and queued requests. Before Native restore/fork, materialization is capped by min(4,queued,128-current-C-debt). Closed lowering independently checks128. Exact receipts release only their own debt once; per-C LegacyHold/global backing ownership and one-time Ready funding do not refill on restart. |
| History/requests | Typed HVI1 bounded range reads, C-scoped cursors/frontiers, explicit immutable LegacySnapshot versus FullL1, receipt overlays and exact old retries. Pages<=32 rows; actual escaped JSON is checked at48KiB-64 before consuming a cursor. No Native restore or legacy scan per history row. |
| Public administration | Strict Profile3/IPC3 and ordered CLI; bounded Consent1 CAS tag11 and capacity/config CAS tag35. Existing wire1/IPC2 stay intact. Handoff tags7–10/family2 remain unsupported; migration-only tags32–34 do not become live IPC writers. |

Principal product files are `apps/kilogram-cli/src/ordered3_cli.rs`,
`ordered_dialog/consent1_admin.rs`, the runtime7 directory and the typed live1
directory. Existing main/ordered2 routing, common purpose/accounting/load/profile
seams and debug controls have narrow supporting changes. The candidate manifest
is the complete path list, including controllers and resource/verification scripts.

The closed operation set comprises Initialize, PrepareRequests, InitializeHistory,
PinAuthority, RenewConsent, Locator, AdmissionPair, Bootstrap, Queue, Materialize,
Pending, Drain, Quarantine, Acknowledge, AckProgress, ObserveHead and Block. It
does not provide an arbitrary key/value writer or generic IPC signer.

## Public same-C renewal and policy coverage

`ordered consent1-renew-propose` and `ordered consent1-renew-cosign` consume the
existing completed invitation, expected C/account/peer, current authority snapshots
and minimum authority floors. Each invocation receives only its own offline Root
via `--account-dir`. Outputs are a completed legacy invitation and raw Consent1;
no messenger vault is created and no Root secret enters Owner7.

The actual public flow executed in F-policy-01 uses the producer commands, current
authority import, stopped frozen d9b `authority-update`/existing-C `context-import`
and indexed `consent1-import` CAS. It covers expiry refusal then legal renewal,
closed stale/conflicting/current-ref cases and unchanged original tuple/session.
Public APIs also include `status3`, `send3`, `request-status3`, `history3`,
`locator3-import/export`, `authority3-import` and `storage-increase-capacity-live`.
The exact test argv is in the contributing records and bound controller source.
Templates require real public files/own Root directories; no private bytes or
bearer tokens are exported in this report.

Renewal cannot clear revocation, signed fork or quarantine. Actual corrected-image
Ready and isolation checks exercise that persistence. Canonical option shapes,
fixed arities/caps, wrong scope/role/suite/format/signature and purpose checks
precede Native work. The debug negative-control signer is external-schedule,
debug-only preparation by the actual authorized sender, not a runtime signing API.

## Required five C/seven F groups and evidence disposition

| Accepted group | Actual coverage and scope |
| --- | --- |
| C purpose | C-closed-l1-02: typed purpose/generation/exact-retry fences, component envelope and Held ledger. C-shared-root6-01: actual8 targeted fresh-Root6/component tests, including lock retention and closed policy/final-stage gates. |
| C bootstrap/captures | Closed envelope/applicability checks and C-review1-01; genuine missing-object/constructor boundaries are F-partials-02 and corrected-image F-review1-own-f-02, not synthetic unit origins. |
| C ledger | Held64 plus distinct head, no refund/cross-C credit and current debt bound component tests; actual64/head/drain/receipt scope from the expressly resumed F-obligations-resume-03, current actual127/128 debt path from F-review1-unacked-01. |
| C history/request | Typed bounded preparation/input checks; F-atomic-01 actual escaped byte cap; small/obligation history; completed source-bound boundary03 frozen suffix/both528 comparison. |
| C admission/resources/shared | Current/final policy purpose tests, hard64/64 window component check, real policy/account/C2 cases, IPC3 rejection and predecessor checks below. No full unchanged100k allocator suite was repeated. |
| F small interop | F-small-origin-01 and F-small-interop-03 whole PASS on their recorded candidate. Later corrected-image own-F/unacked/isolation cases and boundary03 cover current live behavior in their exact scopes. Historical small PASS is not renamed a fresh final-image aggregate. |
| F five partials | F-partials-02 whole PASS after actual frozen control holds: after-offer B, after-grant A, before-init-commit B, before-confirm-commit A, after-receive-confirm B. Actual queued requests and constructor boundaries checked. F-review1-own-f-02 separately proves corrected own genesis/Text F for both real genesis roles after authority advance. |
| F obligations | Accepted resumed set F-obligations-resume-03: actual64 ordinary pending plus separate head, C2 progress, account-pin invalidation,65 drain/exact receipts, Oold/Aold/Fcurrent/R and quarantine through renewal/reopen. It uses build09 and two specifically approved70s waits; it is not a fresh final-image aggregate. Current128 debt/no-extra-Packet/one-receipt admission is a separate corrected-image whole PASS. |
| F policy | F-policy-01 actual public renewal/frozen import/current policy/IPC/capacity path; corrected-image F-isolation-02 and root-accepted completed R3 receiver assertions in F-review1-ready-02. |
| F atomic | F-atomic-01 whole PASS on its recorded image: actual Native staged/durable-before-dispatch parent kills, old-or-new recovery, request/projection/counters/history/escaping. F-capacity-01 kernel assertions accepted separately despite whole matcher FAIL. |
| F real origin | Exactly one F-exhausted-origin-01 whole PASS481.4608671s, frozen d9b A511/B16, same actual pair, complete ACKs/history/request provenance. No second generation. |
| F exhausted boundary/checkpoint | Three whole failed boundary attempts are retained. Root accepted boundary03's completed functional assertions through checkpoint.info; its parent-result is absent. The narrow writer repair then passed actual C and separate F preservation-only, producing the private stopped checkpoint without repeating functional operations. No handoff/Event2/whole-P claim. |

This mapping records reused scopes rather than inventing one aggregate PASS.
Root's narrow acceptances and the exact source/image of each original case govern
them. A fresh Root6/S5.1 Ready result alone never covers the new Root7 live branch.

## Corrective iterations and every contributing attempt

Product review R1 corrected own F capture independently of immutable O, requiring
applicable F in closed bounds/lowering/history readers. R2 capped unacknowledged
materialization before Native and independently at closed lowering. R3 validates
Ready shape/scope before classifying a genesis conflict, retaining prior peer
Ready as the first fork witness. Current corrected-image regressions below cover
these changes; earlier positive fixtures are not repaired/relabelled as new proof.

Controller/infrastructure corrections are separate: frozen reconnect waiting;
bounded70s obligations continuation and exact held-empty-lock preservation;
debug malformed-Ready preparation; exact single-test log matcher; fresh reconnect
barrier; observed migration checkpoints and fixed absolute deadlines; finally
per-record writer closure. No product timeout, lifetime or validation optimization
was used to turn a failed attempt into a PASS.

| Case | Whole result | Charged seconds | Disposition |
| --- | --- | ---: | --- |
| F-small-origin-01 | PASS | 18.7168939 | Genuine small4/4 seed. |
| F-small-interop-01 | FAIL | 26.8071700 | Startup failure; log/image retained. |
| F-small-interop-02 | FAIL | 50.3112762 | Continuation wait failed; retained. |
| C-closed-l1-01 | FAIL | 14.6110299 | Component test assertion/fixture failure; retained. |
| F-small-interop-03 | PASS | 74.5982578 | Scoped real d9b then both-indexed interop. |
| C-closed-l1-02 | PASS | 30.1268124 | Actual CLI1/state3 matching component tests. |
| F-partials-01 | FAIL | 39.6781966 | Frozen stale-locator/admission sequence exceeded controller wait; no product-timeout claim. |
| F-partials-02 | PASS | 154.7601154 | Five real boundaries; controller uses established25s wait. |
| F-policy-01 | PASS | 52.7285073 | Public renewal/policy/IPC path. |
| F-atomic-01 | PASS | 61.7059514 | Atomic kill/recovery and escaping scope. |
| F-obligations-01 | FAIL | 98.6709406 | Bounded pending condition unmet. |
| F-obligations-cold-01 | FAIL | 0.3934474 | Diagnostic failure, not functional evidence. |
| F-obligations-cold-02 | PASS | 0.9957848 | Stopped diagnostic only. |
| F-obligations-ranges-01 | PASS | 16.5876485 | Cold range diagnostic only. |
| F-obligations-ranges-02 | PASS | 16.4971366 | Separate diagnostic; charged. |
| F-obligations-02 | FAIL | 101.7948885 | Bounded pending condition unmet. |
| C-shared-root6-01 | PASS | 47.6905729 | Targeted8-test shared smoke. |
| C-deadline-01 | PASS | 8.5214364 | Real owned-job deadline mechanism. |
| F-obligations-resume-01 | FAIL | 9.6329237 | Held-lock preservation failure before continuation. |
| C-preservation-01 | PASS | 1.5141070 | Held-lock helper probe, exact recorded scope. |
| C-preservation-02 | PASS | 2.9121551 | Positive plus nonempty/unlisted/missing-ownership negatives. |
| F-obligations-resume-02 | FAIL | 40.0101169 | Original bounded condition still unmet. |
| F-obligations-resume-03 | PASS | 161.1341713 | Explicit resumed set, default8, two70s waits; not fresh aggregate. |
| F-isolation-01 | PASS | 54.0250380 | Historical image scope, later corrected-image repetition exists. |
| C-review1-01 | PASS | 20.9155434 | Actual CLI2/state3 R1/R2/R3 components. |
| F-review1-own-f-01 | FAIL | 9.0392997 | Restricted-token local socket10013; no product-defect inference. |
| F-review1-own-f-02 | PASS | 66.2735246 | Native-token genuine own-F role coverage. |
| F-review1-unacked-01 | PASS | 115.0752300 |127 debts+queue4 ->128/queue3; no extra Native at128; one real receipt ->queue2/debt128. |
| F-review1-ready-01 | FAIL | 46.9119333 | Malformed arity stopped in preparation before intended receiver assertion. |
| C-ready-arity-fixture-01 | PASS | 21.5929621 | Focused debug control preparation correction. |
| F-review1-ready-02 | FAIL | 140.1554920 | Kernel1/0 completed131.63s; work140 expired during finalization. Root accepted R3 functional assertions only. |
| F-isolation-02 | PASS | 53.1324232 | Corrected-image genuine different-peer C2/fork/account-pin case. |
| F-capacity-01 | FAIL | 34.7554779 | Kernel1/0 completed20.87s and cache finalized; top-level-name matcher rejected. Root accepted assertions only. |
| F-exhausted-origin-01 | PASS | 481.4608671 | Single genuine asymmetric origin. |
| F-exhausted-boundary-01 | FAIL | 120.1641445 | Work120 cutoff; no functional boundary acceptance. |
| F-exhausted-boundary-02 | FAIL | 568.1367185 | Controller560 cutoff waiting B ready-staged; no complete/partial substitute. |
| F-exhausted-boundary-03 | FAIL | 649.5922320 | Functional assertions completed; private preservation failed on open phase writer; parent-result missing. |
| C-boundary-phase-preservation-01 | PASS | 24.1463993 | Actual repaired Phases/helper under two OS locks on opaque bytes. |
| F-exhausted-preservation-01 | PASS | 67.5439277 | Separate preservation-only6956-file stopped backup. |

All exact argv, exit, elapsed, source/image arrays and case/log/result hashes are
in the ledger and final binding's contributing index. Passes for diagnostics are
not upgraded into functional cases. Earlier failures remain charged without
attribution solely to CPU contention. Auto-review rejections before process
creation for boundary02/03 are separate non-contributing records; each actual
boundary invocation ran only once after then-required direct human permission.
The later standing permission delegates necessary local runs/limits to root.

Ready02 has no retroactive cache PASS or exact kernel child-kill trace. Capacity's
matcher was corrected in the runner, with actual log/negative-vector static
validation; no fabricated whole PASS. Boundary01 physical vault sizes were not a
logical-stage/corruption diagnosis. Boundary02 timing intervals combine source
verification/copy/commit and observer costs. None establishes one bottleneck.

## Final source, static checks and justified reuse

The product CLI/control peer remains build11/stage10:
70226432B SHA256 `69744fca50f44439b4734e699f416fc6b2bb8aac4336e5dad18ed173b2abbf6c`.
Build11 actual argv was `cargo build --locked --offline --jobs 12 --profile
verification --package kilogram-cli --bin kilogram-cli`, features empty,28.3766376s.
Stage10 copied to the two fixed compatibility paths only after checking no live
target;2.0246349s. All33 product inputs still equal build11. Cargo.lock/Cargo.toml,
attributes and resource/stable wrappers retain baseline Git blobs.

Final integration harness is14014976B SHA256
`12bb6adda2a96c518785b654369805f6502b6e69998e28b30b1cde052688156e`, at the same stable
`target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe` path.
Its final52 input bindings match compile22 and both last actual C/F runners.
Old c3e04459 controller/83429f6d harness are retained as noncompiled snapshots;
the genuine functional body/check/run/drop was mechanically identical across
the writer repair. Only writer lifetime and narrow C/F/controller guards changed.

| Current static evidence | Actual result |
| --- | --- |
| S-clippy-18 | Strict state+CLI all-targets PASS44.0439025s;13 state inputs still identical. |
| S-clippy-26 | Strict final CLI all-targets PASS4.1847154s. |
| S-compile-22 | Stable wrapper compile-only PASS20.718447s; actual Cargo6.19s, no test EXE started. |
| S-fmt-30 | Final format check PASS4.0984597s. |
| S-boundary03-completion-policy | Parser0; retained genuine/check/run/drop exact normalized source comparison. |

Locked/offline verification and Jobs12/BelowNormal were used. Runtime tests used
the stable wrapper, never direct network `cargo test`; fixed field/harness EXEs
are identified by content hash, not new hash-bearing names. No live image was
overwritten. The final binding lists94 static attempts and12 earlier failures;
their original logs/argv/features/source versions remain, not claims of current
success. Only separately executed static work is outside C/F. Every wrapper,
cached compile/stage/inventory/copy/wait/kill inside a contributing runner is charged.
Read-only toolchain capture: Cargo1.98.0 (797e8a9bc2026-08-05),
rustc1.98.0 (88d9e12ae2026-08-18), PowerShell7.6.5; exact strings are in the binding.

Reuse assessment checks133 baseline files unchanged, the reference window exactly
equal after visibility-only changes, and profile-publication body equal after the
documented core/parameter extraction and whitespace normalization. Root6/component
purpose gates/load/profile/accounting changes received the targeted shared smoke
and capacity/predecessor assertions. S1/S2 allocator/normal history, ratchet/wire
and transport libraries were not changed; no broad100k repeat is claimed.

Build09 differs from final product in12 recorded paths; those hashes are explicit.
Small/partial/policy/atomic/resumed-obligation historical PASS is reused only in its
recorded assertion scope and under the root correction/continuation decisions.
In particular the old full64/head/drain case was not rerun as one final-image whole
scenario. Corrected own-F,128-debt,Ready and isolation cases cover changed behavior
separately. No byte-identical old-build claim is made for those12 paths.

Protected fixed images are independently hashed in the binding: d9b/S3/exact S4,
accepted S5.1 predecessor, archived pre-review/pre-Ready candidates and the own
relay image. Archives retain their actual old versions; they are not replacement
final EXEs. Private fixtures, targets and caches are excluded from publication.

## Genuine exhausted origin and private checkpoint ownership

The single frozen origin is `.tmp/migration-s5-l1/F-exhausted-origin-01`, generated
only by frozen d9b SHA256d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1.
Its actual elected A is two, B one; A511/B16 Text, frontiers A512/17 and B17/512,
complete receipts/debt0/queue0 and exact527 Event/request/UTF8 histories. Producer
case12328B SHA256 `e1acc55360a45c386dde029c2f9be35cbbdacc08bd2a076a1812ac6c692d5135`.
Original raw producer manifest1884557B SHA256
`73c38f113a7e663d00bba36ae0116a292b7ec0511d4e23b79729f60cf87e2995`
covers6944 files/17855269 bytes excluding itself.

Context is `ef689629a47207362fbc870bf610d921c8e28ca0646471914bd93218fff43810`;
public SID is `TflibOtoo5vbulwTkVuDsE6DrSVTFMHdGT2ZHDDx+oE`. The original was
never typed-opened after freezing. Boundary03's before/after raw verification
manifests are identical1883572B SHA256
`cd7f2f4379be0c972083a03fe94346600afea091dbcd371fa3b910d51ba645be`.

The accepted functional path checked original owner/nonce/secrets/Native copy,
zero replacement constructors, no Packet for exhausted new A requests, old exact
retry/body conflict and old frozen cursor suffix; actual B17 advanced its legal
opposite lane, both reopened indexed peers completed fresh Locator/lease/admission
and later Resume, and both histories reached528. The1778B barrier snapshot SHA256
`545f0849be3035bb86521b03bd3dce5fd972c472cce603f27ae5c722647c8b8f`
contains `l1_rpc kind=9 stage=finished millis=581 result=reply9`. This plus source
and phase completion is the accepted evidence; missing original PID/token/status/
history return objects are not reconstructed as a parent-result.

Actual saved checkpoint-info reports cold frontiers[512,18], debt/pending/queued0,
receipts[511,17] and sources530 on both sides. Receipt hash matches:
`338a74bcbee1249cb1655fa21962833d6d680394769861ca2df4ab98475a4760`.
A Native generation517/hash6c50ea198a9d1c9fc6338c873a7bcda7d169c421d32d65fbe5369c3695c2d247;
B generation88/hashe6e4abc55d1216a3514b40261005d6eb29e9fdecbc0312a78bddfc21fccccf1f.
These are legitimate later Native states, not a claim that packet processing
leaves the original encrypted Native bytes unchanged.

| Ownership | Exact local mapping |
| --- | --- |
| Stopped source | `E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb` |
| A pinned profile | Same source `/two/indexed.profile` |
| B pinned profile | Same source `/one/indexed.profile` |
| Private inactive backup | `E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-preserved/F-exhausted-preservation-01` |
| Private metadata manifest | Backup `/preservation-inventory.json`,6765525B SHA256c2a56327fd1462b471163956fbfb61c6b89a07e142f4828c76011c29cadf28b3 |

The separate F whole PASS67.5439277s retained6956 input files/45597399 bytes,
excluding the added manifest. Source-before/source-after/destination tuples match.
Only two exact held-empty-OS-lock source digests are derived from checked zero
length; all other source and all destination digests are actual byte reads.
All22 original public artifacts remained byte-identical before and after. New
result/logs stayed outside the input. The backup was never activated/typed-opened.
Only existing manifest metadata was read for final verification.

Future reuse may restore raw bytes **only to the original pinned absolute paths**,
with one active branch per Native role/session/counter domain. Never retarget a
profile or run source and backup concurrently. Failed01/02 branches, the genuine
origin and other private prior snapshots remain stopped/local. This submission
does not authorize their reopening, cut3 or any additional fixture traversal.

## Accounting and measurement limits

| Set | Prior immutable S5.1 actual | S5.2 actual / ceiling | Combined actual / maximum |
| --- | ---: | ---: | ---: |
| C |279.3257298|172.0310185 /500|451.3567483 /779.3257298|
| F |1236.2886628|3331.2897354 /3650|4567.5783982 /4886.2886628|
| P |0|0 /0|0 /0|

Remaining C327.9689815/F318.7102646. Four explicit F amendments were activated
before use:1800->2100->2400->2900->3650. The last retained the exact pre-amendment
ledger and semantic identity of every earlier case/charge/amendment. No reset,
refund, hidden fixture generation, P borrowing or later-estimate allocation.
Final ledger446512B SHA256
`8acf1e8a11994e839f032e45e28aefcae7ce713f9eac47791fd7599af320b2fa`
contains9 C and30 F contributors with matching actual case records/sums.

As disclosed in `timing-boundary.md`, the runner stopwatch stops before terminal
case/ledger JSON publication. Independent whole watchdogs cover that finalization;
the publication tail is not claimed as measured elapsed. Older cases without job
telemetry keep that limitation rather than acquiring invented cleanup counters.
Measured long migration/default replay times and enlarged correctness waits are
not release throughput/latency benchmarks. Native constructor/Packet counters
exclude persistence restoration and cannot isolate its cost.

## Remaining boundary and maintainability follow-up

Cut3's actual cutA512/first Event2 n512, Q/Selected/Committed, ALPN2 and mixed/fault
handoff remain unrun. The checkpoint's actual opposite frontier is18, not a
manufactured17. Whole P5 needs one separately authorized continuous scenario; no
concatenation of these runs is such a PASS. GUI, independent user onboarding,
groups/channels, multi-device, wider network/NAT/relay field behavior and MVP
readiness are not established by this cut. The previously deferred crafted A2
relay-batch DoS regression is not executed here; no dependency/security-audit
refresh is claimed. Performance optimization is outside this submission.

Observed maintainability follow-up only: main.rs combines substantial command,
orchestration and test code; L1 validation appears in different storage/runtime
paths and needs careful consistency when later changed; long-test finalization
proved vulnerable to observer file lifetime and loss of in-memory result objects.
The per-record writer repair and real C/F preservation checks address this concrete
incident. No new audit, refactor or architecture change was performed or proposed
as a prerequisite here. Root will separately assess whether a small preparation
is needed before S5.3.

Submission stops for root acceptance. Existing accepted evidence/authority history
is not rewritten. The exact candidate, including permitted navigation changes,
awaits review; no implementation acceptance, commit, push or public release is claimed.
