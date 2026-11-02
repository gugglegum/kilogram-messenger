# Block4G FeasibilityF attempt02 — failed preparation, retained evidence

2026-11-03 Asia/Yekaterinburg. Published baseline `d62a525eca008adfe0e2784645db4e8871986acd` and root preparation identity `670f56d5a5793ee6fb731f74aebcea8a10757e04a785ad1369c6c87b2b652b5b`.

**The single authorized production Prepare failed after staging all seven images. No inventory exists.** The run directory remains empty. There was no production retry, rollback, activation, Run, profile/private seed creation, P allocation, source fix, commit or push. Attempt02 is retained as a failed reserved attempt and must not be erased/reused by this executor.

## Failure and exact cause

Prepare actually exited1 after395.6785757s, without work deadline expiry (1170/1200/30). Both Cargo compilations completed successfully: tests in3m04s, normal CLI in3m00s. The stable runner reported compiled-only. The exact selected harness listing was generated and contains the F selector once. Failure happened in the real inventory construction/admission path before inputs.json could be written.

The published PowerShell `Assert-S54Inventory` reads .git/HEAD (line361), then unconditionally reads the symbolic reference as a loose file (line362). Current .git/HEAD says `ref: refs/heads/master`; `.git/refs/heads/master` is absent. The repository's existing `.git/packed-refs` has exactly one matching line:

```text
d62a525eca008adfe0e2784645db4e8871986acd refs/heads/master
```

Read-only `git show-ref --verify refs/heads/master` resolves that same commit; the ref format is files. The correct HEAD is present in packed storage. A process-local replay of **only these two original published HEAD statements**, using the unchanged actual read/path functions and a failure-observing wrapper, reproduced `S54 missing public path.` at exactly `E:\AI-Projects\kilogram-p2p-messenger\.git\refs\heads\master`. Source lines/text, failing query and stack, current source pin and immutable tiny HEAD/packed-refs observation snapshots are saved in diagnose-01.json. No inventory was reconstructed, persisted or admitted; no check was bypassed and no .git data was written.

The corresponding Rust `head()` in `apps/kilogram-cli/tests/support/migration_s54_admission.rs:157` already handles loose or packed refs with a1MiB packed-file cap and exact-one-match rule. The PowerShell path lacks that fallback. This is the immediate reproduced blocker, not a claim that all remaining admission would pass after correction.

## Actual preflight and execution

Fresh administration: `.tmp/s54-block4g-f-preparation-02`. Reviewed owner/preflight/validation machinery was adapted into fresh files; historical administration was never edited. Before reservation, the native preflight checked installed resolutions, argument/schema interfaces, published test-first/normal-last/completion ordering, parsed adapted scripts, all517 published source files,9images, zero exact-image conflicts, free existing workspace lock, compiler/target libraries, MSVC linker/SDK availability, workers24, unchanged HEAD/index/navigation/F01 and absent F02/P directories. Preflight passed in9.1418119s. Tools resolved to the pinned installed pwsh, Windows PowerShell, system Git/rustup and regular Rust1.98 cargo/rustc. Cargo metadata found524packages;21target rlibs and two SDK versions were present.

Actual preparation entry (through the bounded native owner):

```powershell
& C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe -NoProfile -NonInteractive -File scripts/invoke-migration-s54-whole.ps1 -Action Prepare -Purpose FeasibilityF -PreparationRunId 670f56d5a5793ee6fb731f74aebcea8a10757e04a785ad1369c6c87b2b652b5b
```

Preparation used installed pwsh, process-local Rust1.98 PATH prepend/CARGO_NET_OFFLINE=true, BelowNormal, Jobs2, offline/locked. No persistent environment, tool or ACL settings changed. F450/480/510 and P2350/2375/2400 runtime limits remain unchanged and were not used as the build watchdog.

| Bounded command | Actual child exit | Owner elapsed seconds | Work/whole/cleanup seconds |
|---|---:|---:|---:|
| preflight-01 | 0 | 9.1418119 | 270/300/30 |
| prepare-01 | 1 | 395.6785757 | 1170/1200/30 |
| diagnose-01 | 0 | 14.0653576 | 570/600/30 |

