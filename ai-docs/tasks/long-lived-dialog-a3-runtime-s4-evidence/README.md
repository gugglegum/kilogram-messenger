# S4 verification evidence

Use repository root on Windows. `run-check.ps1` captures source before running,
refuses an existing evidence name, uses verification/locked/offline/Jobs12/
BelowNormal and workspace-local TEMP. Never run a network harness with direct
`cargo test`. Stable test EXE paths and separate fixed application paths are
intentional; revision is recorded by content hash.

Do not run a second CLI stable runner or overwrite the fixed application while
the first CLI harness/its owned children are alive. Independent state/protocol/IPC
targets may run concurrently. Network checks require a native Windows token here:
the sandbox produced loopback10013, recorded separately from product failures.

## Reproduce selected checks

Choose fresh evidence names; commands below show the exact package/selectors and
skip lists. The wrapper compiles tests without running and then executes their
stable paths. A complete final P is one whole command, never stitched from parts.
The current candidate's P controller2950/runner3000 is the single explicit root
review4 replacement authorization after the helper repair and real relay smoke.
The review2 attempt remains FAIL. Original P2400 is still a separate target; if actual elapsed
exceeds it, the report must say so. Full runs shown here are reproduction recipes,
not authorization for additional silent attempts after this candidate's one run.

```powershell
$check = './ai-docs/tasks/long-lived-dialog-a3-runtime-s4-evidence/run-check.ps1'
& $check -Mode run -Name REPLAY-P -Package kilogram-cli,kilogram-transport-iroh -Filter a3_s4_p_full_real_dialogue_offline_two_contexts
& $check -Mode run -Name REPLAY-F -Package kilogram-cli,kilogram-transport-iroh,kilogram-state -Filter a3_s4_f
& $check -Mode run -Name REPLAY-runtime-foundation -Package kilogram-state -Filter a3_s4_ -SkipTest a3_s4_f
& $check -Mode run -Name REPLAY-state-component -Package kilogram-state -Filter indexed_component_
& $check -Mode run -Name REPLAY-state-history -Package kilogram-state -Filter history_
& $check -Mode run -Name REPLAY-state-S3 -Package kilogram-state -Filter a3_s3_
& $check -Mode run -Name REPLAY-codec -Package kilogram-protocol -Filter ordered_dialog_
& $check -Mode run -Name REPLAY-wire-bound -Package kilogram-protocol -Filter maximal_ordered_wire_payloads_fit_normal_batch_and_reserved_head
& $check -Mode run -Name REPLAY-IPC -Package kilogram-runtime-ipc -Filter 'tests::'
& $check -Mode run -Name REPLAY-CLI-G0 -Package kilogram-cli -Filter a3_s3_g0
& $check -Mode run -Name REPLAY-CLI-authority -Package kilogram-cli -Filter a3_s3_component_authority_atomic_reopen
& $check -Mode run -Name REPLAY-CLI-history -Package kilogram-cli,kilogram-transport-iroh -Filter a3_s4_component_ipc_history_escaped_byte_budget_no_cursor_skip
& $check -Mode run -Name REPLAY-schedule-controls -Package kilogram-cli,kilogram-transport-iroh -Filter a3_s4_controller_schedule_lock_
& $check -Mode clippy -Name REPLAY-clippy -Package kilogram-cli,kilogram-state,kilogram-protocol,kilogram-identity,kilogram-runtime-ipc,kilogram-transport-iroh
```

The actual selected final contributor names, exit codes, elapsed sums, source/log/
result hashes and runner EXE hashes are in `final-suite-summary.json`,
`final-source-correspondence.json` and `development-ledger.json`. A replay of the
whole F on current source does not reproduce the historical source-scoped supporting
commands; their conservative accounting is documented in the implementation report.
The selected current full F is `f-full-review3-frozen-1` (all19 tests). Root's
conservative F set also includes guard185.5564679s, controller controls22.3189717s
and targeted real F0935.3227291s; total620.4258161s. The earlier os5 complete-F FAIL
is preserved. Investigation diagnostics119.4238763/122.8123687s have separately
authorized bounds and do not replace acceptance gates.
Heavy history100k and long S3 Native tests are not enabled by these selectors;
the history scale test functions print NOT_SELECTED and return without their corpus.

## Build inputs and fixed applications

Build the current CLI using both CLI/transport packages for the recorded feature
closure, verification/locked/offline/Jobs12. Copy only after old owned processes
have exited. The compatibility L1 image must remain untouched.

```powershell
& $check -Mode build -Name REPLAY-CLI-build -Package kilogram-cli,kilogram-transport-iroh -CargoExtra @('--bin','kilogram-cli')
Copy-Item -LiteralPath 'target/verification/kilogram-cli.exe' -Destination 'target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe'
& $check -Mode build -Name REPLAY-relay-build -Package kilogram-transport-iroh -CargoExtra @('--example','s4_loopback_relay')
Copy-Item -LiteralPath 'target/verification/examples/s4_loopback_relay.exe' -Destination 'target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe'
```

For F10, create a new isolated `git archive` of exact
`7f82036ee5803edd64be98b73572604250f5961c`, extract under workspace temp, copy
`predecessor-probe.rs` into its state examples as `s4_predecessor.rs`, and build
with the extracted Cargo.toml/Cargo.lock, example target, verification/locked/
offline/Jobs12/BelowNormal and a separate `target/predecessor-s3`. The main checkout
is never switched/reset. The retained archive/build provenance documents the
original build. Fixed helper path:
`target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe`.
Positive ComponentRoot5 controls/correct owner and config are part of F10 itself.

## Derived review files

After selected runners finish, `node collect-process-metrics.mjs <run-name> <new-output.json>` aggregates only
public child/controller instrumentation and public fixture artifacts. It does not
open a state vault, IPC token, Root/Device key or Native checkpoint. Commit IO
percentiles describe closed-writer latency; they are not whole materialization or
history throughput. History timings and phase/queue timings stay separate.
Use `p-full-review4-relay-repair-1 review4-full-P-metrics.json` for the current P;
the P4 `final-process-metrics.json` and failed review2 `review2-full-P-metrics.json`
are immutable and must not be overwritten. Each metrics output requires a new name.
`watch-public-artifacts.mjs <run-name>` only copies public direct/relay artifacts
while the actual controller runs; it neither drives nor substitutes acceptance.

After the report is finalized, `node build-review-artifacts.mjs` generates the
derived ledger/aggregates/protected/source/image manifests and exact changed-file
manifest. It reads Git without changing HEAD/index and refuses frozen input or
product-source drift. `assert-frozen-product.mjs` verifies the original mechanical
42-file freeze and four images, with the explicit accepted helper override from
review4. All42 Rust files under apps/crates src directories are independently frozen;
the two42-element sets differ. It also checks Cargo inputs, HEAD and empty index.
`root-documents-reference.json` lists root-owned documents separately for review and
eventual acceptance. The candidate manifest excludes itself from recursive hashing
and excludes root-owned documents from executor allowlist. Supply manifest SHA separately
in the root callback. The manifest is a review candidate, not commit permission.

Original per-command logs/results/source snapshots remain immutable. Earlier FAILs,
selector-controlled stops and superseded candidates are visible in the development
ledger, with caveats for early hook results that are unsuitable for acceptance.
