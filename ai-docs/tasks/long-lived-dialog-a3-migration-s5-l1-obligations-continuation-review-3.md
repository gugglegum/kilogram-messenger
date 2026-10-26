# A3-S5.2 obligations — measured controller correction and budget amendment 2

2026-10-26. Root authorizes the next bounded continuation using the actual default
delivery batch, with explicit test-controller deadlines appropriate to the measured
workload. This is a validation correction and new verification allocation, not
implementation acceptance or a product-timeout change.

Inputs remain the accepted S5 contract and implementation assignment, [amendment1](long-lived-dialog-a3-migration-s5-l1-budget-amendment-1.md),
[continuation review1](long-lived-dialog-a3-migration-s5-l1-obligations-continuation-review-1.md)
and [review2](long-lived-dialog-a3-migration-s5-l1-obligations-continuation-review-2.md).
Their files and all historical case records stay unchanged. This decision overrides
the specific F ceiling, next case ID and two large-workload controller waits below.

## Verified evidence and scope of the conclusion

F-obligations-resume-02 is FAIL40.0101169s, test31.68s, not a whole-job timeout.
Its case JSON is10656 bytes, SHA256
`89eac8d1bc36cd7577c7e3504703fedc8f91b12af49f2056cd9fd2ca3990b667`.
The exported A trace is8314 bytes, SHA256
`700c48f890c1f1cd53eda25e3e3067847c76f3b328dce8322463ba0417069590`.
The [analysis](long-lived-dialog-a3-migration-s5-l1-evidence/F-obligations-resume-02-analysis.md)
and actual four exact-Complete trace rows were read by root.

The byte-preserved input contains203 files; the actual cold source debt is5..69,
receiver pending6..61, and the sender's volatile delivery cursor restarts atNone.
Startup kept real scope/session/keys/Guard, zero constructor/packet calls and
unchanged receiver Native. Exact Complete is possession, not a receipt or debt
retirement. All65 debts remain meaningful while head5 is deliberately held.

Default eight-item deliveries completed at monotonic ticks7811,12986,18058,22457ms
for6..13,14..21,22..29,30..37. Intervals5175/5072/4399ms show actual progress.
Reaching the last batch62..69 fromNone needs eight cycles. Extrapolating the
observed first Complete plus seven times the largest observed interval gives
7.811 +7*5.175 =44.036 seconds. This is a planning anchor, not a throughput
guarantee or proof of every earlier fresh-run failure. A fixed25s wait did not
allow that observed resumed sequence to reach the tail.

No stuck worker, cache divergence or ref leak has been established. The low replay
rate in this unoptimized verification workload remains an open performance
observation; this correction does not prove acceptable user-facing latency or a
performance regression. Preserve the measured data for final assessment rather
than making it disappear when a longer correctness wait passes.

## Keep default production behavior; change only two fixture waits

Do **not** substitute test-scheduler direct_batch32 for the default8 as the proof
of this resumed default-runtime path. Legal larger-batch coverage is a different
configuration; it would not establish default8 completion here. Keep direct_batch
unset, original held head5, default protocol selection/retry behavior, Native<=4
per turn and the accepted per-C/global resource limits.

For the obligations controller only, authorize explicit named deadlines:

- Default replay until actual64 ordinary pending: **70 seconds**. This covers
  the measured44.036-second planning anchor plus startup/scheduling/inspection
  margin, while remaining bounded.
- After the actual reserved head is released/reopened, drain the real65-packet
  workload to its required readable/receipt state: **70 seconds**. This is a
  deliberate bounded allowance for at least17 Native turns, not a new measurement
  or PASS claim. Use one shared absolute70s deadline for both the sender's
  `a.wait(70,5)` debt-zero check and receiver's `b.wait(5,70)` confirmation; the
  current sender wait also checks debt and cannot retain25s while only B is widened.
  Keep the exact counters/IDs/provenance/accounting assertions.
- Small C2 exchanges, quarantine/renewal conditions and unrelated helper callers
  retain their existing25-second waits. Do not globally replace25 with70, change
  product RPC/admission timeouts, or widen malformed-input/authorization behavior.

Expose an explicit controller wait argument/helper for those two call sites rather
than weakening all waits. Return as soon as the exact condition is met. Record
phase durations and progress; timeout still fails. All test work is also subject
to the single whole-attempt deadline below, so phase allowances are not additive
permission to exceed it. No scheduling cursor/debt injection or skip of replay.

## Budget amendment 2 and exactly one next case

Verified current ledger: **C105.3761137/F763.8892556/P0**. All failures and diagnostic
attempts remain charged. Root explicitly increases S5.2 **F from2100 to2400 seconds
(+300)**; C500/P0 remain unchanged. With immutable S5.1 actuals, combined maxima
become **C779.3257298/F3636.2886628/P0**. Append this amendment binding to the live
aggregate ledger, retaining its old entries and ceiling history; do not rewrite
prior plans, case JSON, logs or S5.1 accounting.

Conservative remaining forecast:

| Work | Maximum/planning allowance, seconds |
|---|---:|
| One complete obligations continuation, all overhead included |240|
| Different-peer isolation/signed fork |100|
| Affected S5.1 capacity check |45|
| One genuine511/16 origin |850|
| Exhausted boundary gate |140|
| Reserved contingency before the long origin |225|

763.8892556 already spent + these allowances =2363.8892556, leaving36.1107444
within2400. This is explicit planning, not automatic PASS or permission for new
broad suites, another origin generation or repeated failed attempts. Use actual
timings to reassess at the next callback. Long entry still requires remaining
>=1215, the separately reviewed whole900 guard and root's specific go-ahead.

Authorize exactly **F-obligations-resume-03**, same original stopped active branch
`.tmp/migration-s5-storage/l1-obligations-Iw3nQI` and same obligations filter, bound
to this decision hash. **Whole attempt<=240 seconds; work cutoff210 seconds**,
with30 seconds reserved for actual owned-child cleanup, source/input checks and
finalization. Keep the same dedicated non-breakaway job mechanism; changing its
configured duration alone does not require repeating its already-passing mechanism
probe. If the mechanism itself changes, assess the affected verification normally.

Before typed open, preserve the actual current stopped branch in the new private
immutable `.tmp/migration-s5-preserved/F-obligations-resume-03`, using review2's
exact held-empty-lock exception and actual non-lock byte identities. Do not reopen
the preserved02 snapshot or frozen d9b original as active. Bind and report actual
starting scope/keys/counters/debt/pending/policy generations; do not assume that
failure-time public status equals the last durable shutdown state. Preserve all
old logs/schedules/records; use new case-specific paths. Refuse already-started or
final case IDs pending reconciliation, never overwrite markers.

The corrections are controller/helper/ledger changes. Reuse the exact production
field image if its compiled inputs are unchanged; rebuild only affected harnesses
and bind the new scripts/test source. Do not edit inputs during source-bound runs
or recopy live EXEs. Static preparation is separate; setup/preservation/recovery/
all phases/cleanup inside the contributor remain charged. Retain the documented
inner elapsed versus supplemental outer-observation distinction without claiming
an outer delta is precisely the isolated JSON publication time.

After this one F case, callback root with actual per-phase trace, input/source/
image bindings, complete assertions or last failure boundary, cleanup and totals.
No additional F or automatic repeat before that callback. Root then decides the
remaining small checks and first long origin using actual evidence. No whole P,
S5.3, implementation acceptance or commit/push is authorized by this document.
