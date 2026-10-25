# A3-S5.1 storage implementation — accepted

2026-10-26. Root accepts S5.1 storage migration/admin implementation revision 3
for exact publication. R1–R4 are closed. This is storage-slice acceptance, not
completion of S5, live indexed L1 interoperability, existing-dialog handoff or MVP.

Expected publication parent/HEAD:
`3547cf528f15c15a21cdb9e30b1d7ca35a01297a` on master.
[Submitted revision-3 report](long-lived-dialog-a3-migration-s5-storage-implementation-revision-3.md):
10293 bytes, SHA256 `39685486b2626ad4b507e2cf1f8c3380b17c3c95f48953bc2d6ec02db7f306bc`.
[Candidate-3 manifest](long-lived-dialog-a3-migration-s5-storage-candidate-files-3.json):
79420 bytes, SHA256 `1621f2cc18ad7dc9028066667961582c8574f9f3c5710fa5f2bce931d8cf915a`.
Root independently verified all 227 listed files' sizes/raw hashes/normalized Git
blobs and the exact 228 changed paths including that manifest.

## Acceptance and verification

[Review1](long-lived-dialog-a3-migration-s5-storage-implementation-review-1.md)
and [review2](long-lived-dialog-a3-migration-s5-storage-implementation-review-2.md)
record the findings and their bounded repair authority. The accepted architecture
and historical evidence remain unchanged.

- R1: borrowed V4 metadata/index bounds precede copy/decryption in profile dispatch
  and interrupted-journal recovery; one verified index is reused. The real
  prepared-journal oversize negative preserves primary/shadows/journal and the
  valid recovery control passes. Recovery tests 4/4 on revision-2 final state code.
- R2: every archived locator and peer authority verifies static signatures and
  context/role/account/path pins. Coherent cross-C and foreign-account substitutions
  fail before Fence and resumed Ready. Expired locator and revocation witnesses
  retain exact bytes through Ready without current-permission promotion.
- R3: final-source storage invariants 2/2 and owner/core 7/7; field build inputs,
  fixed executable hashes and source correspondence verified. All five independent
  process scenarios complete on the same revision-2 candidate in the documented
  3+2 execution. The first aggregate failure remains a failure caused by a missing
  fixture argument; no single aggregate PASS or whole-P stitching is asserted.
- R4: saved genesis and captured conflicting Init/Confirm evidence now have
  distinct checks. Old proof must match the actual saved genesis ID/raw/role.
  New proof retains a valid conflicting signed role while satisfying the real
  old writer's capture boundary. Ordinary readable/Native/Head validation remains
  unchanged. The targeted test exercises both production Engine capture methods,
  exact block/history/Native retention through migration and two cold opens,
  and four wrong-signature/foreign-context negatives. It passes 1/1, zero Native
  packet/constructor calls; this is a test admission lease, not a new network
  handshake claim.

Root read the R4 production diff and regression body. The saved revision-2 source
copies match the reviewed manifest hashes; root independently confirmed identical
production prefix/suffix outside the fork branch and verbatim existing test bodies.
The runner's only change is the preauthorized budget metadata. Other product/state/
ratchet/protocol files remain exactly revision 2. Unblocked-origin owner/process
results and per-C history/escrow coverage are therefore reused with the documented
function/branch argument. No full suite was repeated solely for this narrow branch.

Root also independently checked all 33 targeted-case source hashes, all 439 final
build inputs before/after/current, six current/frozen images and 180 protected prior
files. All 38 revision-2 contributors remain identical; the preserved ledger
snapshot matches candidate 2. Strict affected state/CLI all-target Clippy passes
with literal `-- -D warnings`; locked/offline build, compile-only preparation,
format/diff checks pass. Windows fixed paths, stable runner, Jobs12/BelowNormal
are retained. Root's acceptance is based on source/evidence review and independent
integrity checks; root did not rerun tests or open historical fixture databases.

Final fixed CLI/control peer: 67720192 bytes, SHA256
`5685e34704c39edb3d6c7531f0c3c78d590915600201e52b9b9114516a231926`.
The four frozen d9b/S3/exact S4/relay images remain unchanged. The current public
ALPN1 control peer is not represented as an old predecessor executable.

Cumulative 39 contributors: C = 279.3257298/600 seconds,
F = 1236.2886628/1320 seconds, P = 0/2400 seconds. The 120-second F amendment was
explicitly granted in review2 before the single R4 run (54.8614242 seconds).
Failed attempts and original ceilings remain visible; no clock/reset/reclassification.
This allowance is not a budget for subsequent live-runtime work or whole P5.

## Accepted behavior and limits

Production bounded V4 source/Native/readable provenance, atomic same-DB Root7
Fence, bounded resumable copy/verification/Ready, source-backed historical records,
per-C escrow and one-time funding, offline administration/capacity CAS/publication
recovery and version/owner fences are accepted for this slice. Existing IDs, keys,
sealed content, requests, pending items, delivery debt and block evidence survive.

Storage Ready does not enable live messaging. The CLI explicitly refuses a live
indexed runtime until S5.2; no mutable old Engine or fresh Runtime6 fallback is
provided. Full511 continuation, indexed L1 owner/transport, initial-handshake
completion, Consent2/P/A/K/Committed, mixed live delivery and whole P5 remain open.
No public rollout, GUI, groups, multi-device, M1.2.63 or deferred A2 reproducer is
included. The post-power-loss checks establish integrity of this reviewed package
and supported copied seeds, not a whole-disk health assessment.

## Exact publication assignment

Publish only the paths/blobs in
[accepted manifest](long-lived-dialog-a3-migration-s5-storage-accepted-files.json).
It covers candidate 3 plus this review and the manifest itself. Root updates only
README/next-stage/coordination status before freezing that inventory; product code,
executor reports/manifests and per-case/static evidence are not edited by root.
The candidate manifests remain historical submission snapshots.

The existing executor is authorized to make one ordinary commit on master with
the sole parent above, then normal fast-forward push to origin/master. Suggested
subject: `feat: add durable migration for existing dialog storage`.
This is the explicit acceptance/publication instruction required by AGENTS.md and
the user's standing authorization; no renewed human confirmation is needed.

Before staging, compare exact changed paths, raw sizes/hashes and normalized Git
blobs with the accepted manifest, including its separately supplied self hash.
Stage only the explicit allowlist, verify staged paths/blobs, commit, verify its
sole parent and exact committed diff/blobs, then push and verify the remote tip.
No broad add, extra status artifact, evidence rewrite, amend, reset or force push.
Preserve unrelated work. If a check differs, stop and report rather than silently
changing the accepted package. Callback root with SHA, push result and checks;
root independently verifies the published diff/remote before assigning S5.2.
