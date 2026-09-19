# RFC-0104: M1 candidate evidence composition (M0.9.81)

Status: implemented locally; the exact committed candidate must pass the
production verifier before an M1 tag is considered.

## 1. Purpose

Kilogram already has two different accepted results:

- the M0.9.76 cross-network service-free v2 field run proves the current
  direct/relay and volunteer mailbox lifecycle;
- the M0.9.80 independent Windows run proves byte-identical reproduction and
  signed provenance for the network-free conflict appliance.

Repeating the expensive field run after documentation and build-provenance-only
changes would add operator risk without exercising new messenger behavior. At
the same time, merely citing an old run would be unsafe if runtime, protocol or
dependency files had changed. M0.9.81 composes the two accepted results while
failing closed on exactly that drift.

## 2. Candidate record

`M1-CANDIDATE.json` pins:

- field run `20260920-001614` and revision
  `ff38e1b89dc5832abdb2a5d81f7ab3af06e0ceae`;
- GitHub run `35474774356`, attestation `48691093`, revision
  `4e9054ab2ccc6a4c542fb37d486b70e53027dd08` and artifact SHA-256
  `d9f9f450f915cd238137c8498ba965b0dfac92c17982d237c0f18790cb6cacb4`;
- exact protected path lists and canonical `git ls-tree` manifest hashes;
- the bounded stage properties and the residual risks that M1 does not solve.

The record deliberately does not contain private field state, test keys,
runtime IPC descriptors or the GitHub artifact ZIP.

## 3. Fail-closed continuity proof

`scripts/verify-kilogram-m1-candidate.ps1` requires both accepted revisions to
be ancestors of the candidate. It recomputes the exact runtime/protocol Git
surface at the field revision and at candidate `HEAD`; any tracked, staged,
untracked or dependency change inside that surface rejects the candidate.

It separately protects the complete source and build-policy surface covered by
the M0.9.80 attestation. The verifier also reruns the service-free field-kit,
independent-builder and M1 acceptance static gates. There is no option to skip
Git continuity, field identity or attestation identity.

Because the exact runtime/protocol Git surface is unchanged, this composition
does not require another network run. Any future change in that surface must
produce fresh field evidence instead of editing the record.

## 4. Honest M1 boundary

M1 is a Windows technical proof-of-concept baseline. It demonstrates account
and device authority, pairwise E2EE messaging, local encrypted history,
multi-device recovery, direct/relay transport and service-free offline delivery
through exact volunteer replicas.

It does not claim a public security release. In particular, the accepted field
topology used two provider identities on one Alice host and therefore did not
prove physically or operator-independent providers. Sybil resistance,
access-correlation privacy, mobile background delivery, groups/channels,
production signing and update distribution remain later work.

## 5. Stage operation

M0.9.81 is network-free and build-free. It creates no release executable, ZIP,
background service or Git tag. The production verifier is run only after the
candidate files are committed so that `HEAD` and every protected surface are
unambiguous. Tagging and publishing M1 remain separate explicit actions.
