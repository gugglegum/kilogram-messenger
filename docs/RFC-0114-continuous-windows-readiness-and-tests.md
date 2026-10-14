# RFC-0114: Continuous Windows readiness and tests (M0.9.91)

Status: implemented locally; hosted isolation remediation run pending.

## 1. Purpose

Independent offline reproduction is intentionally manual and narrowly scoped.
Before M0.9.91, ordinary changes had no hosted gate proving that the complete
M1 readiness contract and Rust tests still passed outside the development PC.

The new `Windows CI` workflow runs on every push and pull request to master and
may also be dispatched manually. It is a verification workflow, not a release
pipeline.

## 2. Hosted operation

The job uses a GitHub-hosted Windows Server 2022 runner and:

1. checks out complete Git history without persisted credentials, which is
   required by the candidate ancestry and selected-tree continuity proof;
2. resolves the repository-pinned Rust toolchain;
3. performs one locked dependency fetch;
4. enables Cargo offline mode and runs the complete M1 readiness gate;
5. compiles and executes all stable-path Rust test harnesses serially with two
   Cargo jobs and no fail-fast loss of later failures. The ordinary hosted run
   uses Cargo's unoptimized `test` profile so the preceding all-target Clippy
   build can be reused rather than compiling a second profile.

The workflow has read-only repository permission. Its only third-party action
is `actions/checkout`, pinned to a full commit SHA. It does not use the more
privileged `pull_request_target` event and retains no uploaded build artifact.

## 3. Release and evidence boundary

The workflow does not create a release build, archive, attestation or tag. It
does not push commits or publish packages. The separate manual independent
reproduction workflow remains the only attestation-producing path.

A GitHub-hosted Windows runner is not physical-device field evidence. CI can
detect deterministic source, contract and unit/integration regressions; it
cannot replace the accepted Alice/Bob run or the still-pending strict
independent-provider field topology.

## 4. Firewall and resource behavior

Rust tests use the repository's stable-path harness runner. This avoids random
local EXE names and runs harmlessly on the disposable hosted machine rather than
creating new Windows Firewall identities on the user's PC. Harnesses execute
one at a time; compilation and Clippy use two jobs and incremental compilation
is disabled on the ephemeral runner. The test step is bounded to 38 minutes.

The first hosted baseline found two endpoint-lifecycle tests whose internal
10/25-second deadlines were sensitive to running late in the same long-lived
81-test CLI process. They remain mandatory, but a closed JSON manifest skips
them only in that aggregate invocation and immediately runs each exact test in
a fresh process. A failure in either isolated invocation still fails the job;
this is process isolation, not retry-based success or reduced coverage.
