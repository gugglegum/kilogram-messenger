# A3-S5.2 boundary02 readiness — instrumentation only

2026-10-26. Prepared under
[`boundary-continuation-review-1`](long-lived-dialog-a3-migration-s5-l1-boundary-continuation-review-1.md),
7943B SHA256 `a9b3fd75ce257362297a9ec7c4fe00133917a42bf2ecbd4a921d41bfa4870341`.
This is the requested static return for source/policy review. No contributing
case, fixture operation, deadline probe or product build was performed during
this preparation. A new separate root decision and bound approval file are
required before boundary02. The executor has not created that approval.

## Retained lineage and exact change

HEAD remains `73c499818ba9b699c331aa842e64e8432e90c59a`; index is empty.
Only two of the 52 source/script inputs changed from compile19/revision2:

| Input | Before SHA256 | Current bytes / SHA256 |
| --- | --- | --- |
| `apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs` | `a8077b49ba2e02956a94ca352158190854f47d3a6b37b2e624b2bb81743e71f4` | 40620 / `dd3a4b9e6c76daa1056b1bb3d765bcb251c54092ac14f84db3178cc7c5c510bd` |
| `scripts/invoke-migration-s5-l1-case.ps1` | `c9c6841213181af75f3364128cf7d0697d45db5a7a8bf5bfda1b58986c3211ed` | 24964 / `22dcaa81179e39986199664adea130c01c22749b3ea69a34664a60b2d66ca79e` |

Exact deltas against the pre-edit noncompiled snapshots:

- [Controller patch](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary02-controller.patch.txt),
  26817B SHA256 `8eb5555e791eecbb1481ae25941f6a4d69481fdcd627ed0af9bfc93d11f5c613`.
- [Runner patch](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary02-runner.patch.txt),
  5165B SHA256 `820e56dd9b8cd6c1c820c2d7e88aebbd120b8005b9114ab8ce002aa4271454a7`.

`S-boundary02-before.json` binds the pre-edit controller, runner, harness, failed
case/log and ledger. Original controller26779B and runner23612B remain in
`S-boundary01-controller.rs.txt` and `S-boundary01-runner.ps1.txt`; their hashes
are the Before values above. The old stable harness13683712B SHA256
`5ccc3a8a3e4bc76ed811f6d0aad5c93b95381bbab8d6a2c1284b3d66c8eb5ce2`
is retained at
`target/compatibility/long-lived-dialog-migration-boundary01/migration_s5_process-harness.bin.snapshot`.
Neither snapshot is a newly runnable EXE. Original readiness, revision2, their
bindings and original v1 snapshots remain intact. Root-owned navigation and
future S5.3 task files were not edited.

## Controller phases, pauses and child lifetime

`Phases` creates `boundary-phases.jsonl` in the new controller directory. Each
record contains monotonic elapsed seconds, a public phase name, state and detail;
each append writes a newline, flushes and calls `sync_data`. `run` records
begin/end/error and phase duration, checking the same absolute deadline before
and after the real action. An early return/unwind attempts a final controller
error record. A hard Windows job cutoff can bypass Rust Drop; the last flushed
record and persistent stderr then remain the evidence.

The timed regions cover raw origin verification, A/B copying, legacy inspection,
actual A/B migration, public owner initialization/startup, initial interop,
request/refusal/history work, real B17 receive/ACK, restart/reconnect, cold
receipts, final origin verification and private checkpoint preservation.
Legacy A, B and B-after17 separately record the existing lock acquisition,
`OrderedVault::open`, `Engine::new`, Head read, sealed Native read, actual Native
restore, identity-material reads and close. These are the same real calls with
the same assertions; no extra database opens or cached/bypassed verification
were introduced. No secrets are added to the phase records.

