# A3-S5.2 field continuation 2 — correct the malformed-Ready fixture

2026-10-26. Root continuation of
[implementation review2](long-lived-dialog-a3-migration-s5-l1-implementation-review-2.md)
and [native-execution continuation1](long-lived-dialog-a3-migration-s5-l1-field-continuation-review-1.md).
The contract, R1-R3 production corrections and publication gates do not change.
Standing human authorization covers the scoped correction/test/review cycle.

## Verified evidence and diagnosis

Root accepts F-review1-unacked-01 as a scoped corrected field PASS: 115.07523s,
one matching test, no guard expiry, zero owned children after cleanup. Case hash
`bc5b98b29d16a175ebeabfc16498a1d1d0cb2785d07ef3d909b7843ae710b5bc`;
parent-result hash
`552b31096c6a5c505690eeb22eaf98882d5a90c89ce47122458d891f8c5cc17c`.
Root checked actual states: debt127/queue4/packets127 -> debt128/queue3/packets128;
another turn and cold reopen preserve Native and queue; restart packet count0;
one exact receipt permits one packet, debt128/queue2. This is not whole-S5.2
acceptance and does not cover the separate Ready-negative scenario.

F-review1-ready-01 remains FAIL, 46.9119333s, case hash
`9d3419460b7d031833ed904b705cbbe87be4424fef75f2819eea447bb66f5c60`.
All recorded source/image hashes still match. Root inspected the
[analysis](long-lived-dialog-a3-migration-s5-l1-evidence/F-review1-ready-01-analysis.md)
and the actual protocol and debug-hook code. The hook removes one inner Ready
field, then calls production signed_control, whose fields(8) validation refuses
before signing. Sender logs report array arity; this attempt never established
the intended receiver rejection. Ingress control likewise rejects inner arity
before ready_claims, so its receiver marker differs from the later scope marker.
The narrower diagnosis is supported; no R3 product failure is established here.

Preserve failed01, its started marker, raw logs and stopped
`.tmp/migration-s5-storage/l1-review1-ready-7zvKOX` unchanged. Do not turn it into a
positive origin or reopen/repair it for this new attempt.

## Authorized narrowly scoped correction

For the existing debug scheduler's **arity fault only**, construct the canonical
inner control envelope directly: unsigned [1,3,malformed_body], signed under the
existing CONTROL_SIGN domain with the actual fixture peer DeviceIdentity; encode
[1,3,malformed_body,signature]. Verify that signature against the exact unsigned
bytes. The outer Frame retains its real binding/request and valid signature.
Other fault variants continue using the ordinary signed_control helper.

Keep production signed_control/control validators unchanged, including the exact
eight-field rule. Keep the external debug-only scheduler gate, fixed fault enum,
existing size bounds and ordinary receiver policy. Do not introduce an IPC signer,
generic negative-control API, private-state writer, timeout/batch change or
production admission exception. Do not widen production visibility for tests.

Update the arity controller to require a new actual **receiver**
`l1_dispatch_rejected` record containing the arity refusal during that fault
phase. A sender construction error or connection failure is not that evidence.
Retain the existing scope-refusal requirement for the other seven variants and
all unchanged block/Native/valid-after/scoped-fork/prior-Ready/renewal/reopen
assertions. Fresh genuine lineage and current stopped-copy ownership rules apply.

Add one focused component check through the actual construction helper used by
ready_frame, not a duplicated test implementation. It must establish valid outer
Frame parse/signature, exact valid inner signature, intended seven-field Ready
body and rejection by the unchanged production control parser. A control with
normal shape must retain normal behavior. This catches local construction failure
before another field run; it is not evidence of receiver integration.

## Verification and one replacement attempt

Record required fmt, strict Clippy, compile-only, field build/stage and exact new
source/image/harness bindings. Compiled debug-hook changes require a newly bound
image; do not use build10 as if it contained the fix. Preserve old images and
never recopy a live EXE. Use unchanged ordinary production function comparisons
to assess reuse of own-F02 and unacked01. No repeated broad suites are required
solely because the negative Ready fixture changed.

Authorize one **C-ready-arity-fixture-01** with exact focused selector
`indexed_l1_ready_arity_fixture`, kilogram-cli, at least one actual matching test,
work30/whole40, within unchanged C500. Use the stable wrapper, locked/offline
verification and Jobs12/BelowNormal. Preserve/charge a failure and callback
before further contributing execution; no automatic retries.

After the scoped static checks and that C PASS, authorize exactly one fresh
**F-review1-ready-02** with the unchanged exact field selector
`migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork`, work140/whole160.
Use ordinary approved native execution per continuation1. This is not permission
to alter Windows security configuration or bypass an approval rejection.

The narrow runner policy may add this C gate and replace active Ready01 with
Ready02, binding this document and the earlier review/continuation. Retain exact
prior own-F02 PASS, unacked01 PASS and failed Ready01 as immutable provenance.
Serial gates for isolation02/capacity01 must require actual Ready02 PASS; do not
rename any historical case. Retain matching-test, started-marker, source-freeze,
work/whole cleanup and remaining-upper-bounds plus1215 checks. The already tested
deadline mechanism itself remains unchanged; parse and hash the policy update.

After an actual Ready02 PASS, the previously authorized isolation02 (work85/
whole100) and capacity01 (work40/whole45) may continue serially, with their original
selectors and conditions. At any failure, expiry, integrity/binding issue or
approval rejection, preserve/charge the attempt, finish owned-child cleanup and
callback root before another F. No automatic third Ready attempt is authorized.

Current actuals C126.2916571/F1216.3484525/P0; ceilings C500/F2900/P0 unchanged.
Failed Ready01's 46.9119333s remains charged. F remaining1683.6515475 covers
Ready160 + isolation100 + capacity45 + separate long reserve1215 =1520, leaving
163.6515475. No additional allocation, refund, P borrowing or assertions reduction.
Keep the disclosed terminal-publication timing boundary explicit.

Callback with actual results and corrected hook/source comparison, then the
separate exhausted-boundary readiness package. Long511 remains unauthorized until
root independently reviews and grants its exact runner/ledger-bound approval.
No whole-P, commit/push or cut3 yet. Preserve this new root decision and prior
contracts, reviews, evidence, genuine originals and unrelated working-tree work.
