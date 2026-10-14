# RFC-0113: Complete M1 readiness gate (M0.9.90)

Status: accepted locally after committed production verification.

## 1. Problem

The M1 candidate gate composes accepted field and independent-build evidence,
but it intentionally reruns only three prerequisite boundaries. Kilogram has
many older fail-closed contracts. A stale script can therefore stop parsing
without invalidating the candidate, as happened to the UTF-8 Russian field-kit
boundary under Windows PowerShell 5.1.

M0.9.90 adds one release-readiness command with a closed inventory of every boundary contract. A new, removed or renamed boundary fails until the inventory
is deliberately reviewed. A zero exit code is insufficient: every child gate
must emit its own exact success marker.

## 2. Readiness operation

`scripts/verify-kilogram-m1-readiness.ps1` performs, in order:

1. exact inventory comparison for all 32 `verify-*-boundary.ps1` scripts;
2. isolated Windows PowerShell execution of every boundary and exact marker
   validation;
3. the production M1 candidate continuity verifier on a committed clean tree;
4. `cargo fmt --all -- --check`;
5. workspace/all-target/all-feature Clippy with warnings denied, locked
   dependencies, Cargo offline mode, two jobs and BelowNormal priority.

The self-test rejects a missing success marker and a changed inventory. The
separate readiness boundary pins the operation and forbids network commands,
bypass flags, test execution, release builds, archives, pushes and tags.

## 3. Evidence reuse and resource boundary

The gate does not execute Rust test binaries. Current runtime behavior is
covered by the unchanged 83-entry field surface and accepted M0.9.87 lifecycle;
the independently reproduced artifact surface remains covered by M0.9.88. A
manual compile-only verification-profile pass built all 24 test harnesses before
the disposable 6.52-GiB profile was pruned; the 35.37-GiB debug cache remained.

M0.9.90 does not create a release build, archive or Git tag. It creates no
product executable or background process and runs no field network operation.
The offline Clippy flag prevents dependency fetching during the gate.

## 4. Honest release boundary

A green readiness result means the current Windows technical candidate is
internally consistent with all checked contracts. It does not turn residual
risks into completed features: strict independent-provider field acceptance remains pending, operator/network independence remains unproven, Sybil
resistance remains incomplete and access correlation remains visible.

The result is not a public security release. Creating an M1 tag, signing or
publishing binaries remains a separate explicit user decision.

## 5. Acceptance evidence

Revision `56c93aba38648fa5dbe57b22bbf2640c26590c2f` passed the production
readiness command from a clean working tree. It reported `boundary_gates=32/32`,
verified candidate continuity, formatting and offline strict Clippy, with every
stage-boundary mutation flag false.
