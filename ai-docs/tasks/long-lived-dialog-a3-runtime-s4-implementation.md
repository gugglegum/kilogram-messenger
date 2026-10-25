# A3-S4 implementation review candidate

2026-10-25. Fresh S4 production path and selected verification are complete for
root implementation review. **Root acceptance and publication are pending.**
No implementation commit/push has been made. HEAD remains
`9547ed171c64bf5ea529b4ba487acf6aa1a61b5b`, sole parent/S3
`7f82036ee5803edd64be98b73572604250f5961c`; index is empty.

The final complete P passed in **2003.5355583s runner /1979.794s controller**, below
the original2400s target. Review4 authorized controller2950/runner3000 after the
accepted relay-helper repair; this successful run did not need the time above2400.
This is one complete fresh-fixture run, without stitching earlier prefixes/suffixes.
It establishes this measured scenario, not general throughput or whole-A3 readiness.

Normative inputs: [contract](long-lived-dialog-a3-runtime-s4-contract.md),
[contract acceptance](long-lived-dialog-a3-runtime-s4-contract-review-2.md),
[implementation task](long-lived-dialog-a3-runtime-s4-implementation-task.md),
[time accounting](long-lived-dialog-a3-runtime-s4-time-accounting-clarification-1.md),
and implementation reviews [1](long-lived-dialog-a3-runtime-s4-implementation-review-1.md),
[2](long-lived-dialog-a3-runtime-s4-implementation-review-2.md),
[3](long-lived-dialog-a3-runtime-s4-implementation-review-3.md),
[4](long-lived-dialog-a3-runtime-s4-implementation-review-4.md).
All frozen contract/review/evidence inputs and S1/S2 databases are preserved; the
old databases were not opened. Root-owned documentation is separately referenced.

## Production behavior and boundaries

`ordered2_cli.rs` supplies ordinary enrollment/context/consent, typed public profile
and authenticated IPC dispatch. Main remains dispatch. Unknown JSON, saved Device/
context mismatch and unenrolled operations fail before Native restoration. Profile
JSON cannot activate a session. Explicit committed-enrollment resume reuses its keys.

RuntimeRoot6 has prefix06, `KGOD2RT\0`, layout1 and the unchanged schema5 envelope.
Its first committed root contains the required authenticated marker/saved Device.
ComponentRoot5 encoding remains unchanged. Purpose/namespace/profile/root guards
close generic, component, history, protocol and retry writer bypasses. V4 adoption
and S5 fail closed; existing V1 history remains supported through its previous path.

One closed indexed commit contains Head/Ready/Native/history/credits/runtime
metadata. RTM2 holds actual encrypted bytes and authenticated prepaid bounded
capacity. A clean explicit policy abort leaves the handle reusable; IO/abort
ambiguity poisons it until reopen, retaining the directory lock and resolving
lastCommitId without replay of a committed Native transition.

A dedicated owner thread with mailbox16 alternates C turns and stages at most4 real
Native packet trials. A bounded Seen/parent overlay supports dependencies inside a
batch. Native/guard/cache state swaps only after the entire closed commit. A fourth
trial OperationalFailure rolls back the whole batch; invalid MAC quarantines that
C, preserving its readable prefix. A signed same-ordinal fork blocks only its C.

NativeState2 uses public adapter APIs, self-HPKE and exact scoped AAD. One reserved
OTK, Q before Init, atomic B A0/Confirm and durable both-Ready activation precede
Text. Historical proof/Q remain immutable. Current Root2/consent, fresh provider/
time/high-water and actual connection lease are independently checked, including
a final post-staging check immediately before commit. Bounded exact-byte caches
retain current checks. Empty owner turns use poison-checked authenticated counters;
nonempty work still validates its installed snapshot, phase, permissions and lease.

Exact request retry precedes new permission/capacity checks. Queue/source/debt
survive process loss, lease loss and release of RAM refs. Durable opposite receipt
alone produces Delivered. IPC history budgets48KiB of actual escaped JSON before
consuming the next candidate cursor; the escaped-text regression verifies no skip.

