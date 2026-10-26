# A3-S5.2 review1 corrections — static/C submission and proposed field plan

2026-10-26. Baseline73c499818ba9b699c331aa842e64e8432e90c59a. Responds to the
unchanged root implementation-review-1, SHA256
49f8c28e0fd7a46e46df8f93622ac366362a7d48340eb2cd14c6e6e5b4e7fd49.
This is a correction submission, not implementation acceptance or F permission.

## Concrete corrections

R1: runtime7 readable_row captures actual current own credentials as F for own
genesis and Text, independently of the original O. Existing A and initial R/receipt
nonapplicability remains. The closed operation bound requires F; closed lowering
and both full-proof/history readers enforce applicable captures for own and peer
rows. Identical auth bundles deduplicate while O/F remain separate links. Imported
immutable LegacySnapshot handling is unchanged. Old candidate fixtures are neither
rewritten nor relabeled and will not supply positive corrected-field origins.

R2: before session restore/fork, materialization computes
min(4, queued,128-current-C-debt), returning without packet work at128. A persisted
count above128 fails closed. Closed materialization independently rejects a batch
that would exceed128; the own Text debt insertion also guards its final count.
Lifetime512, queue128, actual receipt retirement, funding and C shares are unchanged.

R3: ready validates arity, D/E/SID, peer role, suite/format and fixed32 genesis IDs
before classifying a same-scope contradictory genesis. Malformed controls refuse
without Block. A real conflict uses saved peer Ready as the first witness, and
only falls back to the corresponding saved genesis if no peer Ready exists.
The scope parser is the actual helper used by Owner7.ready, not a duplicate test
implementation. Public owner/dispatch integration remains a proposed F check.

Debug-only transmission/control preparation supports the required regressions:
an actual peer can re-sign a malformed Ready without changing stored controls;
materialization can be held before Native; ACK queries can be held/capped to one;
the existing durable checkpoint mechanism can pause after one real receipt commit.
These require the existing external debug scheduler and do not add a generic IPC
signer, private-state writer or synthetic positive origin.

## Exact source/image and static/C evidence

The evidence directory is long-lived-dialog-a3-migration-s5-l1-evidence/.
S-review1-source-delta.json maps build09 input hashes to corrected build10 hashes;
SHA2563f5c5c85718c7ef2007da9c3d896e12bf2011e98c93f63d5d08a3aa23388aaca.
The root review is bound as a new protected input in S-review1-input-binding.json.

- Clippy17 failed on test-only method visibility and is retained. Tests were
  moved into their actual closed-lowering module and use the closed-operation
  bound; production visibility was not widened for access.
- fmt18, strict locked/offline Clippy18/all-targets, build10 and compile15 PASS.
  Separate static work remains outside C/F. Jobs12/BelowNormal; no direct network
  cargo test execution.
- Stage09 fixed CLI and control peer:70222848 bytes, SHA256
  a3ff9d2ee67e5b01db8f4c922247737d0dee7bb4c639477be3e02d4999b13c56.
  No live image was recopied. Rejected build09 is preserved at the stable
  target/compatibility/long-lived-dialog-migration-prereview/kilogram-cli.exe,
  SHA2564f4c240f10e90f86aaae5bf087bb4056280e7da762d52c68ebfeb24432542d98.
  Frozen d9b/S3/S4/relay and saved accepted S5.1 predecessor were not changed.
- C-review1-01 PASS20.9155434s, actual CLI2/state3 matching tests. Whole120/work100
  guard did not expire, owned children after cleanup0. Case SHA256
  d2c60176228326ae4ade5ceff84e5312e4211a2723d4d5498589d5848829e543.
  Checks cover eligibility127+queue4,128+queue3, corrupted above-limit count;
  malformed/valid-scope Ready classification inputs for both roles; capture
  applicability/missing F; and actual closed materialization lowering refusing
  over-limit negative inputs without changing its root/overlay or durable store.

C envelope/applicability and synthetic negative summary samples are component
evidence, not genuine Native/Event integration. No new F has run since the review.
All historical attempts, including their old candidate limitations, are retained.

## Concrete remaining F proposal — not yet authorized

