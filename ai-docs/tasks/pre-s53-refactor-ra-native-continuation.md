# R-A: one NativeToken legacy attempt and the unstarted field case

2026-10-26. Baseline `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`.
This supplements [runner continuation](pre-s53-refactor-ra-runner-continuation.md)
without changing the original R-A ceiling or activating R-B/S5.3.

## Evidence and decision

Root read `C-ready-legacy-01` log and case. This Restricted attempt is whole FAIL,
exit 1, 10.2520644 measured seconds. Its named kernel failed 0/1 in 0.02 seconds
with `device state I/O failed`, Windows access denied (os error 5). The Ready
assertions did not complete. Source/image equality held, the work deadline did
not expire, and owned-child cleanup/query verified zero children. The repaired
terminal accounting completed and retained the failure.

Case SHA256 `e8212418ddd62cd13c660c717379ed03b46ac74ca8fa10e0c597f5d79bab2735`;
log `194d2262dc09bd15e36843ac7d0087c31da5d44ab1c47a12e3b8159cbdc05890`;
r2 ledger `9e03cf909514390205c965d9dfe902cdb14bc64a62fdd40eb82534294e61f118`.
These and all preceding artifacts remain immutable. The identity I/O error is
also used by account creation; the log does not identify the exact path/syscall.
Restricted execution is a plausible explanation, not a proven cause or product
acceptance.

Authorize one comparison under the user's standing local-test/NativeToken
permission, through ordinary tool execution review, with unchanged Rust, genuine
test setup and images. Do not weaken device-file permissions, change system
settings, replace genuine setup with synthetic state, or bypass a tool rejection.

| Only allowed remaining case | Mode | Work / whole seconds |
|---|---|---:|
| C-ready-legacy-02, same exact legacy selector | NativeToken | 35 / 45 |
| F-ready-scope-01, unchanged public-dispatch selector | NativeToken | 180 / 210 |

Prior charge is conservative C60 plus measured C10.2520644 = **C70.2520644**.
Remaining allocation is **C49.7479356/F210/P0**; the C45 whole ceiling fits it.
The original R-A C120/F210/P0 ceiling and inactive R-B C60 reserve are unchanged.
There is no further automatic retry. Stop on either failure or a provenance,
deadline or tool-approval problem and return the actual evidence to root.

## Minimal continuation setup

Retain the executed c1fce94b... adapter as a raw source snapshot before edits.
Prepare a separate r3 binding and r3 ledger referring to both prior failures and
their distinct measured/conservative charges. Preserve the r2 ledger and binding.
Restrict the adapter to the two cases above and require the declared NativeToken
mode for both. Keep the reviewed execution, parsing, cleanup, accounting,
terminal-failure guards and deadline helper unchanged.

This is a literal case-policy/binding-path continuation, not a new implementation
body. After checking that only those policy/path changes occurred and that all
Rust/build inputs and images still match, the executor may run the two authorized
attempts without another root round trip. No rebuild is needed. A change to the
actual execution/finalization algorithms requires source review before running.

After success, submit the complete R-A candidate for root implementation review,
retaining the first parser attempt as whole FAIL with accepted narrow assertions
and the first legacy attempt as a genuine failed setup. No commit/push before
root acceptance; no S5.3 or R-B implementation yet.
