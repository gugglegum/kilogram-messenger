# Block4V: F06 legacy offline barrier diagnosis and narrow fixture repair

2026-11-03. Baseline `907d30e7f828cf12fbb6b0f91f2dd441efe4e69e`. The retained F06 failure is localized to the **legacy offline-before-queue barrier**, not source capture, memory sampling, migration or mixed-current work. The immediate terminal error is the source-defined static label **`capture deadline cleanup_pending=false`** at command0190. A local fixture compatibility defect is established: it assumes an already idle killed peer must become `ConnectionBackoff` within30s, while the pinned legacy implementation may wait30s for idle Resume and then allow15s for that RPC. The missing transition is not a JSON parser incompatibility.

The repair adds a real close/wait/reopen of legacy owner0 after owner1 is stopped, then requires the original `ConnectionBackoff` state together with restored phase3, send_next3, receive_next3, unacked0 before queuing A3/4. The wait remains30s. No product/protocol/security code, timers, workload or queued-data assertions were changed. Focused pure native regression: old equivalent predicate **failed**, corrected predicate **passed**. This is a source-review candidate; no new production scenario was run and runtime feasibility remains unaccepted.

## Established chronology and evidence limits

Bounded local read-only analysis inspected191 retained command pairs and eight source-identified runtime logs: canonical containment, no reparse/symlink ancestors and size caps checked before reads. Exported evidence contains only source-backed states/types/counters/PIDs/timing and pins. No raw payloads, addresses, identities, tokens, arbitrary error suffixes, private profiles/keys/databases or F01–F05 private fixtures were traversed/exported.

| Saved step | Evidence | Interpretation |
| --- | --- | --- |
| first legacy pair | owner0 PID62400 born T0+114250ms, explicitly killed/waited by120783; owner1 PID61808 born114902, killed/waited by120536 | initial zero-text setup completed; next source-capture/restart path was reached |
| second legacy pair | owner0 PID53692 born120930; owner1 PID63984 born121684 | both originals reopened; imports and two sends each precede the recorded frontier convergence |
| commands0105/0106 | C1 send_next3,receive_next3,unacked0; C2 unchanged1/1 | the pre-stop two-each frontier gate was satisfied |
| command0107 / memory outcome | owner1 sample PID63984; explicit Kill/wait, closed atT0+145429ms | the second owner1 stop completed |
| commands0108–0189 |76 successful status captures, all C1 `Active`/phase3/frontier3/3/unacked0 and C2 Active/1/1 | the next strict Backoff condition never accepted; A3/4 were not queued |
| command0190 | capture classification `deadline`, owned query31780 killed/waited,code1,stdout/stderr0B,EOF true,cleanup_pending=false | first demonstrated terminal capture failure at the phase cutoff |
| abort | owner0 PID53692 closed T0+175478ms, stop=null,aborted=true | consequence of failure, not an explicit scenario stop or proof of its cause |

All139 status captures before0190 and23 memory queries have `capture=success`; query metadata contains no failed memory sample. Memory records show4 legacy/0 current lifetimes. Nothing establishes execution of migration, queued A3/4 or later mixed-data hooks. UTC file metadata helps correlate sequence; memory lifetime times are original-clock-relative. They are not promoted to per-RPC monotonic timestamps.

The known static error constructed from actual command0190 capture metadata and the source's error format plus LF is46 bytes and matches retained stderr prefix SHA256 `0246275d40455e5c97c056fbc65955876fc37ba505cd6d2fe21075d230515a28`. This identifies a closed source-backed label; no hash-based recovery of arbitrary errors/secrets was attempted. `followup-03.json` retains the proof and original pins. It does not infer the lower-level transport error.

Owner0 second-lifetime stderr ends with two source-backed Resume `rpc_begin kind=9` records without matching ends. Its earlier RPC-end records are successful; no later exported dialer state transition establishes why Backoff was delayed. Runtime trace lacks per-line timestamps and exported context identities, so the exact QUIC-loss/Resume timing is **not established**. The established defect is the fixture's unsupported bounded passive-disconnection premise; a separate transport/security defect is not claimed.

## Pinned legacy contract

Inventory legacy executable remains61531648B SHA256 `d9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1`. Accepted A2 build record `ai-docs/tasks/transport-security-a2-evidence/final-build.json` binds that executable to build HEAD `bc86cd25518077cef8977522e0c750f3bee2c0d7`. Local Git comparison verifies:

- `ordered_dialog/runtime.rs` is byte-identical Git blob `fe30077f246999219b4a7585dccf0cdde84f44e3` at historical build HEAD and current baseline.
- `ordered_cli.rs` differs only in later Runtime2 dispatch additions; its complete legacy Status arm is equal. Status exposes a JSON string from runtime field7 with the captured counters; it does not hide Backoff behind a different enum/schema. F06 initial commands0022–0029 actually exposed Backoff, confirming the pinned binary's observed contract.
- Engine files differ overall; relevant `active`, `check_lease`, `close_lease` bodies are equal at historical/current HEAD. The failed initial administration assertion of whole-file equality was preserved and replaced by this narrower proof; whole-engine equivalence is not claimed.

The actual runtime's data_worker probes idle Resume at30s and waits up to15s inside rpc; `ConnectionBackoff` is written only when the role0 established/admission path returns an error. Legacy Status reads the last control.statuses value; it is not a new network health probe. `ordered open/sync` opcode6 only acknowledges/wakes; it does not force immediate idle Resume. Therefore neither passive polling nor treating `Active` as disconnected proves offline-before-queue within the existing30s budget.

