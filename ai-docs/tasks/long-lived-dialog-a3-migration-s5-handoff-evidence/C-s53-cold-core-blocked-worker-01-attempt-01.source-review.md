# C2-Q dormant source: blocked worker

2026-10-31. **Source/static only; STOP for focused review.** Baseline389ff0b863980733dc10bc21cdf43be43ca3bb72. Implements the [single-case contract](C-s53-cold-core-blocked-worker-01-attempt-01.contract.md) under [root acceptance](C-s53-cold-core-blocked-worker-01-attempt-01.contract-accepted.md). No execution/allocation is implied.

## Own delta / raw pins

New [worker](C-s53-cold-core-blocked-worker-01-attempt-01.preflight.ps1), [supervisor](C-s53-cold-core-blocked-worker-01-attempt-01.watchdog.ps1), this note and minimal [Core](../../../scripts/migration-s53-cold-preservation.ps1) selector/root edits. Contracts, old evidence/navigation/helpers and unrelated worktree edits are preserved.

|Source|Raw bytes|SHA256|
|---|---:|---|
|Core|80601|74f53161ddfb4862efb20249e7a6eeac11d9b1c71665a2755fff1fe6b907703d|
|Worker|69272|1ff5ff68b427cfdcb58f0ff6d628681ca60a0551bffbb13daad05385cd5938ca|
|Supervisor|37598|79ff37841d787945cf35c019ffe34db9a44a932800f0e506c2da0af9ff11a64b|

Core +6/-2 only appends deadline-blocked-scan to the C#/PowerShell lists and binds its fixed family/case. Exact inverse equals actual HEAD80325 characters byte-for-byte. Production, inventory-boundary tuple/preconditions, limits, reader/comparer/output algorithms and all five checked sites are unchanged. [Deadline helper](../../../scripts/migration-s5-l1-deadline.ps1) remains6038B/0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45.

The current supervisor's entire start-clock/runspace-guardian setup through BeginInvoke is retained verbatim, including child cutoff27/self29 behavior. New primary-cutoff evidence does not replace those protections. Existing pipe drain/pump bodies and ordinary fixture primitives are reused with the fresh C2-Q prefix/type/closed case.

## Durable witness and real pending wait

One fixed healthy blocked-scan case uses3586B inventory/hash0567f3bb1b6057021c8746377d4ba54e25d1ddca41b29fc9fae05464d1e342c0, five rows52B per mirror, six empty locks and safe-empty-S. Existing two premise traversals verify shape/content/ACL/path/ID/strict ADS;39 file owners/30 enumerators close,15 case directory guards close before factory.

The passive scan_before callback follows capture_s/capture_bkp and ten expected-file events. It requires all32 Core input owners live, peak33, expected set captured/captureQPC>0, completed/observed-set/input-release false, historical H and no observed hash digests/passes. The actual38-field last snapshot is saved; it is never turned into a final snapshot.

Witness is a50-field object with exact identity (worker PID/startUTC/fixed ProcessPath and supervisor PID), allocation/all six pins, shared QPC/frequency, fresh four types/three initializer calls/empty diagnostics, setup/premise/guard evidence and precise events/last Core observation. Worker job Children must be empty and WorkExpired false. The last controller ledger is20returned/15closed/5remaining/current0; five family guards and eight readonly public input streams remain owned separately from Core. Writer protocol is a declared string, not a pre-written claim of completed closure.

A CREATE_NEW witness writer serializes at most65536B, Flush(true), Dispose, then checks its original SafeFileHandle.IsClosed. Only after known closure and another work Check does the callback enter SignalAndWait(ready,block). No source path signals block; unexpected return/exception is an unsuccessful worker. Parent waits only by nonblocking ready polls plus its existing20ms child-exit polls.

Parent creates exactly two fresh fixed manual-reset events; worker only opens the same two. Each current owner box is registered before constructor/open/getter/validation. CreatedNew must be true in parent; collision closes the acquired reference without reset/signal/ACL mutation. Partial construction or failed/uncertain closure is recorded unknown, without close retry. Successful event facts have seven fields plus two five-field role records: parent before2open/after2closed, worker witness2open/created_new=null. Names are fixed; this is not an arbitrary IPC interface.

