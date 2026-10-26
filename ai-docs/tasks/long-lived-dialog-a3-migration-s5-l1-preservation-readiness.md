# S5.2 phase-writer repair and preservation-only readiness

2026-10-26. Static preparation under
[boundary03 completion review](long-lived-dialog-a3-migration-s5-l1-boundary03-completion-review.md),
6604B SHA256 `c98a14c445985acfa4e2295f9519848b73e8316a9d72b764900a6b36750211e0`.
No new C/F, test executable, stopped/frozen-state inventory, copy or preservation
was performed. This package stops for review before either planned case.

Boundary03 remains whole FAIL649.592232s and its parent-result remains absent.
Root accepted only its already completed source-bound functional assertions.
The planned separate preservation result cannot turn that whole failure green
or recreate lost status/history/cursor/token objects. No full boundary rerun or
product change is prepared.

## Exact delta and retained provenance

Only two of52 compiled inputs changed:

| Input | Before SHA256 | Current bytes / SHA256 |
| --- | --- | --- |
| `apps/kilogram-cli/tests/support/migration_s5_l1_exhausted.rs` | `c3e044594d43bcbf60b8055ba203aa8e23220f3b92b35d75f9bbfc049aae0977` | 50456 / `6f1576955aab0c2e9eb1e2f330d549ac35016618769c840ee3a613042c8e058f` |
| `scripts/invoke-migration-s5-l1-case.ps1` | `d3324692346ae12da8e10cb2bbeb2a614d9201d39597eb80536d1e8d34b2aac2` | 32804 / `bad2e0251c8e9dcbe6c93b4de24da840a15715aceaf7892b2b2bad2a6117578f` |

Pre-edit exact controller/runner snapshots are in evidence as
`S-boundary03-controller.rs.txt` and `S-boundary03-runner.ps1.txt`. The old
harness13935104B SHA256
`83429f6d41f566aadfdaddea0b862a0fe71120beec875560f6d9463dfdd932d0`
is retained, noncompiled/non-EXE, at
`target/compatibility/long-lived-dialog-migration-boundary03/migration_s5_process-harness.bin.snapshot`.
`S-boundary03-completion-before.json` binds the snapshots, unchanged ledger and
helper inputs before editing/restaging. Existing readiness/evidence is intact.

- [Controller patch](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary03-completion-controller.patch.txt):
  12230B SHA256 `62f5807345513e6f5a021cb897886d4070224da54826a64255edfec3e0855b2c`.
- [Runner patch](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary03-completion-runner.patch.txt):
  12241B SHA256 `38bdabf3efa8a43dfaccc05c2225bd3668a50104a31c0653c9970a6fdbd94503`.

`Phases` no longer stores a File. Construction creates the JSONL once with
`create_new` and closes it. Each record opens append, writes the same JSON/newline,
flushes/syncs, then closes before the next action. Error paths close the local
writer by Rust Drop. Existing check/run/drop error behavior, absolute controller760
and the complete genuine boundary body from SourceNative onward are identical
after newline normalization. No assertion, hook, migration call, reconnect25 or
raw preservation rule was weakened.

Unchanged actual implementations:

- Raw helper5011B SHA256 `25fe5a7c80821fb83a0fab5117585a48e6afe729094c1d45931aaa1556b044f3`.
- Obligations module24415B SHA256 `68bc85bb08508bb73b9a89344da22518e6d1bc1ee76797c3e1bf50ad4dd06b00`,
  including the unchanged `obligations::preserve_input`.
- Deadline helper6038B SHA256 `0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45`.
- Exact single-test matcher422B SHA256 `b6e1c73804b63997add074f47a1b924d88bb5b3e87ea7188ab4432ecd8dd4dc0`.

## Two narrow cases, not executed

`C-boundary-phase-preservation-01`, CLI selector
`migration_s5_l1_phase_preservation_opaque_held_locks`, work30/whole40:

- Creates a small opaque-byte fixture under the existing owned test root, with
  two real exclusive OS locks held on regular empty synchronization files.
- Runs actual `Phases::run` and the unchanged PowerShell preservation helper.
  The helper must hash/copy the closed phase prefix during the action and retain
  its full source-before/source-after/destination comparison.
- Reads back only opaque C backup bytes: copied phase file must equal the exact
  in-action prefix, opaque bytes and both empty destinations must match.
- After the helper returns, actual phase logging must append the phase end and
  a later record. This intentionally extends only the C source log after the
  frozen copy; no writer runs concurrently with the helper's comparison.
- Uses no Native, database, network, real origin or product runtime. Result is
  created immutably in evidence, outside the opaque input; stdout goes to the
  contributing runner log. Existing Phases checks remain under the outer30/40 job.

`F-exhausted-preservation-01`, CLI selector
`migration_s5_l1_stopped_checkpoint_preservation_only`, work120/whole140:

- Accepts only the stopped original active path
  `E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb`.
  It keeps both original absolute indexed.profile paths.
- Runner binds completion authority and exact case03 case/log/observation/phases/
  checkpoint-info/barrier hashes, plus all22 approved public export SHA256 values.
  The test byte-compares every corresponding public source to the approved export
  before and after preservation, and records actual BLAKE3/public provenance.
