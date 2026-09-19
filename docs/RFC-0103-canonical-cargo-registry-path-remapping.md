# RFC-0103: Canonical Cargo registry path remapping (M0.9.80)

Status: implemented locally; a fresh exact-commit GitHub run is required.

## 1. Problem

M0.9.79 made the Rust toolchain, bundled LLD and the ten Windows native link
inputs identical across the local and GitHub-hosted builders. GitHub run
`35473329174` passed all of those gates but still produced a 1,024-byte larger
executable and correctly failed closed before attestation.

The retained executables exposed the remaining input. Rust embeds source file
paths from registry dependencies in release diagnostics. The local artifact
contained paths below `C:\Users\Paul\.cargo\registry\src`, while the hosted
artifact contained the equivalent paths below
`C:\Users\runneradmin\.cargo\registry\src`. The existing source-root remap did
not cover dependency sources outside the checkout.

## 2. Canonical dual-prefix contract

Every reproducible build now passes both stable Rust flags:

- `<BUILD_ROOT>=Z:/kilogram-source`;
- `<CARGO_REGISTRY_SOURCE_ROOT>=Z:/cargo-registry-src`.

The Cargo registry root is resolved after locked dependencies are available.
An explicit absolute `CARGO_HOME` is honored; otherwise the standard
`%USERPROFILE%\.cargo\registry\src` location is used. A missing or reparse-point
registry root fails closed. The real host path is never retained in the JSON
evidence.

The two virtual roots are deliberately distinct. This preserves useful source
identity in diagnostics without exposing a username or making the executable
depend on the builder profile path.

## 3. Executable leak check

Setting a compiler flag is not accepted as proof that it took effect. After PE
metadata normalization, each local clean-root artifact and the independent
GitHub artifact are scanned before hashing:

1. `Z:/cargo-registry-src` must be present;
2. raw `.cargo\registry\src` and `.cargo/registry/src` markers must be absent;
3. when the current host roots are known, neither the checkout root nor the
   resolved Cargo registry source root may remain in ASCII PE content.

The retained verifier repeats the host-independent checks. Its self-test embeds
both a canonical fixture and a forbidden hosted-runner Cargo path and proves
that the latter is rejected.

## 4. Evidence format v6

`path_remap` is now a structured, fail-closed object containing:

- mode `rustc-dual-prefix-remap-with-pe-leak-check-v1`;
- both canonical mapping identities;
- `canonical_cargo_registry_source_present=true`; and
- `raw_cargo_registry_source_absent=true`.

The local record, independent record and production verifier must agree on the
entire object. Format-v5 evidence cannot be presented as M0.9.80 evidence.

## 5. Scope and acceptance

This stage changes only build provenance. It adds no runtime network surface,
protocol field, background service, release package or automatic workflow
trigger. Diagnostic GitHub artifacts remain bounded and expire normally.

Acceptance requires a clean format-v6 local two-root record for the exact
M0.9.80 commit followed by a manual independent GitHub build of that commit.
Only byte equality may enable GitHub attestation; any further difference must
again fail closed and be investigated from bounded evidence.
