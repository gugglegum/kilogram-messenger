# Combined B attempt: COM comparison failure, rollback and diagnostic correction

19 November 2026. Executor ordinary readback after the user reported failed RecoverInstall-B and did not start A. Source HEAD remains `94d2d31c556f355cd695f0ebeb040980e7b37d4a`; delivered B manifest remains `8a15960ecc53d631b45cf38e22e792d465aab5c9394714004418957e6aa9fa12`. No UAC/setup/export/teardown retry, A recovery, policy/task changes, deployment, commit/push or message tests were performed by this diagnostic turn.

## Actual invocation and cause

Exact B invocation: `setup-6f5c3fe31f11475290ba9c1b58d305c9`, ordinary session created10:26:10.2574993UTC. Launcher-error10:29:11.8202720UTC reports `LAB_SETUP_FAILED_SEE_RECEIPTS`. The final supervisor result has action RecoverInstall, success=false; main exit1, timeout=false, capture_complete=true; owned fallback exit0, timeout=false, capture_complete=true. No old-recovery child ran on B.

Public result bytes SHA256 `abd1eb4bd49c7aefe3e9e5d8b4b39628de71f8f7640f8dc2be1158c779318109` were checked against saved raw base64/byte count/parsed content. All four available logs and SSH proof are complete, not truncated; every chunk offset/size/hash and full hash was verified. Main stderr556 bytes reads `LAB_INSTALL_FAILED:LAB_COM_SPEC_DIFFERS;rollback=rolled_back`. Fallback stdout35 bytes reports rolled_back, stderr0. Main stdout0. Missing old-recovery logs are expected for B.

Primary stage is **verify-installed**, error **LAB_COM_SPEC_DIFFERS**. The public minimal journal belongs to this exact new invocation and is **rolled_back**, rollback_errors empty/count0. This was an installation that wrote intent and applied the fixed transaction, then rolled back, not the earlier pre-intent SSH refusal. The exact failing predicate compares the ordinary COM view of a fixed owned rule against its desired specification. The accepted code did not capture the mismatching field or the owned COM views before removing the rules. Those historical values are unavailable through the permitted public diagnostics; an exact field-level cause cannot honestly be claimed.

Actual retained server-token access did succeed in this attempt: servicePID7980, started134393174573869921, system OpenSSH sshd image, LocalSystem account, actual token user S-1-5-18, elevated and alive. Both IPv4 and IPv6 listeners belong to7980. The then-observed peer-flow list was empty. Authoritative Block rule `{A83E1DFD-D8A5-465E-A190-87A10AB15599}` has the previously observed LocalUser descriptor for SID1004; native verdict was determinate nonmatch -1 for the actual System actor. This proves the supported listener/System-token preflight for that observation, not future differing flow owners or final installed-state/task/DPAPI acceptance.

## Rollback, current state and notification evidence

At10:35:04.2180314UTC, ordinary COM/registry/task/filesystem observations independently found:

- Private disabled; Domain and Public enabled. No owned COM or persistent-registry rules.
- Exact managed B task absent, with specific HRESULT80070002 rather than a swallowed access failure.
- No agent state/ready, baseline or selected owned payloads. No Kilogram processes.
- All four original bin EXE/script hashes remain exact.

Main and fallback final receipts prove both bounded children exited with captured EOF. The wrapper also wrote its terminal error and the public supervisor result is final. An ordinary query cannot uniquely establish whether every remaining PowerShell process is unrelated: command-line WMI access was denied. No other PowerShell process was terminated, and no administrative read or second run was used to resolve that limit. There is no positive evidence of an active installation worker.

The primary verify-installed predicate is reached only after Set-NetFirewallProfile returned and Assert-LabInstalled verified the enabled Private profile/task/CIM rules. Strict COM profile comparison also precedes the failed rule comparison. The reverse private step completed without rollback error, and current Private is Off, consistent with restoration to its previously observed Off state. Thus a profile enable/rollback cycle occurred in this invocation; it is not a pre-intent refusal.

The narrow Firewall operational query10:25:10–10:30:12UTC returned `NoMatchingEventsFound`, not a confirmed event sequence. Reading channel metadata separately failed with `LogInfoUnavailable`/unable to retrieve channel properties. No interval events or notification records establish the timing/count/source of the two Windows notifications. Their association with the profile returning Off is plausible, but not proved per notification. Neither system-wide firewall disablement nor an external change is inferred from the notification alone. No notification/security settings were changed.

## Focused reproduction and candidate source correction

Unregistered COM FWRule objects round-tripped the exact two A rules and, separately via ordinary B SSH, the exact two B rules without differences. They were never registered in FwPolicy2.Rules. This does not reproduce persisted CIM-created rule views from the failed attempt. An initial local A+B in-memory fixture failed on B's interface not existing on A; the corrected host-local probes succeeded. The [official Interfaces contract](https://learn.microsoft.com/en-us/windows/win32/api/netfw/nf-netfw-inetfwrule-get_interfaces) uses friendly interface names; there is no evidence for replacing the strict interface comparison with a GUID/name or case bypass.

Prepared, uncommitted review candidate:

1. Preserve own-rule desired, ordinary and both CIM-store views plus explicit differing keys/values on LAB_COM_SPEC_DIFFERS. Fixed protected package path only, bounded2MiB; report failure cannot replace the primary error. No full Audit/foreign policy/credentials are exported.
2. Include `com-rule-comparison.json` using the existing ordinary-readable bounded diagnostics/chunk/full-hash mechanism.
3. During fixed Setup, perform the strict owned-rule ordinary/CIM comparison after creating owned rules but **before** changing Private. An incompatibility still rolls back the same finite intents; it now refuses without an unnecessary profile enable/restore cycle. Final post-profile verification remains. No guard normalization or applicability exception was added.

Focused checks: new comparison14, existing admission8, diagnostics/export6 PASS, Windows PowerShell5.1. Only disposable diagnostic files and mocked privileged observations/setter were used; public ACL fixture used ordinary NativeToken. New tests cover six exact differing fields, missing fields, report bounds/fixed scope, primary-error preservation, actual production Apply/private routing (mismatch reaches zero setters; valid projection reaches exactly one), and exact public diagnostic bytes. Earlier failed fixture attempts remain separate.

This corrects the diagnostic gap and ordering, not an established persisted field normalization bug. It does not establish that a retry will install successfully. No changed package was deployed or executed. Existing B `SetupFreshB` correctly refuses the newly present journal even though rolled_back. A future retry requires root review of this candidate and a separately concrete exact-owner/terminal-state admission for the evidenced B owner; do not delete/reset the baseline or bypass the absent-intent guard. A remains unstarted. No further privileged probe is proposed outside that review gate.

Evidence: `.tmp/terminal-t1-3e/recovery-setup-B-readback-01` (public receipt/reconstructed logs/SSH proof),02 (current state/operational query),03 (in-memory COM and access-limit metadata). Tests: `.tmp/terminal-t1-3e/recovery-setup-B-com-fix-check-03` (admission/export),04 (final comparison test with explicit before/after input hash). The final review index records exact hashes and the five-path candidate allowlist. Executor sends one substantive callback and stops.
