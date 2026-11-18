# T1.3e: combined one-time setup candidate

19 November 2026. Source/package review handoff only. Implements the [root preparation decision](terminal-t1-3e-autonomous-install-prep-root-decision.md). No UAC, task registration, firewall/profile mutation, account/CIM ACL change or messenger field run was performed in this slice. Product Rust code is unchanged.

## Published base

Accepted candidate5 was committed as `0613a972154d62a4bc2ee9d75a5ec6e035f5920a`, exact parent `2679c3ac4ab329e38ce7d2c66c6c9bac702ff79f`, exact five accepted paths/blobs. Ordinary non-force push succeeded; ls-remote returned that same tip. Receipt `.tmp/terminal-t1-3e/prepare-autonomous-install-review-01/publication5-final.json`, SHA-256 `fff48df64a6eb8a9b9e4a08f43abd0cd2d478274c63594288835ee2484e283b8`. New corrections below are uncommitted and require root review. The index remains empty; unrelated dirty work is preserved, including root's separately recorded coordination update.

## User exit report

Fresh read-only A/B reconciliation at 22:10 UTC found no Kilogram processes, Private Off and no COM-owned lab rules. Latest B field receipt remained t13e10 (18:14 UTC). `.tmp/terminal-t1-3e/exit-report-reconcile-01/{A,B}-current.json` contains the actual observations. The previously captured t13e10 result has primary `receiver monitor did not anchor`, no completed phases/messages; closure errors include `T13E_RUNNER_CHILD`. See [the preserved field result](terminal-t1-3e-t13e10-field-result.md). This slice did not launch a new Kilogram process; the report alone does not establish a new product crash.

## Root review R1 corrections

The [first source review](terminal-t1-3e-combined-install-root-review-r1.md) rejected two races. Prior candidate/package/evidence remain unchanged; all 12 previous source bytes were additionally preserved under `.tmp/terminal-t1-3e/combined-source-review-r1-before-01/source/`, with original pins in `preserved.json`.

Actor admission now shares one Stopwatch deadline between waiting for actor.json, validating its exact process incarnation/token and waiting for the exact Established socket. Production default remains45s; tests can only reduce it. The held process handle is checked before and after every socket read; it is not reopened by PID. Other-process sockets cannot admit the actor. Exit, socket timeout and ambiguity retain PID/creation/read-count/elapsed diagnostics; unknown identity/provider errors still fail. Read-only enumeration avoids treating a native filtered-query ObjectNotFound during connect as proof of readiness.

Setup no longer captures authoritative policy before install.lock or returns a cached pre-lock snapshot. Inside the exclusive installation lock, the production transaction Snapshot calls Get-LabSetupSnapshot for fresh full CIM Audit, predicates and ordinary projection comparison before any journal intent or OS mutation. That same admitted state supplies the immutable rollback baseline. Setup has no tautological cached-digest comparison; the separately exposed legacy Provision digest check is retained. The lock serializes our installers, not Windows-wide policy administration.

Focused result `.tmp/terminal-t1-3e/combined-admission-r1-check-03/checks.json`:8 cases PASS. Production actor loop: delayed file plus socket success, process exit, never-ready timeout, file wait consuming the same socket budget, bad identity. Production backend Snapshot/Preflight and transaction engine with real exclusive file lock: changed Private policy and competing task rejected with one fresh read and zero intent writes/mutations; an admissible profile change becomes the fresh initial rollback state. Only token/socket/CIM/COM observations and OS mutations were fixtures. Earlier01 failure was a missing test-scope files list, preserved;02 passed7 cases,03 adds the explicit shared-budget case. No unchanged heavy suite was repeated.

## Actual SSH-token applicability

`test-terminal-t1-3e-lab-native.cs` retains an exact process incarnation/image/token handle. For the supported single explicit non-callback Allow ACE with mask `FWP_ACTRL_MATCH_FILTER=1`, native AccessCheck evaluates enabled, deny-only and restricting SIDs. The helper returns match `1`, mismatch `-1`, unsupported/unknown `0`. It rejects other masks, multiple/deny/inherited/callback ACEs, SACLs, missing owner/DACL, dead process and AppContainer. An absent primary group is filled solely for AccessCheck's descriptor requirement; this does not change the explicit ACE or requested bit 1. No privileges are enabled.