Only this test's migration calls switch from `Node::migrate`/captured output to
the existing `spawn`/`OwnedChild` helper. Shared helpers and all other tests are
unchanged. Each call retains the normal `branch.upgrade(false)` argv and actual
Ready/zero-migration-Native assertions, then applies the same Node slot/profile/
IPC/locator transition. It records the actual PID, executable/argv, persistent
`migration-a.stdout.log`/stderr or `migration-b.stdout.log`/stderr, their sizes
and BLAKE3 hashes. Child failure is still a failure.

Each invocation owns an external `.control` and `.hit` next to its logs, outside
state. They must be absent initially. The controller observes existing debug
hooks in this exact order:

`fence-committed -> first copy-committed -> verify-staged -> ready-staged -> ready-committed -> profile-staged -> observation-released`.

The real hook pauses its child. On each exact marker the controller checks child
liveness, records the hit, **removes that hit before release**, then atomically
replaces the control with the next complete target using a same-directory synced
temporary file and `persist`. It never truncates control to empty and never
reuses a stale hit. The hook already publishes its own hit atomically. The
release record includes observed/released elapsed times and the actual next
target. `verify-staged` and `ready-staged` are explicitly labelled before
transaction commit; `profile-staged` is before profile publication. Later copy
checkpoints do not pause after the first copy release.

Polling is 20ms with liveness and one absolute monotonic deadline of 560s from
controller entry. The allowance is never renewed per phase, hook or child.
Cleanup explicitly waits for an exited child, or kills and waits for a live
child, before removing the hit and publishing the released sentinel. Existing
`OwnedChild` Drop supplies the fallback on early return/unwind. The pause guard
is declared first so unwinding drops the child before releasing its control.
The outer non-breakaway Windows job remains authoritative on hard cutoff.

There is no runtime claim yet that this instrumented sequence completes in the
allowance. Blocking calls cannot be interrupted by a cooperative Rust check;
outer work580/whole600 remains armed independently, including runner setup and
finalization. Setup time is already spent when the controller begins and is
not regranted by its local560. Controlled pauses, record syncs, waits, copying,
recovery and preservation remain charged to the future attempt. This is an
instrumented correctness/diagnostic run, not a passive performance measurement.

No fence hit still cannot distinguish inventory, initial source verification
and preflight. Fence-to-copy can include revalidation and first-copy work;
copy-to-verify includes remaining copying and further provenance verification.
Those intervals must not be labelled as a single internal sub-operation. Root's
vault verification/provenance/quadratic-search findings remain hypotheses about
cost; no optimization or validation change was made.

The future parent result includes the flushed phase-file path/size/BLAKE3,
phase records and A/B migration PID/argv/log/checkpoint provenance. It retains
all previous functional results. The phase digest covers assertions and
checkpoint preservation; parent-result serialization/write follows and is
deadline-checked, so that digest does not claim to measure its own later write.

## Functional assertions retained

The controller requires the exact case02 at entry, the actual frozen d9b511/16
origin and a fresh `l1-exhausted-observed-*` directory. It creates one active copy
per Native domain, never resumes the failed01 branch and never typed-opens the
frozen original. Checkpoint backup now uses the actual case ID.

All assertions from the reviewed revision2 remain: actual A512/17 and B17/512
Native frontiers; owner identity, nonce, signing/encryption material and exact
Native correspondence; unchanged Session keys/D/E/SID; zero reconstructed Native
objects; old exact request retry and changed-body conflict; refusal of new A
requests before Packet; exact original527 Event/request/UTF8 history and frozen
suffix; actual opposite-lane B17 receive/ACK and FullL1; both final528 histories;
exact cold receipt/frontiers; raw original immutability and private preservation.

The revision2 fresh reconnect barrier is unchanged: both new child instances
must stay alive and expose current mutually matching Locator revisions and
leases/admissions. After a stable fresh-pair token it requires a later exact
successful Resume trace from the fresh A log, within the existing local25s.
Fresh statuses must still match that pair/token/frontiers with zero Packet and
constructor activity. This has not yet been field-executed.

## Static results and source/image binding