- Calls unchanged `obligations::preserve_input` exactly once: profile text load,
  both real empty OS locks held throughout, unchanged raw helper comparisons.
  It creates a new immutable private backup at
  `.tmp/migration-s5-preserved/F-exhausted-preservation-01`.
- No repository/Native/DB open, runtime, migration, recovery, restore, new active
  copy or frozen-origin traversal. It does not instantiate Phases in the input,
  alter the original JSONL or invent a parent result. No backup is typed-opened.
- New result/logs stay in evidence outside the input. The helper's actual returned
  preservation manifest path/hash and source-unchanged/copy-matches facts are kept.

Runner retains all old case policies/immutable IDs and adds only these exact
case/classification/selector/package/deadline and provenance guards. F requires
an actual **C whole PASS**, unexpired work, zero owned children and365 remaining F
(whole140+225). Known live Kilogram processes prevent stopped-only preservation.
Each new case requires its own later root approval bound to case, new runner and
current ledger. F approval must be issued against accounting after C completes.
Neither approval has been created by the executor. Both require exactly one
matching test PASS and normal stable-wrapper finalization.

## Actual static verification

| Record | Result |
| --- | --- |
| S-fmt-29 | PASS formatting,3.8185275s |
| S-clippy-26 | PASS CLI all targets, strict `-D warnings`,4.1847154s |
| S-compile-22 | PASS stable wrapper compiled-only,20.718447s; Cargo6.19s |
| S-fmt-30 | PASS format check,4.0984597s |
| S-boundary03-completion-policy.json | PowerShell Parser0 errors; exact retained genuine/check/run/drop source comparisons |

Locked/offline verification, Jobs12/BelowNormal, fixed paths. No Fast setting was
changed. Compile log explicitly says no test executable started. Policy validation
only parsed AST and compared source; runner/C/F/helper were not executed.
`git diff --check` passes; HEAD73c499818ba9b699c331aa842e64e8432e90c59a/index empty.

New compiled test harness at the same fixed path
`target/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`:
14014976B SHA256 `12bb6adda2a96c518785b654369805f6502b6e69998e28b30b1cde052688156e`.
All52 current compiled inputs match; all33 product inputs/eight images remain
unchanged, with current CLI/controlpeer SHA25669744fca50f44439b4734e699f416fc6b2bb8aac4336e5dad18ed173b2abbf6c.
No product build/stage action or schema/cryptography/verification change occurred.

[Machine readiness binding](long-lived-dialog-a3-migration-s5-l1-evidence/S-boundary03-completion-readiness-binding.json)
49592B SHA256 `afdd0ad89fb55ae3fee6d412b55e8c58ec905d585b4f90dd75b38b6fa301bae3`
contains exact source/harness/patch/static/helper bindings and78 retained public
historical artifacts. Superseded source/harness references denote preserved
snapshots rather than expectations at the overwritten active paths. No stopped
input was traversed or inventoried during preparation.

## Accounting, future commands and stop

Ledger419189B SHA256
`acd66ac604bb83cfee19d99a26122ce5f345b2d4f3eea4d0791e644869251f6e`
is unchanged:37 cases, C147.8846192/F3263.7458077/P0, ceilings500/3650/0.
Remaining C352.1153808/F386.2541923. Maximum F whole140 leaves246.2541923,
above225. No amendment, contributor, refund or P borrowing occurred. Historical
whole failures/split acceptances and terminal-publication timing disclosure stay.

After root accepts this package and issues the separate bound C approval:

```powershell
& scripts/invoke-migration-s5-l1-case.ps1 `
  -Case C-boundary-phase-preservation-01 -Set C -Action Tests -Package kilogram-cli `
  -Filter migration_s5_l1_phase_preservation_opaque_held_locks `
  -LongApprovalFile ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence/C-boundary-phase-preservation-01-root-go-ahead.json `
  -ExecutionMode Restricted
```

Only after actual C whole PASS and the F approval bound to its updated ledger:

```powershell
& scripts/invoke-migration-s5-l1-case.ps1 `
  -Case F-exhausted-preservation-01 -Set F -Action Tests -Package kilogram-cli `
  -Filter migration_s5_l1_stopped_checkpoint_preservation_only `
  -FixtureDirectory E:/AI-Projects/kilogram-p2p-messenger/.tmp/migration-s5-storage/l1-exhausted-observed-NCjBHb `
  -LongApprovalFile ai-docs/tasks/long-lived-dialog-a3-migration-s5-l1-evidence/F-exhausted-preservation-01-root-go-ahead.json `
  -ExecutionMode Restricted
```

Ordinary sandbox execution is proposed because these tests use only local raw
files/processes. The human's direct standing authorization covers any necessary
NativeToken invocation under ordinary tool review; it does not bypass that review.
No command above has run. Root-owned navigation/S5.3 handoff is untouched.
Stop for review: no publication/allowlist finalization, commit/push, product
optimization, whole-P or cut3 is authorized by this preparation.
