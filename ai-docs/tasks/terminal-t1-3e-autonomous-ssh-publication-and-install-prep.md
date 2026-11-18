# Autonomous lab: exact r1 publication and installation-preparation findings

19 November 2026. The accepted r1 coherent diff was committed and pushed before preparing installation. Preparation found real-host problems, so installation remains blocked. There were no UAC launches, task registrations, firewall/profile mutations or messenger field runs.

## Published accepted scope

Commit **2679c3ac4ab329e38ce7d2c66c6c9bac702ff79f**, parent `36053c7bc56d0e00bea8992ae9fb218c50b10d76`, is on `origin/master`. Ordinary push succeeded without force; ls-remote independently returned that exact tip. The commit contains exactly the 15 accepted paths/blobs from allowlist `b9bcf9f943c98e559bd701a33139c2556cbcb6113831f2f722b8f395adc6b56a`. The index was empty before and after publication, and ten unrelated tracked dirty files were preserved byte-for-byte.

Exact receipt: `.tmp/terminal-t1-3e/autonomous-source-review-r1-01/publication-final.json`, SHA-256 `caaa90f23858bf640a10742f4ca3087159023cae76d45c2eba11e175c4e682f0`. This publication does not include the later candidate corrections below.

## Actual Audit and serialization

The accepted pinned r1 packages were copied to ordinary test deploy directories on both hosts. Read-only preparation preserved every failed attempt. Process-only ExecutionPolicy Bypass was needed to load the packaged helpers. A's original per-rule filter reads exceeded the local 100/110-second deadline; B's ordinary SSH context was denied access at Get-NetFirewallProfile.

The candidate obtains A's complete selected policy using one pipeline invocation per filter kind, association-scoped to selected rules with throttle 4. It avoids unrestricted all-filter enumeration, which was denied, and avoids passing an array to the singular AssociatedNetFirewallRule parameter. Every selected filter ID must be unique/present; one actual direct-association projection is compared with the index. The real A capture completed within the bounded invocation, with **870 rules**. Full current captured data and the original digest are in `.tmp/terminal-t1-3e/prepare-autonomous-install-05/A-current-audit.json`; bytes 699716, file SHA-256 `ec9da44aea31b27139580636712519a8b382dcea74413879c127b1a9f82ad1f5`, policy digest `539879e1fa371bd1e10d87875f7d1b4fed187ef279648cadf9c2437dff7e2f56`.

Production Lab-Write/Lab-Read and Save-LabInstallJournal were exercised on disposable copies of those actual data, before any UAC:

| Payload | Bytes | Accepted r1 behavior | New candidate |
| --- | ---: | --- | --- |
| Real snapshot | 695020 | Read rejected by 131072-byte bound | Roundtrip PASS |
| Real initial journal | 700325 | Read rejected; second journal save failed | Roundtrip and second save PASS |
| Audit wrapper | 699716 | Read rejected | Roundtrip PASS |
| Projected public baseline | 7478 | Small enough for default bound | Roundtrip PASS with default bound |

The public baseline is projected from actual profiles/adapter/network and reviewed owned-rule specs; it is not an installed-state observation. The first test's profile-array clone acquired a PowerShell value/Count wrapper; the corrected fixture explicitly clones individual profiles. That fixture issue is distinct from the reproduced production size rejection. Failed data and corrected data are both preserved.

The candidate preserves the **128 KiB default for ordinary control/config/public JSON**, introduces an explicit **2 MiB bound for protected installation policy snapshots/journals/history**, and applies the same bound before writer creation/replacement and on reading. Oversized writes leave existing files unchanged. This is a bounded state-specific correction, not removal of the reader limit. Evidence: `autonomous-real-json-before-01/roundtrip.json` and `autonomous-real-json-after-01/roundtrip.json` under `.tmp/terminal-t1-3e/`.

## Current blockers and exact SSH path