| Record | Actual result |
| --- | --- |
| `S-fmt-26` | PASS, formatting, 3.8494937s |
| `S-clippy-24` | PASS, CLI all targets, `-D warnings`, 3.4830869s |
| `S-compile-20` | PASS, stable wrapper without `-Run`, 21.2038928s; Cargo compile5.41s |
| `S-fmt-27` | PASS, `fmt --all -- --check`, 3.7127638s |

Clippy and compile used locked/offline `verification`, Jobs12/BelowNormal.
Compile log records `status=compiled-only` and no test executable started.
PowerShell Parser accepted the changed runner with zero errors; its AST and
exact source policy assignments are recorded without dot-sourcing/executing
the runner. `git diff --check` passes. No C/F mechanism probe was repeated.

New compiled/staged **test harness only**:
`target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`,
13935104B SHA256 `5a003864af885c09d0274d84f54b2000a774908382169085b42314ee546ec161`.
The superseded compile19 and its noncompiled harness snapshot remain available.
The 52 final compile inputs still match; the delta is exactly the two files
listed above. All33 product inputs match the reviewed product binding and all8
fixed/archived product images match. Current CLI/control peer remain70226432B
SHA256 `69744fca50f44439b4734e699f416fc6b2bb8aac4336e5dad18ed173b2abbf6c`.
There was no product rebuild or product-image staging.

[Machine binding](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary02-readiness-binding.json)
is 35119B SHA256 `fdb1682015099e26bcaee3e710413dbb8f62677fc30a74f5eaf78a4727addc68`.
It contains the source/patch/harness/static/log hashes, 33 product inputs, eight
images, 29 protected historical artifacts, unchanged ledger, parsed policy and
future budget arithmetic. It records no known live Kilogram process and no
boundary02 start/result/approval file at preparation time. No original or failed
fixture bytes were read/opened/copied during this static preparation; current
origin lineage relies on root's retained raw verification and immutable exports.

## Future policy and stopping boundary

The old case01 policy remains work120/whole140 and its immutable ID is unusable
for rerun. Its whole FAIL120.1641445s remains charged; work expired, harness
exit124, owned children after cleanup0. There is no accepted functional boundary
PASS. Accepted origin01 remains whole PASS481.4608671s. Its exported case hash is
`e1acc55360a45c386dde029c2f9be35cbbdacc08bd2a076a1812ac6c692d5135`.

Ledger390894B SHA256
`b9dad4528aecff9a7084492a52e2f411f67cba89cc1af4cc3ea8196cf9bd1462`
is unchanged: C147.8846192/F2046.0168572/P0, ceilings C500/F2900/P0.
Summing all eight C and 27 F entries matches the ledger. Remaining F853.9831428
covers the proposed entry reserve825 = whole600 + contingency225; after a full
whole600 it would leave253.9831428. This is budget arithmetic, not a runtime
estimate, refund or new budget. Existing terminal-publication timing disclosure
is preserved: the independent whole watchdog covers final case/ledger writes;
the stopped stopwatch does not measure that publication tail.

The new exact case02 is F/Tests, CLI only, the same selector and frozen origin,
no resume and no automatic retry, work580/whole600. Runner checks the preparation
authority and exact failed01 case/log hashes, retaining that cutoff in the
ledger. It also retains prior small-gate exceptions as limited acceptance of
functional assertions; historical whole failures remain failures.

Only after root accepts this package and creates a new approval bound to this
runner hash, current ledger hash, actual origin case hash and exact case02 may
the following command be submitted through normal native-token tool approval:

```powershell
& scripts/invoke-migration-s5-l1-case.ps1 `
  -Case F-exhausted-boundary-02 -Set F -Action Tests `
  -Package kilogram-cli `
  -Filter migration_s5_l1_genuine_exhausted512_refusal_receive_history_checkpoint `
  -FixtureDirectory 'E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-l1\F-exhausted-origin-01' `
  -LongApprovalFile 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-boundary-02-root-go-ahead.json' `
  -ExecutionMode NativeToken
```

That command has not been executed. Preparation stops here for architecture and
policy review. Product optimization, P, S5.3/cut3, commit and push remain closed.
