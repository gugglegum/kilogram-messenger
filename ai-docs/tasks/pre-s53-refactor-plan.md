# Pre-S5.3 assessment and bounded refactor proposal

2026-10-26. **Plan submitted for root technical review; implementation and its
verification are not started.** Authority: [pre-s53-refactor-task.md](pre-s53-refactor-task.md),
6948 bytes, SHA256 `aebdb762365ed25b03438794c2d9e4c3ac2c3498f8fee02ef78db1fdfb16435b`.

## 1. Baseline, recommendation and limits

Published baseline is `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`, sole parent
`73c499818ba9b699c331aa842e64e8432e90c59a`. Executor post-push `ls-remote`,
HEAD and origin/master agreed; root independently verified GitHub and the exact
520 accepted paths/blobs. The accepted S5.2 report, candidate, decisions, evidence,
failed attempts and private checkpoints remain immutable.

Recommend two separate small changes, each reviewed and published independently:

1. Share **only the pure Ready1 shape/scope/role/suite predicate** between Engine
   and Owner7. Keep their surrounding algorithms and genesis parsing distinct.
2. Persist detailed completed assertions **before** a fallible preservation tail
   in the test controller, using a small test-only helper that future S5.3 tests
   can also use. Preserve the independent whole-attempt outcome.

Do not extract the entire CLI or merge storage runtimes before S5.3. The observed
problems justify these seams; they do not establish that the whole architecture
or Rust's module/ownership approach needs replacement.

This assessment read source and published documents/evidence and calculated file
hashes only. No product/test/script edits, builds, executable tests, fixture walks,
DB opens, state copies or C/F/P contributions were performed. Only this plan was
created. The queued S5.3 task remains unchanged, SHA256
`75cbc0f9950971820beffb08edbd7c6ccb4465d8be44aea331c51687f1807f7c`;
the new assignment overrides its immediate activation order.

## 2. Findings ranked by benefit and verification cost

| Priority | Established observation | Decision and limits |
|---|---|---|
| 1 | `engine.rs:839-848` and `runtime7/bootstrap.rs:610-618` independently implement the same Ready1 arity/D/E/SID/peer-role/suite/format predicate. R3 demonstrated that omitting this boundary in a new runtime could turn an ordinary refusal into durable fork blocking. | Select R-A below: one private predicate, two existing callers. No shared persistence or fork engine. |
| 2 | `migration_s5_l1_exhausted.rs:1229-1251` runs private preservation before serializing detailed parent observations. Boundary03 completed its functional assertions, then lost its RAM-only parent result on the preservation error. Current `Phases` already closes each writer before the next action (`:43-54`). | Select R-B below: durable assertion snapshot before the tail. The file-handle bug is already fixed; do not redo it or claim to reconstruct the lost result. |
| 3 | `main.rs` has 43675 lines; the principal test module begins at35750. It still combines command types, launch/lock routing, generic runtime/IPC, recovery, mailbox and compatibility orchestration. | Real maintenance debt, but broad extraction has a large caller/review surface. Ordered administration and IPC are already separate; defer global movement. |
| 4 | R1 spans current capture construction, closed lowering and proof reading. R2 spans runtime eligibility before Native and a separate storage limit. These are different trust boundaries, not interchangeable implementations. | Keep the independent guards. Their removal would reduce protection. No new common runtime/storage permission layer. |
| 5 | Wrapper/finalization time exceeds kernel time in recent tests. | Measured combined overhead only. No attribution to hashing, cache maintenance, PowerShell or a particular diagnostic without measurement; no optimization project in this refactor. |

### Main.rs boundaries relevant to the next stage

`Command::Ordered` delegates to `ordered_cli` (`main.rs:488-492,3726-3728`).
`ordered_cli::run` delegates to `ordered2_cli::run` (`ordered_cli.rs:174-176`),
which delegates IPC3 to `ordered3_cli::run`, requiring no offline lease
(`ordered2_cli.rs:388-393`). Current public IPC3 commands and encoding live in
`ordered3_cli.rs`, not the large generic runtime handler. These are the useful
existing adapters for S5.3's operator/IPC surface.

