# Block4P: actual F05 preparation, ready for runtime review

2026-11-03. The one authorized F05 Prepare succeeded on published HEAD `81a61ef3a8ef27cc77315258a2a59ec47e98b7bd`. A fresh process subsequently passed the real complete published `Assert-S54Inventory -State Fresh`; the entire function call took **33.5641038 seconds**, and the whole validation administration took52.3661470 seconds. This is prepared-state evidence, not runtime or whole-S5.4 acceptance. No activation, Run, WholeP workload, private profiles, source change or commit/push occurred.

## Bound inventory and actual commands

Purpose `FeasibilityF`; case `F-s54-mixed-hooks-feasibility-01`; attempt `F-s54-mixed-hooks-feasibility-01-attempt-05`; selector `live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`.

Root run ID `1e886725b06c70d42b9b566c32ff8dc59008fa160a65b21c469c9a01106aedf1`.

Inventory `.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-05/inputs.json`: **117091 bytes**, SHA256 `b90e60597daebed5d5938ffc4e09a8168c40b024dd5b37352ef97edcb176a58c`. Its source implementation and observed HEAD both bind the published baseline above. Source closure520 files; current images9. The run directory contains only `inputs.json`.

Fresh administration `.tmp/s54-block4p-f-preparation-05`, adapted from the preserved reviewed Block4L scripts. Each owner step starts a fresh installed pinned pwsh child. The actual Prepare child command was:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' -NoProfile -NonInteractive -File 'E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1' -Action Prepare -Purpose FeasibilityF -PreparationRunId 1e886725b06c70d42b9b566c32ff8dc59008fa160a65b21c469c9a01106aedf1
```

Actual preflight child: same pinned pwsh, `-NoProfile -NonInteractive -File .tmp/s54-block4p-f-preparation-05/preflight.ps1 -EvidencePath .tmp/s54-block4p-f-preparation-05/preflight-01.json` (receipt records absolute paths). Actual validation child: same pwsh, `-NoProfile -NonInteractive -File .tmp/s54-block4p-f-preparation-05/validate.ps1`. That script times precisely the complete `Assert-S54Inventory $inventory $policy -State Fresh` call, then verifies guard/preimage/origin/dep-info and preservation evidence. No compiler-only replay or closed N/O test matrix was repeated.

| Step | Actual tool returns | Child/owner exit | Whole elapsed seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| preflight-01 | `9630d1` → `a34f66` | 0/0 | 15.1665008 | 270/300/30 |
| prepare-01 | `2db415` → `d14a2e` | 0/0 | 569.6428029 | 1170/1200/30 |
| validate-01 | `5bbf8e` → `f85753` | 0/0 | 52.3661470 | 570/600/30 |

All three receipts record verified actual handle/Job membership, waited child, bounded stdout/stderr EOF, no reader failure or timeout, verified cleanup and after[]. Prepare ownerPID60920, actual childPID52796, started2026-11-03T01:30:09.5617299Z. Prepare cleanup terminated owned residual PID61720; its executable identity was not captured and is not inferred. Preflight and validation cleanup were nonforced. Cargo status and unrelated navigation line-ending notices are retained in original stderr.

The first request to launch Prepare was rejected by automatic approval review before execution, on insufficient authorization for replacing seven live images. Read-only proof `approval-scope-proof-01.json` verified run/public/receipt absence, seven unchanged preflight pins and generated test-artifact destinations exclusively under `target`. Re-review of the identical command cited the human standing local-test/NativeToken authorization and explicit orchestrator dispatch; it was accepted. `approval-review-01.json` preserves both events. **Only one Prepare process executed and one F05 reservation was created.** No bypass, source change or reservation retry occurred.

## Guard, compiler and complete validation

Preflight verified actual shared HEAD against Git, published relevant sources and empty index, fixed F05 identity/deadlines, absent F05/WholeP directories, six installed tool resolutions and installed target/MSVC/SDK libraries, successful native process enumeration with zero exact-image conflicts, workspace lock availability,24 effective workers and sufficient free space. Routine administration setup succeeded before reservation without modifying historical scripts.

The published seven-image guard saved and verified all preimages before any replacement and retained write leases. Six test roles were staged at2026-11-03T01:35:26.5518777Z through01:35:30.2996193Z; final normal CLI stage00 was created/written at01:38:47.7973346Z. Exactly seven stage records exist. The guard outcome belongs to actual Prepare child52796 and records `stage_completed=true`, `preparation_succeeded=true`, `automatic_rollback=false`.

- Preimage manifest `image-preimages/preimages.json`:4830B, SHA256 `b01fba53b373726680aab9de793d435ee0101f27d1ed43b1d72b67c3e2e5d461`.
- Guard outcome `image-preimages/preparation-outcome.json`:2018B, SHA256 `33b9eb88ef84cd3bfbec288aab5a53e561b1456c8b557020bce2b88ab06b6b65`.
- Normal compiler capture `normal.json`:1766230B, SHA256 `1fb40b1a3cc1876d99b1bb08bb4c962b6c984955a9e71ef8dfe94275df8c148c`;447 units,446 raw unit fresh flags true and1 false.
- Test compiler capture `tests.json`:2730767B, SHA256 `28ebd66879a1136d35e76b25916bd411256d1df6f2eecdc835cf557f8b86afba`;501 units,499 raw unit fresh flags true and2 false; six stable artifacts.
- Frozen manifest `frozen.json`:817770B, SHA256 `c3956627234a001e1efd23bf96ea3e8005fe1f801656093718a666368e21c25b`.

These paths are under `.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-05/`. Full exact pins for all original public artifacts are in the adjacent report manifest, without duplicate captures/binaries. Required guard preimage copies are the originals produced by Prepare.

Compilation used actual published compile-only stable runner, verification-scale/x86_64-pc-windows-msvc, locked/offline/Jobs2/BelowNormal, normal Cargo cache reuse and fixed EXE paths. Cargo reported test build5m05s and final normal build3m09s. The compile lane's `frozen_artifacts_verified=False` is its original status before creation of the new frozen manifest; complete subsequent inventory validation checked that newly produced manifest. The migration harness ran only its permitted stable-path `--list --format terse`; the exact F selector occurs once. No selected functional test or network scenario ran.

Fresh complete inventory validation passed all source/HEAD/identity/image/compiler/environment/frozen/embedded/selection checks. Subsequent checks verified all seven saved preimages against preflight, all staged source/destination bytes and outcome roles, test-first/normal-last timestamps, one normal plus six test compiler origins and one normal plus six test dep-info pins **after final normal build**. Embedded test dep-info agrees with inventory. Legacy CLI and relay are unchanged. Source/index/navigation remain unchanged; workspace lock is released; successful native process inventory has zero exact-image processes. No F05 activation/launch/capture/start/claims/assertions/profile artifacts or WholeP directories exist.

## Preservation and proposed later Run

565 immutable historical/production/N/O evidence pins are preserved. F01/F02 run directories remain empty; F03/F04 retain their original eight failure artifacts. Archive/report/admin files and preimages were neither edited nor restamped. Twenty-one old live-path observations (seven generated image destinations, seven selected Cargo origins and seven dep-info paths) were explicitly excluded from current immutable requirements; their historical files remain pinned. New live values are bound by the new inventory, actual guard/preimages and compiler evidence. Legacy CLI/relay stay in immutable preservation. Nonproduction source-check cache was not touched. Finalization additionally compares the outside-report Git status digest with the pre-edit snapshot; only this report and pins are new review artifacts.

`run-proposal.json` provides exact future argv, full inventory-bound environment rows, per-variable process actions, worker/resource settings and external activation fields. The proposed command, **not executed**, is:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe' -NoProfile -NonInteractive -File 'E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1' -Action Run -Purpose FeasibilityF -InventorySha256 b90e60597daebed5d5938ffc4e09a8168c40b024dd5b37352ef97edcb176a58c -ActivationSha256 ROOT_ISSUED_ACTIVATION_SHA256
```

