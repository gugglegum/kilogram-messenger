# RFC-0123: M1.0.0 Windows technical proof-of-concept acceptance

Status: accepted; the annotated `M1.0.0` Git tag is authorized only after the
exact acceptance commit passes the hosted Windows workflow.

## 1. Decision

M1.0.0 is accepted as the first evidence-backed **Windows technical proof of
concept** for Kilogram. It is a development milestone, not a public security
release, a production-ready messenger, or a stable Cargo-library API promise.

The milestone keeps the workspace package version at `0.1.0`. Changing that
version would modify the field-tested runtime surface and the independently
reproduced offline-artifact surface without improving the security claim.

No release archive or binary attachment is required. The canonical milestone
identity is an annotated Git tag named `M1.0.0` on the exact commit containing
this RFC. The tag message repeats the accepted evidence and limitations.

## 2. Accepted evidence

The acceptance composes the existing fail-closed records rather than replacing
them:

- physical field run `20261014-141058`, product revision
  `be80d28ec12b41987bdbfa800016fb86b54b4723`, result
  `verified-reduced-two-host`;
- exact 83-file field-tested runtime/protocol surface
  `b5cb9bc9e06d24c73e63289e662ba019f88b7b052638c6a4b30099470f558095`;
- independent offline reproduction revision
  `68804c93f43b405008cfecf35dc8660db360a3a3`, GitHub run `38507114425`,
  attestation `55683374` and byte-identical artifact SHA-256
  `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`;
- hosted Windows run `38588961507` on revision `9d7895f...`: readiness
  `33/33`, isolated lifecycle `1/1`, aggregate tests `276/276`;
- hosted Windows run `38595445191` on revision `6009aa2...`: readiness
  `33/33`, isolated lifecycle `1/1`, aggregate tests `276/276`, total 277
  executed tests and zero failures.

The tag target must contain no runtime/protocol or independently reproduced
surface drift relative to `M1-CANDIDATE.json`. Before tagging, the exact target
must pass the production candidate verifier, the complete readiness gate and a
fresh green hosted Windows workflow.

## 3. Explicit limitations

M1.0.0 preserves every residual risk from the candidate record:

1. strict independent-provider field acceptance is pending;
2. operator and access-network independence is unproven;
3. Sybil resistance is incomplete;
4. traffic/access correlation is not hidden;
5. field evidence is Windows-only;
6. this is not a public security release.

Additionally, the project has not received an independent cryptographic or
application-security audit. Users must not rely on M1.0.0 for sensitive or
high-risk communication.

## 4. Tagging boundary

The tag is annotated, contains the scope and limitations above, and points to
one immutable commit. Creating it does not:

- publish a GitHub Release;
- upload an executable or archive;
- change protocol, storage or IPC versions;
- change the Cargo workspace package version;
- claim strict provider independence or production readiness.

Any later claim stronger than this RFC requires new evidence and a separate
decision. Moving or replacing the public `M1.0.0` tag is forbidden.