Only the worker launch adds -Mta; witness must report actual MTA. As the accepted contract explains, ready means entry to the controlled pending call, not atomic scheduling/stack evidence. Parent additionally requires the completed witness and the same retained child still alive at cutoff. Missing response, partial file or ready alone never passes.

## External cutoff and cleanup reserve

Before20, parent validates ready/witness exactly once, matches PID/startUTC to its retained Process object/positive native handle and verifies witness plus accepted QPC before work cutoff. Worker setup does not spawn a process after its empty-job-children witness.

Parent Check at Age20 throws the exact marked work_expired cause. Main catch preserves that primary cause/cutoff timestamp independently from cleanup errors. Only a previously accepted witness/ready plus still-matching live child authorizes the recorded primary Kill attempt/return. Actual nonzero child exit/reap must be observed before26.5, EOF before27.5; arbitrary nonzero/early exit/fallback/unknown identity cannot satisfy the oracle. Existing bounded cleanup may issue further Kill calls on that same retained process; it cannot substitute for missing primary kill evidence and is reported separately.

**Reserve fix:** the shared Check uses work20 until cleanupReserve is explicitly set in parent finally; then it uses28 and skips worker-only CheckWork. Thus post-cutoff public pin recheck, closure and watchdog writing do not immediately rethrow the already expired work Check. No deadline is restarted and the independent guardian remains armed. Parent closes events/public streams/pipes and observes eight public inputs before closure; unknown cleanup fails. Report/disarm checks and a final check after guardian completion reject late forced/unknown/age>=28 states.

Expected watchdog is a63-field record: observed_expected_forced_timeout, primary_reason work_expired / primary_mechanism supervisor_work_cutoff, forced=true, guardian_forced=false, primary kill evidence, actual nonzero child exit/reap/EOF, known parent cleanup and owned registered processes0. Ready/accepted/cutoff/kill-before/kill-after/exit/EOF timestamps are separately saved. Defaults are false/null/0 until actually observed; parent result starts1 and becomes0 only after the complete negative oracle. Partial artifacts are retained on failure. Hard fallback firing and cooperative worker timeout are not claimed by this case.

No worker final Core observation or CloseHandle count is invented: both null, finally completion not observed, application cleanup not_observed_forced_exit when forcing occurred. Last Core remaining32/inputreleasedfalse stays historical. OS handle-release inference is conditional on observed process exit; it is not per-handle cleanup measurement. Descendant absence uses the pre-block job Children0 plus no subsequent spawn path, retained child exit/reap/EOF; process termination alone is not claimed to kill descendants.

## Static checks / prospective boundary

PowerShell Parser.ParseFile: Core/W/S zero errors;3/51/30 functions, no duplicate names. All28 shared W/S functions match exactly, including the witness/event/Core oracles and reserve-aware Check. Actual guardian setup matches the accepted parent literally. Fresh C# premise helper delimiters balance as text; no compilation. The exact Core inverse and default git diff --check passed.

Independent in-memory healthy-literal/path model:3586B/hash above,36interned paths/7024B; base12073840+32*384+5*160=12086928;32 retained/peak33 at pre-scan. Inventory accounting3586, fixed scratch524288. Fixture geometry16dirs/13files/3690logical bytes/57strict stream observations. Named events, job handles/threads, public input streams and parent process/pipe handles are separate resources, not silently included in Core's input counter.

Native report names are started.json/witness.json/watchdog.json, at most65536B each/196608 total; source131072 each/pipes4096 characters. Worker has no normal success return for this scenario: any returned execution emits a bounded unexpected-return diagnostic and exits1. The expected worker is forcibly terminated; parent native0 is conditional on the complete timeout oracle and final reserve checks.

Prospective one work20/whole30/lane1/retry0 remains inactive. No fixture/event/native/project-function execution, dot-source/Add-Type/compile, .tmp/private access, allocation/budget/ledger edit, commit/push or S5.4. Required native behavior/exception shape/timing still needs source acceptance and a separate single-use allocation. **Callback and STOP.**
