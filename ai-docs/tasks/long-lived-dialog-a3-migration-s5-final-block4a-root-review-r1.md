# Block4A root review R1 — repair before publication

2026-11-02. Baseline `97a5bfb9360a9c3a95dbb4b9f3d84e9b5540f54c`. Candidate manifest62 paths/raw257543B/SHA256 `baf9c6a82bd573c22d41be2effda447e2b2e9eec7a251ae9e67bb0db1d6a4b1a` is not accepted for publication yet. Keep its source/evidence snapshot immutable before repairs. This is a concrete repair dispatch, not another design-only gate.

## Evidence already independently verified

Root command `ea87f3` verified all62 raw sizes/SHA256/Git blobs, baseline/index, compile02/pure02 identical before/after compiler inputs, all610 captured source hashes against disk and six actual stable image pins. Raw stdout contains9 distinct successful exact tests and6 zero-test aggregate results; PowerShell public report contains24 checks. The five final receipt objects exactly match actual completed exit0 desktop calls retrieved from the executor chat: fmt04 exec-c66e44eb, clippy03 exec-609bf1a8, compile02 exec-fe631b23, pure02 exec-c6481e6a, powershell08 exec-74513866. Root records `.tmp/s54-block4a-01/root-review-calls.json` and `root-evidence-audit.json`. Compile cleanup contained a remaining helper, as reported; this does not establish whole-runtime cleanup success. No test failures are being invented or reclassified here.

## R1. PowerShell compiler validator does not enforce its claimed closed set

`scripts/invoke-migration-s54-whole.ps1`, Assert-S54Compiler (around149–191), checks stable_artifacts.Count and each row in isolation, without uniqueness/exact role-set coverage. It also never checks expected_manifest_sha256/frozen_guard_verified consistency. Rust compiler::artifacts rejects duplicate paths and compiler::validate rejects an invented guard. The outer/public preparation validator must independently reject these malformed captures; a later Rust refusal is not a substitute for its contract.

Root reproduced both through the actual PowerShell Assert-S54Compiler on the final exported public fixture, with no file changes: load pure-02.fixture-index.json, New-S54Policy for its root, load inventory and compiler.tests, invoke baseline (accepted); replace stable_artifacts in memory with six copies of its first valid row (ACCEPTED); restore rows, set expected_manifest_sha256=null and frozen_guard_verified=true (ACCEPTED). Tool `961161`, session59845, finished exit0 confirmed by `addfd4`. This is a public in-memory compiler-validator reproduction, not a whole activation. No immutable fixture was modified.

Repair exact role/path bijection, expected-manifest/guard binding, and the immediately adjacent Rust/PowerShell semantic differences in this same boundary. In particular, PS currently does not enforce Rust's line-tables-only debuginfo, explicit unique package ID map (the reused helper accepts identical duplicate IDs), valid/unique dep_kind tuples, or executable containment under the compiler target/profile prefix. Keep shared helpers where they supply the required policy; add narrow checks where they do not. Do not weaken Rust to match PS. Add a compact table of production-path mutations with expected refusals in both implementations, including the two reproduced cases. Tests must reach the intended validation decision, not succeed because of expired activation or an unrelated stale pin. For the full preparation path, restamp only synthetic public references consistently as needed.

## R2. Declared compiler input closure omits embedded files

`migration_s54_admission_files.rs::relevant/sources` and Get-S54SourcePaths/Test-S54Relevant include selected code extensions plus9 scripts. They omit actual inputs to the selected migration harness:

- `apps/kilogram-cli/tests/support/migration_s54_capture_fixture.ps1`, embedded by migration_s54_capture_tests.rs:146;
- `apps/kilogram-cli/tests/support/migration_s54_memory_fixture.ps1`, embedded by migration_s54_memory_scope_tests.rs:364 and migration_s54_memory_tests.rs:456;
- `ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.candidate.json`, embedded by migration_s53_g1_prerequisite.rs:389.

The existing frozen compiler source-input enumerator also excludes these extensions, so comparing the two filtered sets cannot prove that these build inputs stayed unchanged. They are public compile inputs, not private profiles or an instruction to rerun their historical tests. Admission currently claims a complete source/compiler closure despite this gap.

Extend the explicit closure for actual embedded inputs of the selected build without recursively inventorying ai-docs/history. Reuse compiler dependency evidence if already available or maintain a narrow reviewed explicit embedded-input role set with a check for completeness against actual selected build dependencies. Do not create another graph/manifest framework. Preserve raw original compiler captures; add the missing source evidence through the smallest coherent production/preparation seam so runtime revalidation can detect drift. Include these inputs before/after the affected final build, not just in a post-build assertion. Add focused missing/changed embedded-input refusals through Rust/PowerShell source policy. Do not execute embedded historical fixtures, edit their contents, or replay earlier matrices.

## Finish the repair

Preserve R0 submitted snapshots/readiness/review and failed logs. Create fresh R1 check identifiers/evidence and updated final candidate, rather than rewriting recorded attempts. Fix ordinary lint/test issues autonomously with finite per-command bounds. Run format/strict Clippy/compile-only plus affected stable admission tests and PowerShell public checks on final source. Existing jobs2/BelowNormal/offline/locked/stable-path procedure and all Slice A restrictions remain. No runtime whole/feasibility/private/network activation, normal CLI staging, final production inventory or commit/push. Keep nav3/admin/history unrelated bytes.

Return one substantive callback with revised diff, focused regression evidence and actual source/compiler/image pins; then end. Root will review the repair and either accept for publication or send concrete remaining findings. Safe clock/selector/Job/result implementation remains Slice B; this review does not expand it into Slice A.
