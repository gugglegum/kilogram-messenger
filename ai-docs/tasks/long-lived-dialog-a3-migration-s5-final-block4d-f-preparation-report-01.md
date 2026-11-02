# Block4D — F preparation failed; review handoff

2026-11-02. Actual production Prepare exit **1**, elapsed **1014.5606315 s**, work watchdog **not expired**. This is a preparation/admission failure, not a messenger result or timeout. No accepted `inputs.json` exists. F runtime/time fit and whole S5.4 remain **not_established**.

## Identity and actual execution

Published HEAD `d0e0cd7bfb05132b0769dc468dec027218d9950b`; parent `4c1fd911c57d7c47a166b7aec01e8d174f21799c`; tree `c1f82450418b957d27adcf9da8126d30628a4858`. Case `F-s54-mixed-hooks-feasibility-01`, attempt01; root preparation identity `781ded2ab918d9f46b99394c3e7a912e174d7ab922f31622cad3333cdbbd1cc8`.

Actual pinned pwsh child argv (receipt is authoritative):

```powershell
C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe -NoProfile -NonInteractive -File scripts/invoke-migration-s54-whole.ps1 -Action Prepare -Purpose FeasibilityF -PreparationRunId 781ded2ab918d9f46b99394c3e7a912e174d7ab922f31622cad3333cdbbd1cc8
```

One production invocation only. NativeToken; process-local Rust1.98 regular bin prepended to PATH, `CARGO_NET_OFFLINE=true`, Jobs2, BelowNormal, offline/locked. Independent owner budget1170/1200s includes30s cleanup reserve. F450/480/510 and P2350/2375/2400 unchanged. No persistent environment, installed-tool or ACL change.

Preflight01 failed in administrative source-pin enumeration before production reservation (nested array passed as a path); the administrative script was corrected, with original versions/receipt retained. Preflight02 passed in12.4275708s, actual metadata query succeeded, exact image conflicts0, fresh F/P directories absent, source516, workers24, installed compiler dependencies present. No old matrices were rerun.

## Concrete failure

`scripts/invoke-migration-s54-whole.ps1:328`, `Assert-S54Compiler`: **S54 actual compiler executable bytes.** Final `New-S54InventoryBytes` invokes real `Assert-S54Inventory`; the normal compiler executable no longer matches its captured/staged pin. The admission refused correctly before writing inventory.

| Evidence | Bytes | SHA256 |
|---|---:|---|
| Captured and staged normal CLI | 50413056 | `bf3e4642dacfb44f890e49ce917495f67d1e54b603a6d50794ce8626f4ec4a80` |
| Current actual Cargo normal executable | 50476032 | `681757614eac48ba481b8a907dc5bb30f2942aef2dfff459c360e9f1e0c7f6a9` |

The shared Cargo normal path is `target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe`. `staged-00.json` records that original compiler path matching the first pin at staging. The subsequent compile-only test capture also contains a non-test normal CLI at the same path. Read-only recheck reproduces the single mismatching predicate; all six selected test executable units still match their staged pins. Source sequence builds/stages normal first, then invokes compile-only test compilation in the same target/profile. This supports a build-output overwrite diagnosis; precise dependency/feature causation has not been separately established. No compiler capture or executable was patched to bypass admission.

## Retained evidence and cleanup

Original public evidence: `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility-public-evidence\F-s54-mixed-hooks-feasibility-01-attempt-01`. Occupied run: `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s54-feasibility\F-s54-mixed-hooks-feasibility-01-attempt-01`; it contains **zero files**. Attempt directories are preserved and must not be reused.

Actual normal/test compiler captures, frozen manifest, embedded-before snapshot, exact selector list and isolated selection were created. All seven images were staged exactly once through the guard; seven original preimages remain byte-verified. Outcome: `stage_completed=true`, `preparation_succeeded=false`, `automatic_rollback=false`, owner29956. Stable staged images remain in place; no manual replacement or rollback occurred.

All9 current image pins, seven stage records/preimages, current compiler hashes, public file pins, 516-source preservation proof and navigation pins are in `.tmp/s54-block4d-f-preparation-01/diagnosis.json`; originals and exact administration pins are in the adjacent report `.pins.json`. No large compiler capture or binary is duplicated in this report.

| Public artifact | Bytes | SHA256 |
|---|---:|---|
| embedded-before.json | 756 | `0693fdc3680f11226f73bf908e4cd4c9be06af3d21857e7e4d02f3bb9d49181b` |
| frozen.json | 817455 | `839753d99bcb6b6220c5df196b3fc54dbfd878b9bfc1ba5ec178c24763c65067` |
| preimages.json | 4830 | `40f3aee0c5da460764ab88df9822185493b5f2b33ad51e03fc76eb87cb371ba7` |
| preparation-outcome.json | 2019 | `ae7a43c374fa26678ed4b1ef355976f15c6f4f38e8b805372de1fe42540c1caa` |
| isolated.json | 194 | `0680b4fe55b13e506cb99f826426da7d15145d924a108f27ab53402ad77fcc25` |
| list.txt | 33234 | `87bef9a24c8d9dcc6ab2f8c3d14fbcfca76e0b20016e35bc05bf9792f3e3dcc0` |
| normal.json | 1765774 | `3a73d6d5940d9ed1f1e067b6c95b5fa6ca0f4d81254c637ca2ba58e214a08c07` |
| tests.json | 2729983 | `08fb393211339d8a00d5e33295eeabbf3429ecd904387ef53f653f0ed6d110af` |

Actual owner receipt: child29956, start2026-11-02T17:22:58.7162778Z; actual exit1, waited, bounded stdout/stderr EOF, actual Job membership verified. Final cleanup found owned PID66072 and forced its termination; Job then empty, cleanup verified with no failure. Read-only NativeToken diagnosis independently found zero exact9-image processes and acquired/released the workspace lock. No unrelated process was stopped.

Relevant published source516 matches preflight; HEAD remains d0e0cd7, index empty. Root-owned dirty `ai-docs/README.md`, `ai-docs/chat-coordination.md`, `ai-docs/next-stage-plan.md` retain their baseline raw pins. Historical untracked work preserved. Legacy CLI and relay hashes unchanged.

No activation, launch, capture, runtime claim, profiles/private seeds, F/P messenger or relay launch, P directories, commit or push. Harness enumeration used `--list` only. Successful prepared-state validation was not run because the real preparation failed and no inventory exists.

## Source prerequisite and next gate

Return a preparation-source change for review: prevent subsequent test compilation from invalidating the admitted normal compiler origin. A concrete design option is an isolated normal Cargo output tree, with actual normal capture/dep-info paths admitted explicitly; another is to make normal compilation/staging final and adjust guard ordering while preserving all7 retained leases, exact-once staging and actual compiler-byte checks. Do not merely remove the current executable-byte check, rewrite the captured executable path to the staged copy or synthesize old provenance. Review the chosen order/path policy and its focused regression before publication. No source repair was made here.

A new attempt identity/directory dispatch is required after reviewed/published repair; this failed attempt stays immutable. No retry is authorized by this handoff.

F Run proposal is **blocked/not ready**: there is no admissible input inventory pin, so no valid concrete activation or runnable Run invocation can be issued. Root must first accept the preparation repair and dispatch a fresh preparation, then independently validate its successful receipt/inventory and issue the external activation validity interval. No activation file or runtime capability was written.

Stop after one substantive callback to root.
