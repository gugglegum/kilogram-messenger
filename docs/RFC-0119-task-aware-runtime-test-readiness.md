# RFC-0119: Task-aware runtime test readiness (M0.9.96)

Status: implemented locally; field/candidate and hosted confirmation pending.

## 1. Evidence

GitHub Actions run `38531172733` tested revision
`a45ce0c4d37bd0e639d85a49468503ac042734d8`. The runner applied the M0.9.95
policy exactly: normal process priority and the isolated lifecycle test before
all aggregate harnesses. The isolated test still failed because its recovered
runtime did not publish the IPC descriptor within ten seconds. The same job
subsequently passed all 276 aggregate Rust tests.

The runtime had already reached `relay_status=skipped`, and repeated local
invocations complete successfully around one minute. The observed failure is
therefore a test-harness readiness deadline, not evidence of corrupted
revocation state or a failed runtime endpoint.

## 2. Readiness contract

The affected lifecycle test now uses one test-only helper for both initial and
recovered runtime startup. The helper:

- permits at most 60 seconds for the IPC descriptor to appear;
- checks every 20 milliseconds without an unbounded sleep;
- concurrently observes the runtime `JoinHandle`;
- immediately exposes a runtime error or unexpected clean exit instead of
  waiting for the deadline;
- does not retry, restart or convert an error into success.

The 60-second allowance is deliberately test-local. Production IPC, network,
shutdown and retry policies are unchanged.

## 3. Proof boundary

The implementation is inside the existing `#[cfg(test)]` module. A normal
debug executable was rebuilt after the change and retained the same byte size
as the field-tested executable. Its PE image hash changed because the Windows
debug image carries build identity/debug metadata, so byte identity is not
claimed.

The existing field-surface verifier is intentionally conservative: it hashes
the whole `apps/kilogram-cli` source tree and cannot express a semantic
test-only exception. Consequently, this revision must receive fresh physical
field evidence and a candidate refresh even though runtime product behavior is
not intentionally changed. The independently reproduced `kilogram-offline`
surface is unchanged and its attestation may remain continuous if its exact
protected-surface check still passes.

## 4. Local result and acceptance

The exact lifecycle test compiled and passed once with the new readiness
contract in 61.02 seconds. Formatting also passed. This is only local evidence.

Acceptance requires:

1. a committed implementation revision;
2. fresh physical field evidence for that exact revision;
3. a refreshed candidate/control commit and clean complete M1 readiness;
4. a fresh green hosted Windows CI run that passes the isolated test,
   all aggregate harnesses and the empty-cache-safe final report.

No step in this RFC authorizes an M1 tag or a public security-release claim.
