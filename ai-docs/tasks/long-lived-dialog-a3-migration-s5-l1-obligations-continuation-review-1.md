# A3-S5.2 obligations — bounded continuation decision 1

2026-10-26. Root reviewed the second failed obligations case with independent
read-only GPT-6.1 Sol Max state and protocol reviewers. This is a diagnostic/reuse
decision within S5.2, not implementation acceptance or long-origin permission.

## Corrected evidence and uncertainty

The live-read callback initially reported no selection beyond61. The completed
failed branch log contradicts that observation, and the executor has acknowledged
the correction. Preserve the final original log:

`.tmp/migration-s5-storage/l1-obligations-Iw3nQI/two/a.stderr.log`, 738 bytes,
SHA256 `042b10b35f54c868493e4e69056322afee076297c8c63b1ae29dfa15624d4958`.
Line12 selects62..69 with `direct_after=61`. Thus Complete for54..61 was processed
and the next outgoing call occurred. The selection trace precedes reservation,
Event/auth loading, frame signing, owner return, RPC and final Complete; it does
not establish that62..69 was sent or accepted.

The log's last write was2026-10-26T03:08:39.6087754Z and the completed case log's
last write03:08:39.7852448Z, about176ms later. This is filesystem timing evidence,
not a precise per-packet duration. The pending64 condition has a25s bound, and
OwnedChild::drop kills/waits after a failure. The final batch may simply have
reached the next step near that deadline. No confirmed stuck worker/Complete was
found, and the cause of slow progression remains open. No failure is reclassified.

The state review found no hot/cold range-cache divergence: after+1 is correct,
range Work/caches are operation-local, and successful commits update current root.
The actor consumes a job before bounded rotating background work. The earlier
peek-before-Native fix preserves lease/frontier/Guard/final commit/credit gates,
but the repeated failure proves it was not a sufficient explanation or repair.

Protocol review also noted that owner outgoing and check_lease mailbox awaits
precede the inner RPC timeout. Trace needs those boundaries as well as actual
RPC begin/write/finish and checked exact Complete, with monotonic elapsed times
and public scope/ordinal/ID only. A new RPC-begin line alone cannot diagnose a
wait before reaching RPC. Freeze source before build/stage; do not edit beneath
an ongoing source-bound build or overwrite a live image.

## One authorized continuation, without a new F allocation

Current aggregate/per-attempt binding: C44.7378423/F714.2462150/P0, including the
101.7948885-second F-obligations-02 failure. The [amended ceiling](long-lived-dialog-a3-migration-s5-l1-budget-amendment-1.md)
remains C500/F2100/P0; F remaining1385.7537850. **No budget increase or reset.**

Authorize one named focused F continuation of the exact stopped failed branch
above, with a **150-second whole contributing-attempt maximum**, including source/
image/input inventories, preservation/setup, runtime, assertions and owned-child
cleanup. Prepare/rebuild/stage separately as static-only. The controller must
reserve cleanup inside that maximum; record actual elapsed even on failure or an
overrun. An upper-bound150 would leave1235.7537850, still above the1215 long-run
reserve, but does not fund the other remaining small checks. This is diagnostic
work, not the first511 run. Callback root after this one attempt before more F.

Reuse conditions:

1. Verify the exact two runtime roles/processes and exclusive locks are stopped;
   no other active branch of their Native domain may exist. Keep the frozen real
   d9b source untouched. Do not regenerate65, recreate a session, inject Native/
   Head/Event rows or reset queue/debt/policy to make the scenario easier.
2. Before changing the failed branch, make a byte-only stopped input snapshot or
   equivalent immutable complete preservation of its actual private state files,
   plus raw size/SHA inventory. Keep private fixture files local. The preserved
   copy is not opened as a second runtime. Resume only the original active branch.
   Preserve the old logs, schedules, case JSON and original failed result; use new
   case-specific trace/log/schedule paths and do not overwrite a.stderr.log.
3. Bind input to F-obligations-02 source/image/case records and this decision.
   Inside the charged attempt, record the actual starting C/roles/D/E/SID/key
   fingerprints, counters, debt/pending ranges, public config and policy/locator
   generations using legal typed/public reads. Close any diagnostic repository
   before starting the owner. Verify the claimed input rather than assuming the
   failure-time status was the exact last committed state at shutdown.
4. Expect the authentic65 saved source debts and previously observed56 pending,
   with head5 held and the existing C2. If final persisted state differs, record
   the exact difference and select only a valid continuation; do not manufacture
   the missing test condition. Contradictory scope/keys/provenance requires stop.
   Normal restart may publish fresh monotone locators through the real public path.
5. Use bounded time-stamped owner/RPC/Complete traces to observe the next delivery.
   If it reaches actual64 pending, continue the remaining full64+reserved-head,
   O/A/F/R, exact debt/quarantine and C2 assertions where they fit the same150s.
   Keep correctness assertions and existing per-operation product timeouts.
   Do not silently lengthen the25s condition or equate partial progress with PASS.
6. A resumed case PASS can establish its explicitly executed assertions and real
   recovery from the preserved input. It does not relabel F-obligations-01/02 or
   prove that their original enqueue-to-delivery duration met the condition.
   Do not concatenate prefix/suffix into whole P or a claimed fresh aggregate PASS.

The result must identify the last completed boundary, actual wall/monotonic times,
exact source/image/input hashes, beginning/end logical state, assertions executed
or still outstanding, process cleanup and the updated immutable charged record.
If progress is still incomplete, return the evidence rather than performing
another speculative repeat. If the cause is controller timing, distinguish it
from runtime throughput and propose a justified validation change for root review.

After the attempt root will re-evaluate the remaining obligations/isolation/
affected-capacity checks and long-origin reserve using actual timings. The long
whole900 supervisor and readiness callback remain required; no511 generation,
S5.3, implementation acceptance or commit/push is authorized by this decision.