**A:** the current records show `xray_tun` Public and the selected Wi-Fi Private. The initial LAB_PROFILE_WIDE_NETWORK_EFFECT was a formatting mismatch: the native adapter GUID had braces/capitalization, while the config used an unbraced GUID. The candidate compares GUID values. A is the outbound SSH client; it has no running sshd service and does not need one. The guard now models A's client/request/reply path and B's server/request/reply path; it retains B's inbound allow requirement and conservative checks of explicit blocks.

After those corrections, the captured A policy still produces **LAB_SSH_FILTER_UNCERTAIN**, correctly preventing provisioning. Exactly one possibly applicable inbound-reply Block rule remains: `{D7B3C06E-6AEE-44D5-806A-5B5798393C89}`, with LocalUser `O:LSD:(A;;CC;;;S-1-5-21-1776862868-2140151431-2099235586-1012)`. Other address/port/program constraints are Any. Intended A user is SID `…1001`. The accepted finite guard does not interpret this conditional ACL; it cannot silently ignore it or infer effective access solely from the unequal user SID. The exact rule is in `prepare-autonomous-install-05/A-ssh-policy-blockers.json`. No foreign rule was changed. A narrow token/ACL applicability proof needs engineering review before enabling Private; a broad SSH exception is not proposed.

**B:** current ordinary SSH user is `S-1-5-21-2671144489-3236106012-1032288181-1006`. Get-NetFirewallProfile reports CIM access denied. Get-Service sshd is also unavailable in that context; this does not prove the service is absent. Registry and ordinary COM observations confirm the system OpenSSH server path and one inbound TCP/22 allow rule with wildcard ports/addresses, Private category, and Private firewall disabled. Those observations are incomplete relative to the production full CIM Audit. See `prepare-autonomous-install-02/B-collection.json` and `prepare-autonomous-install-05/B-ordinary-com-ssh.json`.

Therefore no full B snapshot/journal/public-baseline roundtrip is claimed. Nor is the future ordinary InteractiveToken task's ability to read the CIM provider established by this SSH denial. Root must choose/prove the intended read-only context/backend and obtain the full concrete B Audit; do not change account groups, CIM ACLs, VPN or service settings to bypass the issue.

## Candidate and verification for root review

The new candidate is **not accepted r1** and is **not published**. It changes common JSON bounds, installer snapshot collection/role/GUID checks, the server-side r1 fixtures, and adds focused preparation checks. Candidate packages were copied, never invoked, to each host's `deploy/autonomous-install-prep-review-01`. Local review copies are `.tmp/terminal-t1-3e/prepare-autonomous-install-review-01/package-A` and `package-B`:

- A manifest SHA-256: `3fb4e05e84e46fa0c1fb1020f1af3f80edcfcd20c4e7fdcd94e7cac84af5b884`
- B manifest SHA-256: `b1186ea09effe1ca3f711a04e09b91140c805ec51455c867646b0b0997a48454`

Focused results: **9** preparation groups PASS (index/projection identity and collisions, exact and plus-one default/policy byte limits, old-file preservation, GUID form and actual client role); **21** r1 policy/recovery groups PASS with the server fixtures bound to B and server reply-block coverage; **13** installer regression groups PASS. The unchanged long native lifecycle suites were not repeated. Actual full A association mapping and actual large JSON roundtrip supplement the boundary fixtures. No Rust rebuild/tests were needed.

The new source diff, proposed next allowlist and immutable evidence pins are in `prepare-autonomous-install-review-01/preparation-review.json`. Syntax checks passed. Final read-only delivery receipts confirm **Private Off and zero owned active/persistent lab rules on both hosts**. Earlier timeout, access-denial, array-binding and fixture failures remain preserved.

The promised single installer launcher per host and separate final teardown are not armed or offered for execution: A's conservative guard and B's missing full Audit prevent a reviewable safe installation action. Root owns candidate review and the two concrete blocker decisions. Existing lifecycle authorization remains valid; no new user permission request is needed. After those decisions, bind the launchers to the accepted package and reviewed real Audit digests, then perform the separately dispatched one-time installation and runtime task/DPAPI/Setup proof.