Minimal clears default discovery/IP/relays before explicit operator configuration.
All IP-enabled policies require loopback binds, including Auto+explicit relay.
RelayOnly publishes signed URL-only locators with empty IPs. Real route, endpoint,
TLS exporter/ALPN/Hello/proof/locator bind each lease. Every boot commits a newer
locator revision for a fresh endpoint before export; same-live export retry is exact.

Bounds remain:2 connections acquired before TLS/spawn, TLS15s/admission30s absolute/
round15s/lease<=300s; bidi4/uni0, receive4MiB, frame1MiB; mailbox16; RAM refs128 with
64/64 across two C; sends/normal repair pages<=16 with separately funded head; Native
<=128KiB; composite commit accounting<=192MiB including32MiB arena; observed process
private bytes<=384MiB. Disk debt may exceed128. Debug hooks schedule real operations
and faults without substituting packet results, counters, rows or leases.

## Final verification sets

All current builds/tests use verification/locked/offline/Jobs12/BelowNormal. Windows
harnesses compile without running and execute only through the stable runner. Native
Windows-token runs are distinct from recorded sandbox loopback10013 failures. No
release/scale override, heavy100k history or long S3 corpus was used. Fixtures are
fresh, isolated under `.tmp/a3-s4`; fixed application paths contain no revision name.

| Set | Exact final contributors | Runner seconds / ceiling | Result and source evidence |
|---|---|---:|---|
| P | `p-full-review4-relay-repair-1` |2003.5355583 /2400 original;3000 authorized ceiling| PASS,1 complete scenario; `p-full-review4-relay-repair-1-source.json` |
| F | `f-owner-eligibility-r1-guards-1`, `controller-schedule-lock-controls-1`, `f09-controller-repair-1`, `f-full-review3-frozen-1` |620.4258161 /1200| PASS;185.5564679+22.3189717+35.3227291+377.2276474;19 full cases plus17 supporting executions |
| Components/regressions |10 exact selectors in `components-aggregate.json` and evidence README|370.2900657 /600| PASS;109 observed executions, including3 NOT_SELECTED scale sentinels without their corpus |
| Strict Clippy | `final-review4-clippy-1`, six packages/all-targets/`-D warnings` |4.0943831 /separate| PASS; final P/Clippy/current52 Rust files and2 PS files identical |
| Static | `static-final-review4-2` |4.9456385 /separate| PASS rustfmt,2 PS parsers,9 Node syntax checks, diff, source/image/Cargo/HEAD/index freeze |

[Final suite summary](long-lived-dialog-a3-runtime-s4-evidence/final-suite-summary.json),
[complete source correspondence](long-lived-dialog-a3-runtime-s4-evidence/final-source-correspondence.json)
and the [development ledger](long-lived-dialog-a3-runtime-s4-evidence/development-ledger.json)
contain every contributor's exit code, elapsed time, source/log/result hashes and
stable harness EXE hashes. Sums include runner overhead; parallel wall time does not
replace them. Development failures remain separate. Static/build costs are separate.

### Whole P evidence

Fresh fixture `P-i8jQD9`; no failed fixture was resumed. Controller checks passed:
640 one-way A Text with kill256/graceful512,20 duplex A2/B32 DH rounds, at least16 DH
generations each, and the actual delayed B7 tail after A8 without changing A8 own
chain/rotation. B was really stopped while A materialized256 offline Text; A then
restarted offline with the same Native hash. B returned with64 ordinary pending
packets plus a separately funded lowest head. One actually held64-reference C1 window allowed C2
32/side to converge while C1 still retained64 pending and256 debt. B was killed
following normal pages2 and aggregate5, a nonempty ACK was lost, and A restarted.

After direct catch-up, already-materialized B640 remained held as the only B debt.
Both processes restarted RelayOnly against own loopback relay
`http://127.0.0.1:41917/`; signed locators had no IPs. Importing latest locators
continued the exact B-only event
`3c153d0e15218b28cc612e924884cd722783a856a91eace45126b79001fc5931`
to Delivered without a new A send/open. Both processes then restarted again before
the exact history and old-cursor checks. Initial selection/proof/Readies stayed exact.

