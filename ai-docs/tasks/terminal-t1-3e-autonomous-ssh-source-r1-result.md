# Autonomous SSH lab: source review r1 corrections

19 November 2026. Addresses all three findings in `terminal-t1-3e-autonomous-ssh-source-review-r1.md`. This is a delta from review-04 (`b9fd94b76c3eecddce107b9661d95858de4df40370d458fc03959be9c295a7f9`), not installation acceptance. No real task registration/execution, firewall mutation, messenger field run, commit or push occurred.

1. **SSH preflight.** The production guard now evaluates local/remote IPv4 addresses, CIDR/netmasks/ranges and port sets/ranges, exact program/service, interface/profile and security constraints. An allow must cover the chosen server address/port, peer and every legal TCP client source port; matching only the current source port cannot establish reconnection. Possibly intersecting inbound sshd or outbound ssh-client blocks reject preparation. Unsupported potentially applicable constraints yield `LAB_SSH_FILTER_UNCERTAIN`. The finite supported subset intentionally refuses constrained interface/security expressions it cannot prove. It does not add exceptions or widen existing rules. Native Audit captures the configured sshd service image/running state; the guard checks it against the fixed system OpenSSH path. A live SSH route is checked separately. Ephemeral source ports are excluded from the immutable policy digest so Audit and Provision can use different sessions. The concrete real-host Audit still requires review before enabling Private; these synthetic policy tests do not prove a working real SSH reconnect.

2. **Transactional begin.** The launch helper publishes its created process reference immediately after Process.Start, before redirect setup can fail. Admission catches failures before creation and before the helper returns. It preserves the primary launch diagnosis and any prior worker result, cleans only processes proven to belong to the native session Job with a bounded cleanup loop, then publishes a failed result. Active state is released only after the Job is proved empty. Otherwise it stays blocked as `launch_ownership_unconfirmed`, including across agent restart. Repeated delivery of the same failed request returns the same failure receipt. A normal retry does not restart the agent.

3. **Recovery versus campaign teardown.** Interrupted/failed installation uses durable phase `recovering`, ending at retryable `rolled_back`; an installed campaign uses `removing`, ending at `removed`. The production file-cleanup function is shared with the focused tests. Recovery verifies the complete package-file and public-baseline set before deleting any of it; it preserves the original initial snapshot, phase history and unrelated run evidence. Corrected-package provisioning can then proceed. Final campaign teardown preserves legitimately updated ordinary files and evidence. Resuming `recovering` retains recovery intent after an interruption.

## Focused verification

| Evidence | Result and boundary |
| --- | --- |
| `.tmp/terminal-t1-3e/autonomous-r1-policy-files-02/checks.json` | 21 grouped checks PASS using the actual SSH guard and production transaction/journal/file-cleanup functions. SSH policy/service facts are synthetic; Task/firewall/network OS boundaries are mocked. Journal/files, drift refusal, recovery and retry are real temporary filesystem operations. |
| `.tmp/terminal-t1-3e/autonomous-r1-startup-01/checks.json` | 6 focused native cases PASS: exception before process creation, successful next attempt; exception after creation before return, successful next attempt; uncertain cleanup blocks admission; block survives restart and the owned Job is empty after test cleanup. Both successful retries use the same live agent/session. Real Windows Jobs and mailbox echo/quit are used; fault throws exist only in the copied launch/cleanup boundary, with exact instrumentation recorded. Limit 240 seconds, ordinary NativeToken, BelowNormal launch. |
| `.tmp/terminal-t1-3e/autonomous-source-review-r1-01/installer-regression/checks.json` | The existing 13 grouped installer checks PASS on the final source. No real task/firewall mutations. |

The earlier passing 20-group policy/files run and 13-group installer run remain in their original directories. No failed r1 runtime attempt was overwritten. The already accepted native-12 lifecycle suite was not repeated: the changed startup path has focused native coverage above, while unrelated lifecycle paths retain their review-04 evidence. PowerShell parsing and Python AST checks passed.

The last focused native process deliberately exits 1 when the test verifies refusal to restart from an unresolved ownership state. Its completion receipt has `ok=true`; this is the expected negative test, not a Kilogram application exit. The preceding failed-launch and successful-retry receipts prove cleanup and the unchanged agent session separately.

## Packages and next gate

Current packages are `.tmp/terminal-t1-3e/autonomous-source-review-r1-01/package-A` and `package-B`. Manifest SHA-256:

- A: `8fbcf855cb673b934300cda4ddf45208ec337d6ae889b38f7d28346f9f0f9265`
- B: `92d63cc05a08d7354b1e35be64dc2ac87d7db169ad5bb207973c69a0965e54f7`

The task/config/rule specification is byte-equivalent to review-04: the task XML hashes and explicit SID/DACL/action/logon parameters are unchanged. Package pin checks bind the changed source bytes to that same specification. Therefore the prior successful TASK_VALIDATE_ONLY receipts on the corresponding A/B hosts are reused by exact XML/spec equality; no new registration/validation calls or expensive unchanged field checks were needed.

`autonomous-source-review-r1-01/source-review.json` indexes the new source/evidence pins and reused baseline receipts. `delta-from-review04.diff` and its manifest distinguish changed and added paths from the accepted baseline. `review.diff` and `publication.allowlist.json` describe the complete proposed coherent publication, including the unchanged inherited supporting hunks. Prior package directories remain preserved and superseded.

Reproduce only the new focused checks with fresh evidence directories:

```powershell
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' -B scripts/test-terminal-t1-3e-lab-check.py .tmp/terminal-t1-3e/<fresh-startup-id> --startup-only
& "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/test-terminal-t1-3e-lab-r1-check.ps1 -Evidence .tmp/terminal-t1-3e/<fresh-policy-files-id>
```

Root owns source/package re-review. Publication and one-time provisioning remain pending explicit acceptance. Actual ordinary SSH task read/run access, task-context DPAPI, real Setup/IPC and two autonomous messenger attempts remain later runtime gates. The old privileged fault suite stays deferred.
