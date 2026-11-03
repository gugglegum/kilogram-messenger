# Block4T: actual F06 preparation, ready for runtime review

2026-11-03. The one authorized F06 Prepare succeeded on published HEAD `907d30e7f828cf12fbb6b0f91f2dd441efe4e69e`. A fresh pinned process passed the actual complete published `Assert-S54Inventory -State Fresh`; the whole function call took **33.8477516 seconds**, and validation administration took 54.6161197 seconds. This establishes prepared state only. F feasibility, runtime admission and whole S5.4 remain unaccepted.

## Identity, inventory and actual execution

Purpose `FeasibilityF`; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-06`; selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`. New root run ID `7d620f3dc567c342782db79db51c41335912e2205052080629871ef8762f6218`.

Inventory `.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-06/inputs.json`: **117304B, SHA256 `37268d5168da78a3b1a9d7f7a7b627762384b282e1235391aa936c8bfcc76ac7`**. Source implementation and observed HEAD match the published baseline. Source closure 521 files; current images 9. The run directory contains only `inputs.json`.

Fresh administration `.tmp/s54-block4t-f-preparation-06` reuses reviewed Block4P administration with current identity, baseline and preservation requirements. Historical scripts are unchanged. Each owner starts a fresh installed pinned pwsh child; original receipts preserve full argv, executable, process identity, timing, EOF, Job membership and cleanup.

Actual Prepare child:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s54-whole.ps1' '-Action' 'Prepare' '-Purpose' 'FeasibilityF' '-PreparationRunId' '7d620f3dc567c342782db79db51c41335912e2205052080629871ef8762f6218'
```

Actual full validation child:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' '-NoProfile' '-NonInteractive' '-File' 'E:\\AI-Projects\\kilogram-p2p-messenger\\.tmp\\s54-block4t-f-preparation-06\\validate.ps1'
```

The validation script measures precisely the full `Assert-S54Inventory $inventory $policy -State Fresh` call, followed by guard/compiler/preservation checks. Preflight used the same pinned pwsh with fresh `preflight.ps1 -EvidencePath preflight-01.json`; its exact absolute argv is in its receipt. Owner entrypoints use `owner.ps1 -Step preflight-01`, `-Step prepare-01`, `-Step validate-01` respectively.

| Step | Actual tool returns | Child/owner exit | Whole elapsed seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `955bb0 → 65aed8` | 0/0 | 17.2546158 | 270/300/30 |
| prepare-01 | `a12b21 → a4174f` | 0/0 | 612.2037996 | 1170/1200/30 |
| validate-01 | `4386bc → 8e34ee` | 0/0 | 54.6161197 | 570/600/30 |

All receipts show verified actual handle/Job membership, waited child, bounded output EOF, no reader failure or work expiry, verified cleanup and after[]. Prepare owner PID 27996, child PID 64748, started `2026-11-03T02:46:19.5623296Z`. Prepare cleanup terminated owned residual PID 61668; its executable identity was not captured and is not inferred. Preflight and validation cleanup were nonforced. Cargo output and unrelated navigation CRLF notices remain in original logs.

The first Prepare request was rejected by automatic approval review before execution because authorization for replacement of shared generated images/origins/dep-info was considered insufficient. Read-only proof verified all 21 replacement paths inside this workspace's `target`, unchanged prior pins, absent F06 reservation/receipt, and guard preimage protection. Re-review of the identical command cited explicit human local-test/NativeToken authorization and root dispatch and accepted it. `approval-scope-proof-01.json` and `approval-review-01.json` preserve the facts. **Only one actual Prepare process and F06 reservation occurred.** No retry or workaround occurred.

## Guard, compiler and Fresh validation

Preflight verified current source closure, actual shared HEAD=Git, empty index, exact F06 purpose/identity, absent F06/WholeP directories, pinned installed tools and target/MSVC/SDK dependencies, successful native process enumeration with zero exact-image conflicts, workspace lock availability, effective 24 workers and available disk space.

The seven-image guard saved all original preimages before replacement and held write leases. Six test stages were recorded from `2026-11-03T02:52:07.0126014Z` through `2026-11-03T02:52:10.0974237Z`; final normal CLI stage00 was recorded at `2026-11-03T02:55:39.5823755Z`. Exactly seven stage records exist. Guard outcome belongs to actual child 64748 and records `stage_completed=true`, `preparation_succeeded=true`, `automatic_rollback=false`.

- `image-preimages/preimages.json`: 4830B, SHA256 `d4820499dbf8244a2fff590f6e2b537017ad0723d0fff7796918f16f1985b14e`.
- `image-preimages/preparation-outcome.json`: 2018B, SHA256 `9341c2c55416949af41cd64e8240c3f2c9e24253a034fd9fece8df5436436d6e`.
- `normal.json`: 1766540B, SHA256 `578179100fe0c79481e8e321ada6eb6f7ae870d7f870bf13fef8424c647738be`; 447 units, raw fresh flags true=446, false=1.
- `tests.json`: 2731153B, SHA256 `811317e287892f9625c11919b4975f5f75d010b6c6719df05220b5e0de6d2403`; 501 units, raw fresh flags true=499, false=2.
- `frozen.json`: 817925B, SHA256 `37f11e3f5822c2ff9f8df60fe3a325faf26e73f37fad08da370f8294a8fa24dd`.

