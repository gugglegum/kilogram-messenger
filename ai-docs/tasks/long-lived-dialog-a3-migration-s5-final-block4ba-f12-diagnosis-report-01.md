# Block4BA: F12 diagnosis and bounded fixture repair contract

2026-11-10. **Diagnosis only; proposed correction awaits root review.** HEAD `dbc8ebb7ade3b797b71234f898c507d44c2b6dd0`. F12 remains a consumed failure. S5.3 closed; S5.4 open; full F and WholeP remain unproved. No source edit, build, test, preparation, activation, replay, attempt allocation, commit or push was performed in BA.

## Evidence and last successful operation

Admin: `.tmp/s54-block4ba-f12-diagnosis-01`. The reader used AZ's recorded top-level names, not a directory scan. It inspected command pairs 1315–1322 first, then 1307–1314 to recover the source-defined window tuple and distinguish the final polling state. One guarded reread of stdout1322 confirmed that tuple in the last successful query. Total: **16 unique pairs / 32 files; 175541 bytes actually read including reread and final seal**, below 32 pairs, 64KiB/file and 2MiB. Before every content access: canonical direct-child containment, regular file, no reparse ancestors/files and size checks. No raw copies, private argv, bodies, keys, profiles or databases were read or exported.

All 16 captures report success/code0, waited exit and both EOFs; no kill or cleanup pending. **Last observed success is status3 command1322**, not a failed CLI/capture. All 16 status snapshots agree on:

| Actual B owner (role1) | Logical C2 / physical slot0 | Logical C1 / physical slot1 |
| --- | --- | --- |
| mode / encoding | L1 / Legacy7 | Active / Mixed7 |
| next | [5,5] | [185,35] |
| cuts | null / null | [5,3] |
| queued / debt / pending | 0 / 0 / 0 | 0 / 0 / 0 |
| lease_current / generation | false / null | true / 1 |
| native_generation | 8 | 86 |

Window is `[limits,current,peak]`, not a pair (`runtime2/window.rs:44`). Commands1307–1314 and1322 show `[[64,64],[0,32],[34,34]]`: **B C1 still reserves32**. The original reader omitted the three-part window in its first sanitized output; the correction and extra read are recorded, not overwritten. Capture owner PID36356 and harness PID51516 match the current run metadata. Individual status processes exited successfully. This does not identify the executable of AZ's separately reported forced PID51584.

## Established error; localized boundary

The public outer record contains a complete 49-byte stderr prefix hash `9429bd4b41beb1dedd46ec97ab2da6f62aa11618f2ccd6a2c8ce4c8e82a08408`. A finite comparison of three current source-defined deadline messages and eight UTF-8 prefix/quote/newline compositions per message gives exactly one match:

`Error: absolute S5.3 process controller deadline` followed by LF.

The message originates at `migration_s53_deadline.rs:156–163`. BA did not open harness stderr. This establishes the static deadline-check error; it is not an inference from elapsed time, launcher124 or F11. `Phase` shortens the effective deadline (`:126–151`), and `Controller::wait` installs that local phase (`migration_s54_control.rs:398–413`). Thus the message **does not mean the original450-second controller deadline expired**. AZ observed native exit at T0+328250ms; original450/480/510 limits and the AZ failure classification remain unchanged.

The strongly localized failed boundary is the **old held-ACK lease/window release barrier after stopping A**, `migration_s54_feasibility.rs:697–705`. Its two B status calls per iteration require C1 lease false, generation null and C1 current-window0. The observed repeated B-only statuses have exactly the contradictory state: lease true/gen1/window32, while C2 has already lost its lease. The ACK arm/read path (`:232–270,693–697`) precedes this barrier; its ACK is an outgoing push, not a query reply. The next A reconnect is after it (`:706`). Earlier page barriers target earlier frontiers and keep ACK disabled; `live`, cuts and histories do not have this final repeated two-B-query pattern. No exact stage-entry timestamp was persisted on failure, so execution of the individual stop/reconcile instruction and the precise check call within this wait are control-flow deductions, not separately recorded receipts.

**AW1's history boundary was passed as a necessary control-flow conclusion.** Frozen40 capture/validation is unconditional at `feasibility.rs:429`, before offline129 and later sends through184; the actual C1 frontier185 proves execution progressed beyond it. That capture checks first16 at[7,35] plus terminal suffix24 (`migration_s54_feasibility_data.rs:69–111`). Retained-cursor verification at `feasibility.rs:500` and the later histories call at691 also precede the localized barrier. BA did not replay those private histories and does not promote this conclusion to whole-F success.

## Cause classification and limits

The proven failure is **fixture observation deadline exhaustion while the peer's ALPN2 lease and held reserve remained current**. There is no evidence of a status-capture failure or original outer deadline exhaustion.

The source exposes a concrete timing assumption in that fixture:

