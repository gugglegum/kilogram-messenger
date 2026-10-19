# RFC-0222: M1.1.98 exact-binary two-host rollout portability

Status: verified by the M1.2.5 two-host rehearsal.

## Purpose

M1.1.98 is an evidence milestone rather than a new authority or wire feature.
It requires one rollout test harness and one ordinary CLI, each identified by
an exact SHA-256 and copied under a stable executable name, to produce the same
result on two physical Windows hosts.

The field run must prove all of the following on both hosts:

- the complete rollout crate suite passes;
- the named end-to-end rollout rehearsal passes;
- an absent-store observer holds without creating state;
- the operator observer also holds without creating state;
- the transferred binaries match the locally measured SHA-256 values.

The tracked record is bounded to revision, hashes, byte counts, anonymous host
labels and pass/fail outcomes. Raw logs, durable rollout state, Account or
Device identifiers and secrets remain in ignored disposable directories.

## Accepted evidence

Run `m1205-rollout-20261019T071251Z` exercised exact implementation revision
`2f005c124a8333af4cd7a09bd36dee93cb1111a8` on the local Windows PC and the
remote physical Windows laptop. Both hosts passed 56/56 rollout tests and the
named end-to-end rehearsal, and both observer variants returned
`hold-without-create`.

The verification-profile harness SHA-256 was
`8a0733cf380367fc7696bf815463967cccfc816a63ae7281288021842d06a10a`; the
debug CLI SHA-256 was
`56bfdd7900084a09080b1e19b2dc9b3897316a95e2c3d6d0ab76cf859a33ef01`.
The compact record is
`CANONICAL-SYNC-V2-ROLLOUT-END-TO-END-TWO-HOST-EVIDENCE.json`.

## Security boundary and non-claims

The SSH connection was only a test-control and artifact-transfer channel. The
run did not execute the peer network, create a listening P2P socket or change
runtime IPC. It therefore validates executable portability and dormant
control-plane behavior, not NAT traversal, relay behavior or an independent
security review.

No release build, archive, Git tag, production trust root or public rollout
was created. The public default remains `legacy-only`, automatic enrollment
remains forbidden and first-contact capability stripping remains unresolved.
