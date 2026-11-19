# B terminal rollback retry: implemented source/package review

19 November 2026. Implements `terminal-t1-3e-B-rollback-retry-task.md`. The accepted five-path diagnostic/pre-profile change was first published as **31194e699932bdc6be8e44dee1b5183cd97fa883**, parent94d2d31c556f355cd695f0ebeb040980e7b37d4a. Non-force push to origin/master succeeded and ls-remote matched. Exactly five accepted blobs, allowlist SHA adcb6002..., were committed; ten unrelated tracked changes and the root task note were preserved. Publication receipt: `.tmp/terminal-t1-3e/B-rollback-retry-publication-01/publication-final.json`, SHA256 `01c7b5d53d1c0027050d74aa5e53de768ec02981b9326eba51401bcd7870f096`.

The additional retry slice below is **uncommitted and not deployed**. No new UAC, host installation/recovery, policy/task mutation, A launch, remote diagnostic probe or message test occurred in this stage.

## Fixed B entry and original admission

`PackageRetryB` builds a complete15-payload package and one fixed `RetryInstall-B.cmd`. The existing bootstrap accepts only its fixed RetryInstallB entry; launcher and installer explicitly reject sideA. The original RecoverInstall/A recovery logic and old package files remain untouched. SetupFreshB still requires an absent journal; the new operation is a distinct SetupRetryB.

Before any new intent/pin write, SetupRetryB holds the exact campaign install.lock and checks:

- Exact first owner `setup-6f5c3fe31f11475290ba9c1b58d305c9`, phase rolled_back, rollback_errors empty and unchanged fixed desired specification. Its original full five intents and original15 file pins must match the protected accepted B package manifest `8a15960ecc53d631b45cf38e22e792d465aab5c9394714004418957e6aa9fa12`; every original payload is hashed.
- Original protected fresh-audit snapshot/hash and initial.json must agree. The original PS5 Lab-Write bytes (compressed snapshot plus one LF) are reconstructed from that immutable audit to bind the initial file before the first retry record exists. Neither journal nor initial is reset/rebased.
- Original history entries have only the known owner, fixed want/initial/pins and supported finite phases/intents. Existing raw history hashes are retained; unexpected or changed history is refused.
- No owned task/rules/copied payload/baseline/live agent; profiles equal the original baseline and unrelated policy/network remain compatible. Idle-agent and image locks also refuse an agent startup/update race without state. They are held through the new intent and released before the ordinary task probe, so the task can start normally later.

The existing fresh in-lock Audit/transaction still follows admission, including its intervening-file collision check. New pins are adopted only at the existing installing/zero-intent write. Old primary_failure is cleared only for this new retry intent; the previous journal, including its primary failure/pins, is archived unchanged.

## Own subsequent clean rollbacks

The protected campaign `B-terminal-retry.json` records one fixed runner `B-terminal-rollback-retry-01`, exact retry package hash, original owner/manifest/audit/initial/want, history pins and up to16 distinct attempted owners. It is not an ordinary approval file or a general retry service.

After the initial new intent, a provenance record is persisted before mutation begins. A crash between intent and record cannot grant a future retry. The invocation's own bounded cleanup may still restore its exact journal/pin scope, but missing or incomplete provenance remains a refusal for a new attempt.

Only a terminal rolled_back journal with zero rollback errors, successful owned-absence/baseline/history proof, exact terminal journal hash and a clean protected record is admitted again. The prior owner's protected package bytes must match this same retry package. The latest record must own the journal; earlier attempts must also be clean. Unknown/replayed owners, different packages, pending records, changed initial/history/journal, residual state and the attempt bound are refusals. One invocation performs one450s main attempt and, only for its own intent, one180s RetryRollbackB fallback under the existing native job supervisor. No automatic user/UAC retry loop exists; wrapper1080s remains unchanged and B creates no A SSH actor.

This allows later clean rollbacks of this exact reviewed package without a source edit just because its owner changed. Incomplete/unknown states still need review, and a newly reviewed different package is not automatically admitted by an old provenance record.

## Diagnostics and evidence

The full package includes accepted strict COM/CIM comparison evidence and pre-profile ordering. Public bounded diagnostics additionally include retry-admission/outcome receipts with owner/package/initial hash/history count or failure proof. The campaign ledger and full audit remain protected. No case/GUID/default normalization was invented; a persisted COM mismatch can still occur, and this is explicitly an instrumented retry, not an established successful installation.

Final focused checks: **88** counted cases/bundles — retry36, package1, comparison14, preserved A recovery14, existing in-lock admission8, native supervisor9 and export/ACL6. Windows PowerShell5.1. The two retry transaction fixtures use real exact original15/new15 package bytes, production Write/Undo/CanRemove/private Apply/transaction/provenance functions, real immutable initial/history files and mocked privileged OS boundaries. Both deliberately fail with an exact InterfaceAlias mismatch at pre-profile-comparison, perform zero profile enables, roll back cleanly, retain the initial/history, and adopt new pins only at intent. The second distinct owner is admitted from its own protected clean provenance with no source change. The mismatch field is a test fixture, not a claim about the missing historical B field.

Negatives include owner/package/pins, semantic and byte-level initial drift, changed/missing history, errors/incomplete phase, residual task/rule/active agent/locked agent/payload/baseline/profile/foreign policy, missing install lock, orphan/missing/pending provenance, modified terminal journal and attempt bound. Production Preflight refuses a file inserted after admission before intent. Production supervisor routing selects450/180s SetupRetryB/RetryRollbackB only for the current intent, and rejects prior/missing ownership or incomplete capture as success. SetupFreshB remains strict. Actual token/task/DPAPI installation proof is still a future live gate.

Earlier package-01/02 are preserved superseded build snapshots; package-03 is final. The28/32-case earlier focused runs passed and remain preserved; final36 expands coverage. No failed attempt was repaired in place. A recovery fixture only adds its new mode flag=false when reusing the production Write block; its14 semantic/immutable-baseline checks pass unchanged in meaning.

## Concrete package and next gate

Final local package: `.tmp/terminal-t1-3e/B-rollback-retry-package-03/package-B`, exact17 files (15 payloads, manifest, RetryInstall-B.cmd). Manifest SHA256 **a135cfcb8e6c01d4c20d822e4dec4da9c1df3853d7b06a55a95039d591a0e9e0**. Future ordinary destination is fixed `C:\Users\test\KilogramLab\terminal-t11f\deploy\autonomous-B-rollback-retry-review-01`; it was not created or uploaded. CMD maximum line1506; completed bootstrap encoded14040 characters, parsed without executing it and within the CMD/direct-process bounds. Every payload matches current review source.

Review allowlist: six modified retry/bootstrap/diagnostic/installer/launcher/A-fixture sources, two new focused B checks and this result note (nine paths), relative to published31194e69. The final review index includes exact source/package/test/publication hashes and the coherent diff. Old local A/B package18-file sets are rechecked by their accepted manifest and per-file hashes without modifying them. Root reviews this complete candidate/package before any publication of the retry slice or changed deployment/user action. Executor returns one substantive callback and stops.
