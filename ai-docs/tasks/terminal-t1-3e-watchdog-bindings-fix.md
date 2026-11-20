# Exact watchdog bindings after sender-crash field01 preparation failure

2026-11-20, parent `5714080abbcfae199beab3907417bc97eb624737`. Review submission only; no publication, installed cleanup, agent stop/Wake/Restore or new physical fault performed.

## Change

The failed field01 preparation and incomplete A original6 cleanup are documented in [the retained attempt note](terminal-t1-3e-sender-crash-field-attempt-01.md). This is a harness deployment/restore defect; product durability remains **NOT EXERCISED**. All failed field01 evidence and staging directories are preserved.

`test-terminal-t1-3e-normal.py` no longer hardcodes the historical watchdog at any of its three generator sites. The local SFTP wrapper takes the exact repo watchdog SHA256 from validated `plan.sources`. Both receiver byte-upload and SFTP metadata prepare/verify take the exact **currently installed** receiver watchdog SHA256 from passive inspection of that host. Incoming payload hashes are independent. Both real execute callsites use the same narrow selector after inspection, including finally cleanup.

The selector requires read-only inspection, idle/unblocked/healthy readiness and a complete known six-file cohort, then selects one exact installed pin. It covers original→candidate, candidate→original and already-original cleanup. Mixed/foreign bins and dirty inspection refuse. There is no runtime alternate-hash list or disabled guard. Native Jobs, budgets, byte/content checks, ordinary agent update and cleanup gates are unchanged.

The sender-crash entry changes only its fixed attempt/evidence binding to `hint-crash-02` / `sender-crash-live-02`. All functional predicates and limits remain unchanged. The previous plan/evidence/source snapshots remain historical, never rewritten to new pins.

## Focused evidence

Evidence root: `.tmp/terminal-t1-3e/watchdog-fix-prep-01`.

- `regression-02/checks.json`: **3 tests PASS**, actual native PS5, ordinary token, generated production snippets and real existing watchdog/Job/file IO. Unmodified published generator bodies are AST-extracted from parent5714080a, retained with their source hash. The old local wrapper refuses current repo source before child creation; corrected wrapper launches the harmless child and proves empty owned Job. Old receiver/metadata guards refuse the changed installed guard; corrected guards pass. Wrong source/destination/stale pins refuse before child/payload/intent; verify refuses and preserves every staged byte.
- The receiver fixture uses historical original watchdog for initial deployment and a comment-only candidate watchdog fixture for restoration, so repo-source, current-installed and destination hashes are all different. This is synthetic disposable IO, not a live SFTP or installed-agent test. Existing native receiver tests additionally use the actual current repo guard bytes.
- `regression-02/actual-host-command-lengths.json`: actual A/B initial/restore/already-original manifests fit the unchanged `<7400` encoded-command cap; metadata maximum7332.
- `existing-01/checks.json`: **8 selected existing checks PASS**, including SFTP argv/dispatch/error receipts, native client success/nonzero/deadline Jobs, exact metadata/content checks, exact stdin byte budget/short/hash failure/deadline, both execute callsites, functional-failure restoration, foreign refusal and already-original cleanup. Synthetic lifecycle/dispatch models are explicitly distinguished from actual native IO.
- `regression-01`: retained test-setup failure after successful local-wrapper regression; an overlong disposable receiver path hit the existing encoded-command cap. Only the fixture path was shortened; no production bound was changed. `runs.json` retains that failure.
- `cleanup-validate-01` and `cleanup-validate-02`: actual installed **read-only** preflight PASS. The second exercises the concrete command wrapper with real guard180/200, exit0 and empty owned Job. It verifies A PID22656/start134396630314254680/session1f573d11b9874dcdaed9ab7ee4dc490f, healthy idle candidate6, same-session singleton agent Job and canonical15 object IDs/full owner/group/DACL SDDL. No update/Task.Run/DPAPI/Restore was invoked.

No Rust build, network messenger runtime, general suite, B update or new field attempt was run. The new watchdog regression file is a review/check artifact; the existing runtime `SOURCES` closure remains complete and carries updated hashes for its changed normal/check/sender-crash files.

## Concrete cleanup and later packets

`packet-01/cleanup-plan.json`, `cleanup-a.py`, `run-cleanup.ps1` and `packet-01/commands.txt` form the bounded A-only packet. Default invocation is read-only. The fresh stage and evidence are pinned; the explicit run branch requires exact reviewed acceptance, new accepted publication commit, current raw source pins and matching published blobs. It uploads verified local original6 through the corrected existing uploader, then invokes existing `Agent.update(originals)`. It checks candidate6/readiness/identity/canonical15/singleton Job before and original6 with the same preservation gates after. B is untouched. Upload/update outcome uncertainty blocks further mutations; receipts and stages are retained, with no force or blind reapply.

The known original6 asset bundle and prior A backup originals remain available. No sole backup or prior failed stage is moved/deleted. A remains healthy idle candidate6 pending review.

`packet-01/restore-acceptance.template.json` is authorized=false with commit=null. After this block's exact accepted publication, bind it to that **new** verified commit; unchanged metadata-safe Restore plan remains `b100c66939b65ae0d1dd5aaca4b1bf2a9396db14ae031f0db29fa188290b2d0f`, operation `restore-a-02`. Runner `db3cd102...` and helper `190eb4c4...` are unchanged. Original6 cleanup must conclusively pass first. Do not use historical File.Replace Restore.

`packet-01/field02-plan.template.json` preserves the functional contract and all limits. Only changed source hashes, attempt/evidence bindings and explicitly unknown A identity/token-observation fields differ. It is intentionally **not executable** and acceptance remains false/unbound. Post-Restore A identity is unknown. Furthermore, restoring old15 returns an original agent without the loaded crash policy: the unchanged field predicate still requires separately reviewed policy-capable agent activation/current readiness and fresh actual token proof before filling A's field identity. This packet does not silently authorize another refresh or relax that gate. Root must review actual identity/readiness and fresh B availability before any field02 activation.

Review artifacts include an exact scoped source/doc allowlist with raw/blob snapshots and a recursive evidence inventory. Foreign worktree changes are excluded. Commit/push and installed operations await explicit orchestrator acceptance; after one substantive callback, idle without polling.
