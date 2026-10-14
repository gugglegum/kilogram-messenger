# RFC-0124: Post-M1 immutable acceptance and continuous CI

Status: accepted for post-M1 development.

## 1. Problem

The pre-M1 candidate verifier proves that the current `HEAD` still matches the
exact protected surfaces and evidence selected for a proposed milestone. That
contract must fail after any post-acceptance source or manifest change. Using
it unchanged in continuous integration would therefore make ordinary,
legitimate development permanently red after M1.0.0.

Refreshing the old candidate record would be worse: it would make historical
field and reproduction evidence appear to cover code that was never tested by
those runs. Accepted evidence is an immutable historical fact, not a rolling
approval of future commits.

## 2. Decision

Post-M1 Windows CI anchors historical acceptance to the immutable annotated
`M1.0.0` tag. A network-free verifier requires its exact annotated-tag object,
commit, source tree, candidate-record blob, acceptance-RFC blob and decision-log
blob. It also requires that the accepted commit remain an ancestor of the
current `HEAD`.

After that historical check, CI evaluates the current boundaries, formatting,
strict Clippy and Rust tests from the current checkout. The two statements stay
separate:

1. the exact M1.0.0 snapshot remains the snapshot that was accepted; and
2. the current branch passes its present-day regression and architecture gates.

The candidate-preparation mode remains available when a future milestone needs
fresh evidence. Omitting `-AcceptedTag` continues to require current `HEAD` to
match the selected candidate surfaces exactly.

## 3. Evidence integrity

This mode does not refresh, rewrite or reclassify `M1-CANDIDATE.json`, the
accepted physical field run, the independent reproduction, or their residual
risks. It does not claim that later commits inherit the M1 field or
reproducibility evidence.

`M1.0.1` is a governance snapshot containing licensing, security-reporting and
public-project metadata. It is not a replacement technical acceptance record
and does not strengthen the M1.0.0 security claim.

## 4. Operational boundary

The verifier uses only local read-only Git queries. It performs no network
operation and does not create or move a tag, checkout another tree, mutate the
working tree, build a release artifact, create an archive, or publish an
attestation.

The hosted workflow retains read-only repository permission, pinned action
identity, two Cargo jobs, stable executable paths and the ordinary unoptimized
test profile. A green run is regression evidence for the current revision; it
is not new physical-device field evidence or an independent security audit.
