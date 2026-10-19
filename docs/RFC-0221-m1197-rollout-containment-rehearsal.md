# RFC-0221: Rollout containment and corruption rehearsal

Status: accepted locally for M1.1.97.

## Defect found and closed

The first controller implementation correctly entered emergency containment,
but did not explicitly stop a later healthy normal policy from proposing the
next migration state while the current Device state was still contained. That
would have made the controller an implicit containment-clear authority.

The controller now holds with
`containment-requires-explicit-recovery` whenever current Device disposition is
emergency containment and no new containment proposal is needed. Only a
separate explicit Device recovery ceremony may clear it. The release policy,
controller, observer and runtime cannot do so.

## Exact health binding

Controller health matching now checks the signed policy ID, exact release
artifact ID, rollout phase and cohort upper bound. This is defense in depth:
the read-only store already authenticates each record and checks the current
policy binding, while the pure evaluator no longer assumes that every caller
performed that join correctly.

## Deterministic model rehearsal

One exhaustive test evaluates all 640 combinations of five release phases, two
release dispositions, selected/unselected cohort, clean/zero-tolerance health,
four bounded activation targets, two current dispositions and evidence
present/absent. Every proposal must require explicit Device approval and deny
release-policy signing and runtime activation.

A normal proposal must come only from a normal current disposition, advance
exactly one state, remain within the policy target and never follow a
zero-tolerance event. A containment proposal must preserve the current target,
require authenticated canonical evidence and never enroll legacy-only state.

## Durable failure rehearsal

Read-only inspection is exercised against an absent store, a foreign pinned
authority, tampered health, a missing current health record and a truncated
redb file. Absence is a non-mutating hold; every partial, foreign or corrupt
state fails closed. Symlink rejection remains covered by the implementation
boundary. Coordinated rollback of the complete database without an external
witness remains an explicit non-claim.

## Scope

This local rehearsal executes no network and adds no runtime or desktop
consumer. The CLI remains read-only and cannot apply a proposal. Production
key management and policy delivery are still absent. Peer/sync wire, runtime
IPC and the public `legacy-only` default are unchanged. M1.1.98 must repeat the
stable harness on two physical Windows hosts before this packet is considered
field-portable.