Serial runs, one active isolated copy per Native domain, full contributing elapsed.
No old candidate directory with missing own F is repaired or reopened as a
corrected positive fixture. Sources and images freeze before each case.

| Group / selector | Genuine lineage and exact new assertion | Whole forecast upper seconds |
|---|---|---:|
| migration_s5_l1_review1_genuine_partial_own_f_both_genesis_roles | Two new actual frozen d9b partials via the real control peer: Offered A after-grant and Granted B before-init-commit. Stop/preserve; legal public authority advance/update before migration; original Native retained. Actual own genesis Oold/Fcurrent and ordinary own Text F, legal one absent Session constructor only. |120|
| migration_s5_l1_review1_unacked128_before_native_and_one_exact_receipt | Single fresh active copy of the immutable genuine4/4 seed, both migrated. Default native path creates127 real debts with actual receipt queries held; hold materialization to accept4 requests, release to exactly128/queue3. Another turn/cold reopen invoke no extra packets. One actual signed stored peer receipt, queried singly and paused after commit, frees exactly one additional materialization, leaving queue2/debt128. |240|
| migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork | Fresh genuine4/4 branch, both indexed. Actual authorized sender re-signs eight malformed Ready bodies; receiver public dispatch refuses with unchanged block/Native; real valid exchange succeeds afterwards. Actual same-scope contradictory genesis produces fork with prior Ready witness, survives public Consent1 renewal and reopen. |160|
| migration_s5_l1_different_peer_c2_survives_signed_fork_and_account_pin | Fresh actual three-device frozen origin, C1 two genuine Text rows, C2 independent peer; repeat on corrected image rather than relabel historical isolation PASS. |100|
| migration_s5_process_capacity_crash_and_predecessor_fences | Fresh isolated genuine small source; affected S5.1 capacity CAS/crash/recovery and exact S3/S4 predecessor controls. Currently unrun. |45|
| One frozen d9b A511/B16 original | Exactly F-exhausted-origin-01 with source/producer/raw inventory, work850/whole900 guard and separate root readiness go-ahead. No generation has run. |850|
| Exhausted L1 boundary | Refusal/retry/history/receive/ACK/reopen plus stopped indexed checkpoint at its actual absolute paths. Controller still being prepared; no handoff or first Event2. |140|
| Reserved contingency before first long entry | Unspent reserve; minimum remaining entry1215 remains unchanged. |225|

New field controllers are in tests/support/migration_s5_l1_review1.rs and compile15.
The R2 proposed100s wait for the actual127-debt workload and case upper bounds
are controller planning allowances requiring root review, not measured results or
product timeout changes. R3 ordinary waits remain25s. The longer existing70s
obligations controller remains scoped to its two previously accepted workloads.

Current immutable charged arithmetic: C126.2916571/F979.0484649/P0. C remains
inside500. Remaining F under2400 is1420.9515351. The conservative full forecast is
979.0484649 +120+240+160+100+45+850+140+225 =2859.0484649. It therefore needs at
least459.0484649 additional F seconds; propose F2900 (+500) with40.9515351 slack.
Combined maxima would be C779.3257298/F4136.2886628/P0. This is a requested
allocation, not an applied ceiling: live ledger remains F2400. No P borrowing.

## Affected-check assessment and pending boundary

R1 changes every new own readable/full-proof path; R2 changes every materialize
entry; R3 changes Ready replay/rejection. Prior functional PASS remains exact
historical evidence on its recorded source/image. The new genuine field groups
must establish their corrected behavior. Existing common Root6/Component5 gates,
per-C window/lease invalidation implementation, imported LegacySnapshot algorithm
and S5.1 capacity algorithms were not edited by these fixes; source comparison
can support scoped reuse, with the named affected capacity check still required.
An additional full obligations repeat is not requested solely to erase old failures;
its own F omission is addressed by distinguishing corrected-field assertions.

Before first511: finish the boundary controller, source/image binding and narrow
readiness package; root independently checks the whole900 mechanism and remaining
>=1215, then gives the explicit first-long approval. Current source preparation
includes asymmetric driver rejection/retry checks and stopped-origin provenance;
it has only been parsed, not used to generate511. No acceptance/publication/cut3
or whole-P permission is implied by this correction submission.
