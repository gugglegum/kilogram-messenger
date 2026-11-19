# B instrumented retry: exact singleton-address cause and scoped repair

19 November 2026. Ordinary readback after the user reported the accepted RetryInstall-B.cmd failed. Source baseline `ef3b7d5779145d39a647b752e934772f676965fd`, deployed manifest `a135cfcb8e6c01d4c20d822e4dec4da9c1df3853d7b06a55a95039d591a0e9e0`. This stage performs readback, local source repair and disposable checks only. No repeat UAC/install/recovery, policy/task/account/SSH/VPN change, A launch, key-file read, commit/push/deployment or user launch request.

## New actual invocation

The new owner is **setup-33ab20c7666e46e4bdacd13f1ca6a735**, created11:41:05.6013014UTC; action RetryInstallB. Its launcher-error was written11:45:17.9463504UTC. This is distinct from the earlier6f5c3fe3 owner. The pinned ordinary deployment manifest matched a135cfcb. Raw public result SHA256 **a95be0ed148e4e9781153837e49896202e73487362892ad19f4e9d59116c6eba** matches saved bytes/base64/length/parsed content.

Main exited1, fallback0; both timeout=false and capture_complete=true. Primary **LAB_COM_SPEC_DIFFERS** at **pre-profile-comparison**. Terminal reporting stage is B-retry-terminal-proof, retaining that primary. Main stderr556 bytes reports LAB_INSTALL_FAILED:LAB_COM_SPEC_DIFFERS;rollback=rolled_back. Fallback stdout35 bytes reports rolled_back, stderr0; main stdout0. Current minimal journal belongs to33ab..., phase rolled_back, rollback_errors0. Retry admission kind known_origin, prior owner6f5c3fe3..., origin manifest8a15960e, history_files1. The retry outcome confirms verified/retryable, owned_absent, history/pins preserved, initial raw SHA256 **ef680f33b876e1ca5bad20999c1ebbac16bd42494fd2d27f9683bed98c669e0f**. The admission/outcome hashes agree on that initial and a135 package.

All available samples were complete and not truncated: four logs, SSH proof1421 bytes, COM comparison7126, retry-admission350, retry-outcome376. Chunk offsets/counts/base64/byte hashes and full hashes were checked. Expected missing old-recovery logs were not treated as errors. Original protected logs/journal were not retried through denied ordinary access.

## Exact observed cause

The comparison captured **only LocalAddress and RemoteAddress differences**, in both owned rules. Desired specification and PersistentStore/ActiveStore CIM views agree on each scalar address; the COM view represents it as a singleton range:

| Rule | Field | Expected and both CIM stores | Actual COM |
| --- | --- | --- | --- |
| Discovery-B | LocalAddress | 239.255.75.72 | 239.255.75.72-239.255.75.72 |
| Discovery-B | RemoteAddress | 192.168.0.140 | 192.168.0.140-192.168.0.140 |
| Iroh-B | LocalAddress | 192.168.0.152 | 192.168.0.152-192.168.0.152 |
| Iroh-B | RemoteAddress | 192.168.0.140 | 192.168.0.140-192.168.0.140 |

Program, ports, interface, action, profile and supported security fields match. This establishes the exact new attempt's representation mismatch; the older6f5c attempt did not capture its field values, so this is not substituted for missing historical evidence. The unchanged captured bytes are retained as `scripts/fixtures/terminal-t1-3e-B-com-singleton.json`.

The strict pre-profile comparison refused before Set-NetFirewallProfile True. Reverse cleanup completed without errors; this attempt did not require a profile enable/restore cycle. The System listener-token proof also succeeded: PID7980, actual S-1-5-18, expected sshd image/account/listener ownership, no peer flows at the observation, native nonmatch -1 against the recorded SID1004 Block. It remains proof of that observed supported actor set, not future owners.

