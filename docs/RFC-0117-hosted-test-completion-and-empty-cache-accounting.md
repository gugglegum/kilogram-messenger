# RFC-0117: Hosted test completion and empty-cache accounting (M0.9.94)

Status: accepted locally; hosted confirmation pending.

## 1. Context

GitHub Actions run `38519726403` checked exact revision
`3887e06e58c060d19cbd84eb5fefdb86d2f613ad`. The complete M1 readiness gate
passed, every aggregate Rust harness passed, and the one deliberately isolated
revocation/restart test passed in a fresh process. The test runner then invoked
the read-only Cargo cache reporter.

One Cargo profile directory existed but contained no files. On that hosted
Windows PowerShell execution, an empty pipeline produced no `Measure-Object`
summary object. Strict mode therefore rejected access to a nonexistent `Sum`
property and changed an otherwise successful job to exit code 1.

## 2. Correctness rule

Cache accounting now distinguishes three exact states:

- a missing tree is zero files and zero bytes;
- an existing empty tree is zero files and zero bytes;
- a non-empty tree uses the single `Measure-Object` summary and preserves its
  exact file and byte counts.

Zero summary objects are accepted only as the empty-tree result. More than one
summary object remains an error. Access to the optional `Sum` value goes
through the PowerShell property collection, so strict mode never dereferences
a missing property.

## 3. Regression boundary

`cargo-cache-maintenance.ps1 -SelfTest` creates a unique temporary root and
proves missing, empty and three-byte cases. It refuses cache mutation switches
in self-test mode and deletes only the exact temporary root it created.

The existing closed Windows CI boundary statically requires the self-test
contract and executes it through Windows PowerShell. This adds no Rust build,
network action, release artifact, archive, service or Git tag. The number of
M1 boundary gates remains 33 because the regression belongs to the existing
Windows CI contract.

## 4. Evidence and limits

Run `38519726403` is accepted as evidence that the full Rust suite itself
passed, but not as a green CI baseline because the job result was red. A fresh
hosted run of the committed fix must pass both readiness and the complete test
step before M0.9.94 is accepted.

This change touches only verification tooling and documentation. It does not
change the 83-entry field runtime/protocol surface or the 63-entry independently
reproduced offline surface, so it requires neither another physical field run
nor another artifact reproduction.

## 5. Local acceptance

Implementation revision `520793ff17d35d22323f984765b43f4ad8e1c5d3`
passed the self-test in Windows PowerShell 5.1 and PowerShell 7, the normal
37.48-GiB cache report, the Windows CI boundary and the complete clean-tree M1
readiness gate. Readiness reported `boundary_gates=33/33`, candidate, format
and locked/offline strict Clippy verified, with all mutation flags false.

Local acceptance does not turn run `38519726403` green and does not authorize
an M1 tag. The remaining acceptance condition is a fresh successful Windows CI
run of the committed state.
