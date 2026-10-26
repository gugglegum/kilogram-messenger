# Ready field attempt 01 — local negative-control construction failure

2026-10-26. Actual one matching test FAIL46.9119333s; work140/whole160
did not expire; owned children after cleanup0 and no fixed field/harness
executables remained live. No subsequent F was started. The first arity case
failed its25s receiver-marker wait; none of the eight Ready rejection assertions,
later valid exchanges or genuine scoped-fork/renewal assertions passed.

Case11680B SHA256
9d3419460b7d031833ed904b705cbbe87be4424fef75f2819eea447bb66f5c60.
All51 recorded source/image hashes still match. Native execution was ordinarily
approved through require_escalated. This is distinct from the prior10013 failure.

The stopped branch `.tmp/migration-s5-storage/l1-review1-ready-7zvKOX` is retained.
Only existing text logs and source were inspected after failure, not its DB.
Exports preserve sender and receiver stderr byte-for-byte:

- sender303B SHA256
  911ab88c1975a915b96bbbdc0bab662cdad81eceb69eb8426a849c03f7b13a88:
  repeated `l1_outgoing_closed slot=0 OD1 array arity`;
- receiver342B SHA256
  429327a01181c060055afd3e355d4a52fbca31dbbdc7e5a93023a667b297eebb:
  repeated peer reconnect closes; no expected Ready refusal marker.

Source diagnosis: test_support.rs ready_frame removes one inner Ready field for
the arity fault, then calls wire::signed_control. The production protocol helper
at ordered_dialog_v1.rs431 requires body.fields(8) before signing. Consequently
the debug sender refuses construction locally, consistent with its log, before
transmitting that malformed Ready. This failure therefore does not test receiver
Ready behavior and is not evidence that the R3 production correction failed.

A second controller expectation also needs narrowing: on actual ingress,
wire::control checks the inner8-field envelope before ready_claims, whereas the
`Ready shape/scope refusal` context currently wraps only ready_claims. An actual
malformed-arity control will be refused by the earlier parser with the ordinary
arity error. The negative test must observe the receiver's corresponding actual
dispatch refusal rather than require the later parser's marker for this case.

Proposed focused correction, awaiting root decision: in the debug-only malicious
transmission hook, sign/encode the canonical control envelope directly with the
actual authorized DeviceIdentity for this one malformed arity case. Verify the
constructed signature against its exact unsigned envelope. Preserve production
signed_control/control validators and their normal8-field requirement. Other
faults continue using signed_control. In the controller, require the actual
receiver l1_dispatch_rejected arity marker for arity; retain the existing scope
marker for the other seven, and all unchanged block/Native/valid-after/fork/
renewal/reopen assertions. No generic signer IPC, private-state mutation, timeout,
batching, or product admission-policy change is proposed.

Any change to compiled debug input requires a newly bound image/harness and static
checks; prior image is not evidence for the changed hook. Genuine own-F02 and
unacked01 remain exact historical corrected PASS with their source/images. No
automatic Ready02 or other F is authorized after this failure.

Preceding unacked01 actual one matching test PASS115.07523s (test101.27s),
case SHA256bc5b98b29d16a175ebeabfc16498a1d1d0cb2785d07ef3d909b7843ae710b5bc;
parent-result SHA256552b31096c6a5c505690eeb22eaf98882d5a90c89ce47122458d891f8c5cc17c.
It actually established127+4 ->128+3, no additional packet another turn/reopen,
and one exact receipt permitting one packet ->128+2. Work220/whole240 did not
expire; owned children0. Imported genuine4/4 original was only byte-copied.

Charged C126.2916571/F1216.3484525/P0; ceilingsC500/F2900/P0;
remainingF1683.6515475. A newly authorized Ready160 + isolation100 + capacity45 +
long-entry reserve1215 would total1520, leaving163.6515475. No budget increase is
requested. Both failed attempts remain charged. Long511/publication/cut3 remain
closed; the disclosed terminal-publication timing boundary still applies.