At12:03:42.3137139UTC, independent ordinary checks found PrivateOff, Domain/PublicOn, zero owned COM/registry rules, exact B task absent with80070002, all15 copied payloads absent, baseline/state/ready absent, no test CLI/bootstrap processes; all four original bin hashes remain exact. Each absence was classified from an ordinary readable/not-found result, not protected AccessDenied. A was not run or altered. Both bounded children are confirmed exited; no broad process scan or denied WMI retry was used.

## Minimal source repair

The COM converter collapses only a **complete canonical IPv4 x-x token with equal endpoints** to x, with explicit octet bounds. The change is limited to LocalAddress/RemoteAddress. A closed range whose equal endpoints are x contains exactly x, so the allowed address set is unchanged. Different endpoints, wider ranges, lists, IPv6, leading-zero/short/malformed forms and unrelated fields receive no new normalization. Existing plain /32 handling remains. No global equality relaxation or SID/interface/case/default bypass was added.

The regression recreates the captured old converter output exactly using the unchanged published converter and the captured DTO boundary; it reproduces the failure before the fix and matches the actual expected and both CIM views after the fix. Production full comparison accepts the equivalent view but rejects larger/different ranges and port/interface/action/user/CIM changes. These are local replay fixtures, not newly registered Firewall rules or a live installation test.

## Exact package/provenance transition

A new package changes its own hash, so the old same-package retry record must not be silently repurposed. One additional fixed transition admits only the actually observed33ab... terminal owner, old package a135cfcb, the exact exported initial hash ef680f33..., zero rollback errors, old protected package/files, exact clean terminal journal hash/want and unchanged protected history. Unknown or incomplete owners/packages/initials remain rejected.

At the next admitted intent, the prior protected ledger is archived **byte-for-byte** to `B-terminal-retry-a135-predecessor.json` without overwriting conflicting evidence. All prior attempt records remain in the active ledger, including33ab's original package hash. A migration record links the old ledger hash and fixed owner/package/initial to this exact new package. Later own clean retries validate that immutable predecessor and preserve all initial/history checks; they do not require another source patch merely because a new owner was generated. Different future packages are not automatically authorized. The existing absent-intent guard and bounded450/180s main/owned fallback remain; no automatic retry loop.

The new ordinary target is versioned `deploy/autonomous-B-rollback-retry-review-02`, leaving the accepted old01 destination and all protected packages/results untouched. Transition code was prepared only; no protected campaign record was accessed/updated by this source stage.

## Focused verification and concrete package

**104** counted cases/bundles passed under Windows PowerShell5.1: captured singleton comparison27, exact transition12, existing B retry36, strict comparison14, A legacy recovery14 and package1. Transition tests execute the unchanged accepted a135 helpers to produce old-format clean provenance, then exercise current admission/production Write/completion, raw predecessor retention, initial/history preservation, own continuation, and negatives for owner/package/initial authority/phase/errors/pending/history/pins/archive conflict/corruption. Protected roots and the external initial-hash authority are explicitly mapped to disposable fixtures; privileged observations remain mocked. No policy registration or system mutation.

Complete local package: `.tmp/terminal-t1-3e/B-singleton-package-02/package-B`, manifest **608764ea40c2ae380392fbdaf0348b65c3970e4e5b5a878d5373328947a749f0**,15 payloads+manifest+RetryInstall-B.cmd =17 files. All payload/source pins and fixed B02 action/path/manifest/launcher pins pass; CMD max1506 and completed bootstrap encoded14040, parsed without executing. Package01 is a preserved superseded build; use02. Existing local a135 and original A/B packages remain unchanged.

The eight-path candidate contains four modified sources/checks (readiness, recovery, launcher, package check), the exact captured fixture, two focused checks and this result note. It is uncommitted and not deployed. The observed COM defect is repaired in replay; actual subsequent install/task/DPAPI success remains unproved. Root reviews this exact diff/package before publication, deployment or another user action. Executor returns one substantive callback and stops.

Evidence: `.tmp/terminal-t1-3e/B-retry-live-01` (public receipt, reconstructed samples, current state), `B-singleton-check-01`, `B-singleton-upgrade-check-01`, `B-singleton-regression-01`, and the final review index with exact hashes.