- `Controller::stop/kill_wait` abruptly kills and waits A (`control.rs:528–605`); it does not send a graceful QUIC close to B.
- The outgoing ACK pause is before `raw_rpc` (`runtime7/network2.rs:148–172`). Its 15-second RPC timeout therefore does not enclose this held push. Incoming ACK-query replies have a different15-second envelope (`:392–410`), and cannot justify this outgoing-push barrier.
- `Pause::wait` waits for actual connection closure or control release, within its configured pause deadline (`verification.rs:262–284`); the fixture config permits120000ms (`control.rs:99`). `established` then drops/joins the incoming task and closes the binding (`network2.rs:679–699`). Lease status itself checks `connection.close_reason()` and other gates (`owner.rs:37–61`, `runtime.rs:322–365`).
- The endpoint builder leaves connection idle timeout at the dependency default (`crates/kilogram-transport-iroh/src/ordered2.rs:106–121`). Cargo.lock selects iroh1.1.0/noq-proto1.2.0. Their locally pinned source supplies a **30000ms connection idle default**, with iroh heartbeat5s/path-idle15s overrides. A30-second fixture wait has no margin over that connection timeout or the subsequent task/owner cleanup. Killing one process is not proof the peer has already observed connection closure.

**Most economical explanation: a fixture deadline race against abrupt-peer-loss detection and cleanup.** The negotiated timer, last transport activity, exact connection close reason/time and worker cleanup timing were not captured. BA therefore does not prove an internal transport defect, leaked permit or guaranteed success from a longer wait. Product changes are not justified by this evidence. If a larger bounded observation still fails, the exact missing facts are the old binding's close-event category/time and whether its paused task/permit was dropped; no additional history/profile/key/DB access is needed.

## Minimal proposed correction and regression

1. Change only F's old-held-ACK release observation to a **named, bounded60-second phase**, clipped to the existing original deadline. Keep lease false + generation null + actual mapped-slot window0 as mandatory gates **before** reconnect. Do not clear hold_ack, force-close B, forge lease/window state, use graceful shutdown instead of kill, or bypass the gate. This is a candidate fixture correction, not a claimed runtime fix. Record a closed static stage/reason on failure at this boundary (no arbitrary error/status export); review the exact narrow representation with root before implementation.
2. Preserve the original450/480/510 limits. Before allocating any runtime, establish that60 seconds plus the remaining measured tail fit the original remaining budget and the held pause has not exhausted its120-second lifetime. If not, return the timing plan to root; do not silently enlarge budgets. No new attempt or activation is authorized by BA.
3. Add a focused real-process/real-QUIC regression of **outgoing held ACK, abrupt A kill, B closure detection and reserve32→0, then fresh A admission and receipt replay**. It must reproduce the old race with a controlled closure observation after30 seconds, prove the revised barrier waits for real closure/cleanup, and reject retained lease/reserve until its bounded failure. A unit test of a numeric timeout constant or mocked status booleans alone does not exercise this failure. Build without execution, then use the stable Windows harness; keep EXE paths stable. Root must approve the targeted selector and limits before any runtime; do not replay the historical/full matrix.

Required starting state: actual originals/ready mapping; B role1, C1 slot1 in F12 (resolve mapping, do not hardcode), C1 Active/family2/lease current at[185,35], A debt178 and B debt0, both pending/queued0, C2 L1 at[5,5]. Validate the held outgoing receipt batch from7, reserve32, old binding/generation and actual A killed/waited outcome. B must stay in the **same warm incarnation** while A restarts cold with zero replay; fresh binding/request must differ while saved receipt multiset/digest remain equal. Keep disable_ack ordering, suspend-prefix/live controls and the immutable-history checks. After replay release the hold, require debt0 and unchanged histories; no pass from seeing a new connection alone.

Adjacent assumptions checked: P has the same30-second release wait (`migration_s54_catchup.rs:140–150`) and needs separate review before WholeP. Do not copy F's from7/debt178/frontier[185,35] into P's from385/debt256/frontier[641,640]. Preserve role versus physical slot, C1/C2 shares, outgoing versus query-reply timeout envelopes, and warm B versus cold A. Later F receipts/normal termination/reopen assertions were not reached/proved by this diagnosis.

## Preservation and handoff

The final seal confirms all536 source pins and nine production images, navigation/foreign tracked bytes, four accepted AY/AZ documents, HEAD and empty index unchanged; all32 inspected raw files match their before pins. Supporting sanitized findings, static comparisons, dependency-source facts and the before/after seal are pinned in the companion manifest. AY/AZ documents remain pending combined publication; failure has not been restamped.

**BA raw access is now closed.** Raw pin envelopes are referenced only as documents. Future historical audits must exclude command-log paths before hashing or following any pin graph; old F01–F11/AU/AM16/AD16 and all private origins remain closed. One substantive root callback, then stop for architectural review. No commit/push before acceptance and instruction.
