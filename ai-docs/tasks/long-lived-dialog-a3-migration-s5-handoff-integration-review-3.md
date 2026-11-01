# S5.3 integration review 3: source closure and first infrastructure allocation

2026-10-27. Root `01a164ec-589c-7b41-bc34-3ba158d1f22b` accepts the r3
corrections R2-C1/C2/C3 for the limited execution below. This is source/readiness
acceptance, not integrated runtime acceptance or permission to commit/push/S5.4.

Readiness r3 SHA256 `78a62a50480c1f5d882f456f2dcc947d0eee021d417f519a015de5236c126afe`;
binding r3 `cf11b404339cd0ef1cd2666c43e29dc814a0f9c0837ccc446c7c64cc451db0a6`;
seal r3 `74e3a0f232a60581b0c4d96e88010e576c40829259a6a4b22fa7097b7c3b5ef5`.
Baseline `64c2359788ead360314793e47741adacb83b8d4f`, empty index, no live Kilogram.

Root verified all 821 inputs + 11 fixed images + one separate shell dependency,
21 seal anchors/diagnostic records, and all 828 preserved r2 input/image/submission
rows: 1682 hash checks passed. All four static before AND after inventories match
the live 788-path source/build inventory; exact r2/r3 delta is 11 paths.
Actual completed executor command results in turn
`01a17d6e-161a-77a3-85f4-8902631fe3ea` were independently inspected:

| Final static action | Actual command item | Exit / observed seconds | Inner seconds |
|---|---|---:|---:|
| fmt-check-final-01 | `exec-d90fb47c-65c6-4ece-b746-a62cfe0ba5b0` | 0 / 7.365 | 5.2909183 |
| clippy-final-02 | `exec-fb8da0b7-2734-4a2d-b34c-d3cc02d38012` | 0 / 15.622 | 13.720702 |
| build-final-01 | `exec-5c00e093-087c-45a9-b4ba-542b0f1ce48a` | 0 / 57.133 | 55.1679806 |
| compile-final-01 | `exec-f06c687f-3b78-47ee-a815-9fbf0d80adee` | 0 / 56.786 | 54.9079102 |

Full static prefixes are `S-s53-r3-`. Arguments confirm five-package all-target
strict Clippy, locked/offline build and six stable compile-only harnesses without
Run. No runtime evidence is inferred from compilation. The later failed literal
prose patch did not change the observed compile exit0.

Independent source review confirmed:

- Net disconnect uses the bindings returned by actual admissions, independently
  of current owner leases. Explicit absent-lease assertions after renewal are
  followed by actual fresh L1 admission; production consent gates remain intact.
- The fixed two-slot, cfg(test), serde-skipped progress counter advances only
  after actual Head RPC/reply acceptance/current-lease validation. Each exporter
  must complete two observed worker opportunities before the notification-count
  assertion. The fresh exporter must differ; the absolute 12s barrier is capped
  by the case deadline. No synthetic progress or timing-only success remains.
- One explicit shell is checked and retained through outer/adapter/inventory/env/
  Rust/metadata-script/children and predecessor validation. No PowerShell PATH
  rediscovery remains. Root confirmed the actual current host and the bound
  regular file:301368 bytes, SHA256
  `362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139`.

## First execution allocation

Only the immutable activation
`long-lived-dialog-a3-migration-s5-handoff-evidence/S5.3-integrated-r3-small-gate-01.activation.json`
grants these two sequential attempts, under the unchanged r3 binding:

1. `C-s53-route-regression-01-attempt-01`: kernel1/work35/whole50 seconds.
2. `C-s53-metadata-r2-01-attempt-01`: kernel20/work50/whole70 seconds, only after
   root verifies and preserves the first actual final return and successful
   full predecessor proof in a later distinct call.

Root will execute these small independent verification gates directly using the
reviewed absolute PowerShell host and the stable wrapper. This retains actual raw
tool results directly in the root producer's context. All staging, setup, baseline
metadata, source/image verification, test execution, cleanup and result finalization
belong to the attempt. NativeToken execution is covered by the standing user
authorization; no system setting changes. Jobs12/BelowNormal, one staging lane,
no direct cargo-test execution, discovery or product/Native fixture generation.

The root-only actual-return contract hash is
`a27c857d21fc62d4900509a76f3b9bf0c0cc5d92e100e95905cb158cd750c232`.
Preserve raw actual returns/arguments/history, outer/case/input/log/assertion
anchors; rehash and verify counts/baseline/cleanup and full elapsed before issuing
a receipt. Charge each actual attempt once. Any failed or unknown result stops
this allocation; no implicit retry or successor.

Initial actual remains S5.3 C721.424/F0/P0, combined
C1308.8726109/F4716.3201489/P0. Reserve only C120/F0/P0 within unchanged ceilings
C1140/F1200/P0. This leaves C298.576 unreserved and C418.576 unspent at activation.
Unused reservation is released only after actual charges are reconciled. Combined
C385 remains inactive and cannot automatically follow; fitting it under the current
ceiling requires first-gate actual <=33.576s or a separate explicit root decision.
No longer C/F/P matrix, private checkpoint preparation, generation511, publication
or S5.4 is activated. All previous historical scope/failure limitations remain.
