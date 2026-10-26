# A3-S5.2 boundary continuation review 1 — instrumented retry preparation

2026-10-26. Root response to the actual F-exhausted-boundary-01 cutoff. Standing
human authorization covers focused review/correction cycles. This decision
authorizes static preparation only; a new bound root go-ahead is required before
another contributing attempt.

## Retained result and lineage

F-exhausted-boundary-01 remains whole FAIL120.1641445s, exit1, work120 expired,
whole140 owner survived to publish the record, and owned children after cleanup0.
The matching harness started and was terminated with exit124. No completed
boundary or partial functional PASS is accepted. Preserve:

- Case12277B SHA256
  `072b522817e7f62e9bbc0f930f3d3969c2b56675c4ae348931e4b113d895753b`.
- Log2365B SHA256
  `b7b5efa02401eef44abd6488f64b2f0ca3a5b975e90e877945fb51d9e69f55ab`.
- The stopped active directory `.tmp/migration-s5-storage/l1-exhausted-Cxme0k`,
  including origin-before verification1883572B SHA256
  `cd7f2f4379be0c972083a03fe94346600afea091dbcd371fa3b910d51ba645be`.
- Current ledger390894B SHA256
  `b9dad4528aecff9a7084492a52e2f411f67cba89cc1af4cc3ea8196cf9bd1462`.

Root verified all54 recorded source/image inputs still match. Existing artifacts
establish that raw origin verification and both copies/configs completed, but no
first public indexed owner was started. Missing fine-grained timing and captured
migration-child output prevent attribution to a particular call. Physical vault
sizes/timestamps are not logical migration-stage or corruption verdicts.

Root independently rechecked the frozen original after the cutoff by raw metadata
and hashes only: all6944 files/17855269 bytes match its producer manifest SHA256
73c38f113a7e663d00bba36ae0116a292b7ec0511d4e23b79729f60cf87e2995.
No repository/Native/runtime was opened. The accepted origin PASS remains valid.
Keep both the immutable original and failed active branch stopped and untouched.

## Source findings and diagnostic scope

Read-only source review found substantial work before first startup, without
establishing which cost dominated this attempt:

- OrderedVault::open (ordered.rs206-216) performs full vault verification, then
  shadow restoration. verify_with_records (vault.rs750-772) loads/decrypts primary
  rows; restore_verified_shadow (ordered.rs760-807) authenticates all rows and
  reads/decrypts them again while comparing filesystem shadows. Engine::new itself
  only initializes device/claim/specs. Lock acquisition uses try_lock, not a wait
  loop; journal recovery is a separate possible cost.
- Fresh storage_migration::upgrade validates the source at three distinct stages.
  Provenance checking revisits signed events, Text projections, receipts and
  credentials, and restores existing Native state. Constructor/packet counters
  exclude persistence restore, so zero counters do not exclude its cost.
- migration7/copy.rs prepare_item scans Event entries for every authored row and
  repeatedly converts the same ID to hex. This is quadratic metadata work, plus
  repeated linear searches elsewhere. Its measured impact is still unknown.

Do not remove, cache, bypass or weaken validation, alter production sources, change
the build profile, or infer a product fix from these hypotheses. First make the
same complete boundary attempt observable. No separate diagnostic execution is
needed during preparation.

## Authorized controller and runner preparation

Preserve the currently reviewed controller a8077b49... and stable harness
5ccc3a8a... as exact noncompiled snapshots before changing/restaging them. Keep all
old readiness/bindings, static records, whole outcomes and product images intact.

Prepare one controller-only instrumented continuation with the same test selector
and all existing assertions, including the revision-2 fresh reconnect barrier:

1. Append flushed public phase records, with monotonic elapsed and begin/end/error,
   around raw verification/copies, A/B legacy inspection, migration, owner startup,
   history/receive/reconnect and final preservation. Within legacy inspection
   distinguish lock acquisition, vault open, Engine creation, Head/raw access and
   existing Native restore. Do not serialize secrets or add extra state opens.
2. For this test's actual A/B migration commands, use the existing owned spawn
   helper with persistent stdout/stderr files instead of holding Command::output
   entirely in memory. Retain exactly the normal command/arguments, result/status
   assertions and Node transition semantics. Record actual child PID and public
   command/log provenance. Keep the old shared test behavior unchanged where
   possible; disclose any necessary helper refactor in the source delta.
3. Observe existing debug migration checkpoints using an external, per-invocation
   control path: fence-committed, first copy-committed, verify-staged, ready-staged,
   ready-committed, profile-staged, then a nonmatching released sentinel. Each
   matching hook briefly pauses until released; this is not passive observation.
   Record the exact hit, remove its marker before release, and atomically replace
   the control bytes with the next target. Keep controls/logs outside state.
   Do not leave a stale marker or expose transient empty control bytes that could
   skip the next hook. verify-staged and ready-staged precede transaction commit;
   label them accurately. A missing fence hit does not distinguish inventory,
   initial verification and preflight costs; retain that diagnostic limitation.
4. Use one absolute monotonic controller/child-observation deadline, current child
   liveness and guaranteed owned-child cleanup on every success/error/timeout.
   Do not renew the allowance per checkpoint. The existing final reconnect local
   limit25 remains unchanged. The outer non-breakaway job remains authoritative.
   Include phase records, actual hook release behavior and migration logs in the
   future parent result. All wait/copy/recovery/capture costs stay charged.

Prepare a new exact case F-exhausted-boundary-02 with work580/whole600 and entry
reserve600+225=825 seconds. Preserve boundary01's original120/140 policy and
immutable ID. Boundary02 must require its own later root approval bound to the
new runner/current ledger and actual origin case, plus exact retained failed01
case/log provenance. Keep NativeToken through ordinary tool approval, the exact
selector/package/frozen-origin directory, fixed EXEs and no automatic retry.
No approval file may be fabricated by the executor during preparation.

The prospective boundary02 starts with a new isolated copy from the same frozen
origin; it does not resume/repair/retarget the partially processed failed branch.
Only one branch per Native domain may be active. No second origin generation,
new synthetic positive state, fixture opening/copying, contributing C/F run or
deadline probe is authorized during this static preparation.

Current actual C147.8846192/F2046.0168572/P0; ceilings C500/F2900/P0 unchanged.
F remaining853.9831428 can cover one whole600 attempt while retaining253.9831428,
above the225 contingency. This arithmetic is not a runtime prediction or a budget
increase. Keep the failed120.1641445 charged and the terminal-publication timing
disclosure. A further overrun/failure will require a new root decision.

## Return concrete readiness before execution

Run appropriate locked/offline compile-only/static checks, Jobs12/BelowNormal,
without starting a test executable. Produce the exact controller/runner delta,
phase/pause/cleanup design, parsed-script/static records, new harness/source
binding, unchanged33 product-input/image checks and current ledger. Preserve
original readiness records and issue a new revision rather than rewrite history.
Callback root for source and policy review; stop before any boundary02 execution.
No product rebuild, commit/push, whole-P or cut3 is authorized by this decision.