Final C1 A936/B640 Text (1578 sources including2 genesis per peer), C2 A32/B32
(66 sources including2 genesis per peer); both peers have receipts936/640 and32/32,
queued/pending/debt0 in both C. Total1640 Text+4 genesis, **3288 committed calls and
3288 attempts,0 aborted difference**,4 Account/4 Session constructors. There were
24 bilateral binding hashes,63 normal C1 pages capped16 and1 actual full64 funded
head. Attempt/commit counters are kept separate even though they coincide here.

History measurements are separate from overall scenario time:

| Read | Rows / pages | Seconds |
|---|---:|---:|
| A C1 before duplex, cursor saved |640 /40|6.065|
| Final A C1 |1576 /99|20.162|
| Final B C1 |1576 /99|24.512|
| Final A C2 |64 /4|0.716|
| Final B C2 |64 /4|0.452|
| Saved old A cursor suffix |624 /39|7.584|

Text/request/event IDs, exact UTF8, opposite histories and absence of duplicates are
asserted. P02 elapsed checkpoints are226.858/498.912/638.034s;20 duplex rounds end at
1377.750s; C2 convergence while C1 holds64 is1555.026s. Raw activation waits, per-range
queue progress and catch-up waits are retained separately. The final logged catch-up
sample is192.305s with16 remaining receipts; convergence occurs before its unchanged
240s local deadline. That log does not emit an exact final catch-up completion time,
and activation wait alone is not an all-inclusive enrollment setup measurement.

Closed-writer p50/p95 milliseconds: Queue18.422/21.497; Materialize96.027/120.862;
Drain109.672/145.875; Acknowledge47.037/56.612; AckProgress289.946/417.464. These elapsed
repository/gate timers exclude preceding Native trials; cross-process sums are not
wall time or CPU attribution. There were311 actual private-memory samples, maximum
36536320 bytes. Per-boot status guards check the application/Native/connection/mailbox
bounds; final restarted-boot stats do not claim lifetime maxima for earlier boots.

[Full P metrics](long-lived-dialog-a3-runtime-s4-evidence/review4-full-P-metrics.json)
SHA256 `c5eb48c35d18a0f21bd79e63518bc4605db605d6cb7b08071dc344a02ceff48f`.
Direct and relay public captures are separate; relay capture waits for the actual
P08 delivery assertion. Native CIM exact fixed-path inventory after completion is0
owned processes (`review4-full-P-cleanup.json`).

### F01–F12 correspondence

Full `f-full-review3-frozen-1` has5 actual TLS/owner cases,9 process cases,4 state
fences and1 transport route table. Its19 cases passed in377.2276474s.

| Gate | Exercised behavior |
|---|---|
| F01 |14 policy/lease/generation cases: own/peer revocation, lower/equal-changed Root, consent validity, failed/backward clock/provider,2 post-staging expiry/close aborts, stale captured generation, actual expired signed-locator lease and prior connection generation. Successful abort reusable; protected records unchanged. |
| F02 | Same-cert renewal retains old Q/proof; old Offer requires explicit historical support after current renewal, before constructor; wrong Q rejected at genesis/Text/Ready/receipt boundaries. |
| F03–F06 |11 actual parent kills across Offer/Grant/A0/atomic B A0-Confirm/Ready/Active/queue/materialize/pending/readable/receipt boundaries, plus2 enrollment kills and ordinary offline kill. Same request/reopen/native constructors; fence-only CreationConflict versus explicit committed resume. |
| F07 |5 backend fault schedules, actual prewrite/aftersync materialization ambiguity, ACK cursor IO, configured quota/capacity refusal then increase/reopen/retry. Physical-full-volume behavior is not claimed. |
| F08 |8 bypass/version categories across process/state/actor tests; actual d9b L1 fixture with4 Text and V1 history; old ALPN/existing-origin/unknown JSON or pins/component adoption refused. |
| F09 | Fourth real Native trial fails and whole4-packet batch rolls back; old prefix readable/no ACK/C2 progress/retry. Signed fork/pre-native Q/invalid MAC after prefix separately exercised. |
| F10 | Future layout/magic/revision/suffix/marker/internal purpose/retry negatives; exact7f82036 predecessor with correct owner/config and old/current ComponentRoot5 controls; Runtime6 old-writer refusal before mutation. |
| F11 |2 actual post-locator-commit kills, distinct ports/endpoints and consecutive revisions; third boot progresses existing B-only debt after latest import; lower/equal-changed/expired rejection and same-live exact export retry. |
| F12 |2 actual pre-Hello TLS occupants, third refused before spawn; bidi4/uni0, close/deadline release and fresh admission; real endpoint mismatch, Minimal/no defaults/URL-only routes; R1 Auto+relay loopback positive and wildcard/LAN/global/link-local negative table. |

