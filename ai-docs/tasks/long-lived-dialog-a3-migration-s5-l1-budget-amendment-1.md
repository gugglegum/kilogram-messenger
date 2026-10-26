# A3-S5.2 verification amendment 1 — F2100 and enforceable long-run gate

2026-10-26. Root decision before any further contributing run, following the
executor's budget callback. The human has already authorized autonomous S5 work,
review/fix loops and continuation; no new human permission is required.

The [implementation assignment](long-lived-dialog-a3-migration-s5-l1-implementation-task.md)
and [plan acceptance](long-lived-dialog-a3-migration-s5-l1-plan-review-2.md) remain
unchanged historical inputs. This document explicitly supersedes only their new
S5.2 F ceiling and makes the pending long-origin gate precise. Scope, assertions,
all previous charges, publication gates and the queued S5.3 dependency remain.

## Verified current accounting

Root read each contributing JSON and independently summed the current ledger:

| Set | Actual S5.2 seconds | Contributors |
|---|---:|---:|
| C |44.7378423|2|
| F |364.8719099|6|
| P |0|0|

F-small-origin-01 PASS18.7168939, F-small-interop-01 FAIL26.80717,
F-small-interop-02 FAIL50.3112762, F-small-interop-03 PASS74.5982578,
F-partials-01 FAIL39.6781966 and F-partials-02 PASS154.7601154 are all retained.
Total failed F elapsed is116.7966428. None is refunded or attributed solely to
Ollama. The environment observations remain user reports with unknown contention
onset, followed by the user's explicit CPU-clearance signal.

Root read the five PASS boundaries in F-partials-02 and the controller correction
note. The controller18-to25-second change follows the existing wait_old bound;
product timeouts were not changed. This is evidence of those assertions passing,
not acceptance of the complete implementation. Pre-migration queued requests are
created in the first three partial cases; the two phase3 cases do not exercise
that same queued-origin assertion. Report that applicability accurately.

## Allocation and arithmetic

Authorize **S5.2 F<=2100 seconds**, an explicit +300 from1800. C<=500 and P=0 are
unchanged. Published S5.1 actuals remain C279.3257298/F1236.2886628/P0. New combined
maxima are **C779.3257298/F3336.2886628/P0**. Current S5.2 F remaining becomes
1735.1280901. Do not rewrite the old plan/assignment or old per-attempt evidence.

The callback forecasts these still-required small checks (estimates, not PASS):

| Remaining check | Forecast whole-run seconds |
|---|---:|
| Policy/public renewal/IPC3/capacity |80–100|
| Atomic/history-byte boundaries |80–100|
| Full64+head/captures/quarantine/C2 |100–130|
| Different-peer C2 isolation/signed fork |70–100|
| Affected S5.1 capacity regression |30–45|
| Total |360–475|

The conservative whole forecast is364.8719099 spent +475 small checks +850 origin
+140 exhausted boundary +225 unspent contingency =2054.8719099. F2100 leaves
45.1280901 above that forecast. The proposed F1950 would leave only120.1280901
after the named upper bounds, instead of225. Root chooses the explicit +300
allocation to preserve the stated reserve rather than silently shrink it.

This does not authorize extra broad suites, automatic failed-run repeats or another
511 generation. Reuse already-passing cases only where exact affected-source
analysis supports it; rerun changed relevant paths as necessary. Every actual
attempt remains charged and can change the remaining forecast. Later cut3/whole P
allocations are still separate, and P is not spare F.

## Resume small gates; repair runner before long generation

Resume the required small C/F checks under these amended limits. Update only the
live S5.2 aggregate ledger ceiling and append an amendment binding to this file's
path/hash with the previous/new limit and actual consumed totals. Preserve all
prior contributors, immutable per-attempt JSON/logs and S5.1 ledger. Inventory and
source/image binding for new attempts must reflect the actually updated runner.

Root inspected the current `scripts/invoke-migration-s5-l1-case.ps1`: its general
limit reads the ledger, but the long-origin check still hardcodes1800 and1090
(850+140+100). The generator has individual waits, but no enclosing900-second
whole-attempt deadline is presently evident in either script. No long origin has
run, so these are pre-execution corrections, not a retrospective overrun claim.

Before the one long origin:

1. Use the amended ledger's actual F ceiling and total, not a literal old1800.
   Require at least **1215 seconds remaining =850 origin +140 boundary +225
   contingency**, after the mandatory small gates. Require the long action to be
   classified as F, with the genuine511/16 frozen-d9b origin and immutable case ID.
2. Enforce **900 seconds for the whole seed attempt**, not per message or per wait.
   Include inventory/setup/generation/copies/validation/logging and owned-process
   cleanup. Reserve cleanup within that bound; a timer reported after completion
   or a timeout only on one child command is not sufficient. Use the accepted
   parent-owned process/cleanup pattern; never kill unrelated processes by name.
   Windows background Start-Process launches use WindowStyle Hidden.
3. Preserve the original frozen image/DB rules and one active Native-domain branch.
   A failed/overrun attempt remains fully charged and returns to root before retry.
   Do not use product timeout changes or weaker correctness assertions to fit.
4. Send root a narrow long-origin readiness callback with the runner/controller
   source hashes/pointers, deadline/cleanup mechanism, actual completed small-gate
   results and current ledger/reserve arithmetic. Root checks this specific gap
   before giving the first long-origin go-ahead; no new architecture review or
   human confirmation is required. Continue the independent small work meanwhile.

The callback is a narrow preflight for the observed missing whole-run enforcement.
It does not authorize handoff, whole P, implementation acceptance or commit/push.
Submit the complete S5.2 implementation and final source-bound evidence to root
at the existing review boundary; S5.3 follows accepted and verified publication.
