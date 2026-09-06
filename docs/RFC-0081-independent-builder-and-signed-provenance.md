# RFC-0081: Independent builder and signed provenance foundation (M0.9.59)

Status: implemented but not externally executed in M0.9.59.

## 1. Problem

M0.9.50 proves that two clean source/target roots on one Windows host produce
byte-identical `kilogram-offline` executables. That does not exclude a shared
host, compiler installation or build-environment fault, and its SHA-256 record
is not signed public provenance.

M0.9.59 adds a deliberately manual second-builder path. It must not turn every
ordinary commit into a release build, consume local CPU, create local ZIP
archives or execute any Kilogram network process.

## 2. Manual independent reproduction

`.github/workflows/independent-offline-reproduction.yml` has only the
`workflow_dispatch` trigger. The operator supplies:

- the exact lowercase 40-character commit already tested by the M0.9.50 gate;
- the lowercase SHA-256 shared by both local clean-root executables.

The workflow refuses a selected GitHub revision that differs from that commit.
It checks out without persistent credentials, resolves the repository-pinned
Rust toolchain, fetches only `Cargo.lock`-pinned dependencies and then performs
a frozen release build of the isolated, network-free `kilogram-offline`
package. It repeats the same disabled-incremental, commit epoch, source-path
remap and `/Brepro` linker boundary as M0.9.50.

The GitHub-hosted Windows job uses two Cargo jobs. This is an external builder,
not a claim that the mutable hosted runner image is a bit-for-bit pinned VM.
The record therefore preserves its image name/version, Rust/Cargo identity and
workflow run identity.

## 3. Fail-closed comparison and attestation

The workflow writes `INDEPENDENT-BUILDER.json` and `SHA256SUMS`, but invokes
GitHub artifact attestation only when the executable SHA-256 exactly equals the
operator-supplied local hash. A mismatch remains downloadable for bounded
diagnosis and then fails the job; it is never presented as matched provenance.

Third-party actions are pinned to exact commits. Job permissions are explicit:
read-only repository contents plus the OIDC, attestation and artifact-metadata
permissions needed to publish the signed statement. The generated attestation
binds both the executable and builder record to the GitHub repository,
workflow and source revision.

The Actions artifact has a 14-day retention and compression level zero. This
is temporary manually requested CI evidence, not a Kilogram release, not a
committed binary and not one of the local portable ZIP packages previously
removed from the everyday development loop.

## 4. Independent verification

`scripts/verify-kilogram-independent-builder.ps1` requires:

1. a valid M0.9.50 record whose two local executables are still present and
   byte-identical;
2. a matched external record for the same clean commit;
3. identical SHA-256 and length across both local builds, the external record
   and downloaded stable-name `kilogram-offline.exe`;
4. GitHub CLI verification of attestations for both the executable and the
   builder record;
5. exact repository and signer-workflow identity, exact source commit and a
   GitHub-hosted rather than self-hosted runner.

Production verification exposes no attestation-skip parameter. Its network-free
`-SelfTest` constructs bounded synthetic records, accepts the consistent set
and rejects a modified executable without invoking GitHub.

`scripts/verify-kilogram-independent-builder-boundary.ps1` statically rejects
automatic push/release/schedule triggers, floating action tags, packaging,
self-hosted execution, missing hash gate or missing provenance constraints.

## 5. Honest completion boundary

M0.9.59 implements and locally tests the workflow and verification contract.
It does not claim that independent reproduction has happened. That claim is
allowed only after:

- a clean exact-HEAD M0.9.50 reproducibility run has produced a local record;
- an operator manually dispatches the workflow for the same commit/hash;
- the external job succeeds and publishes attestations; and
- the downloaded evidence passes the production verifier.

No release, tag, GitHub workflow run, local release build, ZIP or network-bearing
Kilogram process is created by this milestone itself.