All three receipts establish actual child/Job membership, wait, stdout/stderr EOF and verified cleanup. Preparation cleanup found one remaining owned Job member, PID61388, and forcibly terminated it; after cleanup the Job had no remaining members. Its executable identity was not recorded, so no name is inferred. This was cleanup following child exit1, not a timeout. Preflight/diagnosis cleanup needed no forced termination. The121-byte stderr in those successful commands is the existing Git CRLF warning for root-owned chat-coordination.md; it is not a failed source check. Prepare stderr additionally contains the successful Cargo completions and the actual missing-path error.

## Retained state and verification

Production run: `.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-02` (empty).

Public evidence: `.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-02` (22 retained files, including the seven binary preimages).

The guard outcome is `stage_completed=true`, `preparation_succeeded=false`, `automatic_rollback=false`, owner68236 matching the actual preparation child. Exactly seven CreateNew stage records exist. Six test records were written21:13:23–21:13:26UTC, followed by current-cli21:16:34UTC. Each saved preimage matches its actual preflight/leased original. Each stage source and destination pin is currently readable and byte/hash-equal; all seven current image pins match the failed guard's staged outcome. Actual normal/test origin and dep-info paths after the final build were also read/pinned. No old F01 capture-to-current-image relation was required or repaired.

Normal.json, tests.json, embedded-before.json, frozen.json, isolated.json and list.txt are genuine output from the published preparation, retained without rebinding. Legacy CLI/relay pins remain unchanged. All517 relevant sources, HEAD/index and root navigation are unchanged. All38 original failed F01 artifact/administration pins still verify, and the original F01 run remains empty. The workspace lock can again be opened exclusively; a final successful native process inventory found zero exact retained image processes. P directories and all F activation/launch/capture/claim/profile artifacts remain absent. No foreign process was stopped.

Prepared-state Assert-S54Inventory validation was **not executed as a successful prepared check**, since no inventory was created and actual Prepare failed. Consequently there is no exact inventory pin, no whole prepared-inventory duration, and no admissible activation or F Run proposal. No runtime fit or whole S5.4 acceptance is implied.

Two administrative observations are disclosed without treating them as product failures: an optional progress-only PID filter did not identify the preparation child and was discarded rather than interpreted as a zero-process inventory; the adapted diagnosis owner emitted an unused validate argv array, but its recorded argv and actual child ran diagnose.ps1 only. The prepared validation script was not invoked. Both original preparation administration and diagnosis code are retained.

## Proposed next correction, for review only

The smallest coherent correction is PowerShell HEAD-resolution parity with the existing Rust policy: retain bounded4096-byte HEAD reading and local refs/heads/component restrictions; use the loose file when present and valid, otherwise read bounded1MiB packed-refs with strict UTF8 and require exactly one case-sensitive matching reference and a valid40-character lowercase digest. Existing read-time/nonreparse checks, expected observed-head comparison and all admission positions must remain; unreadable/reparse/malformed present loose refs must not become accepted fallback. Do not manufacture a loose .git ref or run pack/unpack/GC/configuration commands as a workaround.

Focused regressions should use new tiny public data for detached/loose/packed refs, no match/duplicate/case mismatch, malformed digest/UTF8/bounds and reparse/lookup failures. Add a cheap supported-HEAD check to future read-only preflight before reservation. No implementation or new tests were started in this task. Root must review the source correction and decide the next legal allocation/completion step while preserving this failed attempt.

## Artifact links

The adjacent `long-lived-dialog-a3-migration-s5-final-block4g-f-preparation-report-02.pins.json` pins this report and links the original administration/public artifacts, current sources/images and failed F01 manifest. It copies no large captures or binaries. Immutable HEAD snapshots are under the fresh administration; original Git metadata pins in diagnose-01.json are explicitly observations of mutable Git storage, not a promise that unrelated checkpoint packing cannot change that file later.

No commit/push is requested by this report. Awaiting root review of the concrete failure and next correction.
