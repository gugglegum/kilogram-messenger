# C2-I reduced resource caps accepted

2026-10-31. Supporting synthetic evidence accepted under [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md), [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md) and [simplified workflow](../../verification-workflow.md). Whole S5.3, G1 and S5.4 remain open.

[Source review](C-s53-cold-core-resource-caps-01-attempt-01.source-review.md): root cc4246 independently inverted four closed Core changes to published parent 766289555b68680537ef30ba8abeeb90643b4c49 and verified inventory bytes/bindings and record/path models. AST0, twelve common oracle bodies and supervisor/lifecycle deltas were checked; production limits and old selectors are unchanged.

One [allocated](C-s53-cold-core-resource-caps-01-attempt-01.root-allocation.json) work20/whole30/lane1/retry0 run, without source corrections or retries. Executor turn 01a18f2e-cb0a-7163-ba2c-3b0bb78b8172; actual app item exec-1ed677ce-e768-4d6d-92a0-86b343c3b364, chunk c2307f, exit0, native4.8302542s/app4830ms; complete output, no native session/continuation or orchestration yield. [Full return](C-s53-cold-core-resource-caps-01-attempt-01.run.json) matches the independently read app item/command/output. Initial/before UTC2026-10-30T23:39:43.347Z; complete return23:39:50.367Z: request-through-return7.020s. Worker3.8888775s and supervisor4.5096116s overlap and are not additional charges.

[Cases](C-s53-cold-core-resource-caps-01-attempt-01.cases.json) and [watchdog](C-s53-cold-core-resource-caps-01-attempt-01.watchdog.json) confirm:

| Case | Exact refusal | Acquired pre/final | Final peak handles / remaining |
|---|---|---:|---:|
| row-cap | row_count_cap / inventory / inventory | 0 / 0 | 27 / 0 |
| entry-cap | entry_count_cap / inventory / inventory | 0 / 0 | 27 / 0 |
| handle-cap | handle_cap / capture_bkp / bkp | 5 / 8 | 32 / 0 |

All six Core observations were independently checked across all38 fields, exact three-exception semantic chains/native0/bounded_refusal, QPC order and cleanup. All cases locks6/capturedfalse/captureQPC0/hashpasses0/hashbytes0. Row/entry history null; handle retains historical H; all four content digests null. Handle case reaches the32nd current file handle and refuses the temporary33rd stream slot; current and registered handles close. This is reduced-cap behavior, not production or OS exhaustion; entry-cap is not separate directory_count_cap evidence.

Each case has two healthy physical/content/ACL/no-follow checks,39 file results closed,30 enumerators disposed and15 controller guards closed before factory. Geometry46 dirs/39 files/169 stream observations/11018 default bytes/named0. Global guard ledger50/50/0. Fresh4types, three initializers once, compiler_errorfalse/diagnostics[]. Child31224 exit0/reaped, both EOF, stdio/managed inputs disposed, children0, no forced termination/output overflow/unknown cleanup.

Root359713 verified four complete JSON report pins and nine public input pins before/after/live. Native reports26782 bytes total, each within65536; raw bytes preserved. No private data, keys, network, Rust, native false-close or whole-attempt acceptance claimed.

Accounting CLOSED once: C30 charge7.020/release22.980. S5.3 C1556.3764909/F1562.2882567/P0; combined C2143.8251018/F6278.6084056/P0. Ceilings C1580/F1642/P0; active0; inactive C23.6235091/F79.7117433/P0. Historical pure r75 remains separately accounted. The activated allocation remains immutable historical evidence, not a new permission.

Publication scope: Core plus this attempt's worker, watchdog, source-review, allocation, three native reports, run collector and this note. Preserve unrelated worktree changes and navigation; no new test run needed for publication.