The small remaining launch dispatch in `main.rs:4118-4150` orders Profile3,
indexed-runtime refusal, Profile2, old ordered profile and generic runtime.
`async_main:3678-3781` also owns outer locks, enrollment lease transfer, ordered
ingress routing and the post-recovery profile fence. Extracting it would require
equivalence coverage across unrelated legacy/recovery branches, without removing
the observed R1/R2/R3 risks. Defer moving either the entire Command enum or this
coordinator. If later S5.3 implementation needs a new command adapter, extend the
existing IPC3 module and leave the outer routing order unchanged unless root
explicitly accepts a separate change.

### R1/R2 are not candidates for a cosmetic merge

R1's current own F is obtained from current credentials in
`runtime7/bootstrap.rs:14-58`; immutable original O is a separate argument from
the InitialPair or actual original event. Closed lowering independently requires
own F (`migration7/live1/data.rs:206-214`). Writer and full-proof reader already
share `check_capture_shape` from `live1/types.rs:363-375`; the reader additionally
checks stored capture hashes and actual receipt linkage (`history.rs:302-345`).
An old Engine has no identical Root7 FullL1 capture/persistence purpose to share.

R2's `runtime7/data.rs:101-122,501-506` calculates
`min(4, queued, 128 - debt)` before restoring Native. Old outbox stops a one-item
materialization at128 (`outbox.rs:195-200`). Closed Root7 lowering separately uses
checked addition for the entire atomic batch before `l1_change_check`
(`live1/data.rs:399-414`). These differ in operation granularity, ownership and
error behavior. Keep all three and the existing shared L1_MAX_UNACKED constant.
Active2's eventual larger limits are not introduced here.

## 3. R-A: one private Ready1 scope predicate

### Exact scope and API

Allowed production files: new
`apps/kilogram-cli/src/ordered_dialog/ready1.rs`, declaration in `mod.rs`,
the predicate call sites in `engine.rs::Engine::ready` and
`runtime7/bootstrap.rs::ready_claims`. Targeted characterization tests may be
added to existing `repair_tests.rs`; existing `runtime7/review1_tests.rs` remains
the indexed parser regression, with no need to move or duplicate it.

The private module exposes only a parent-visible function with this boundary:

```rust
fn scoped_fields<'a>(head: &Head, peer: u64, body: &'a Value)
    -> Result<&'a [Value]>;
```

It performs exactly the existing `fields(8)` and the identical short-circuit
predicate for D, E, SID, peer role, suite1 and format1, retaining the current
error text. It borrows existing values, opens no storage, verifies no signature,
does not mutate Head, and deliberately does **not** parse genesis fields3/4 or
classify a fork. Signature verification remains the existing `wire::control`
call at each caller. No public crate API, new dependency or storage type.

Engine replaces only its inline scope block, at its present position **after**
the two event lookups. Owner7's `ready_claims` calls the helper at its present
position **before** its event lookups and retains its existing eager parsing of
both fixed-size genesis IDs. It retains the `Ready shape/scope refusal` context.
The authenticated outer frame C/D/E checks in `runtime7/protocol.rs:560-568`
remain independent and unchanged.

### Equivalence trap requiring explicit preservation

Engine currently uses `f[3].fixed()? != a0.id()? || f[4].fixed()? != b0.id()?`
(`engine.rs:849`), whereas Owner7 first parses both IDs (`bootstrap.rs:620`).
Thus a valid-scope body with a conflicting first ID and malformed second ID can
reach Engine's signed-fork path while Owner7 refuses during parsing. Their
event-lookup/error order also differs. This is an observed compatibility
difference, not a new confirmed defect or permission to correct it.

Do not replace Engine's expression with Owner7's full `ready_claims`. Do not
move either caller's event reads, expected-body comparison, lease check, phase
check, prior-Ready witness selection, fallback genesis access or commit. Preserve
Engine's existing eager `unwrap_or` fallback evaluation as well. If root wants
to harmonize these negative cases, that needs a separate semantic finding and
decision; it is excluded from R-A.

### Invariants and observable result

