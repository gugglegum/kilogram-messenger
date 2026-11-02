# Block4E — admission cost and narrow repair contract

2026-11-03 local (measurements2026-11-02 UTC). Baseline `77eb8c52e5cb73bd29f8bade2808e360650ddccc`. **Do not spend F02 yet: the cost concern is confirmed as a source-readiness issue.** One unchanged real compiler pair took **370.5912998s**. This is not a full runtime-admission measurement or an F/P time-fit result. No accepted source was edited.

## Actual measurements

Pinned pwsh/installed process-local Rust1.98, NativeToken, BelowNormal; one bounded `owner.ps1 -Step measure-01`,1170/1200s with30s cleanup reserve. Actual exit0/412.5287564s, waited/EOF, Job empty without forced cleanup. Production compiler functions had no wrappers, breakpoints or AST changes; only outer stopwatches were added. Phase journal writes happen after each timer stops. Native setup, cardinality analysis and journal overhead are outside those component timings.

| Component | Seconds |
|---|---:|
| Normal `Assert-S54Compiler`, one unmodified call | 168.3909899 |
| Tests `Assert-S54Compiler`, one unmodified call | 202.2003099 |
| Compiler pair, sequential sum | 370.5912998 |
| Strict read/decode whole normal/tests captures, outside compiler | 1.1885661 / 1.1070516 |
| Embedded dependency predicate | 3.9593399 |
| Full `Assert-S54Embedded`, separate read-only follow-up | 14.3219312 |
| Before/after source/capture/image/old-F01 preservation reads | 10.2980477 / 9.0984137 |

Full embedded **includes** its dependency predicate: do not add14.32+3.96. Whole capture decoding does not include the per-message parsing/shape walks inside compiler checks. The old409.1905123s validation included other work; neither409s nor this412.53s owner receipt is one predicate. These are single warm read-only observations, not distributions or runtime benchmarks.

## Proved call map

All counts below are the successful path. Earlier failures may stop it. Every PS inventory call invokes normal+tests at [inventory:369](E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1:369); `Test-S54Admission` delegates at [392](E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1:392).

| Boundary | Full compiler pairs | Original-clock charge | Source |
|---|---:|---|---|
| Preparation inventory creation | 1 PS | Outside runtime T0; preparation watchdog | [Prepare:632](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:632), [NewInventory:383](E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1:383) |
| Outer pre-start | 1 PS | T0 already recorded; before claim/launcher | [T0:495](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:495), [call:507](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:507), [pre-start:484](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:484) |
| Frozen leaf pre-native | 1 PS | Same original budget, before harness launch | [313](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:313) |
| Rust load before native claim | 1 Rust | Original sampled owner clock includes startup/inspection | [load:175](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_admission_runtime.rs:175), [bridge:220](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_admission_runtime.rs:220) |
| Rust result persistence pre-write and durable tail | 2 Rust | Original controller deadline | [303](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_admission_runtime.rs:303), [321](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_admission_runtime.rs:321), [FnOnce tail:78](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/attempt_result.rs:78) |
| Frozen leaf post-native | 1 PS | Original work/outer bounds | [324](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:324) |
| Outer persisted-transcript final revalidation | 1 PS | Original work/outer bounds, no clock reset | [559](E:/AI-Projects/kilogram-p2p-messenger/scripts/migration-s54-outer.ps1:559) |

Thus runtime success contains **4 PS pairs (8 compiler calls)** and **3 independent Rust pairs (6 capture validations)**. Rust inspection calls its compiler validator at [801](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_admission.rs:801); that validator loops normal/tests at [403](E:/AI-Projects/kilogram-p2p-messenger/apps/kilogram-cli/tests/support/migration_s54_admission_compiler.rs:403). Lightweight capture membership/IPC checks are additional; they are not extra full compiler pairs. Optional operator inspections are not counted as automatic runtime calls.

The two pre-native PS pairs are sequential (outer validates before spawning leaf; leaf validates before native). Applying this measured pair as a **sample-equivalent proxy** gives741.1825996s before native and1482.3651992s over four PS pairs, excluding all other inventory, Rust and functional work. These are estimates, not measured runtime durations or lower bounds. They are large relative to F450/480/510; concern is not disproved. P2350/2375/2400 fit is also not established. No deadline/workload change is proposed.

## Measured avoidable hotspot

At [compiler:300](E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1:300), each workspace manifest and src target scans all517 source rows. Each row evaluates live `Get-S54Path` on **both** the source and the same query. `Where-Object` materializes the entire match set; it does not stop after finding membership. `Get-S54Path` at [13](E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1:13) performs filesystem existence/reparse walks, not just lexical normalization.

