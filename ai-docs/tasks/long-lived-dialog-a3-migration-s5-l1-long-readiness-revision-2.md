# A3-S5.2 first-long readiness revision 2 — fresh reconnect barrier

2026-10-26. Response to long-readiness-review-1,5714B, SHA256
c492cbe874cdc5460f56fbefd42569229fef913f40471fec412c5df7cb24de6e.
This is controller-only preparation for root review. No origin/boundary execution
or other contributing C/F has been performed. All execution/publication gates
from the original readiness remain closed.

The original readiness document13088B/SHA256
220b3cc1a74af235c6979b17c4e0ea2757f72aa2c8b1b6c758287beaae89b63a
and original binding22114B/SHA256
9c2e3ccb053169a60a249074397521f7284bd73dfbefa5e3def276a89f1057cc
are unchanged. Read this delta together with those preserved origin/provenance,
history/receipt/checkpoint, command and accounting details.

## Preserved original and exact controller delta

Before editing, the20858-byte controller was copied byte-for-byte to the
noncompiled snapshot
`long-lived-dialog-a3-migration-s5-l1-evidence/S-long-readiness-v1-controller.rs.txt`,
SHA256feaf65e3e6698ff56d5fad69c89f5d5423efb22f6d44b8da30c299e45000bf61.
The old13648896-byte compiled harness is also preserved, not executed, at
`target/compatibility/long-lived-dialog-migration-readiness-v1/migration_s5_process-harness.bin.snapshot`,
SHA256dba6d218adbdbe6a9c34add3419157b15bb6cced01c958f11cb3d5b9b899314b.
The preservation record is S-long-readiness-v1-preservation.json. Original
compile18 provenance and all earlier evidence remain immutable.

Only one compiled source changed:
`apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs`, now26779B,
SHA256a8077b49ba2e02956a94ca352158190854f47d3a6b37b2e624b2bb81743e71f4.
The actual diff against the snapshot is
`long-lived-dialog-a3-migration-s5-l1-evidence/S-long-readiness-r2-controller.patch.txt`,
8510B, SHA2561511f642203af92742c6e2fe4aa091b264022a26350a057d4787034355b60548.
No production protocol, shared Node wait predicate, scheduler implementation,
runner/deadline policy, source factory or fixture has changed.

## Barrier before the final both-indexed zero-replay assertions

The prior stored-frontier Node::wait calls no longer stand in for reconnect.
Final A starts with a new external context-scoped schedule containing only
trace_rpc=true; B keeps ordinary scheduling. The A log path must be absent before
that invocation and is freshly created by its actual child. Both child handles
are checked alive during the barrier.

Capture each restarted owner's newly published own Locator revision before the
two ordinary imports. One local25-second bounded wait requires:

- Both relevant C contexts have lease_current, a positive lease generation,
  publication_ready and no current policy error. Each new process reports at
  least one admission. Own Locator revisions match the captured fresh values;
  each peer revision matches the other restarted owner's own revision.
- Once that pair is observed, retain the two lease generations/admission counts
  and a fresh-log line offset. If either admission/generation changes or the pair
  ceases to be current, reset that observation. Do not use an earlier log success.
- Under a stable pair, require a later exact entire A log line of the actual form
  `l1_rpc kind=9 stage=finished millis=<ASCII digits> result=reply9`.
  Begin/write/lease-only lines, error replies, wrong kind/result, suffix/prefix
  lookalikes and an old process log cannot satisfy this completion predicate.

After the barrier, issue fresh A and B status requests. Recheck the current
pair/Locator correspondence, unchanged lease generations and admission counts,
and the expected512/18 frontiers with debt/queue0. Only then run the unchanged
Session/D/E/SID, packet0/constructor0 and Native-hash replay assertions. All later
refusal/retry/body-conflict, exact history/frozen suffix and actual receipt checks
remain in their original order and scope.

The future parent result contains reconnect_barrier with the actual A/B PIDs,
current public contexts/admission counts, expected Locator revisions, stable
generation/admission token and bounded elapsed. It also records the fresh A log
path, line offset, exact completion line/one-based line number/millis, and a
byte-for-byte prefix snapshot at indexed-reconnect-barrier.log with size/BLAKE3.
The stopped checkpoint backup retains that log evidence. No bearer or private
Native material is exported. This describes implemented future assertions, not
a completed connection or a measured field result.

The25-second local barrier does not raise the proposed boundary work120/whole140
limit or origin work850/whole900. It changes no product timeout, batching, packet
schedule or admission policy. A failure still stops the future attempt and is
charged in full; there is no automatic retry.

## Final static binding and unchanged execution boundary

Strict all-targets Clippy23, compile-only19 and fmt-check25 PASS under the existing
locked/offline verification workflow, Jobs12/BelowNormal. No test executable was
started. Compile18 is superseded for the new controller, while its original raw
record/harness provenance is retained. The new stable migration_s5_process harness
is13683712B, SHA256
5ccc3a8a3e4bc76ed811f6d0aad5c93b95381bbab8d6a2c1284b3d66c8eb5ce2,
at target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe.

All52 final compile inputs match; compared with compile18 only the exhausted
controller differs. All33 ordinary build11 production inputs and every fixed/
archived image in the original binding still match. Both field EXEs remain
70226432B/SHA25669744fca50f44439b4734e699f416fc6b2bb8aac4336e5dad18ed173b2abbf6c.
There is no new product build or restaging claim.

New binding:
`long-lived-dialog-a3-migration-s5-l1-evidence/S-long-readiness-revision-2-binding.json`,
23657B, SHA2564116a5c3fc17ffb0c8b2efca0e71db98a4f22ff42ff091de3eb73ac94b0a3f67.
It binds the authority, immutable originals/snapshots/delta, new source/harness,
static records, all current compile inputs, unchanged product/images and ledger.

HEAD remains73c499818ba9b699c331aa842e64e8432e90c59a and index empty.
Runner remains23612B/SHA256
c9c6841213181af75f3364128cf7d0697d45db5a7a8bf5bfda1b58986c3211ed;
reviewed deadline helper remains0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45.
Live ledger remains SHA256
5218356ef3d08914f1ce1c839da558a519a6ff33455f2635b7345f36a7b725c5:
C147.8846192/F1444.3918456/P0, ceilingsC500/F2900/P0,
F remaining1455.6081544 and long reserve1215. Historical Ready02/capacity01 whole
FAIL records and their exact limited acceptances are unchanged.

The original prospective commands/selectors and root approval-file bindings stay
the same. The origin directory and origin started records are still absent.
Only a later root go-ahead can authorize first511; boundary needs its own decision
after actual origin result/accounting. No origin/boundary, new C/F, whole-P,
commit/push, product deployment or cut3 is authorized by this revision. Stop for
root review of the corrected controller and static binding.
