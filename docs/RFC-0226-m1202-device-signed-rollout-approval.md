# RFC-0226: Device-signed rollout approval bridge

Status: M1.2.2, verified locally, dormant.

## Authority stays with the local Device

The rollout controller remains advisory. A proposal, even when backed by a
root-verified policy bundle and healthy local evidence, cannot write or sign
canonical-sync activation state. The bridge accepts only the exact
`ProposeDeviceActivation` variant whose controller flags still require an
explicit Device approval and deny release-policy signing authority.

Preparation requires the explicit `UserOrTestOperatorApproved` authorization.
The current Device signs a bounded approval that commits to the trusted policy
bundle ID, release-policy ID and epoch, proposed target and disposition, Device
ID, exact activation predecessor ID and epoch, a fresh random nonce, and the
complete candidate activation bytes. The candidate is independently signed by
the same Device through the existing activation-record authority. A release or
containment operational key is therefore unable to manufacture either layer.

## Stale-state and replay behavior

Apply re-verifies the Device signature and candidate record and exact-matches
the caller's current trusted bundle and controller decision. It then compares
the observed activation head to the predecessor committed by the approval.
Any changed bundle, policy, proposal, Device, head, bytes or signature fails
closed. A byte-identical already-applied candidate is the only idempotent replay.

Emergency containment preserves the existing authenticated canonical evidence
and cannot be approved from a legacy or evidence-free head. The bridge performs
no filesystem write, runtime IPC or network operation; persistence remains an
explicit later transaction boundary.

## Verification and non-claims

Four focused tests cover explicit approval, canonical round trip, exact replay,
hold rejection, tamper, wrong Device, stale head and evidence-preserving
containment. The complete rollout crate passes 44/44 tests, rustfmt and
locked/offline strict Clippy.

M1.2.2 does not add a desktop button, activation persistence, production trust
root, runtime consumer or containment recovery. Peer wire, sync wire, runtime
IPC and the public `legacy-only` default remain unchanged.
