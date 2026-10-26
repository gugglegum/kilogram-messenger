# R-B: durable assertion snapshot before the test tail

2026-10-26. Implement the already accepted R-B plan against published baseline
**`9016b578bed8b8b5e43dd0e2e94edc376804e4f0`**.

Root independently verified R-A's sole parent eafd0f8..., all92 committed
paths/blobs/modes, unchanged candidate/acceptance/raw bytes, empty index and the
sole excluded queued S5.3 task. GitHub comparison of9016b57... against master is
identical, ahead/behind0/0. No publication work remains for R-A.

Authority: [accepted plan](pre-s53-refactor-plan.md),
[plan review including legacy provenance clarification](pre-s53-refactor-plan-review.md)
and [R-A acceptance](pre-s53-refactor-ra-acceptance.md). This activates R-B only;
S5.3 remains queued until this change is accepted and published.

## Implementation boundary

Implement the small test-only `persist_before_tail(path, envelope, tail)` helper
in `apps/kilogram-cli/tests/support/attempt_result.rs`, its module declaration in
`support/migration_s5_l1.rs`, and the existing exhausted controller's result/tail
seam in `support/migration_s5_l1_exhausted.rs` (baseline lines1221-1253).

The helper creates a new snapshot file without overwriting, writes pretty JSON,
flushes and syncs it, and closes its handle before calling the fallible tail.
An open/write/flush/sync/collision error prevents the tail. A tail failure
propagates as failure while leaving the snapshot unchanged. A successful tail
returns its actual result. No automatic retries, recovery framework, Drop-only
recording or promise of crash-atomic publication.

After successful `checkpoint.info`, move the existing22 observations into a
single reusable value and persist it before `checkpoint.private_preservation`.
The snapshot path belongs to the fresh controller output area, outside node state
directories. Keep the actual preservation call and the later phases in their
original order. Reuse the22 observations for the final parent JSON; do not rebuild
them from a failed attempt or a historical file.

| Before-tail fields | Existing values |
|---|---|
| migration_a_observation, migration_b_observation | migration_a, migration_b |
| source_a, source_b | source_a.public, source_b.public only |
| original_before, original_after | corresponding values |
| ready_a, ready_b, copied_a, copied_b | corresponding values |
| refused_before, refused_after, received_a | corresponding values |
| reconnect_barrier, reopened_a, reopened_b | reconnect, reopened_a, reopened_b |
| old_first_page, old_suffix | first, suffix |
| history_a, history_b, cold_a, cold_b | rows_a, rows_b, cold_a, cold_b |

Keep three fields strictly post-tail: `checkpoint` is the actual preservation
result; `phase_trace` and `phase_records` describe the actual later journal.
If a pre-tail journal is included, give it a distinct name. Never serialize the
private SourceNative object in place of its existing public projection.

Snapshot classification is `functional_assertions_completed` and explicitly
does not establish a whole-attempt verdict. Include the real case/run/output
identity. The helper receives an explicit envelope; it does not read environment
variables or discover old evidence on its own.

The legacy exhausted caller explicitly uses `unavailable_legacy_runner` because
its old wrapper has no durable pre-run inventory binding. Preserve its caller
contract, case03 guard, functional body,760-second allowance and phase/cleanup
behavior. Do not add a mandatory environment variable or substitute a historical
case as current provenance. The old runner, preservation helper and obligations
module remain unchanged.

New budgeted verification and future S5.3 callers use `bound_pre_run_inventory`
with the actual pre-run inventory's path, digest and identity. For the three
component tests, the new wrapper supplies that real binding before execution;
an invalid supplied binding must not silently fall back to an unbound mode.
Keep ordinary standalone lifecycle tests self-contained if possible, using an
explicitly labeled opaque/unbound test envelope when no managed invocation was
requested. Such a fixture is not a claim of product/source provenance and must
never be confused with the bound budgeted run or legacy controller.

## Verification and activated budget

Activate the reserved **R-B C60/F0/P0** allocation in its own ledger. It is
independent of R-A's unused remainder. Overall allocated refactor ceilings remain
C210/F210/P0 after the explicit R-A revision; R-A's closed charge remains
C118.3327995 (including conservative60) / F148.7417507 / P0.

One allowed executable case: `C-result-lifetime-01`, **work45/whole60**, declared
NativeToken through ordinary tool review under the standing human permission.
It runs exactly the three planned lifecycle tests with prefix
`pre_s53_attempt_result_`:

1. `pre_s53_attempt_result_persists_before_failing_tail`: the actual tail can
   reopen/read the exact closed snapshot, then fails; the original failure
   propagates and the snapshot stays unchanged.
2. `pre_s53_attempt_result_refuses_overwrite_and_skips_tail_on_write_error`:
   existing-file collision and a genuinely invalid destination preserve old
   bytes and do not invoke the tail.
3. `pre_s53_attempt_result_success_keeps_separate_whole_outcome`: the actual
   successful tail result reaches its caller, while the snapshot retains its
   limited classification and contains no whole PASS.

Use temporary opaque data, no Account/Session/Native/DB/network fixture. Source
review separately proves the real controller seam's22 fields and unchanged
functional body. These tests do not rerun the old long controller or certify a
future whole S5.3 attempt. The old missing parent result remains missing and its
whole FAIL remains FAIL. Do not traverse/open/restore the exhausted checkpoint
or backup, generate511, rerun bulk100k, or spend any F/P allowance.

Use the fixed stable harness paths, locked/offline Jobs12/BelowNormal, and one
serial source/build/staging lane. Run fmt-check, strict CLI all-targets Clippy
and compile-only staging, with compile last. No product CLI build is needed for
test-only edits. Preserve any overwritten previously executed harness contents.

Add only the small verification support needed for this new baseline/case,
preferably separate result-case/static adapters so published R-A adapters and
evidence stay unchanged. Reuse the corrected patterns: explicit result-object
shape and scalar totals, real environment deletion/restoration, exact test names
and3/0 summary, actual wrapper/whole exit, verified cleanup, independent owned-job
deadlines through finalization, durable pre-run inventory and failure retention.
Keep the existing deadline helper and stable runner unchanged. No new general
runner framework. Earlier raw outcomes and ledgers remain immutable.

There is no automatic retry. Any failure, timeout, insufficient provenance or
tool rejection stops execution for a focused root decision. A successful kernel
or saved snapshot alone does not establish a whole PASS.

## Review, documentation and publication

Implement and submit the small source diff plus verification wiring for root
review before the executable C case. Static compilation may proceed in a frozen
lane. Root will check the helper/controller field map and new caller provenance;
an independent reviewer is already familiar with this seam. No new broad plan is
needed. After source clearance run the one case and submit its evidence, report,
reuse assessment and exact candidate paths/raw hashes/normalized Git blobs.

Update ai-docs navigation (README.md, next-stage-plan.md, chat-coordination.md)
only as needed to link the actual R-A publication, R-B result and the order before
S5.3. Preserve historical result/decision files; changing current navigation is
explicitly in scope. Preserve the queued S5.3 task unchanged and outside this
candidate until its later assignment.

Stop for implementation acceptance. After root accepts the exact diff it will
assign a separate normal commit/push; then root verifies publication and resumes
S5.3 mapping against that actual published baseline. Do not begin S5.3, product
handoff, UI, general refactoring or adjacent milestones during R-B.
