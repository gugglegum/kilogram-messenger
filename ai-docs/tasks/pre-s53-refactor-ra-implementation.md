# R-A: shared Ready1 scope predicate — implementation submission

2026-10-26. Submitted for root implementation review at
`eafd0f873eb31b64377e8e65e06bc84a911f6dc6` (parent
`73c499818ba9b699c331aa842e64e8432e90c59a`). No commit/push yet.
R-B and S5.3 remain unstarted.

## Result and exact code boundary

`ordered_dialog/ready1.rs::scoped_fields` is one private, side-effect-free
implementation of the existing eight-field Ready1 D/E/SID/peer-role/suite/format
predicate. `mod.rs` declares the private module. Exactly two existing production
sites call it: `Engine::ready` and Owner7's `bootstrap::ready_claims`.

The original predicate text is identical. Whole-file comparison proves that
`engine.rs` and `runtime7/bootstrap.rs` each differ from the published baseline
only by that single substitution. Engine still reads both saved events before
the scope predicate and short-circuits its genesis comparison. Owner7 still
validates scope and eagerly parses both claims before reading saved events.
Engine's eager fallback evaluation, prior-Ready witness choice, signatures,
lease/phase checks, expected-body comparisons and persistence are unchanged.
The existing difference for conflicting first genesis plus malformed second
genesis is deliberately preserved.

No wire/storage format, cryptographic primitive, Native lineage/counter,
authorization order, independent storage guard, debt/credit limit or transition
semantics changed. There is no CLI/main.rs extraction or new public API.
[Source comparison](pre-s53-refactor-evidence/S-ra-source-equivalence.json)
records the full production-file equivalence.

The new genuine legacy characterization uses the existing Pair and real
administrative/Native/vault APIs, with explicitly component-only leases. It
checks both roles, 16 signed malformed refusals with unchanged Head/Native/block,
16 exact valid retries, two short-circuit forks, saved peer Ready as first
witness, and durable blocking after cold reopen. It does not claim TLS admission.
The separate F scenario exercises actual authenticated public owner dispatch.

The seven-field negative fixture alone uses the existing debug fixture's exact
CONTROL_SIGN/sign/self-verify/envelope pattern. Eight-field cases still use the
production signer; production signer/parser and `test_support.rs` are unchanged.
[Fixture comparison](pre-s53-refactor-evidence/S-ra-arity-fixture-source-comparison.json)
proves the entire test file changed only at that preparation seam after C03.

## Verification outcome and retained failures

All executable attempts used the stable Windows runner, fixed EXE paths and
Jobs12/BelowNormal, with locked/offline compilation. NativeToken cases were
launched through ordinary `require_escalated` tool review under the standing
human authorization; the case's mode field is a declaration, not token
introspection. There were six attempts, four whole FAIL and two whole PASS.

| Attempt | Actual whole outcome | Recorded time / charge | Kernel and scope |
|---|---|---|---|
| C-ready-scope-01 | FAIL, process exit1 | Draft sample43.6945432s is a lower bound; final actual unknown; conservative C60 charge | Named indexed parser1/0 in0.00s and stable wrapper0. Root accepted only these source-bound assertions. Terminal ledger aggregation then failed. |
| C-ready-legacy-01 | FAIL, exit1 | Measured C10.2520644s | Restricted kernel0/1 in0.02s, identity I/O/os5. Exact syscall was not localized; no Ready success. |
| C-ready-legacy-02 | FAIL, exit1 | Measured C11.3103439s | NativeToken kernel0/1 in0.90s after two enrollments; empty CLOCK_FILE left by the runner caused os3 at `Pair::new -> engine::now`. |
| C-ready-legacy-03 | FAIL, exit1 | Measured C11.9990029s | Kernel0/1 in4.08s. Production signer correctly refused the test's seven-field body before receiver dispatch. |
| C-ready-legacy-04 | **PASS, exit0** | Measured C24.7713883s | Matching kernel1/0 in5.43s; all genuine two-role characterization assertions completed. |
| F-ready-scope-01 | **PASS, exit0** | Measured F148.7417507s | Matching public-dispatch kernel1/0 in131.31s; eight signed malformed controls refused, valid continuation, actual scoped fork, renewal/reopen and saved witness checks. |

C04 and F both have exactly one matching named test and one 1/0 summary, normal
wrapper exit0/completion, no work expiry, exact source/image equality after the
case, and successful owned-child cleanup/query reporting zero children. Their
whole process exits were independently observed as0. No Kilogram process was
left running at submission. A kernel summary alone is not a whole verdict.

The first parser JSON's original optimistic PASS field and the original ledger's
`started`/zero-charge state remain byte-for-byte. The separate
[root-authorized reconciliation](pre-s53-refactor-evidence/C-ready-scope-01-reconciliation.json)
identifies the actual whole FAIL and conservative C60 basis. The other failed
case/log/start/input records and r2-r4 ledgers/bindings are also preserved.
No failed attempt was renamed or replayed under its old case ID.

Verification support was corrected under explicit root review:

- PSCustomObject records retain one reference in the ledger; explicit validated
  scalar aggregation includes dictionary/round-tripped rows. The accepted small
  in-memory check covers shape, reference updates and invalid durations.
- Late work expiry fails closed; origin equality is set only after comparison;
  observed kernel counts survive a wrapper/finalization failure; unknown cleanup
  remains unknown. Terminal failures block subsequent cases even with a stale
  PASS in the ledger. The original owned-job watchdog is unchanged.
