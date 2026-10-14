# RFC-0122: Hosted Windows readiness acceptance (M0.9.99)

Status: accepted.

## 1. Purpose

M0.9.96 replaced the fragile fixed ten-second test readiness wait with a
bounded task-aware wait that also observes early runtime task completion.
M0.9.97 and M0.9.98 then refreshed physical field evidence and the composed M1
candidate. The remaining gate was a fresh, independent, full-history hosted
Windows run of the committed candidate.

## 2. Accepted hosted run

GitHub Actions Windows CI run `38588961507` executed exact revision
`9d7895f28853d43eaecad15897b2ca5d1c279713`. The single read-only job completed
successfully in 38m41s.

The hosted checkout passed the complete M1 readiness gate:

- all `33/33` closed boundary contracts;
- production candidate verification;
- formatting;
- locked/offline strict Clippy with two Cargo jobs;
- no network action from readiness, release build, archive or Git tag.

The stable-path runner then passed the exact isolated lifecycle test first:
`1 passed`, `0 failed`, `81 filtered out`, 96.61 seconds. Every aggregate
harness subsequently passed. Their non-zero test counts sum to 276, for 277
executed tests including the isolated test. The runner emitted
`status=passed`, `isolated_test_count=1`, and `cache_above_warning=false`.

## 3. Scope of the result

This run closes the hosted Windows confirmation gate for the current technical
candidate. It confirms that the task-aware readiness change works on an
independent cold Windows runner and that all aggregate harnesses still pass.

It does not change the recorded security boundary:

- field evidence remains the reduced two-host result;
- strict independent-provider field acceptance remains pending;
- operator/network independence and complete Sybil resistance are not claimed;
- access correlation remains visible;
- this is not a public security release.

No M1 tag is created or authorized by this RFC. Tagging and version naming
remain a separate explicit product decision.
