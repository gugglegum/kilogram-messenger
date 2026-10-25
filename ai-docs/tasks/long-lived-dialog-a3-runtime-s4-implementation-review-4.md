# A3-S4 implementation review 4: local relay helper readiness failure

Root decision, 2026-10-25. Whole implementation remains unaccepted; no publication.

## Verified outcome

Root inspected the complete F summaries: `f-full-review3-frozen-1` passed19 cases
(5 actor,9 process,4 state,1 route) in377.2276474s. Conservative contributing F
total is620.4258161s including review3's243.1981687s subtotal, below1200s.

The authorized one-run P exception was consumed. `p-full-review2-exception-1`
failed in1901.2816169s runner/1895.47s harness, before the aggregate deadline.
Root verified its failure summary, helper stderr, helper source and controller
readiness sequence. The helper published `http://127.0.0.1:57214/`, then exited
with unadorned Windows os5. The first RelayOnly runtime subsequently failed its
unchanged15s registration deadline. B-only relay continuation and final exact
history/old-cursor assertions did not execute;3287 committed calls are not3288
and this run remains FAIL.

The helper writes its ready file, reopens it through read-only File::open, and
calls sync_all. This is a concrete writable-handle defect on Windows, compounded
by publishing readiness before the fallible sync. The historical stderr lacks
operation context; preserve that limitation instead of claiming a recorded
syscall trace. This finding concerns the test's local operator relay helper,
not a demonstrated defect in the frozen messenger runtime.

The watcher sampled relay-configured profiles alongside older locator files
during transition. Preserve that artifact as a transition observation, never as
proof of signed URL-only locators or successful relay traffic. Whole P acceptance
must come from the actual controller assertions and final complete run.

## Authorized narrow investigation and repair

1. Preserve current failed logs/results/metrics, the original helper source and
   image hash69cad3e0e39b5c23a163b0d2a99b15d42faf0a2433bb909397a64cda81eb7aa5.
   A single short startup check on the old fixed helper with fresh control files
   may confirm the standalone failure; do not open or resume the failed P vaults.
2. Fix only local relay helper readiness and controller liveness diagnostics.
   Write and sync the URL using the same writable handle to a fresh same-directory
   temporary file, then atomically publish readiness after successful sync. Add
   operation/path context. No read-only sync, non-atomic ready contents, destination
   ACL changes, missing-sync fallback, transport/protocol deadline changes or
   public/external relay. Keep owned child cleanup and loopback-only binding.
   Prefer existing dependencies/ordinary file primitives over new dependencies.
3. Controller readiness/liveness checks should report an exited helper and its
   bounded stderr promptly, including around RelayOnly starts. Existing runtime
   registration15s/admission30s/round15s/lease limits remain exact. Readiness or
   helper liveness alone does not substitute for actual relay message delivery.
4. Compile the helper and stable test harness without touching the frozen CLI or
   product42 source files. Keep the same fixed helper EXE path after all old
   processes stop; record the new helper hash and exact source/controller diff.
5. Run one bounded<=180s focused real own-relay smoke on a fresh isolated fixture.
   Reuse the same real Pair/Relay/controller paths. Establish a direct pair and
   retain one actual B-only committed Text debt; restart into RelayOnly, verify
   fresh signed URL-only locators with empty IPs, import them and prove exact
   B-only delivery/receipt without a new A send. Check exact readable Text/IDs,
   helper liveness, unchanged selection/native constructor counts and clean child
   shutdown. Keep substantive main-P assertions unchanged. A smoke PASS is not a
   replacement or stitched suffix for the failed full P.

Return the exact patch, pre/post helper/source hashes, standalone/smoke results,
and frozen product correspondence to root before any new full P. There is no
unused P exception at this point. Root will decide on a new complete attempt only
after reviewing this repair and actual own-relay evidence. No whole-F repetition
is required merely for helper-only changes; demonstrate unchanged F paths and
source correspondence. Existing component/F time budgets stay unchanged; this
focused failure investigation has its separate explicit180s bound.

## Adjacent observations, outside this repair

A bounded source search also found read-only hit-file sync patterns in unchanged
legacy OD1 test_support::checkpoint and ordered-vault test pause hooks. Directory
syncs in peer-reenrollment-staging and store are Unix-only. Do not change those
legacy hooks or invalidate their historic results speculatively in this repair;
record them for a separate Windows test-hook audit if they are exercised later.
S4 product code remains frozen throughout the present helper investigation.

## Repair accepted for verification; one replacement complete P authorized

Root inspected the entire helper, controller diff and focused smoke assertions.
The exact14128-byte patch has SHA256
`e399233bec2f5a5914c037cf51396bb2cd554c23dbee6e57cdff0da5ed4c858c`.
It fixes writable-handle sync followed by same-directory atomic publication and
adds bounded helper stderr/liveness context. Main P assertions/counts/timeouts
remain unchanged. The old helper also failed a single independent fresh startup
in0.0804791s, publishing ready before its os5 exit.

New fixed relay image:17057280 bytes, SHA256
`b8723b8cb546a0ceccf3179415d580b073f0a36d2f9a458ecfd15c66bf2c6317`.
CLI remains d5d8c8c118925dfde67e37fdb32aa5a120293cde258e93843dd1939fddb15560.
Root rehashed both images, the archived old sources and current Rust sources.
Relative to failed P, only the helper example and positive.rs changed. The42-file
mechanical freeze list contains the helper and predecessor probe but excludes two
in-src test modules; only the helper differs in that list. Independently, all42
Rust files beneath apps/crates src directories match the failed-P snapshot. These
are different42-element sets, not conflicting claims about a changed runtime.
Every Rust entry in the successful smoke snapshot matches current contents.

`review4-own-relay-smoke-1` passed in50.2735936s runner/24.181521s controller.
Root checked the real held B-only debt, endpoint/revision change, four URL-only
empty-IP locators, production import and delivered receipt, exact Text/request/ID
history on both peers, unchanged selection,4 Account/4 Session constructors,
10 committed packet calls and owned-child cleanup. This supports the helper repair;
it does not replace any part of complete P. Strict Clippy also passed.

The helper repair is accepted for full verification. Root now authorizes exactly
one new complete P on a fresh fixture, using this helper and unchanged CLI, with
controller2950s and runner<=3000s. This is a new explicit replacement attempt after
a concrete helper repair; the review2 attempt remains consumed and FAIL. It is
not an increased ceiling or permission for repeated silent retries. All1640 Text,
4 genesis/3288 committed calls, DH/late-tail/offline/full-pending/C2/restart/B-only
relay/final-history/old-cursor and memory/capacity assertions remain mandatory.
If elapsed exceeds original2400s, report that original target as unmet under the
explicit exception; otherwise report the actual complete measured result.

An announcement-only edit identifying review4's replacement authorization is
permitted, with a new source snapshot; no scenario logic or timeout edit is allowed.
Reuse the proven F620.4258161s and unchanged component evidence with explicit source
correspondence; the separately bounded relay investigation does not replace those
gates. Preserve every failed run and invalid transition snapshot with its limitation.

On a new failure, preserve/callback without further retries or changes. On success,
finish the implementation report, complete verification matrix and exact proposed
publication manifest, then callback for root's final source/evidence review. Include
root review documents separately in the eventual accepted allowlist. Commit/push
still requires root's final implementation acceptance; this decision does not grant it.