- The temporary process-environment helper distinguishes absent, present empty
  and nonempty values. Actual absence uses the Env provider, and restore retains
  the original state. Its actual helper passed the root-authorized diagnostic in
  a disposable process; no persistent/User/Machine environment was changed.
- The static adapter pins native Cargo cwd and preserves primary execution and
  final-inventory errors separately. The initial fmt attempt really ran from
  the explicit workspace cwd and is retained as originally recorded.

## Static checks and image binding

| Static attempt | Result | Seconds |
|---|---|---:|
| S-ra-fmt-01 | PASS, initial formatting | 4.0903995 |
| S-ra-compile-01 | PASS, compile-only | 58.6811798 |
| S-ra-build-01 | PASS, fixed CLI/control staging | 34.4834859 |
| S-ra-clippy-01 | PASS, CLI all-targets `-- -D warnings` | 20.9448416 |
| S-ra-fmt-check-01 | PASS | 4.3194128 |
| S-ra-fixture-fmt-check-01 | PASS, corrected test | 4.5152911 |
| S-ra-fixture-clippy-01 | PASS, corrected test / strict CLI | 8.2713950 |
| S-ra-fixture-compile-01 | PASS, compile-only last | 30.6887331 |

The last test-only change required fresh strict Clippy/fmt/compile gates. It
changed only the library test image; the other three staged harnesses and fixed
product/control/frozen images retained their contents. The product CLI was not
rebuilt for a cfg(test)-only change. Later case-policy/budget edits affect only
the execution adapter and are separately bound; Rust/build inputs remain exact.

| Fixed image | Bytes | SHA256 |
|---|---:|---|
| `target/compatibility/long-lived-dialog-migration/kilogram-cli.exe` and `kilogram-l1-control-peer.exe` | 69796864 each | `a0270e2ba163393b1c66655ef5ff6d64965c1365c39fd8c8c7d859b6c7ae72f3` |
| `target/verification/deps/kilogram-cli-lib-kilogram_cli-tests.exe` | 42982912 | `d699b3181b9bf5f2496242a13256c23badded4fbdb6bad76356a747952ca3585` |
| `target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe` | 14014976 | `4964258679d0b45057200677ce48e2631e7d09d22ad4e921261828354fd58d15` |

Old executed adapters and replaced harness contents were retained as raw,
non-executed snapshots before edits/staging. Actual attempt source/image
inventories are persistent before harness execution. Final r5 binding is
175263 bytes, SHA256
`15c3c29b8b9f8e84863f0e926405e8a16b94e850d85e5ba9a97d7ee42797d4f1`.
All820 inputs and7 images still matched after F. The mutable ledger is excluded
from frozen inputs. All eight static attempts remain separately recorded;
cached compilation/staging and finalization inside C/F remain charged there.

## Provenance, reuse and accounting

F used a fresh copy of the accepted genuine small4/4 origin
`.tmp/migration-s5-l1/F-small-origin-01`. Its result matched the published producer
log and frozen d9b image identity. Within the charged F attempt, actual before
and after inventories of197 original files were equal. The original was not
typed-opened. This is a current raw integrity comparison, not an invented
producer-time inventory. All copy, migration, waits, renewal, reopen and cleanup
are included in F's whole runner. The test's detailed parent JSON remains in its
stopped private run at `.tmp/migration-s5-storage/l1-review1-ready-g8oviW`;
the submitted log/whole record bind the completed named test to its source/image.

Final charge is **C118.3327995 / F148.7417507 / P0**. C comprises conservative60
plus measured58.3327995; it is not an entirely measured elapsed total. Root
explicitly revised R-A C120 to C150 before C04 by adding C30 for normal successful
finalization. Prior charges and old ceilings remain unchanged in their historical
records. Current remaining allocation is C31.6672005/F61.2582493/P0; it is unused.
The separate R-B C60 reserve remains inactive, making the revised overall
refactor ceiling including that reserve C210/F210/P0.

The correction/clearance chain is in [plan review](pre-s53-refactor-plan-review.md),
[runner continuation](pre-s53-refactor-ra-runner-continuation.md),
[Native comparison](pre-s53-refactor-ra-native-continuation.md),
[environment correction](pre-s53-refactor-ra-environment-continuation.md) and
[fixture/budget decision](pre-s53-refactor-ra-fixture-continuation.md).

[Final reuse assessment](pre-s53-refactor-evidence/S-ra-final-reuse-assessment.json)
records seven changed source paths and748 unchanged source blobs,820/7 final
binding checks with zero mismatches,516 immutable published S5.2 non-scope
candidate files with zero raw mismatches, all six attempt outcomes and charges.
R1 own-F, R2 debt, storage/atomicity/credits/history/isolation and S5.1 evidence
retain their original unchanged-branch/source/image scopes. They are not
relabeled as fresh aggregate coverage. No bulk100k,511 generation or whole-P run
was performed. S5.2's previous ledger/failed and split outcomes are unchanged.

## Submission boundary

The candidate allowlist includes only the five scoped Rust files, two verification
adapters, current assignment/plan/review/continuation chain, this report and new
refactor evidence. It excludes the queued S5.3 task (unchanged SHA256
`75cbc0f9950971820beffb08edbd7c6ccb4465d8be44aea331c51687f1807f7c`),
private images/data, target outputs and unrelated paths. Index remains empty.

No R-A action traversed, opened, restored or activated the exhausted NCjBHb
checkpoint or its private backup. No R-B durable-assertion seam, S5.3 handoff,
Event2 continuation, public rollout or MVP acceptance is claimed. Stop at root
implementation review; publish only after explicit acceptance of the exact diff.