Use fresh pinned x64 installed pwsh, NativeToken/BelowNormal. In the original parent environment prepend the regular Rust1.98 bin **once** to process PATH and set process `CARGO_NET_OFFLINE=true`; preserve the exact bound `RUST_LOG` and require the other16 bound environment variables absent. Preserve absent `TOKIO_WORKER_THREADS`, actual available/effective24, Jobs2/offline/locked/skip-cache. The original parent PATH hash is `32afa634aa3fe135e1ae545834facb755758a036d57f369377b3455a99075f88`, configured owner PATH `b27e052877463bc883e77849d2511124a10d9331d76cebf9cc7f8d29281d6313`, actual fresh child PATH `f2cb8ccdec5684217d6216cc31889021372fae7451c49482644268d748c2ac6a`. Fresh Run child must match every inventory environment presence/hash; no persistent environment changes. RUST_LOG hash `aa63925edf225e26d79eca9cebc7639de6dfe576d2d9e4183e162f99e38be7ee`.

External activation fields, proposed only: schema1; classification `root_s54_feasibility_activation`; new inventory path/117091B/SHA above; exact F05 case/attempt/selector/run ID/run path; implementation81a61ef3a8ef27cc77315258a2a59ec47e98b7bd; execution_mode `NativeToken`; retry `root_reallocation_only`; deadline controller450/work480/outer510, origin `owned_launch`, clock `windows_tick_count64`. Root must assign fresh `not_before_utc_ms`/`not_after_utc_ms` and create activation after review. No activation file or validity interval was issued here.

Stop for root review and a separate runtime assignment. Prepared-state success does not establish F feasibility, runtime admission, performance or whole-S5.4 acceptance.