After the change there is **one** implementation of the scope predicate instead
of two. There remain two runtime adapters and their original persistence/fork
algorithms. Correctly signed wrong-scope/shape controls retain ordinary refusal;
genuine same-scope genesis conflicts retain each caller's accepted classification
and exact first witness. Preserve block survival, current authorization and
lease order, binding, Native generations/counters, atomic commit boundaries,
wire/control/receipt bytes, and all old/new storage formats.

Review the extracted predicate against both original blocks. Characterization
must exercise a genuine Engine handler, using the existing small direct Pair
setup: wrong scope refuses without block/Native change; a later valid Ready is
usable; same-scope conflict retains the prior Ready witness and block on reopen.
Include the mixed invalid-genesis case above to prevent accidental unification.
No synthesized positive Native/Head/Event lineage. Negative parser inputs remain
explicit component checks.

Fresh indexed integration uses the existing exact public-dispatch test
`migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork`: eight signed
malformed cases, valid continuation, actual scoped fork, renewal and reopen.
Its old split result is context, not a PASS for the new image.

## 4. R-B: retain completed assertions before the test tail

### Exact scope and ownership

Allowed test files: new `apps/kilogram-cli/tests/support/attempt_result.rs`, its
module declaration in `support/migration_s5_l1.rs`, and the result-construction/
tail seam in `support/migration_s5_l1_exhausted.rs:1221-1253`. No product code,
Node/Native algorithms, raw preservation helper, obligations module, deadline
implementation, stable wrapper or existing S5.2 case/ledger files change.

A small test-only `persist_before_tail(path, assertions, tail)` function writes
an explicitly scoped assertion snapshot with `create_new`, pretty JSON,
flush/sync and a closed file, then invokes the supplied fallible tail. It must
not hold a file or borrow a live writer while the tail runs. A collision or write
error returns an error and does not execute the tail. No overwrite/retry/recovery
framework, Drop-only result recording, automatic restore or inferred PASS.

The controller constructs its existing detailed observations after completed
functional checks and `checkpoint.info`, then saves
`assertions-before-tail.json` in a **fresh controller output area outside node
state directories**, before `checkpoint.private_preservation`. Keep the original
tail order and final aggregate result fields, including the actual preservation
result and post-tail phase provenance. Reuse the observation value instead of
reconstructing observations after failure. S5.3 controllers can use this same
private helper when they are later implemented; no S5.3 controller is added now.

The snapshot states `functional_assertions_completed` and explicitly says the
whole-attempt verdict is not established by this file. Bind case/run path and
the outer pre-run source/image inventory to it; the new verification wrapper
must persist that inventory before invoking a test. Snapshot hashes and the
actual later whole result are separately recorded. An absent, partial or invalid
snapshot is not completed evidence. Writing extra test metadata is an intentional
observability change; product behavior and the test's required success conditions
stay equivalent. No claim of crash-atomic filesystem publication or guaranteed
recovery after disk/power failure.

### Before/after criterion and verification

Before: the preservation `?` can discard all detailed RAM observations before
the parent JSON is written. After: the same observations have a closed persistent
snapshot before preservation begins; a failing tail still fails the test and
whole runner, while leaving that snapshot available for narrowly scoped review.
The final full result still requires the tail and normal runner completion.
The existing 760s controller allowance and outer deadlines are not reset or
extended by this helper. Phase logging and cleanup behavior stay unchanged.

Use actual helper calls with temporary opaque data to test:

- `pre_s53_attempt_result_persists_before_failing_tail`: the tail can read the
  exact snapshot through a new file handle and then deliberately fails; the
  original failure propagates and the snapshot remains unchanged.
- `pre_s53_attempt_result_refuses_overwrite_and_skips_tail_on_write_error`:
  create-new collision and a real invalid destination preserve old bytes and do
  not run the tail.
- `pre_s53_attempt_result_success_keeps_separate_whole_outcome`: the successful
  tail result reaches the caller; the snapshot retains its limited assertion
  classification and cannot become a whole PASS marker.

These are lifecycle/failure-boundary tests, not a duplicate JSON serializer.
They create no Account, Session, Native, DB or network node. Source review maps
the actual controller's assertion fields and final aggregate fields before/after.
Do not rerun the historical case03, generate511, walk NCjBHb or activate its backup
to test metadata ordering. Its missing parent result remains missing, its whole
FAIL649.592232s remains immutable, and the separate accepted preservation result
is not replaced. Component verification establishes this new helper/seam only;
it does not re-execute the historical functional body or certify a future S5.3
whole attempt. The latter must still earn its own complete result.

