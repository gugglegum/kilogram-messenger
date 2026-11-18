# Autonomous lab implementation: targeted corrections before acceptance

2026-11-19. Reviewed source-review `b9fd94b76c3eecddce107b9661d95858de4df40370d458fc03959be9c295a7f9` and diff `546fe10bac06fdeb2c4e20db00b22859955e7a8d188af7d6d6af0ad4e4305d94`. Independently verified 302 indexed pin entries (296 unique files), including sources, dependencies and evidence. Read agent/controller/common/Job implementation, installer and focused checks, and both inherited supporting diffs. Native lifecycle results are accepted as local synthetic evidence, not installed-task/SSH/DPAPI proof. Architecture remains accepted; installation and publication await these concrete corrections.

## 1. SSH preflight can accept rules that do not preserve SSH

`Assert-LabNetwork` checks only selected properties of an allow rule. A rule for sshd/TCP/22 with the correct peer but LocalAddress=192.0.2.99 and RemotePort=1 passes. It does not allow the intended SSH connection. A TCP inbound block LocalPort=1-65535 also passes because the block predicate recognizes only literal 22 or Any. Root reproduced both with the actual function in Windows PowerShell 5.1 on synthetic snapshots, without host mutations.

Correct the effective SSH-path test conservatively for relevant address/port sets and ranges, program/service, interface/profile and security constraints. Do not claim an allow proves access when ignored filters can exclude it; unsupported potentially applicable filters must produce an explicit uncertainty result. Check both current connection and reconnection with a new client source port. Keep the installer scope finite rather than attempting a general firewall engine. Add focused false-allow and range-block regressions using actual guard code. On the real hosts inspect the concrete pinned Audit before enabling Private; preserve VPN/Public/SSH and do not auto-add broad SSH exceptions.

## 2. Failed worker launch strands active state

In agent `begin`, state.active and attempt.json are written before `Start-LabProcess`. If creation throws before `$child` is assigned, the outer request catch only emits LAB_INTERNAL. The completion branch runs only when `$child` exists, so this attempt never becomes terminal and future begin/update/stop are refused. Writing cancel/end cannot retire an absent worker. This is a source-level finding; existing native startup guard coverage concerns an already created process.

Make begin admission transactional. Record the primary launch failure and prove absence/cleanup of any partially created owned process before releasing active state. Keep ambiguous ownership blocked. Test an actual injected launch exception before returning a process and successful next attempt in the same agent, with no manual restart. Cover a later partial-start failure as needed without weakening Job containment or bounded cleanup.

## 3. Recovery teardown loses failed-install intent

`Invoke-LabTransaction(Teardown)` changes journal.phase to removing before Undo. Production Undo(files) removes installation copies only when phase is installing or rollback_incomplete; it therefore preserves files when recovering an interrupted/failed installation. The transaction then marks removed, while Provision accepts only rolled_back for a retry. A corrected package cannot resume the failed installation without ad hoc file/journal cleanup. The fake backend tests undo files unconditionally and do not exercise this production distinction.

Preserve whether cleanup is recovery of a failed install or final teardown of an installed campaign. Recovery must remove only verified files created by that failed install, preserve initial baseline/history and leave a retryable state. Final campaign teardown should still preserve user-updated ordinary files and evidence. Test interruption during installing, incomplete rollback followed by recovery, and subsequent successful corrected-package provisioning using real temporary journal/files and the production file cleanup path; mock only the OS task/firewall boundary.

## Next executor action

Fix these three together with autonomous focused reruns. Preserve prior evidence and return an exact delta from review-04, regenerated package/allowlist pins and clear test boundaries. Do not repeat unchanged expensive lifecycle tests unless the changes invalidate them; agent startup changes do require focused native coverage. Avoid expanding into the old fault suite or full messenger suite. No actual installation, firewall mutation, task registration or publication yet. After corrected source/package acceptance, publish the coherent allowlist and prepare the one-time installer workflow under existing authorization.
