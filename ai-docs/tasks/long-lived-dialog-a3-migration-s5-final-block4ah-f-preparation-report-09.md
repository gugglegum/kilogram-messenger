# Block4AH: F09 Prepare failed at work cutoff — consumed, no retry

2026-11-03. **The sole F09 Prepare did not finish within its1170second work bound.** Owner exit1,actual child exit124,work_expired=true,elapsed1172.2915546s. Test compilation/staging completed;normal compilation was still in progress when the watchdog stopped the owned Job. F09 is failed/consumed and must not be reused. This establishes a preparation-time/resource blocker,not a product test/integrity failure. No compiler error was emitted before cutoff;normal build success remains unestablished.

No valid prepared inventory exists. Full `Assert-S54Inventory -State Fresh` was **not executed**,no Fresh duration or fresh prepared19row environment proof is claimed,and **no Run proposal or activation is created**. No Run/WholeP/closed test matrix/source repair/rollback/commit/push followed failure.

## Exact binding and actual command

HEAD `6b4aad648104dfdce518f5e8ef7aa668858d975a`,root run ID `909b16f80e79824e6755b47db920328639535cdcb1cd7870e01a32eadc9384b3`,Purpose FeasibilityF,case F-s54-mixed-hooks-feasibility-01,attempt F-s54-mixed-hooks-feasibility-01-attempt-09,selector live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public. Original command record below is historical evidence of this consumed attempt,not an instruction to rerun it:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s54-whole.ps1' '-Action' 'Prepare' '-Purpose' 'FeasibilityF' '-PreparationRunId' '909b16f80e79824e6755b47db920328639535cdcb1cd7870e01a32eadc9384b3'
```

| Step | Actual tool chunks | Child/owner exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `b60d8d -> de4838` | 0/0 | 29.6080603 | 270/300/30 |
| prepare-01 | `3d5b1d -> 75308d` | 124/1 | 1172.2915546 | 1170/1200/30 |
| postcheck-01 | `c378b2 -> 0fd0ee` | 0/0 | 22.1382477 | 570/600/30 |

Actual Prepare owner PID57756,child PID47412,child start `2026-11-03T07:30:19.4497970Z`,original monotonic tick541092781. Error is `whole-attempt work cutoff; cleanup reserve active`. Handle/Job membership,child wait and bounded stdout/stderr EOF are verified;reader_failed=false. Forced cleanup before list `[47412, 32972, 32688, 6640, 41376]`,after `[]`,verified=true,failed=false. PID47412 is the actual pinned pwsh child;PID32972 was earlier observed as its conhost.exe child. Executable identities of32688/6640/41376 at cutoff were not captured and remain unknown;earlier PID samples are not termination-time identity proof. No foreign process was killed. Read-only postcheck needed no force and verified zero exact-image conflicts and released workspace lock.

## Completed work and cutoff phase

Preflight passed before reservation:524accepted source pins/shared HEAD=Git,current F09/run binding,absent attempt/WholeP roots,nine images,successful native0conflict enumeration,workspace lock,installed tools/dependencies,x64/24effective workers,offline/locked,Jobs2/BelowNormal. Explicit adapted constants and21allowed live paths were checked before reservation;no numeric-wide replacement or initial setup error occurred.

The existing production seven-image guard saved all7current preimages. Cargo test compilation finished in **13m21s**,as recorded by Cargo;stable runner returned compiled-only and six test images were staged as01..06. Completed tests capture `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility-public-evidence\F-s54-mixed-hooks-feasibility-01-attempt-09\tests.json` is 2732359B,SHA256 `54e01d8132c724871eb256cdf1bc8facc5f13410b781500f754cd205d6e2ab40`;units=501,raw artifact records=501,fresh true=492,false=9. This is compile evidence,not seven functional tests or a listing run.

The subsequent normal `cargo build --package kilogram-cli --bin kilogram-cli --profile verification-scale --target x86_64-pc-windows-msvc --jobs 2 --offline --locked --message-format=json-render-diagnostics` phase was entered (published `scripts/migration-s54-preparation.ps1:161`,repeated compiler progress in current stderr),but normal.json/stage00 never completed. That argv is identified from unchanged published implementation;normal child command-line arguments were not separately sampled. Owned-tree CPU samples showed compiler work and changing rustc PIDs;they do not establish a normal-build result or precise standalone duration. The1170bound was left unchanged. No attempt was restarted or extended.

## Preserved partial state — not runnable as a prepared set

Reserved F09 run directory is empty:inputs.json absent. Public top level has only image-preimages/,embedded-before.json,tests.json. Exactly16public files remain:7preimage binaries,preimages.json,6stage records,embedded-before.json and tests.json. No normal.json,frozen.json,isolated.json,list.txt,staged-00.json or preparation-outcome.json. The guard did not produce a success/failure outcome record before forced process termination;no such result is fabricated.

Read-only postcheck verified all7saved preimages against original preflight bytes,all6staged destination bytes against their stage records,all6test compiler origins still matching saved stage source pins and6dep-info matches. The normal runtime destination still equals its preflight/preimage bytes. Legacy CLI and relay are unchanged. Six live test destinations have new accepted staged bytes while normal remains old: **this partial image set is not prepared/validated for Run**. Current21allowed live output observations are pinned without rollback or restoration. They may contain partial/cached compiler work;no fresh-normal/frozen provenance is claimed.

No test executable was started (actual compiled-only output);even permitted stable `--list` was not reached because it follows completed normal staging. No runtime claim/profile/key/DB/f-origin was created or opened. inputs/activation/launch/capture/start/native-start/assertions are absent;WholeP P01 roots absent. Successful bounded postcheck is only a read-only failure diagnosis,not replacement Full Fresh validation.

## Historical/source preservation and handoff

1947approved flat public/admin/report/current-source pins,all524sources,3current navigation baselines and outside-report NUL-status are preserved. Source/index unchanged,HEAD above,index empty. AE/AF/V/Z repairs,compiler policy,workload,F450/480/510/P2350/2375/2400 and source-check diagnostic cache are unchanged. Only existing production preparation wrote its authorized target/cache outputs;no source repair or unrelated dirty change occurred.

F01/F02 remain empty,F03/F04 retain8names,F05/F06/F07/F08 retain89/397/337/339shallow names. No historical f-origin/profile/key/DB contents were read. All historical .log files were excluded before hashing (279excluded paths recorded),including administrative logs;AD16raw inputs were never expanded. Historical report/pin JSON and superseded deliveries are hashed as documents only. Old21live observations and old source/navigation hashes were excluded before current checks,without restamping old manifests. Original failed outputs,guard preimages/stages and compiled tests capture remain in place.

Report/pins are the only proposed new workspace review documents;no source candidate diff. `candidate.json` records exactly those2regular100644documents with raw/normalized Git blobs for review only. Actual command/exit/cleanup receipts and process samples are indexed in tool-return-observations.json;read-only postcheck pins current partial artifacts. Unused success-report/finalizer templates were not executed;validate-01.json and run-proposal.json do not exist.

Return blocker to root and stop without polling. Further preparation needs a separately assigned fresh attempt after root diagnosis;F09 cannot be repaired/reused. F01–F09 consumed,S5.4 remains open;WholeP P1 passive30s Backoff and later B25 coverage unchanged. No runtime proposal is returned for this failed preparation.
