# T1.3e combined recovery/setup: source and package review

19 November 2026. Implements `terminal-t1-3e-recovery-setup-source-task.md` against published baseline `d6e06fc20dd22e0ca320cdc8d70dc444a36dac57`. Review only: no deployment, UAC, live task/rule/profile/journal mutation, source commit/push or message test. The previous actual installation failures remain the last live result.

The confirmed causes are unchanged: A main and recovery `LAB_TASK_SCHEMA`, B pre-intent `LAB_SSH_FILTER_UNCERTAIN`. The published semantic task verifier is reused without changing its contract.

## Implemented behavior

`lab-server.ps1` obtains the exact running sshd service PID/account/image and observed TCP22 listener/established peer-flow owners. Supported listener ownership is the service PID; differing established-flow owners are included, up to16 unique processes. Native process handles retain creation time, image and actual process token together. The service actor must be System; supported flow actors must be System or the configured ordinary account without elevation. AppContainer, dead/changed owners, wrong image/account/role and inaccessible tokens abort. All retained relevant tokens must determinately not match a possibly applicable LocalUser Block. Any match blocks, otherwise any unknown aborts. No name exception, SID-inequality shortcut, collector-token substitute, privilege adjustment or foreign policy write exists. Existing native AccessCheck descriptor and group/restricting semantics are unchanged.

The in-lock authoritative snapshot records relevant Block CIM views, listener/flow bindings and actual token metadata in protected `ssh-policy-proof.json`. The public result includes its bounded bytes/hash/completeness, alongside separately named old-recovery, main and fallback logs/error summaries. As with existing logs, over64KiB is explicitly head/tail truncated; full hash is available only up to2MiB. No full Audit/initial policy or credentials are exported.

`RecoverInstall` uses the existing protected bootstrap and separate finite children under the existing native job supervisor. A first invokes `RecoverOld`: exact old owner `setup-e4125cddbe3f4dba9d3017aec19af6cd`, recovering/rollback_incomplete phase, unchanged desired specification, original package04 manifest `5e7b721c...`, exact original11 file pins, initial snapshot and history. Admission checks task semantics, owned rules/files and unrelated policy before mutation. Cleanup retains old pins, uses the corrected task verifier and appends journal history. It produces a protected proof only after rolled_back, owned task/rules/files/agent absence and immutable initial/history checks. Missing/failed/timed-out/incompletely captured old recovery prevents Setup. `SetupRecoveredA` repeats completion checks under the fresh install lock before any new pin write. Only the existing installing/zero-intent transition can replace old pins with the new15-file package. B requires absent journal both in supervisor admission and again inside the installer lock; it has no old cleanup branch.

Each host has one combined UAC entry, with ordinary follow-up reading the final SID-Read-only result. No active deadline starts before UAC acceptance. Old recovery180s + main450s + owned fallback180s =810s. Wrapper1080s leaves270s for bootstrap, three bounded process/capture/cleanup tails and reporting; per-child tails allow3s exit wait, up to6s EOF capture,2s cleanup and2s guard drain. A SSH actor lasts1200s, longer than the wrapper by120s. Fallback still requires the new invocation's exact journal owner and confirmed child cleanup.

## Evidence and limits

Eleven focused checks passed,141 counted cases/bundles: server15, real ordinary native token12, legacy recovery14, combined routing12, existing admission8, native child supervisor9, captured task29, installer13, fixed diagnostics/export6, existing guards/recovery21 and package static checks2. Windows PowerShell5.1 was used. Native token and disposable public ACL checks passed with ordinary NativeToken; no UAC or actual server token was acquired. New routing tests execute production functions/installer blocks with privileged OS observations/mutations mocked. Legacy cleanup and journal/history writes use real disposable files and exact original11 package bytes. The task fixture is the previously captured actual Task Scheduler XML/DACL.

Failed attempts remain in `recovery-setup-source-check-01/02/03`: two fixture corrections (empty enumeration Count under StrictMode; expected TASK_SCHEMA versus actual TASK_DRIFT for changed privilege) and restricted-sandbox token/ACL differences. Fresh corrected attempts passed. No failure directory was repaired in place. The first index assembly expected an installer-check directory although that check writes a single evidence file; that failed assembly is preserved in review-01. Final check paths and all pins are indexed under `.tmp/terminal-t1-3e/recovery-setup-review-02/`.

Actual B administrative SCM/socket/token access, System-token applicability and authoritative live CIM rule results remain future installation gates. Fixtures and ordinary token tests do not prove those capabilities. No live installed-lab success is claimed.

## Concrete packages for root review

Both packages are local, not deployed, under `.tmp/terminal-t1-3e/recovery-setup-package-01/package-A` and `package-B`. Each contains15 pinned payloads, manifest and two commands: `RecoverInstall-A/B.cmd`, `Teardown-A/B.cmd`. Future fixed ordinary deployment suffix is `deploy/autonomous-recovery-setup-review-01`; old package04/deployment/evidence are preserved.

| Host | Manifest SHA-256 | Combined CMD maximum line | Combined bootstrap encoded characters |
| --- | --- | --- | --- |
| A | `fd61b762f912a550accc0e2c2bd741ef1d71fc5dfdcf681620bd092ea075ea48` | 1586 | 14132 |
| B | `8a15960ecc53d631b45cf38e22e792d465aab5c9394714004418957e6aa9fa12` | 1498 | 14040 |

All payloads match current review source; exact fixed command targets, launch/source+manifest pins, completed bootstrap template, PowerShell parsing, CMD8191 and direct-UAC32767 limits were checked without executing commands/bootstrap. Review allowlist contains only the six modified installer/native/setup/launch/diagnostics/bootstrap sources, two new server/recovery helpers, five focused checks and this result note. Unrelated worktree changes remain outside it.

Executor submits the coherent diff, package bytes and evidence index to root and stops. Root owns source/package acceptance before any changed deployment or administrative execution. No commit/push was made for this candidate.
