# S4 implementation review 1 — incomplete P and bounded diagnosis

2026-10-25. Root/orchestrator review checkpoint. **Implementation is not accepted;
no implementation commit/push is authorized.** The accepted contract and ordinary
publication authorization remain in force. This checkpoint authorizes diagnosis,
not a larger time budget or a partial replacement for the P scenario.

## Evidence and blocking result

Baseline is `9547ed171c64bf5ea529b4ba487acf6aa1a61b5b`. Reviewed candidate is
identified by [P4 source snapshot](long-lived-dialog-a3-runtime-s4-evidence/p-full-final-4-contract-budget-source.json),
with production CLI SHA256
`43f7fade4d28a046b451b8eb35f7502144035f4c402b317d7fa0ca576604fbfc`.
Root independently matched current bytes for runtime, owner, data, network,
admission, native, policy, indexed root, runtime repository and transport modules
to that snapshot before diagnostic instrumentation.

[P4 result](long-lived-dialog-a3-runtime-s4-evidence/p-full-final-4-contract-budget-result.json)
is exit 1, runner **2360.3300036 s**, controller deadline 2350 s. It did not reach
relay/P08 or final P09 exact histories and all 3288 committed packet operations.
The complete-P acceptance condition is therefore unsatisfied, even though the
runner terminated within its 2400 s ceiling. All four original attempts stay
preserved; prefix/suffix stitching cannot turn them into a whole-P pass.

Root read [derived metrics](long-lived-dialog-a3-runtime-s4-evidence/final-process-metrics.json)
and sampled the raw controller output. A640 reached 800.136 s; the 20 duplex rounds
finished at 1790.887 s. Successful closed-writer elapsed sums to about 514.708 s
across both processes. This is neither CPU attribution nor scenario wall time,
and excludes preceding native/crypto/read work and mailbox/network waits.
1291 Materialize and 1133 Drain commits show that many turns contained fewer than
the permitted four packets. The only completed history measurement was 640 rows,
40 pages, 8.237 s; the incomplete scenario is not a history throughput benchmark.

The reported F contributing commands sum to 576.9843874 s. Root verified that
arithmetic, but has not yet accepted source-scoped F reuse, complete coverage or
the final implementation manifest. Subsequent production changes require a new
source-correspondence decision and relevant checks.

Root also checked the actual test-result lines, not only runner exit codes:
full batch4 F ran 18 passing tests and its supplement 10 passing tests (overlapping,
not 28 distinct gates). The ten selected passing component/regression commands
ran 109 tests in total, including IPC, prior codecs/storage, S3 rollback/reopen
and escaped-JSON history pagination. The separately recorded broad selector run
was stopped and is not counted as a passing contributor. Static candidate checks
report fmt/helper syntax/diff PASS. The staged index remained empty and the
protected development-environment document retained SHA256
`53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f`.

## Read-only implementation review so far

Examined critical paths include the distinct RuntimeRoot6/ComponentRoot5 loaders
and writer fence; first-root runtime bootstrap; directory-lock retention on reopen;
reject-only precommit gate and explicit abort versus poisoned IO recovery;
current-policy and historical-selection cache keys; native snapshot/AAD and
guard/frontier checks; atomic Materialize/Drain/receipt persistence; monotonic
authority and actual TLS/locator admission; bounded transport configuration and
per-context reference accounting. The subsequent route-table comparison found R1
below. This is a limited review checkpoint, not an
independent cryptographic audit or full S4 acceptance.

The P driver was also inspected for its 640-message restarts, 20 duplex rounds,
late B7 tail, offline 256, full pending/head repair, actual C1/C2 overlap,
page-2/page-5 kills, lost ACK, B-only relay continuation and final identity/history
assertions. The omitted tail of the failed execution still has to run successfully.

## R1 — enforce S4 loopback binds for Auto with explicit relay

