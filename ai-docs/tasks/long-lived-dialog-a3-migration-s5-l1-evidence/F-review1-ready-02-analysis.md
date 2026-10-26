# Ready02 — functional test PASS, whole attempt FAIL at work cutoff

2026-10-26. This is a split result, not a whole-case PASS. No further contributing
run has started. Isolation02 and capacity01 remain unrun.

F-review1-ready-02 is immutable FAIL140.155492s, exit1, work_expired=true,
work140/whole160, owned_children_after_cleanup0. Whole deadline did not terminate
the owner; the case and updated ledger were published. Case11684B SHA256
20540287fc8e478ba72bdfc49786f8b6c6a90e02b298ce3b1fdef7745278e7be.
All51 source/image bindings match after the attempt; no fixed field or stable
harness executables remain live. Execution used approved native-token invocation.

The actual matching Rust test completed all assertions and printed its PASS,
then libtest reported one passed, zero failed,131.63s. The remaining zero-test
harness finished, and the stable wrapper printed status=passed. The wrapper then
started its ordinary Cargo cache status child. The work cutoff fired; its log
ends with Cargo cache status check failed and the runner reports the expired
work allowance. The whole case therefore remains FAIL even though the functional
test had finished. Log ordering locates this in final cache/status work; no
separate kernel child-termination event trace was collected.

The completed parent controller wrote its72040-byte result before returning:
F-review1-ready-02-parent-result.json, SHA256
fcfb7da3b292b8427f99f49e20fa55a067cc0a727f7936900179520e3edcccf0.
Its stopped active branch `.tmp/migration-s5-storage/l1-review1-ready-GMDLmk`
is retained. Only existing JSON/text logs and source were read after the failure;
the branch DB was not reopened. The exported parent result contains:

- Eight actual signed fault phases: arity,D,E,SID,role,suite,format,genesis-shape.
  The corrected arity input reached actual receiver dispatch and was refused;
  the other seven produced their actual receiver scope refusals.
- Per-phase unchanged Native/packet counts/no block, followed by successful real
  valid exchange, under the unchanged test assertions and25s ordinary waits.
- A separate same-scope genesis contradiction producing the durable signed fork;
  the saved first witness equals the prior peer Ready rather than genesis.
- Public Consent1 renewal leaves the fork and Native unchanged; a cold reopen
  also remains blocked. The actual parent completed and wrote all outcomes.

Prior Ready witness hash:
e34a5de50713febcb22e3f5712c43fed8314611399aedc759ca474b2ec3c19d3.
Fork proof hash:
4ddbf90dc6e5baa1c6d0dd6c1e35004f5a5c41ac444de8813702e8ff5f67edf5.
These are actual completed assertions, not permission to overwrite/relabel the
whole contributing case or ignore its cutoff and cost.

The preceding focused C-ready-arity-fixture-01 passed21.5929621s, actual one
matching test, no expiry, children0; SHA256
6bf73878224b84184a2190bf7caf82d60d67403972e173d918c71c38f62748f7.
fmt20/strict Clippy20/build11/compile16 passed. Clippy19's new-test unwrap lint
failure remains retained, then was fixed with fallible error handling.
Fixed CLI/control-peer70226432B SHA256
69744fca50f44439b4734e699f416fc6b2bb8aac4336e5dad18ed173b2abbf6c.
All ordinary runtime7 and closed-state production files and protocol validators
remain unchanged from the prior own-F02/unacked01 evidence; only the debug hook,
its controller and runner policy changed. Exact scoped comparison is in
S-ready-fixture-correction-binding.json.

Current charged C147.8846192/F1356.5039445/P0, ceilingsC500/F2900/P0;
remainingF1543.4960555. All failures and full elapsed remain charged, and the
terminal-publication timing boundary remains disclosed. Isolation100+capacity45+
long-entry reserve1215 would require1360, leaving183.4960555. A new full Ready160
as well would require1520, leaving23.4960555; no such repeat is authorized.

Root decision requested: inspect whether these completed genuine functional
assertions suffice as scoped evidence while preserving the whole-attempt FAIL,
and decide the next gates. There is no automatic third Ready attempt, deadline
change, maintenance bypass, assertion reduction or subsequent F. Long511,
publication, whole-P and cut3 remain separately closed.
