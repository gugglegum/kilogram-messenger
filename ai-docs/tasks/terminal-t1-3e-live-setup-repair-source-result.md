# Live setup repair: semantic verifier and fixed diagnostic export candidate

19 November 2026. Implements [root's bounded source task](terminal-t1-3e-live-setup-repair-task.md) from accepted HEAD `e4cbe4409a3ad4a237691585b8381b440a897066`. Source/package review only; no new UAC, installation, task Run/Delete, message test, profile/rule mutation or access to the known inaccessible original logs. The residual A task and protected initial journals are untouched. Primary A/B errors remain unknown until the reviewed retrospective export is actually run.

## Task semantics

Assert-LabTask now accepts only four omitted defaults witnessed in the captured actual task: least-privilege RunLevel, enabled logon trigger, enabled settings and demand start. Required identity/logon/action fields remain mandatory; duplicate nodes are rejected even for optional defaults. Local account names/SIDs are resolved to the exact configured SID. Principal/action Context remains exactly AgentUser, one principal/logon trigger/Exec and one container of each expected type. Group identity/required privileges/token overrides and conditional/inherited/non-simple DACL ACEs are unsupported; existing exact read-run/full masks and protected three-principal DACL checks remain.

The actual XML/COM/DACL capture is preserved as `scripts/fixtures/terminal-t1-3e-task-A-normalized.json`.29 focused checks accept it and the explicit template, resolve aliases and reject elevated/wrong/missing users, false settings, S4U, duplicates, changed action/context and widened/unprotected/foreign/duplicate DACLs. The exact production Undo task branch was exercised with that capture and a fully mocked Schedule.Service/delete boundary: safe normalized ownership reaches only mock deletion; elevated tamper is rejected first. No actual task operation occurred.

References: [Task Scheduler schema](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-schema), [default low privilege / security contexts](https://learn.microsoft.com/en-us/windows/win32/taskschd/security-contexts-for-running-tasks), [name-or-SID logon identity](https://learn.microsoft.com/en-us/windows/win32/taskschd/logontrigger-userid). These justify the supported normalization; they do not justify defaulting required identity or command fields.

## Future failure diagnostics

The fixed installer labels admission/audit/predicate/projection/mutation/verification/recovery stages. Before rollback replaces the current exception, the initial primary summary is preserved separately in the protected journal. A top-level administrative-child trap writes bounded primary and terminal summaries to fixed main-error/recovery-error artifacts only inside the validated protected package stage. Library/ordinary import does not install that trap. Reporting failure never replaces the original exception.

The existing supervisor publishes selected primary/terminal stage/type/message/HRESULT/error-id, exact exit/timeout/capture-completeness and minimal journal phase/owner/rollback status. Raw protected logs remain original. Public stdout/stderr exports are bounded to64KiB per log: full small file, otherwise32KiB head and32KiB tail with exact offsets/data/hash. Full-source SHA-256 is computed only up to2MiB; larger sources explicitly have no complete full hash. Every chunk has its own hash. Missing, denied, malformed, truncated and incomplete evidence are distinguished. Initial/want/full Audit/credentials are never selected from the journal. Output JSON uses an explicit2MiB bound, and the intended ordinary SID gets file Read permission without Write/Execute.

Exit0 without confirmed stream capture is not success. Capture/startup/cleanup errors no longer masquerade as a confirmed timeout; unknown outcomes remain unknown. Owned recovery remains conditional on the exact current setup-session intent, with no recovery of an older/concurrent lab.

## Small retrospective export

`lab-export.ps1` supports only fixed A/B scopes: A `setup-e4125cddbe3f4dba9d3017aec19af6cd` with original manifest `5e7b721c...`, B `setup-3b96084a7c5a49889a0d9fcb61fb1e1e` with `ce75a4ac...`. No original-session/file-path/command parameter is exposed. The exact old package manifest and admin/System-only source-directory protection are checked before reading. Only main/recovery stdout/stderr and selected journal/error status are exported; original bytes are never rewritten. Missing B recovery/journal is explicit, not success or an invented phase.

The approved bootstrap's byte snapshot, pin checks, protected directories/copy and intended-SID public-file read model are shared by setup and export. A source equivalence check proves that body unchanged apart from a fixed Setup/Export entry selection and bounded structured bootstrap-error metadata. No new service, stored credentials, admin SSH or broad dispatcher. The export package contains only common helpers, diagnostics, shared bootstrap, fixed exporter and existing Job helper:5 payloads plus manifest and one CMD.

One eventual Export-A/B.cmd per host requests one UAC. No timer precedes acceptance. Afterwards one fixed read-only child has a native45s guard,48s parent wait and owned cleanup; the ordinary wrapper waits75s. Bootstrap and child use BelowNormal. Collection failure publishes its real bounded exception when the protected public destination is available. UAC cancellation is recorded by the ordinary launcher. No installer/Apply/task/rule operation is part of export. Its success means log collection only, never installed-lab success.

Concrete local review packages under `.tmp/terminal-t1-3e/live-repair-review-01/`:

| Purpose | A manifest SHA-256 | B manifest SHA-256 |
| --- | --- | --- |
| Fixed export,5 payloads | `fea3edaf484a517e455d009f583f962ce6a75a3a190a3f38126548126a4796c2` | `4a0f6f3ab01fd34cb0abf6427e8a17e774073ca3a04de5a9dd4a9cc8a5c9a3e7` |
| Future corrected setup,13 payloads | `a3eb0ef1dae5092ca38973377c48ac07eb57495a8cd818455618592125849b7f` | `f044bbb66c5d456312719313e2302dcf554f1ca3015a21b6d2a1679608e2c01a` |

Export-A/B.cmd are bound to each ordinary base's `deploy\autonomous-setup-export-review-01`. Packages have not been delivered/activated. Setup packages are review artifacts only: do not rerun Install into A's incomplete state. Root decides owned recovery/reinstall only after the actual primary logs and source/package acceptance, preserving the initial journal.

## Verification and limits

- Task29 PASS, including actual captured metadata and production task Undo with mock COM deletion.
- Installer transaction13 and admission8 PASS after stage instrumentation; no changed policy/locking semantics intended.
- Supervisor9 PASS: real own synthetic children, exact stderr/exit1, actual production error trap with only CommonData mapped to a fixture, pre-intent failure, recovery failure, native timeout,70KB/2MiB+1 output, and incomplete-capture exit0 rejection. Public initial/want sentinel fields never leak.
- Export6 PASS: exact small-log bytes/full hashes, selected status, original-file preservation, absent B recovery, malformed/bounded journal, package scope/pins, shared bootstrap/CMD syntax and large public JSON. NativeToken confirmed the intended SID's explicit Read-only/no-Execute ACE on a disposable file. This is not a claim that a real administrative export/effective ACL has been exercised.
- All four concrete packages reopened and pinned,27 source/generated PowerShell inputs parsed, bootstrap direct-process and CMD lengths checked. Original bootstrap protected-copy/ACL logic equivalence passed. No Rust build/network cargo harness, unchanged long lifecycle suite or repeated inaccessible-log probe.

Failed fixtures remain separate: task01..03 exposed a PowerShell test-array construction issue before04/05 PASS; supervisor01 hit the aggregate fixture's128KiB writer bound before an explicit2MiB test receipt,02/03 PASS preceded final09-case04; export02 did not observe its test SID ACE in sandbox, NativeToken03 and final export checks passed. These were not product/privileged retries. Old actual attempts and package04 remain preserved.

The review index/diff/allowlist accompany this note. Candidate is uncommitted, index empty. All unrelated dirty work/root coordination is preserved. Root's next action is source/package review, then a separately dispatched fixed export; no user launch request is made now. Executor stops after callback.