Full F/controls/targeted F09 shared exact source snapshot
`9b785ac1ca3be8e7478f9f354425f748b95a81889dd0aa4a0f97c6fb5b05d665`
and application CLI d5d8c8. The later relay example/controller changes are outside
exercised F functions and shared `positive::hit/selected`; review4 explicitly reuses
these results. `review4-relay-source-correspondence.json` records the exact boundary.

Component reuse is scoped in `component-source-reuse.json`: protocol/IPC/state shared
codecs/readers remain exact. S3/history runs predate the S4-only snapshot read-tx
change in runtime2.rs; they exercise ComponentRoot5/history, while current foundation,
component and complete F checks cover that S4 snapshot. Legacy CLI S3 tests/engine
match HEAD. Escaped-history opcode4/body and state reader are exact. This is explicit
source-scoped reuse, not a claim that all historical snapshots are identical.

## Accepted narrow repairs and preserved failures

Review1/2 accepted R1 loopback enforcement and a measured removal of repeated owner
installed-record reads. The two separately authorized diagnostic runs took
119.4238763/122.8123687s. They observed17874 reads/21.0316s removed, data CPU
88.34375→70.03125s (-20.7%), wall58.240759→57.562526s (-1.2%); no general wall-speed
claim follows. Components370.2900657s plus diagnostics242.236245s=612.5263107s is
visible and is not claimed to fit the600s acceptance component ceiling. Exact patch
SHA256 `0632b53c1a0caccd3072a1e1d58a94105eda0ceccd36589e2fa112434f0ccd0d`.

Review3 accepted test-controller atomic persist diagnostics and at most500ms retry
only for real Windows raw5/32/33 persist failures, retaining the same synced temporary
file, without destination deletion/truncation or ACL/attribute changes. Real deny-
delete lock controls passed: released lock2 attempts/21092us with old/new exact;
persistent lock returns os5 at505717us preserving old bytes. Targeted F09 then passed.
The historical F09 unadorned os5 syscall/cause remains unclassified. Patch SHA256
`8e66c7bb3f8c2b96ed482957878f590adf8a5311e95cc1490ac9de38d8102f6f`.

Review4 accepted helper-only writable-handle sync to a fresh sibling followed by
atomic rename, plus contextual bounded16KiB stderr/liveness checks. Old helper
standalone startup reproduced ready-before-exit1/os5 in0.0804791s. The original
historical stderr alone did not identify its syscall. Focused real own-relay smoke
passed50.2735936s runner/24.181521s controller within its separate180s investigation
bound, proving exact B-only traffic and clean children. It is not a suffix of full P.
Exact14128-byte patch SHA256
`e399233bec2f5a5914c037cf51396bb2cd554c23dbee6e57cdff0da5ed4c858c`.
Adjacent unchanged legacy OD1 read-only pause-file sync hooks remain outside this work.

Original per-command logs/results/sources and failed aggregate metrics are immutable.
The machine-readable ledger includes all recorded run-check commands and cumulative
runner costs; additional static artifacts are referenced separately. This table
makes the significant development attempts visible:

| Development attempt | Runner seconds | Result/limitation |
|---|---:|---|
| p-process-1 /2 |343.1840845 /338.8101823| Earlier source, early local240s FAIL |
| burst-1 |192.9333079|128-message diagnostic PASS, not full P |
| p-full-final-1 |585.5830845| FAIL following256 with batch1/local240s |
| p-full-final-2-batch4 |317.1270524| FAIL first256/local240s with advancing debt |
| p-full-final-3-idle-read |643.0040112| Controlled stop from erroneous historical-budget subtraction; exact owned cleanup retained; not whole P PASS |
| p-full-final-4-contract-budget |2360.3300036| FAIL2350s controller deadline,3265 committed calls; relay/final history not run |
| f-full-final-1 |392.2260049| Earlier F11 expected-text/F08 PowerShell compatibility failures |
| f-final-f08-f11-state-supplement-1 /2 |54.0954109 /86.3850198| First readiness/OEM failure; second earlier PASS superseded by full F |
| f-full-final-2-batch4 + idle-read supplement |576.9843874 total| Historical composition, superseded by complete current F |
| f-full-review2-frozen-1 |365.2830575| F09 Windows os5 FAIL;5 actor+8 process PASS; state/transport not executed |
| p-full-review2-exception-1 |1901.2816169| Consumed exception FAIL: helper os5, first RelayOnly registration timeout15s;3287 calls, final relay/history gates absent |
| regressions-protocol-ipc-final-1 |164.9958355| Controlled stop of accidentally broad179-test selector; no current P interference |
| static-final-review4-1 |3.0297381 tool wall| Invocation FAIL before checks: friendly wrapper does not accept fmt; corrected by ordinary cargo fmt in separate preserved run |

Early policy/kills PASS runs exposed a later Windows hook sync defect and are not
acceptance contributors. The prior review2 relay-public-artifacts file sampled a
configuration transition alongside older direct locators; it is preserved as an
invalid URL-only proof. The current watcher waits for actual P08. No failed or
controlled-stop run is relabelled PASS, deleted or merged into the current whole P.

## Fixed images, source manifest and review stop

| Fixed path under target/compatibility | Bytes | SHA256 |
|---|---:|---|
| long-lived-dialog-runtime/kilogram-cli.exe |66188800|d5d8c8c118925dfde67e37fdb32aa5a120293cde258e93843dd1939fddb15560|
| long-lived-dialog-runtime/kilogram-relay.exe |17057280|b8723b8cb546a0ceccf3179415d580b073f0a36d2f9a458ecfd15c66bf2c6317|
| long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe |5344768|6932db6bc228cdd38fc358792cc5cd41fcfd1e5445c087a7c0f535ce4f0c1e4e|
| long-lived-dialog-baseline/kilogram-cli.exe |61531648|d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1|

The original mechanical42-file classifier includes helper/probe and omits two in-src
test modules. Only its explicitly authorized relay helper differs from review2.
Independently all42 Rust files under apps/crates src remain exact; current whole P
and final Clippy/current tree match all52 Rust files. Cargo inputs remain unchanged.

[Evidence README](long-lived-dialog-a3-runtime-s4-evidence/README.md) gives exact
selectors, replay/build commands, profile/priority, predecessor archive provenance,
fixed paths and no-overwrite rules. `protected-inputs-final.json` records frozen
inputs/HEAD/parent/index. Development environment remains clean, unchanged SHA256
`53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f` and excluded.

[implementation-files.json](long-lived-dialog-a3-runtime-s4-evidence/implementation-files.json)
is the exact executor candidate allowlist with raw bytes/SHA256 and normalized Git
blob IDs; it excludes its recursive self hash, supplied separately in callback.
`root-documents-reference.json` lists root README/coordination/task/clarification/
reviews separately for the eventual accepted publication list. No file is staged.
No vault, IPC token, Root/Device key, private checkpoint or EXE enters deliverables.

The requested next action is root's final source/evidence/allowlist review. Stop here:
no commit/push until its separate explicit acceptance and publication instruction.
S5/migration, whole-A3/MVP, GUI/groups/multi-device/mailbox/M1.2.63 and deferred A2
crafted DoS remain outside this result.
