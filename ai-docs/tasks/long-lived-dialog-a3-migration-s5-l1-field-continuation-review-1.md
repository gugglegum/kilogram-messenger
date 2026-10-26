# A3-S5.2 field continuation 1 — one native-execution retry

2026-10-26. Root continuation of
[implementation review2](long-lived-dialog-a3-migration-s5-l1-implementation-review-2.md).
The contract, corrected candidate and all acceptance boundaries remain unchanged.
Standing human authorization covers the focused verification and review loop;
execution outside the sandbox still goes through the ordinary tool approval layer.

Root inspected the actual failed log and
[analysis](long-lived-dialog-a3-migration-s5-l1-evidence/F-review1-own-f-01-analysis.md).
F-review1-own-f-01 failed in frozen CLI locator-import while connecting to local
runtime IPC, with Windows error 10013. The one matching test ran, but failure occurred
before the partial-phase marker, authority advance, migration and own-F assertions.
This is an environment-refusal observation, not proof of a product defect or of
the particular Windows policy responsible. A restricted-token loopback limitation
is plausible; one otherwise unchanged native-execution attempt is justified.

The 11184-byte failed case SHA256 is
`43b7cf2e3e7815ef1c258c7f502ec705a4df2f281ad077b2d1d588f4f3210c0c`.
All recorded source/image hashes still match. Work100/whole120 did not expire;
owned children after cleanup0. Retain the failed result, started marker, logs and
stopped branch `.tmp/migration-s5-storage/l1-review1-own-f-0-XrlSo6` unchanged.
Do not repair or reopen that branch as the new positive origin.

Authorize exactly one new case **F-review1-own-f-02**, with the same exact selector
`migration_s5_l1_review1_genuine_partial_own_f_both_genesis_roles`, F/Tests,
kilogram-cli, fresh genuine origins, work100/whole120, stable wrapper,
locked/offline verification and Jobs12/BelowNormal. Request native execution via
the normal `require_escalated` tool approval mechanism, explaining the observed
local IPC denial. This is not permission to change Windows Firewall, ACLs, security
software, product code, test assertions, timeouts or executable paths. If approval
is rejected, do not work around that decision; report it and retain the pause.

A narrow runner-policy amendment is authorized: replace the first active sequence
entry with F-review1-own-f-02, bind this continuation's hash as well as review2,
and require the exact failed 01 case above as provenance. Do not delete, renumber
or relabel 01; never reuse its ID. Subsequent serial-pass gates must require 02 PASS
instead of 01 PASS. Keep the actual-matching-test check, remaining-upper-bounds
plus1215 gate and immutable started/final records. Parse the amended runner and
record its hash. The deadline mechanism itself remains unchanged:
`0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45`.
No new product build or repeated component test is required for this policy-only
change; record unchanged source/image/harness binding explicitly.

After an actual 02 PASS, the other four unstarted groups from review2 may proceed
serially, with their unchanged IDs, selectors, work/whole guards and reserve
conditions. Use the same ordinarily approved native execution for these local
network harnesses, recording execution mode. At any failure, guard expiry,
integrity problem or execution-approval rejection, stop and callback root before
another F. No automatic third own-F attempt or other substitution is authorized.

Actual charges are now C126.2916571/F988.0877646/P0. The failed 9.0392997 seconds
remain charged. Ceilings stay C500/F2900/P0; no new allocation or refund is made.
Remaining F1911.9122354 covers the unchanged 1880 forecast with 31.9122354 slack.
Recompute this before execution and preserve every earlier case byte-for-byte.

Long511 still needs separate readiness and explicit approval; the whole900 guard,
minimum remaining1215, immutable original and single-active-branch rules persist.
No commit/push, cut3 or whole-P run. Callback with actual results or the next
concrete failure; the human need not copy messages between chats.