**Must fix before acceptance.** Contract section 8.1 states that an S4 IP bind is
explicitly supplied by the operator **and loopback**. In
`crates/kilogram-transport-iroh/src/ordered2.rs`, `RouteConfig2::effective` enforces
`bind.ip().is_loopback()` only for DirectOnly or configurations without a relay.
Consequently Auto with Some(U) accepts wildcard or non-loopback binds, contrary
to the common S4 rule. This was confirmed by the source condition, without
starting an external listener.

Apply the loopback condition to every IP-enabled S4 policy; RelayOnly must still
forbid a bind entirely. Add bounded configuration regressions for Auto with an
explicit relay: IPv4/IPv6 loopback succeeds, wildcard and non-loopback addresses
fail before endpoint creation. Preserve the accepted route matrix and include
this transport delta in final F12/source verification. Root sent the finding to
the executor with instructions to preserve/finish the current diagnostic first.

## Authorized next action

Root sent the executor a bounded diagnosis task in the existing GPT-6.1 Sol Max
chat. Finish already-running short checks, inspect existing evidence first, then
if needed run **one isolated short verification diagnostic with watchdog at most
300 s**. It is diagnostic evidence, not acceptance P. Measure owner queue wait and
execution, whole materialize/drain versus native trials/closed writer, relevant
snapshot/read/verification costs, data-round waits and process CPU if available.
Instrumentation must preserve source hashes and expose no secrets.

Concrete inspection hypotheses, not established causes:

- Status opcode 1 performs snapshot/export/current checks for each context and
  calls `native`, which forks Account/Session even on cache hit merely to read
  Guard; `Pair::state` polls every 75 ms.
- Verification schedule loading performs repeated filesystem reads in clock,
  policy and operation hooks.
- Delivery and ACK pages become individual owner calls interleaved with data
  turns; actual native batches are commonly smaller than four.

Executor must report the measured dominant work and a narrow proposed correction
before another full P. No profile optimization, count reduction, security-gate
weakening, larger native batch, borrowed context slots, larger time ceiling or
commit/push has been authorized. The complete accepted P/F/resource/deadline
requirements still govern the eventual implementation review.

## Diagnostic result and narrow correction authorized

Root read [diagnostic attribution](long-lived-dialog-a3-runtime-s4-evidence/diagnostic-attribution.json)
and the actual successful short-run result: runner119.4238763 s, controller75.553578 s,
data58.240759 s (64 one-way plus34 duplex Text), with98 Text and4 genesis overall.
This remains non-P diagnostic evidence. Data-phase process CPU was46.28125 s and
42.0625 s; lifetime owner categories and overlapping spans are not added to it.

The status/native-fork hypothesis was narrowed by evidence:598 forks took only
about0.06 s and actual packet encrypt/decrypt about0.04 s combined. Hooks took1.13 s;
234 status operations7.84 s. The unconditional owner-loop `installed` probe ran
17874 times and consumed21.0315975 s, about19.4% of the108.4893818 s disjoint owner
service sum. Snapshot spans were another3752 calls/12.5419337 s, nested in callers.

Root authorized a minimal correction: remove unconditional disk-backed installed
probes from empty scheduling turns, relying on authenticated poison-checked
root/ledger counts and the existing early materialize/drain guards. Actual work
must still execute snapshot/phase/current/provider/time/lease validation and the
closed commit gate. Preserve round-robin, yields, batch<=4, recovery and cancellation.
No speculative native-fork rewrite, changed polling cadence or broader batching
change is requested. Implement R1 separately and compare the same short diagnostic
on an isolated new fixture with source/image hashes before another full P. Root
will assess the measured effect and exact guard preservation first.

Root inspected the resulting narrow source edit: the unconditional installed read
is removed from the owner loop; materialize still checks poison-protected queued
count before `require(c,false)`, and drain checks pending/head credits before
`require(c,true)`. Existing recovery checks and round-robin remain around these
calls. R1 now applies loopback to every IP-enabled policy, with a route-table test
covering loopback positives and wildcard/non-loopback negatives before bind.
Execution of that new test and the comparable diagnostic is still required.
