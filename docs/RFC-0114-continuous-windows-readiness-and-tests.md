# RFC-0114: Continuous Windows readiness and tests (M0.9.91)

Status: hosted execution operational; complete product tests passed in run
`38519726403`. M0.9.94 fixed the post-test empty-cache accounting defect; its
confirmation run `38523647852` exposed a separate hosted scheduling margin in
the isolated runtime-restart test.

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

The test step still runs when readiness fails, provided the locked dependency
fetch succeeded. The failed readiness step continues to fail the job. This
separation is necessary when a legitimate product edit intentionally makes old
candidate evidence stale but still needs hosted regression feedback.

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

The first hosted baseline found two endpoint-lifecycle failures late in the
same long-lived 81-test CLI process. A closed JSON manifest initially proved
both exact selectors and ran them in fresh processes. Run `38513111049` then
separated their causes: revocation/restart passed only in isolation, while the
pairwise locator test failed alone because its append-only runtime journal path
crossed the legacy Windows path limit. M0.9.92 fixes the storage boundary and
returns that test to the aggregate harness. The remaining isolated invocation
is mandatory and still fails the job on any failure; this is process isolation,
not retry-based success or reduced coverage. It now runs exactly once before
the aggregate harnesses and the hosted test process uses normal process priority.
Local callers retain the interactive-friendly default order and `BelowNormal`
priority unless they explicitly request the hosted policy.

Run `38519726403` on exact revision
`3887e06e58c060d19cbd84eb5fefdb86d2f613ad` passed the complete readiness gate,
all aggregate harnesses and the isolated revocation/restart test. The workflow
was nevertheless red because its final read-only cache report encountered an
empty Cargo profile directory and Windows PowerShell returned no
`Measure-Object` summary object. M0.9.94 treats a missing or empty tree as exact
zero and exercises that case in a fresh temporary directory from the closed
Windows CI boundary. An empty Cargo profile directory is valid state, not a
test failure; malformed or multiple measurement results still fail closed.

Run `38523647852` proved the cache fix was not yet reached: all 276 aggregate
tests passed, while the isolated restart test crossed its internal ten-second
IPC-ready deadline after the restarted endpoint had already bound successfully.
The same exact test passed locally at normal priority in 59.82 and 59.85
seconds; timestamped output placed restart IPC readiness 8.66 seconds after
the preceding runtime stopped. That leaves too little scheduling margin when a
`BelowNormal` child runs after the complete hosted aggregate suite.

The hosted runner therefore restores normal process priority explicitly and
runs the isolated fresh process before the aggregate harnesses. The test is not
retried, its internal assertion is unchanged, and every aggregate and isolated
test remains mandatory. This is a CI scheduling correction outside the exact
field-tested runtime/protocol and independently reproduced offline surfaces.