Actual proof: `.tmp/terminal-t1-3e/combined-setup-05/actual-ssh-token.json`. The ordinary system ssh.exe had A's SID1001 and an established `192.168.0.140 -> 192.168.0.152:22` socket. The exact captured foreign SID1012 LocalUser descriptor returned mismatch, and the actual A SSH-path guard passed against the pinned real A Audit. Native checks also passed user match, enabled group match, unsupported deny descriptor, restricted-user rejection, restricting-World match and deny-only-World rejection. Ephemeral token copies were used only for tests. Foreign firewall rules were preserved.

The setup launcher starts this fixed pinned SSH connection after UAC is accepted. The elevated installer reopens its PID/creation time/system image/ordinary SID, checks the actual established socket and retains its live token through Audit and Verify. The administrator collector's own token is never substituted. B currently needs no conditional LocalUser interpretation; unsupported conditional server rules remain a preflight failure.

Microsoft references: [FWP principal condition/access mask](https://learn.microsoft.com/en-us/windows/win32/fwp/permitting-and-blocking-applications-and-users), [AccessCheck](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-accesscheck), [restricted-token semantics](https://learn.microsoft.com/en-us/windows/win32/secauthz/restricted-tokens).

## Current ordinary readiness

`lab-readiness.ps1` uses INetFwPolicy2/INetFwRule3, native NLM and .NET interface identity, plus numeric netsh UDP range. It checks exact owned rule/profile/network fields. Local persistent raw rule values are pinned; a bounded registry fingerprint covers local firewall rules, profile inputs, GPO and MDM inputs. This also detects changed fields not exposed by COM, including previously approved DynamicTransport defaults. Unsupported security/profile/interface variants, duplicate/unexpected owned rules and local-policy modification restrictions fail closed.

Fresh ordinary observations in `.tmp/terminal-t1-3e/combined-current-01/` passed on both real contexts: A 0.534s/1279 values, B ordinary SSH 1.044s/654 values. No privileged CIM access was granted or needed. Native Wi-Fi identity/address/Private category and UDP49152–65535 matched the fixed scope. Public xray_tun on A is preserved. Captured prior A COM/native profiles/network were independently compared with the pinned real CIM Audit in the focused check.

COM is not used as a full policy Audit. During setup, authoritative fresh admin CIM facts are compared against COM/native own-rule views for both ActiveStore and PersistentStore; effective source must be Local, local merging/inbound rules must be enabled, and native network/profile facts must agree. Public baseline v2 preserves the administrative relevant projection and the separate ordinary projection. Every ordinary check rereads current COM/native/registry state; the baseline pin alone never proves current state. Baseline v1 is rejected rather than silently downgraded.

[INetFwRule3 fields](https://learn.microsoft.com/en-us/windows/win32/api/netfw/nn-netfw-inetfwrule3) and [local policy modification state](https://learn.microsoft.com/en-us/windows/win32/api/netfw/nf-netfw-inetfwpolicy2-get_localpolicymodifystate) define the supported API boundary.

## One installation action / separate final teardown

Concrete R1 local review packages are `.tmp/terminal-t1-3e/combined-package-04/package-A` and `package-B`. Package manifest v2 pins exactly 11 files. Each contains `Install-A/B.cmd` and separate `Teardown-A/B.cmd`; they are bound to the fixed eventual deployment path `<base>\deploy\autonomous-combined-install-review-01`. These launchers are prepared for review, not deployed/armed or requested from the user. Prior package03 remains preserved.

- A package manifest SHA-256: `5e7b721ccd38192e2dfbb7e674446ab84cb3e8c74de4ffbcb3fc7ab6ae8a39f4`.
- B package manifest SHA-256: `ce75a4ac647d6886e5d7ad8c2424051691fedbc0c11c45d23a4bf64cfd3ecdda`.

One UAC invocation snapshots fixed package bytes through held handles, checks all embedded pins, and copies verified bytes into an admin/System protected staging directory before loading package code as administrator. Readable public success/error receipts are published separately under a protected directory; the ordinary user receives read access only. Initial policy/journal and full Audit remain protected. Private SSH key contents are never copied or printed; only existing paths and public host-key pins are in the package.

Inside that invocation, Setup performs fresh full CIM Audit, validates actual scope/SSH applicability and ordinary capabilities, then enters the fixed transaction. The prior ordinary full-Audit prerequisite is removed. Unsupported preflight facts abort before task/firewall/profile mutations. Administrative staging/evidence files are the only preceding writes. Initial baseline/history are preserved. Setup/idempotent Verify starts the exact InteractiveToken/LeastPrivilege task and requires its real ordinary read-only readiness plus synthetic DPAPI roundtrip before declaring installed. A failure rolls back the transaction; rollback stops the owned idle agent before deleting its task/files. Final teardown keeps network/unrelated-policy/owned-scope drift checks and does not require proving A's conditional SSH rule merely to restore the prior profile.

Active setup child: 450s; fixed recovery child: at most180s; actor admission45s inside that budget; task readiness25s plus reply22s; ordinary readiness child15s with owned-Job cleanup. No actor or active-work deadline starts before UAC acceptance. The SSH keepalive is750s and only the launcher's exact process handle is terminated. The ordinary wrapper waits at most690s after UAC acceptance. Setup supervisor captures actual stdout/stderr/exit/timeout. Recovery is allowed only for a protected journal intent stamped with this setup session; preflight/idempotent failure cannot tear down an earlier or concurrent lab. Incomplete cleanup/recovery remains an explicit failed result with preserved evidence. There is no arbitrary elevated command argument or reusable admin SSH facility. Bootstrap and setup workers run BelowNormal.

Ordinary attempts remain passwordless/without UAC after successful setup. Their readiness runs in a bounded inherited Job child; synthetic DPAPI bytes only, no seeds. Failure/timeout must leave the idle agent reusable, or mark cleanup uncertainty and block work. The persistent agent has no idle invitation deadline.

## Verification and limits

- Existing installer transaction/XML/DACL checks:13 groups PASS; role/policy/recovery checks:21 PASS; preparation checks:9 PASS. The unchanged long lifecycle suite was not repeated.
- New COM/CIM/readiness checks:7 groups PASS. Actual in-memory INetFwRule objects (never added to policy), every own-rule field drift, unsupported security/source, effective profile drift, network/backend drift and current raw-policy drift with unchanged baseline pin.
- Native readiness:4 cases PASS in `combined-ready-native-02`: real local InteractiveToken/logon2/session1/hive/DPAPI; setup collector talking to the real agent with only task Run/policy boundary mocked; child failure and real15s timeout, then successful retry in the same agent/session. No task was registered.
- Native supervisor:4 cases PASS in `combined-supervisor-native-01`: success, actual exit1 with exact two-stream bytes, no recovery of another setup, real native timeout and owned recovery. Child installer was synthetic; production supervisor/Job/capture were used.
- R1 final package manifests/source pins and all generated CMD/elevated bootstrap syntax checked.18 PowerShell inputs parsed; Python ASTs parsed. CMD command lines fit8191; elevated encoded bootstrap12448/12360 chars fits direct process creation32767, not CMD transport. No UAC execution. The final admission-check version was additionally parsed/executed in its8-case run.

Earlier native-token helper failures01/02, output-parameter fixture04, COM array fixture failures and test-scope variable failures remain in their original directories. They were not repaired/retried in place. `combined-ready-native-01` preserves the earlier3-case result;02 adds the setup collector case. `combined-package-01/02` are superseded review copies, never installed. All new test children are gone.

Reproduction uses Windows PowerShell5.1 `-NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/test-terminal-t1-3e-lab-{install,r1,prep,combined,launch,admission}-check.ps1 -Evidence <fresh path>` and the pinned Python runtime for `scripts/test-terminal-t1-3e-lab-ready-check.py <fresh path>`. Source/package generation uses installer `-Mode Package -Side A/B -Output <fresh path>`. No direct Cargo test or Rust build was used.

Remaining acceptance boundary: root reviews this coherent diff and concrete packages first. Actual full B administrative Audit, real registered-task/backend proof on B and real one-time install/teardown have not been exercised; the reviewed fixed setup is responsible for proving those before success and preserving failure/rollback evidence. No field retry, new source commit or push is claimed for this candidate. Executor stops after callback.
