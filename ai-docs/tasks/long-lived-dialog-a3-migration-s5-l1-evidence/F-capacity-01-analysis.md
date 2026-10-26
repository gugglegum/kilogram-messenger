# Capacity01 — completed test, runner matching false negative

2026-10-26. Whole contributing F-capacity-01 remains FAIL34.7554779s, exit1.
Work40/whole45 did not expire; owned children after cleanup0. No subsequent
contributing run was started. The runner's failure must not be silently changed
to PASS or removed from accounting.

Case12206B SHA256
6121aaeded1b847fdfc219d577e26c40a4b8f5b37486c0fa461fc3b0c990f14d.
Full log6622B SHA256
0856083c34de6b4034b7b36707a4a21f6f81e0aca24fde9d7574013ec68cc709.
All51 recorded source/image bindings match; no fixed field/harness processes
remained in the available inventory. Native execution was ordinarily approved.

The actual root-module test
`migration_s5_process_capacity_crash_and_predecessor_fences` completed and libtest
reported one passed/zero failed in20.87s. Both actual parent-kill phases printed
their M08/M09 PASS: capacity-staged and capacity-committed. The assertions cover
old/new atomic capacity outcomes, exact retry/CAS/profile update, unchanged funding
and identity, actual archived S3/exact S4/d9b Root7 fences, positive predecessor
controls and negative corruption/profile cases. The stopped branches are
`.tmp/migration-s5-storage/capacity-version-g2Y9gw` and
`.tmp/migration-s5-storage/capacity-version-ajJiky`. They were not reopened by this
post-run diagnostic; only existing log/source text and hashes were read.

The remaining harness also finished; the wrapper printed status=passed and its
ordinary cache diagnostic completed, including unchanged no-prune/no-vacuum
flags. Unlike Ready02, no deadline expired and no cache diagnostic exception
occurred. The subsequent runner matching assertion caused exit1.

Confirmed runner defect: the introduced exact-name pattern requires a `::`
module prefix before every selector. All prior focused cases were nested module
tests. Capacity is a root-module test, whose actual libtest prefix is
`test migration_s5_process_capacity_crash_and_predecessor_fences ...`; requiring
`::` incorrectly refuses it. The actual selector was run, not a zero-test match.

Proposed narrow policy fix, awaiting root decision: permit an optional complete
module prefix before the exact escaped selector:
`(?m)^test (?:[^\s]*::)?<escaped selector> \.\.\.`.
Keep the exact-name delimiter and separate one-passed/zero-failed and wrapper
status checks. Static evaluation over the immutable actual log yields old
matcher=false, proposed matcher=true, one-test-PASS=true, wrapper-passed=true.
No runner/product/controller/deadline source change has been made after failure.
A future correction must preserve this historical FAIL and its original hash.

Immediately preceding F-isolation-02 was a whole PASS53.1324232s,
work85/whole100, no expiry, children0, ordinary cache finalization completed.
Case SHA256bb10d5256bbf8087e24a558c7ed4050dc1c96f2390d0c2e4495be5bda4257d70;
exported parent SHA256
9efe68dbecfbe52c968425541ed6b2474ba5b4358d2462291d848d12a2e9cc55.
It re-established genuine different-peer C2 isolation/progress after signed C1
fork and shared-authority update on the corrected69744fca image.

Current C147.8846192/F1444.3918456/P0; ceilingsC500/F2900/P0;
remainingF1455.6081544. All costs remain charged. Long-entry reserve1215 fits
with240.6081544 above it, but first511 remains separately unauthorized.
Ready02's whole FAIL/accepted functional scope is unchanged. The common cache
status step succeeded in both subsequent ordinary wrappers without a bypass.

Root must decide whether to accept the completed capacity assertions under this
explicit runner false negative and authorize the narrowly checked matcher fix,
or require other focused evidence. No automatic rerun/fabricated PASS, publication,
whole-P or cut3 is authorized. The terminal-publication timing disclosure remains.