| Capture | Raw units | Workspace messages | Queries | Source rows scanned | Membership Get-S54Path calls |
|---|---:|---:|---:|---:|---:|
| normal |447|28|56|28,952|57,904|
| tests |501|36|72|37,224|74,448|
| pair |948|64|128|66,176|132,352|

A separate four-query retained-data probe measured the **exact original expression** at 8.8097155s; one fresh canonical source-index construction at 1.2932121s; four indexed queries with fresh query-path checks at 0.0232761s; a closing fresh pin/reparse pass across all517 sources at 4.7046243s. All four original/index membership results match. This is a small operation probe, not an optimized full compiler measurement.

Scaling the four-query sample to128 queries suggests about282s of repeated scan work (roughly76% of the370.59s pair). This is **coarse attribution**, not an internally timed exclusive share: path distributions, OS caches and probe context differ. Remaining time includes schema/graph/per-message decoding, other live path checks, canonical unit comparisons, tool/image reads and hashing. No exact residual breakdown or optimized F fit is claimed.

## Proposed minimal implementation contract — requires root approval

Only change `Assert-S54Compiler` and small local helpers in the same existing PS file; add focused regressions. Do not change wire schemas, Rust policy, roles, markers, workload, process topology or clocks.

1. Preserve the existing inventory source enumeration and fresh source pin loop ([344–347](E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1:344)). Inside **each compiler invocation**, construct a local `HashSet<string>(OrdinalIgnoreCase)` from one fresh `Get-S54Path` pass over admitted source paths; reject canonical duplicates. Do not store it in inventory, a global, another role/process or an authority token.
2. Replace only the workspace membership scan by `Contains(Get-S54Path(query))`. Perform the fresh query check for **every** manifest/src decision, including repeated query strings. Retain the existing manifest/src checks outside membership, raw/output path validation and all capture wire/profile/graph/unit/tool/image checks.
3. Before successful compiler return add a closing fresh `Read-S54Pin` pass over **every** admitted source. This includes fresh path/reparse, length/hash and read-time identity checks, covering persistent mutation of an unqueried source after index creation. Keep current config/environment/clock/claim/ownership/I-A-L-C and result revalidation at their existing boundaries; leave independent Rust inspection intact.
4. The index contains **pure membership strings**, not cached filesystem observations. This deliberately replaces a redundant observation schedule; it is not a claim of identical per-stat traces or an atomic filesystem snapshot. Fresh per-query checks and the closing all-source pass are conditions of this proposal. No cache crosses invocations/normal-tests roles/processes/authority boundaries. If exact observation-trace equivalence is required, this contract needs revision rather than silently dropping checks.

The ignored public prototype supplies the existing inventory precondition through its own fresh head pin read, then tests membership and a fresh tail. Its14checks pass: case/separator parity, foreign membership, canonical duplicate refusal, content drift, missing paths and a junction inserted **after** indexing. It explicitly demonstrates that membership can stay true on changed content while the fresh tail refuses it. These fixture results are not production optimization acceptance.

Required implementation regressions: real serialized normal/tests positives; existing malformed/profile/graph/raw/role/foreign-source negatives and failure reasons; queried and unqueried source content/missing/reparse mutation between index/query/tail; tool/image/config/capture drift at existing checks; F02/P01 bindings; deterministic O(source+queries) membership-observation counts, excluding unrelated retained checks. After implementation measure one real pair again before considering F02. A source index alone does not prove overall admission or F time-fit.

## Evidence and boundary

The adjacent `admission-cost-evidence-01` folder contains byte-identical profiler/owner scripts, phase journals,3native receipts/logs, cardinality, prototype results, source call-map/contract and final seal. Original real captures/images remain in place and are linked by pins; no large unchanged captures are copied. `measure-01` exit0/412.5287564s; `followup-01` exit0/23.1647412s; `seal-01` exit0/11.4928543s. All waited/EOF/actual Job membership/empty cleanup verified; no timeout or forced cleanup. Seal stderr is the existing Git CRLF warning for root navigation, not a failed check.

Before/after and final seal verify517 current relevant sources, captures, all9 images/origins/selected dep-info, old38 failed-F01 pins, baseline HEAD, empty index and root navigation3 raw pins. Native exact-image inventory succeeded with0processes. Accepted project source is unchanged. Only ignored diagnostics/public fixture files and this review report/evidence were added; no Cargo, image staging, production Prepare/inventory/activation/claim, F02/P01 directories, messenger/private/profile/network execution, commit or push. F/P runtime remains **not_established**. Return for root approval of this measured patch contract; do not allocate F02 on an assumed cheap admission.
