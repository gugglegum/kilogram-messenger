# A-only executable agent refresh and restore preparation

2026-11-20. Source review packet, parent `3d2fc09b9067308680cf090673fdee5511f9e8a0`. Installed activation is not authorized by this preparation.

The previously accepted crash harness exact11 was committed and pushed as `3d2fc09b9067308680cf090673fdee5511f9e8a0`, parent `b52bda60412610f86e77936ceb618d682d105b81`. Publication proof: `.tmp/terminal-t1-3e/sender-crash-publication-01/publication-final.json`, SHA256 `9b45b2c7e92594ca839cdf9c9778a9dc6b7038b592d0448dac2bc0669354b660`. Live origin/master was verified at the same commit during this preparation. Accepted11 source remains unchanged.

## Executable scope

`scripts/test-terminal-t1-3e-refresh-a.ps1` defaults to read-only Validate. Explicit Apply/Restore requires the reviewed plan/hash, fresh fixed operation, and an authorized acceptance bound to runner hash, phase, operation and current accepted commit. The packet templates have `authorized=false` and require the accepted runner commit; they do not permit activation.

Only the concrete old15/new15 A payload pair is accepted. Manifest SHA256: old `fc3bf5b0713877c07b335f048922b2c9d52b75ba816a0509f41774c55facbead`, new `5a2784caff98850ddffe26e1a49e837e49da634abe7f098dff8b1729c7eda764`. Only agent/common are replaced; other13, lab.json, task, ACL and campaign baseline are not written. B is not contacted. Existing product images are reused; no Rust build or physical crash run occurred.

The runner validates source pins, all15 payload files, config, existing TaskSpec/security descriptor, campaign baseline and the exact known idle process/session. Both native WindowsIdentity groups and raw token groups/attributes, process start/image/session and the named Job containing only the agent must match. The starting identity is from earlier observations (PID44024/start134395772085441302/session261956141a2e4d5ca0aa8382f882bad2); its current availability is deliberately unclaimed. Drift fails before stop or replacement.

Stop uses existing Stop-LabIdleAgent and preserves new stop request/response bodies in the journal, even when stop fails. A retained Process handle must confirm exit. Existing exclusive agent/images locks protect the transaction; finally releases the first lock when acquisition of the second fails. The caller never kills an installed process, deletes locks or edits control state.

Only two paths are staged/backed up and File.Replace is used. Attempted calls are recorded before invocation, including an exception after replacement. Rollback restores only known outputs, uses the actual captured prior bytes, and preserves foreign drift/backups. All15 must match before Wake. An unchanged payload can be reawakened after a stage failure only when stop and both locks were conclusively obtained.

Wake calls the unchanged existing task through the installed agent action. Success requires a genuinely new process/session, native/raw ordinary token and idle Job proof, then the existing bounded check with real interactive DPAPI/logon2/session/SID/baseline binding. Candidate loaded policy must be `owned-runtime-crash-v1` / `b9b280677ed1cb0d612223bfab7ecad306ec976ae5b042a76b77db581aa9da65`. Old restore uses its normal readiness contract and does not require new crash-policy fields. Raw task XML/SDDL, readiness intent/response and final identity are preserved. Work/outer watchdogs are180/200 seconds.

After an unsuccessful Wake, rollback can stop only a conclusively identified idle actor. Unknown actor, blocked state, inconclusive Job, foreign bytes or failed rollback are concrete blockers: evidence is preserved and no force/UAC/task registration/firewall/ACL fallback is attempted. A timeout of the controller can leave a partial transaction requiring reviewed recovery; the runner does not claim powerloss recovery or unconditional automatic rollback.

## Focused checks and limits

`scripts/test-terminal-t1-3e-refresh-a-check.ps1` final run: `.tmp/terminal-t1-3e/agent-refresh-prep-01/focused-04`, exit0,18 scenario groups, native PS5, watchdog60/80 seconds. Actual File.Replace/FileStream locks and existing Stop-LabIdleAgent against disposable directories with no state/agent were exercised. Actor/task/Wake/readiness/DPAPI IO is mocked, including the production Wake/check glue. These checks do not prove Task.Run permission, actual installed token/DPAPI or a successful installed refresh.

Coverage includes Apply, reverse old restore without new fields, failure between replacements, exception after an actual replacement, second-lock failure/finally release, wrong initial hashes, foreign edits before/after replacement and captured by File.Replace, known versus unknown failed-Wake rollback, pre-replacement stage failure, loaded-policy/identity/readiness negatives and fixed source/payload/side/budget bindings. Final source pins are in focused-04/source-pins.json. Runner SHA256 `c0363f127171f74fde5653ff835d3cd5bc4b2701018b482e04dab37d829e61a9`; check SHA256 `63bd90149fbcbbcadb93c3284628c0fdf9d22735fe248f096b26fa959fb03a24`.

Earlier focused-01/02/03 outputs and source snapshots remain intact. All passed their then-current checks; focused-01 reported incorrect case names because its fixture loop reused the name variable. That reporting defect was fixed before focused-02. Two source-only wrapper command mistakes (a foreach pipeline and a variable colliding with a dot-sourced typed parameter) were corrected without changing installed state; they were not test-suite failures. Unchanged crash/native/policy suites were not repeated.

## Activation packet for the next review

`.tmp/terminal-t1-3e/agent-refresh-prep-01/packet/activation-packet.json` indexes the concrete packet. `apply-plan.json` SHA256 is `e8ba6c908e67ae087a56ada238b62e4a31e12fbbce8d4e2a948b518d2adab18e`; source/payload-only binding check passed without installed IO. `commands.txt` gives exact PS5 validation/apply commands, and reverse-plan materialization from the successful Apply result's after.ready identity. The restore template is intentionally nonexecutable until that future identity is available and reviewed. Both operations use fresh evidence directories under `.tmp/terminal-t1-3e/agent-refresh`.

Next permission boundary: architect review of runner/check/note and concrete packet, then publication and explicit installed activation instructions. Task.Run, installed A/B stop/update, actual DPAPI probe and field crash remain unexecuted. Field template remains nonexecutable until future refreshed A readiness is proved. Return this packet and idle; no polling or neighboring stage.
