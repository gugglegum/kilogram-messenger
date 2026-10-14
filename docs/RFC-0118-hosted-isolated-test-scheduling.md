# RFC-0118: Hosted isolated-test scheduling (M0.9.95)

Status: implemented locally; hosted confirmation pending.

## 1. Evidence

GitHub Actions run `38523647852` checked exact revision
`75ad3d89cc0c5e154fa0c162a78ccf716c5d003b`. Readiness passed, and all 276
aggregate Rust tests passed. The only failure was the separately executed
`live_runtime_applies_revocation_republishes_ticket_and_retires_ratchet` test.

Its restarted runtime reached `relay_status=skipped`, proving that its loopback
endpoint had bound, but did not publish the IPC descriptor before the test's
ten-second readiness deadline. The previous hosted run had passed the same
single invocation in 69.84 seconds. Two local exact invocations at normal
priority passed in 59.82 and 59.85 seconds. Timestamped local output measured
8.66 seconds from the first runtime's completed shutdown to the recovered
runtime's IPC descriptor.

## 2. Scheduling contract

The stable-path runner keeps its local defaults: `BelowNormal` process
priority and isolated tests after aggregate harnesses. The Windows CI workflow
now explicitly selects:

- normal process priority on its disposable hosted runner;
- the closed isolated-test set before the aggregate harnesses;
- one invocation of each exact selector in a fresh process;
- no retry and no conversion of a failure into success;
- the same serial aggregate execution and no-fail-fast reporting.

The order and priority are explicit parameters rather than implicit CI
environment detection. Unknown order values are rejected by PowerShell
parameter validation. Failure to apply the requested normal priority is fatal.

## 3. Boundary

The Windows CI boundary requires the exact workflow arguments, runner
parameters, two mutually exclusive order branches and observable status
markers. Existing exact manifest validation still proves that each isolated
selector resolves to one test and that the aggregate invocation skips only
that exact selector.

No Rust source, network protocol, runtime storage, IPC schema or release
artifact changes. The 83-entry field runtime/protocol surface and the 63-entry
independently reproduced offline surface remain byte-identical. No physical
network rerun or independent artifact reproduction is required.

## 4. Acceptance

Local acceptance requires the Windows CI boundary, complete M1 readiness and a
focused exact isolated-test pass. Final M0.9.95 acceptance requires a fresh
green hosted Windows CI run that reaches and passes the corrected empty-cache
report. It does not authorize an M1 tag or public security-release claim.

The local stable-path runner was then exercised in the exact hosted mode. The
isolated test ran first and passed once in 60.64 seconds; the aggregate CLI
harness subsequently passed 81 tests in 530.22 seconds with the isolated test
reported as the single intentional filter. The runner reached `status=passed`,
reported `isolated_test_count=1`, and the empty-cache-safe report completed.

Implementation revision `c6793136ef0839c6c854227929fa9dfe32cc3f74`
subsequently passed the complete clean-tree M1 readiness gate: all 33 boundary
gates, candidate continuity, formatting and locked/offline strict Clippy were
verified. No test executable was started by readiness and every mutation flag
remained false. Hosted confirmation is still required.
