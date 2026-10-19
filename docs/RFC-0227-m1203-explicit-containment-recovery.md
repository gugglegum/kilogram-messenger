# RFC-0227: Explicit Device-signed containment recovery

Status: M1.2.3, verified locally, dormant.

## Recovery is a separate authority ceremony

Emergency containment cannot be cleared merely because a later release policy
or health snapshot looks healthy. M1.2.3 introduces a separate bounded
recovery approval that requires the explicit
`UserOrTestOperatorApproved` marker and a signature from the same Device that
owns the current activation chain.

A root-verified normal policy and exact matching local health are necessary
preconditions, not recovery authority. The normal policy must cover at least
the already contained migration state. Health must include a successful runtime
start and canonical sync, and must contain no runtime/sync failure, interruption,
pause, downgrade, authority mismatch, corruption or other zero-tolerance event.

## Preservation and exact binding

The approval commits to the trusted bundle, complete fixed-schema health
binding, Device, exact containment head ID and epoch, authenticated canonical
evidence, fresh nonce and complete candidate activation bytes. The candidate is
independently Device-signed with the existing activation primitive.

Recovery changes only the disposition from emergency containment to normal.
It preserves the migration state and canonical evidence byte-for-byte. A
changed policy, health snapshot, Device, containment head, candidate or
signature fails closed. Only the byte-identical already-applied candidate is an
idempotent replay. `canonical-required` remains outside the public rollout
controller and cannot be recovered through this ceremony.

## Verification and non-claims

Four focused tests cover successful recovery and replay, missing explicit
approval, unhealthy evidence, changed health/head, wrong Device, tamper and the
`canonical-required` exclusion. The complete rollout crate passes 48/48 tests,
rustfmt and locked/offline strict Clippy.

The ceremony performs no filesystem write, runtime IPC or network operation.
It does not add desktop UX, activation persistence, a runtime consumer or a
production trust root. Peer wire, sync wire, runtime IPC and the public
`legacy-only` default remain unchanged.
