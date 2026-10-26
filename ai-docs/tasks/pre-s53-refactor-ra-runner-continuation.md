# R-A runner failure: scoped acceptance and continuation

2026-10-26. Baseline remains `eafd0f873eb31b64377e8e65e06bc84a911f6dc6`.
This decision supplements [plan review](pre-s53-refactor-plan-review.md).
No R-B or S5.3 work is activated.

## Observed first attempt

`C-ready-scope-01` is **whole FAIL**: the wrapper process exited 1 while
aggregating its terminal ledger with `Measure-Object -Property elapsed_seconds`.
The current record is an OrderedDictionary rather than a normal property object.
Root's small in-memory PowerShell diagnosis also showed that a mixed object/
dictionary collection can omit the dictionary row from this aggregation.

The named indexed parser test completed 1 passed / 0 failed in the actual log,
and the stable wrapper returned 0. The draft case recorded exact source/image
equality after execution, a successful owned-child cleanup/query with zero
children, and no work expiry at its sample. Those observations do not establish
whole completion: the next accounting step failed.

- Draft case SHA256: `ce888144ce10b607ebac8ba5da9efcf80736d8ec2dd709c4b947207881fc3005`.
- Log SHA256: `7cff4984426e7a6a8d17bfae74af21e91591b305b85aa7790abb0861d8b62b5d`.
- Unfinished ledger SHA256: `50fb687f4f5ffc60211353425992ddab350d949abc06770da76542ccc2901d18`.
- Executed case adapter SHA256: `d9b1a26c05ce42661322632fe26aa95302576b25e7ace4323c93467ee7f92506`.
- Pre-run binding SHA256: `a561122881f4e1e951473ea9e6e062a6e39688e1e17014c3cc288357cc61ebd1`.

The sampled elapsed time is 43.6945432 seconds, a lower bound. A reliable final
measurement was not retained. Charge **C60 conservatively**, explicitly as an
allocation based on the assigned whole ceiling, not as measured elapsed time.
Keep the draft PASS field, started ledger, log, start record and original binding
byte-for-byte, accompanied by a separate reconciliation record that identifies
the actual whole FAIL and the conservative charge. Do not repair history in place.

Root accepts the completed pure parser assertions for their narrow scope on the
unchanged Rust source/image. No replay of this C test is needed: the remaining
genuine legacy characterization and indexed public-dispatch case still have to
complete independently. This is not acceptance of the whole R-A implementation.

## Narrow runner repair

Before editing, retain the executed adapter as an immutable raw source snapshot
under the new refactor evidence directory, with its exact digest above.

Normalize the current result into a PSCustomObject before writing it or adding
it to the ledger. Preserve a single object reference so all later elapsed and
verdict updates reach both the case record and ledger. Use a reliable aggregation
over every intended row; verify the mixed/round-tripped object shape with a small
in-memory PowerShell check. Preserve the actual primary error and retain a
terminal accounting/publication failure as FAIL or explicitly incomplete. A
draft success record is never an independent whole verdict.

Keep all four previously reviewed error-reporting corrections, the unchanged
owned-job deadline helper, exact stable selectors and fixed image paths. No Rust,
test body, historical S5.2 script or product image change is required. Reuse the
completed compile/build/Clippy/fmt checks with exact Rust/build-input equality;
bind the new execution adapter separately.

## Remaining allocation and source-review gate

Use a separate immutable pre-run binding `R-A-pre-run-binding-r2.json` and a new
continuation ledger `contributing-cases-r2.json`. Reference the original binding,
unfinished ledger and reconciliation record; do not overwrite them or introduce
a general permission to ignore failed cases.

The new ledger allows only the two previously unstarted cases:

| Case | Set | Work / whole seconds |
|---|---|---:|
| C-ready-legacy-01 | C | 45 / 60 |
| F-ready-scope-01 | F | 180 / 210 |

Remaining R-A allocation is **C60/F210/P0**. Together with the conservative C60
charge, the original active ceiling **C120/F210/P0** is unchanged. The R-B C60
reserve remains inactive. There is no parser retry or extra field attempt.

Prepare the narrow repair, reconciliation and new binding; send their exact
hashes for root source review before C/F execution resumes. If either remaining
attempt fails or exhausts its deadline, stop and report. After successful
completion, submit the complete R-A candidate with the whole FAIL and the
scoped parser acceptance disclosed; publication still requires root acceptance.