Relevant local source anchors: `ordered_dialog/runtime.rs` rpc143–197, data_worker843–899, status1077–1101, established1106–1127, role0dialer1226–1240, StatusIPC1298–1338 and open/sync1398–1408; `ordered_cli.rs` legacyStatus363–375; `migration_s54_control.rs` start397–462/kill_wait468–548 and memory_scope lifetime registration. Exact historical/current blobs and relevant equality booleans are in `followup-03.json`.

## Minimal correction and invariants

Only `migration_s54_feasibility.rs` and its focused test module change (61 insertions/4 deletions). After the existing stop1, the scenario uses existing `s.stop(0)?; s.start(0)?;` wrappers. Stop includes owned kill/wait/reconciliation; start deletes the old IPC binding, starts the same original directory with a fresh runtime/endpoint, and registers the actual new lifetime with the existing memory observer. No shell/process bypass or direct state/key manipulation was added. This adds one legacy owner0 lifetime; the existing36-lifetime cap is unchanged.

A new process has an empty in-memory lease/status map; owner1 is already waited dead. The same pinned role0 admission timeout15s now governs the first failed admission, instead of waiting for loss detection on an idle established link. The gate still requires actual `ConnectionBackoff: ` from that fresh runtime and now also phase3/frontier3/3/unacked0. It accepts no arbitrary disconnected-looking string and cannot accept startup Backoff with empty counters. Reopen cost is charged inside original F450/480/510; no additional clock, sleep or enlarged wait was introduced. Timing success is not guaranteed by pure tests and still needs a separately authorized new attempt after source review/publication.

Only after this gate are original A3/4 queued. Original pre-migration `send_next==3`, stop/reopen, migrated `next==[3,3] && queued==2`, durable queued replies, later cuts5/3, source identities, encryption/native accounting, recovery, memory/lifetime observation and every remaining scenario operation/assertion are retained. The scope proof reconstructs the original entire scenario file by removing only the new helper/reset and restoring the old inline call; the result is exactly equal. Thus all other scenario bytes and all preexisting tests remain equal. Product runtime/engine/transport/storage/encryption sources remain unchanged.

## Focused verification and preservation

The before variant extracted the original predicate without changing behavior; full-file inverse transformation proves equivalence. Its focused native regression fails on sanitized command0022 bootstrap Backoff, which lacks restored C1 state. The after variant rejects saved command0108 Active, rejects bootstrap and9 negative mutations (wrong phase/frontiers/debt/status forms), and accepts only the source-backed positive contract fixture. That positive fixture is explicitly synthetic, not a saved successful post-repair Run. These checks test the stricter gate; they do not simulate the actual transport restart or establish runtime feasibility.

| Step | Original tool returns | Child/owner exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- | --- |
| ps-01 | `966006 → 966006` | 0/0 | 1.3615532 | 570/600/30 |
| ps-02 | `c52e3b → c52e3b` | 1/1 | 1.0927654 | 570/600/30 |
| ps-03 | `3cf3c5 → 3cf3c5` | 0/0 | 1.5386554 | 570/600/30 |
| format-01 | `2bf32f → 2bf32f` | 0/0 | 1.7391098 | 570/600/30 |
| compile-01 | `084348 → d522dd` | 0/0 | 37.7763204 | 1170/1200/30 |
| rust-01 | `4f780b → 4f780b` | 1/1 | 3.7793474 | 570/600/30 |
| format-02 | `3b3ce0 → 3b3ce0` | 0/0 | 1.6827147 | 570/600/30 |
| compile-02 | `e62a8f → 8c174f` | 0/0 | 38.7165898 | 1170/1200/30 |
| rust-02 | `c84ee2 → c84ee2` | 0/0 | 8.2875012 | 570/600/30 |
| seal-01 | `7c02fc → 7c02fc` | 0/0 | 6.4600546 | 570/600/30 |

All actual receipts have verified handle/Job membership, waited child and bounded output EOF, no reader failure/work expiry, verified cleanup and after[]. Compile01/02 used owned forced cleanup for residual PID63680/63416 respectively; executable identities were not captured and are not inferred. Other cleanup was nonforced. `ps-02` is a preserved administrative proof failure from the overly broad whole-file equality assumption; `ps-03` corrected that proof. `rust-01` is the intentional failing before test; rust02 is the successful after test. Failed artifacts were not repaired/reused in place.

Both compile steps are compile-only, profileverification, offline/locked Jobs2/BelowNormal, through `scripts/run-cargo-tests-stable.ps1`. Actual Cargo test argv contains `--no-run` and `--target-dir target/s54-diagnostics-source-checks`. Focused native execution uses only that mutable nonproduction cache's fixed migration harness and the exact selector in selectors.json. No production image/origin/dep-info was rebuilt or replaced; no network scenario or closed matrix ran. Scoped rustfmt and whitespace checks pass.

Final seal verifies1061 protected pins,390 original allowed command/runtime log pins,519 other source files, navigation hashes and empty index. Native process enumeration succeeded with0 relevant live images. F01–F06 artifacts and production images/origins, retained preimages, T/U reports/pins and consumed run state remain immutable. F07 and WholeP directories remain absent; F06 is not reusable. Current candidate source differs from the historical F06 preparation source; no attempt is made to restamp/rebind that inventory or claim its old images implement this repair.

The review candidate includes the two source files, this report/pins and **unchanged Block4T/Block4U reports/pins** (8 paths total). Original large evidence is referenced, not copied. Outside-allowlist status is compared with the pre-edit baseline. No commit/push, new Prepare/Run, activation, F07 purpose transition or WholeP was performed. Stop for root review. Whole S5.4 remains unaccepted.
