# R-A: exact negative-arity fixture correction

2026-10-26. Baseline `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`.
Supplement to [environment continuation](pre-s53-refactor-ra-environment-continuation.md).

`C-ready-legacy-03` is whole FAIL/exit1, measured 11.9990029 seconds; its kernel
failed 0/1 in 4.08 seconds with `OD1 array arity`. The environment correction
removed the previous empty-path failure. Root and executor independently found
the new test's preparation error: it creates a seven-field body and calls
`wire::signed_control`, which correctly requires eight fields before signing.
This aborts before that malformed control reaches Engine::ready.

Preserve raw case SHA256 `437b61c69448ffdd62a82b2653cb147e7ab917a1053c9ea6de4e67db354b5ddc`,
log `c33816cc31992909953187705fad42a4ed1b72639e28b2e45083cccf6fc80693`,
and r4 ledger `24cfc8364933bda1836bd8fda9d4c7bd8c0e61ad8bcdc692a25108422bc780f5`.
No Ready success is claimed for this failed characterization.

## Exact allowed correction and verification

Change only the new characterization in `ordered_dialog/repair_tests.rs`:
for its exact seven-field negative body, use the existing debug fixture's pattern
from `test_support.rs:145-152`: encode unsigned version1/kind3/body, obtain actual
CONTROL_SIGN signing bytes, sign with the genuine peer device, verify that
signature, and encode the four-field control envelope. Keep normal eight-field
cases on the normal production signer. Retain all malformed cases, both roles,
unchanged refusal/fork/Native/witness/reopen expectations and the genuine Pair.
Do not weaken or edit production signers, parsers, Ready handlers or test_support.

Retain the executed fc007719... adapter and affected current test-harness images
before replacement using raw non-executed snapshots. No private account/fixture
copy or execution is part of that static preservation. Keep the old evidence and
source bindings intact.

Run source-bound fmt-check, strict locked/offline CLI Clippy and compile-only
stable harness staging for this test-only source change, Jobs12/BelowNormal.
Run compile-only last so its staged harnesses are current before contributing
execution. Retain every static attempt. The product CLI/control images and their
production Rust inputs are unchanged; do not rebuild them needlessly.

Submit the small test diff and new readiness binding for root review before
executable verification. The only case-adapter changes are literal case policy,
binding and ledger paths. Keep the reviewed environment helper and all execution,
deadline, accounting and failure handling unchanged.

## Remaining allocation

Prior charge is conservative C60 plus measured C33.5614112 = **C93.5614112**.
Before any C04 execution, root revises the active R-A ceiling from C120 to
**C150/F210/P0**, leaving **C56.4385888/F210/P0**. This is an explicit additional
C30 allocation, not a change to past measured/conservative charges. Keep the old
ceiling in historical ledgers. The inactive R-B C60 reserve remains unchanged;
the revised overall refactor ceiling including that reserve is C210/F210/P0.

Reason: the earlier successful parser kernel's 43.6945432-second sample included
22.53 seconds of compilation and about 21.16 seconds of other combined work.
The failed legacy attempt did not reach the stable wrapper's normal finalization.
A complete two-role test plus that normal finalization needs more margin than
the previous proposed C20/25 allocation. This estimate identifies no performance
regression or specific overhead bottleneck and does not authorize extra attempts.

After root source clearance, a separate r5 binding/ledger may run only:

| Case | Mode | Work / whole seconds |
|---|---|---:|
| C-ready-legacy-04, same exact selector | NativeToken | 40 / 50 |
| F-ready-scope-01, still unstarted | NativeToken | 180 / 210 |

The single C50 whole ceiling fits the revised remaining allocation. No automatic retry
or borrowing from R-B. If C04 fails or overruns, stop before F and report the
evidence; if C04 passes, perform the already assigned F once. Tool review and
standing human permission still apply.

After both pass, submit the complete R-A implementation and candidate for root
acceptance before commit/push, with all four earlier whole failures and the
accepted narrow parser assertions explicitly retained. R-B/S5.3 remain inactive.