## 5. Proposed verification allocation, not an activated budget

Use a **new** refactor evidence directory/ledger, separate from all S5.1/S5.2
ledgers. Proposed supporting wrapper:
`scripts/invoke-pre-s53-refactor-case.ps1`, restricted to the following cases,
selectors and ceilings. Its small accounting/deadline adapter and metadata are
verification support, reviewed with R-A; it must not rewrite the historical
S5.2 runner to bypass its fixed decisions. No attempt is authorized by this plan.

| Logical change / proposed case | Exact selector or operation | Set | Work / whole ceiling, seconds |
|---|---|---|---|
| R-A / C-ready-scope-01 | `indexed_l1_review1_ready_scope_shape_precedes_genesis_classification` | C | 45 / 60 |
| R-A / C-ready-legacy-01 | New `pre_s53_ready1_legacy_refusal_and_fork_boundary` using genuine small direct Pair setup | C | 45 / 60 |
| R-A / F-ready-scope-01 | Existing `migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork`, on a fresh stopped copy of the genuine small origin | F | 180 / 210 |
| R-B / C-result-lifetime-01 | Prefix `pre_s53_attempt_result_`, exactly the three lifecycle tests above | C | 45 / 60 |

Requested new cumulative ceiling: **C180 / F210 / P0 seconds**. R-A's proposed
share is C120/F210/P0; R-B's is C60/F0/P0. No automatic retry reserve. Stop and
callback on failure, overrun, insufficient origin provenance or unexpected branch
change; root decides any focused continuation. S5.2's remaining allowance is not
reused. Its actual C172.0310185/F3331.2897354/P0 and earlier S5.1 accounting stay
unchanged. No whole-P, handoff or performance acceptance is included.

Measured anchors from published evidence:

- C-review1-01 was whole20.9155434s, including CLI2/state3 components.
- Ready02's matching kernel was1/0 in131.63s. Its whole attempt FAIL140.155492s
  reached work140 during finalization; the log's `status=passed` preceded cache
  finalization and does not make the whole run pass. F180/210 is a bounded forecast
  with margin, not a newly measured result.
- Actual opaque phase/preservation C was kernel0.88s / whole24.1463993s;
  separate stopped preservation was kernel46.26s / whole67.5439277s. These
  differences measure combined runner overhead; they identify no bottleneck.

F-ready-scope uses the origin referenced by Ready02's published case record:
`.tmp/migration-s5-l1/F-small-origin-01`. Its provenance and before/after raw
integrity checks, copy, migration, waits, renewal, reopen and cleanup are all
charged to F. Do not read or activate that input during this assessment. Later
preflight must establish no other active branch of that Native domain. If the
input is unusable, report the dependency; no free replacement generation.

Later execution uses locked/offline verification, **Jobs12/BelowNormal** under
the current assignment, compile-only first, then only
`scripts/run-cargo-tests-stable.ps1` for harness execution. Static compile/stage,
strict CLI Clippy (`--all-targets -- -D warnings`, literal argument array),
fmt-check and scoped diff checks receive separate source-bound static records.
Record the test wrapper's actual cached compile/stage and finalization work
within each C/F attempt even after separate compilation. Never run a network
harness with direct `cargo test`.

Keep the existing fixed paths, hash their **contents**, and do not encode a
revision into the EXE name:

- `target/verification/deps/kilogram-cli-lib-kilogram_cli-tests.exe`;
- `target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`;
- `target/compatibility/long-lived-dialog-migration/kilogram-cli.exe` and
  `kilogram-l1-control-peer.exe` for genuine field nodes;
- the unchanged fixed d9b baseline peer as required by the existing test.

Use one serial source/build/staging lane and no recopy over a running image.
The owned Windows job's independent whole deadline remains armed through result,
ledger and cleanup finalization. Charge setup, source/image checks, files,
fixtures/copies, waits, failures and cleanup. Record matched test counts, actual
exit/whole verdict and zero owned children; a kernel summary or saved assertion
prefix alone is insufficient. Retain every started attempt even if final output
is incomplete; root must account for that failure rather than silently omit it.

