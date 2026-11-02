# Block4D preparation-order repair — implementation submission

2026-11-02. Review candidate on published baseline `d0e0cd7bfb05132b0769dc468dec027218d9950b`; no commit/push. Production F02 preparation and F/P runtime remain **not_established**.

The accepted shared order is implemented: compile-only tests → stage6 under retained leases → final normal compile/stage1 → orchestrator complete7 → final selected origin/dep-info recheck → existing full compiler/inventory admission. No later Cargo build occurs in production preparation. Normal provenance remains the actual Cargo executable path.

The stable preparation runner now admits an empty stage, stages exactly its six fixed roles, reads their pins through retained handles and leaves all-seven completion to the orchestrator. This avoids incompatible extra file opens while leases remain held. Existing preimages, workspace lock, live-image refusals, exact-once staging and no-rollback behavior remain. The default runner lane is unchanged.

F is closed to `F-s54-mixed-hooks-feasibility-01-attempt-02` in Rust and PowerShell; fixtures, paths and marker identities derive from that policy. F01/foreign substitutions are refused. P remains fixed attempt01. Runtime budgets F450/480/510 and P2350/2375/2400, selectors, workloads, oracles and runtime product code are unchanged.

## Actual focused checks

| Check | Exit | Elapsed seconds | Evidence |
|---|---:|---:|---|
| Scoped rustfmt: four changed Rust files | 0 | 0.7762312 | `fmt-01.receipt.json` |
| Strict Clippy: CLI tests, scale MSVC, literal -- -D warnings | 0 | 16.3431195 | `clippy-01.receipt.json` |
| Three exact Rust pure/public tests via stable runner | 0 | 153.4689658 | `pure-01.receipt.json` |
| 35 shared order/guard +28 public purpose checks | 0 | 48.6053572 | `public-01.receipt.json` |
| First actual two-build sequence: administrative carrier failure | 1 | 343.0515625 | `compile-01.receipt.json` |
| Second actual shared guarded two-build: noncanonical-root administrative failure | 1 | 795.2788572 | `compile-02.receipt.json` |
| Read-only full compiler/dep-info admission of retained actual captures | 0 | 409.1905123 | `validate-01.receipt.json` |
| Final read-only preservation/process/lock/PS parse checks | 0 | 12.7725805 | `final-01.receipt.json` |

All commands used pinned pwsh and process-local installed Rust1.98, Jobs2/BelowNormal/offline/locked. Build watchdog1770/1800s with30s cleanup reserve; ordinary checks570/600s. No timeout occurred. Actual argv, native child exits/waits, EOF, Job membership, output pins and cleanup are in the receipts and paired logs.

Pure tests used the existing `verification` stable paths, distinct from the seven guarded scale destinations. Only three exact isolated selectors ran (P public admission, F full public admission including F01/foreign refusal, and closed F02/P01 identity); aggregate tests were all skipped. There was no network/messenger/profile/private-seed execution.

The benign shared guard regression demonstrates the previous normal-first overwrite failure, six-test-first retained leases, refusal of missing/duplicate/foreign roles and early completion, final normal identity, continued normal/test origin drift refusal, retained preimages and failure cleanup. Full actual PS inventory readers admit exported public F02/P01 fixtures and refuse F01/F03/foreign selector; public fixtures cannot supply production runtime authority.

First actual build verification (`compile-01`, exit1) completed both builds and shared origin comparisons, then its administrative proof passed the in-memory normal capture to the strict wire schema reader, producing `S54 closed object DepKind.` No compiler-origin conflict was observed. It was preserved with scripts, captures, seven preimages/outcome and logs. The administrative caller was fixed to read the written normal capture through `Read-S54Json`, and fresh guarded `compile-02` completed both builds/staging and both full compiler predicates, then failed the subsequent embedded dep-info check because its administrative caller supplied a forward-slash root to a canonical Windows-prefix check. The same recorded dep-info passes with `policy.Root`. Separate bounded read-only `validate-01` revalidates both serialized compiler captures, all actual origins and embedded dep-info using the canonical root, without Cargo or staging. Both original exit1 receipts and guard outcomes (`stage_completed=true`, `preparation_succeeded=false`) are retained unchanged. No production attempt was retried or source validation weakened.

## Actual compiler preservation proof

Both actual build verifications invoke the same `Invoke-S54CompileStage` used by production; successful read-only validation completes their evidence. Its captured stage-result has six test executable origin pins before/after final normal, six selected dep-info pins before/after, and final actual normal origin/dep-info. All pairs match. The real `Assert-S54Compiler` passes separately for serialized normal and test captures, retaining profile, environment/source, graph, raw unit and all-seven origin-to-staged byte predicates. Embedded source/dep-info closure also passes.

Final normal origin: 50413056B, SHA256 `9d9b5da5badf500a004299a8d8cfda8ab8eb6a89c7bea18fbbc89a5e1f032065`. All9 current image pins and source517 pins are in `compile-02.result.json`; seven preimage/stage/outcome pins and chronology are in `final-verification.json`.

Before any new build, both conflicting normal binaries from the original failed F01 were copied and hash-verified in ignored repair storage. The original F01 report and38 evidence pins remain unchanged; its run stays empty. Both original conflicting binaries and both actual build-verification preimage sets remain in ignored storage with pins. Text/JSON/log copies needed for review are in the adjacent repair evidence folder; no binaries or large preimages enter the candidate.

Final native metadata query succeeded with0exact9-image processes; workspace lock was released. All successful/failed command Jobs were emptied and verified, including explicitly recorded forced cleanup of owned build descendants. Legacy CLI/relay hashes, baseline HEAD, empty index and root navigation3 raw pins are preserved. Unrelated tracked and untracked work is excluded.

## Candidate and next gate

Exact allowlist/raw SHA256/normalized Git blob candidate: `.tmp/s54-block4d-preparation-order-repair-01/candidate.json`. It includes10 source files, this report, the original failed-preparation report/pins and the explicitly copied repair evidence directory. Root task/navigation notes and historical unrelated files are excluded. Large original captures/preimages are linked by pins; no independent old source snapshots or old matrices were regenerated.

After source review and explicit acceptance, root may dispatch publication, then a fresh production F02 Prepare with a new root preparation identity. That preparation must return successfully and produce an admitted inventory before root issues activation. This source task created no production F02/P01 directories, inventory, activation, Run capability, profile/private seeds or runtime success claim. No commit/push occurred.