These original artifacts are under `.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-06/`. Adjacent report pins reference every original public file and required guard-produced preimage; captures and binaries are not duplicated.

Actual compilation used the published compile-only stable runner: verification-scale/x86_64-pc-windows-msvc, locked/offline, Jobs2/BelowNormal, regular Rust1.98 bin prepended once and permitted cache reuse. Cargo reported tests 5m35s and final normal build 3m21s. The compile lane's original `frozen_artifacts_verified=False` precedes creation of the new frozen manifest; subsequent full Fresh validation verified that manifest. Stable migration harness execution was limited to the permitted `--list --format terse`; exact selector occurs once. No selected functional test or network scenario ran.

Complete Fresh validation passed source/HEAD/run/attempt, nine images, compiler/environment/frozen/embedded/selection checks. Additional checks verified all seven preimages against preflight, all stage source/destination bytes and outcome roles, test-first/normal-last order, one normal plus six test origins and one normal plus six test dep-info pins **after final normal build**. Embedded dep-info matches inventory. Legacy CLI/relay, source files, index and navigation remain unchanged. Workspace lock is released; successful native process enumeration found zero exact-image processes. F06 activation/launch/capture/start/claims/assertions/profiles and WholeP directories are absent.

## Preservation and later Run proposal

All 1471 immutable historical evidence pins are preserved. F01/F02 remain empty; F03/F04 retain original eight failure artifacts; F05 retains all 89 shallow top-level names and approved immutable artifacts without raw-log reanalysis or private profile traversal. Historical scripts, reports, captures, binaries and archived preimages were not rewritten or restamped. Twenty-one live generated image/origin/dep-info paths were explicitly excluded from immutable old observations and may change only through this guard. Their new bytes are pinned by F06 evidence. The nonproduction source-check cache was not touched. Finalization verifies outside-report Git status against the baseline and navigation raw hashes; only this report and its pins are new review documents.

`run-proposal.json` contains executable path and executable pin separately, exact argv, all 19 environment rows/actions, worker/resource policy and external activation fields. This is a fresh administrative reporting adjustment only; historical finalizers and production sources are unchanged. Proposed command, **not executed**:

```powershell
& 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -NoProfile -NonInteractive -File 'E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1' -Action Run -Purpose FeasibilityF -InventorySha256 37268d5168da78a3b1a9d7f7a7b627762384b282e1235391aa936c8bfcc76ac7 -ActivationSha256 ROOT_ISSUED_ACTIVATION_SHA256
```

Use fresh pinned installed x64 pwsh, NativeToken/BelowNormal. In the original parent environment prepend `C:\Users\Paul\.rustup\toolchains\1.98.0-x86_64-pc-windows-msvc\bin` exactly once to process PATH; set process `CARGO_NET_OFFLINE=true`; inherit exact RUST_LOG; require all other bound variables absent. Original parent PATH SHA256 `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`; configured owner PATH `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`; fresh child PATH `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. No persistent environment changes. The fresh Run child must match every inventory presence/hash:

| Variable | Required fresh child state |
| --- | --- |
| `AR` | absent |
| `ARFLAGS` | absent |
| `CARGO_HOME` | absent |
| `CARGO_INCREMENTAL` | absent |
| `CARGO_NET_OFFLINE` | present; SHA256 `b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b` |
| `CC` | absent |
| `CFLAGS` | absent |
| `CXX` | absent |
| `CXXFLAGS` | absent |
| `INCLUDE` | absent |
| `LIB` | absent |
| `LIBPATH` | absent |
| `PATH` | present; SHA256 `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a` |
| `RUST_LOG` | present; SHA256 `aa63925edf225e26d79eca9cebc7639de6dfe576d2d9e4183e162f99e38be7ee` |
| `RUSTUP_HOME` | absent |
| `RUSTUP_TOOLCHAIN` | absent |
| `VCTOOLSINSTALLDIR` | absent |
| `VSINSTALLDIR` | absent |
| `WINDOWSSDKDIR` | absent |

`TOKIO_WORKER_THREADS` remains absent, actual available/effective24; Jobs2/offline/locked/skip-cache, architecture x64. Tool resolutions, installed tool pins and Cargo config presence are bound by inventory and must still match. The recipe and its exact runtime descriptors are in the proposal.

Proposed external activation fields: schema1; classification `root_s54_feasibility_activation`; exact inventory path/117304B/SHA above; F06 case/attempt/selector/run ID/run path above; implementation `907d30e7f828cf12fbb6b0f91f2dd441efe4e69e`; execution_mode `NativeToken`; retry `root_reallocation_only`; deadline controller450/work480/outer510, origin `owned_launch`, clock `windows_tick_count64`. All fields are materialized as a proposal in administration only. Root must supply fresh `not_before_utc_ms`/`not_after_utc_ms` and create activation after review. No activation or validity interval was issued here. WholeP deadlines remain2350/2375/2400.

No runtime Run, WholeP allocation, private profile/seed access, production source edit, commit or push occurred. Stop for root review and separate runtime dispatch.