## 6. Reuse, implementation order and rollback boundaries

1. Root reviews this plan and chooses the exact scopes/allocation. No human
   permission renewal is needed; this is the assigned technical gate.
2. Implement R-A as a small predicate extraction, with characterization tests
   separate in the diff. No changed semantics hidden as cleanup. Record exact
   source/EXE hashes, compile/Clippy/fmt, then the two C cases and one F case.
   Submit implementation, evidence and exact path/blob allowlist; stop for review.
   Publish only after root acceptance and its commit/push instruction.
3. Against the independently verified R-A publication, implement R-B's test-only
   ownership change and its three C tests. Submit its own source-bound evidence
   and allowlist, stop for review, then publish only when assigned.
4. Resume the separate S5.3 mapping only after root accepts and verifies the
   actual refactor publications. No S5.3 behavior is implemented by these changes.

R-A does not change kilogram-state, crypto, protocol codecs or migration layout.
Reuse unchanged R1/R2, atomicity, credits, history, isolation and S5.1 evidence for
their original exact scopes, with baseline/current source equality recorded.
Do not relabel them as fresh full-coverage evidence for a new image. A changed
Ready branch receives the fresh focused checks above. If R-A expands into data,
storage, lease ordering or permission handling, stop and revise affected checks.
R-B affects only controller metadata lifetime; the already-tested raw preservation
helper and phase-writer repair remain unchanged. No bulk100k or long511 rerun is
justified by either selected seam.

The two changes have no on-disk product migration or wire transition to undo.
Before publication, rollback is limited to that change's own reviewed paths,
preserving other work and all evidence. After publication, any rollback is a
normal reviewed forward revert, never reset/amend/force. Do not delete test
artifacts or erase failed accounting to obtain a clean result.

## 7. Source anchors and completion boundary

All pointers above refer to published `eafd0f8...`. Selected raw source anchors
were checked during this assessment:

| Source | Bytes | SHA256 |
|---|---:|---|
| `ordered_dialog/engine.rs` | 35219 | `d67136ee020a7ebfb42e6b905cb7a0310d2e3f85e5248f7c294e9d91f280fa0e` |
| `ordered_dialog/runtime7/bootstrap.rs` | 22350 | `48b265f82a08f0501e65d0663cdea86cd7877c839f114dfb1fe950bdd0b6abaa` |
| `ordered_dialog/runtime7/review1_tests.rs` | 2151 | `6610a3a030efb69e931248004ef15d0690b5dcbfe235e4fa54a2728792c66242` |
| `ordered_dialog/repair_tests.rs` | 25603 | `2000df2e75e3385377b4dfa9a7cdf76111b7ccaf59190c1428781ed704ff309e` |
| `tests/support/migration_s5_l1_exhausted.rs` | 50456 | `6f1576955aab0c2e9eb1e2f330d549ac35016618769c840ee3a613042c8e058f` |
| `tests/support/migration_s5_l1.rs` | 20509 | `60b896e3aa4b511fe06e3307f5e988760f76bc172bb693af18ef87663405be34` |
| `tests/support/migration_s5_l1_review1.rs` | 17596 | `9f4b792567f15790cc9b851d0c42f275231130c78fbfc85b7aa0a67e01f63f8e` |

First four paths are under `apps/kilogram-cli/src`; the last three under
`apps/kilogram-cli`. Published [implementation review1](long-lived-dialog-a3-migration-s5-l1-implementation-review-1.md),
[final report](long-lived-dialog-a3-migration-s5-l1-implementation.md),
[acceptance](long-lived-dialog-a3-migration-s5-l1-implementation-acceptance.md)
and [preservation readiness](long-lived-dialog-a3-migration-s5-l1-preservation-readiness.md)
provide the original defects, accepted limits and measured evidence.

Deliver this plan's raw bytes/hash and findings to root, then **stop before
implementation or verification execution**. No refactor commit/push is part of
this assessment. NCjBHb and its private backup remain stopped/inactive at their
existing pinned absolute paths; neither was traversed or opened for this plan.
