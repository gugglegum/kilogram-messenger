# RFC-0219: Dormant fail-closed rollout controller

Status: accepted for M1.1.95.

## Decision only, never authority

The controller combines an already verified signed release-policy cohort
evaluation, its exact authenticated installation-local health snapshot and the
current Device activation state. Its output is either a hold reason or a
proposal for the existing Device-signed activation ceremony. It cannot write
activation state, cannot sign as a Device, and always reports runtime
activation authority as false.

Missing policy, missing health, mismatched health, legacy-only policy and an
installation outside the cohort all hold. A healthy selected installation can
advance by exactly one activation state per decision. The controller never
proposes `canonical-required`; expanded rollout is still dual-reader and does
not change the public default.

## Containment

Signed containment policy or any zero-tolerance local event requests emergency
containment. That proposal preserves the current activation target so durable
canonical evidence and selected-session resume remain readable. It is emitted
only when the installation is already beyond legacy-only and has authenticated
canonical evidence. Otherwise the controller holds and cannot use containment
as an enrollment path.

Containment cannot clear itself. Normal rollout never rewinds an activation
state, and a changed cohort assignment cannot silently deactivate a previously
activated installation.

## Boundary

M1.1.95 is a pure dormant evaluator inside `kilogram-rollout`. There is no
runtime, desktop or CLI consumer, no production release key, no delivery or
uploader, and no automatic Device approval. Wire and IPC formats are unchanged
and the public default remains `legacy-only`.
