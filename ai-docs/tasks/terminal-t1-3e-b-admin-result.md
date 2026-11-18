# T1.3e administrative diagnostics on B

Accepted on 2026-11-18. The B-only attempt `t13e05` passed the single-UAC administrative diagnostic. The fresh Audit matched the approved policy. Apply opened the ordinary control pipe, S returned `open`, the first X returned `rolled_back`, and the captured child exited 0 with complete streams and empty stderr. An independent post-run check found Private Off, no owned rules, test processes or pipe, and both original test executables at their expected hashes.

The protected firewall source uses atomic file replacement for journal updates and checks the native CIM `EnforcementStatus` codes for Full (1). NetSecurity on B projects that code as `Enforced`. The check also validates rule count, enabled state, ownership, policy source and all expected filters. Rejections record the scoped rule status fields. The capture wrapper preserves both raw child streams, and a secondary rollback failure leaves the primary Apply error intact.

## Scope and evidence

The published scripts provide the audited firewall bridge, captured child launcher, capture-only CMD generator, ordinary observer and test-image Restore helper. The live result uses the separately reviewed combined B05 bootstrap. Generated packages, local deployment inputs and raw policy reports are retained with the local test evidence. A's administrative bridge and the A–D two-host field test remain pending.

| Recorded artifact | SHA-256 |
| --- | --- |
| Approved canonical B Audit policy | `c79adfd11f614153990dd41b07355ef8e906a27183204daf8c6782051fd4fde6` |
| Applied firewall source | `fe5d0280b79ea64cc4a3401f1e037cf4bdb1c0d00868f196fb97b69d40a3fab8` |
| `t13e05-result-review.json` | `688df2f1c4cd8a646b6f623ab64b49574179ffc20982cb26bfee13c2630d02b0` |
| `watch-result.json` | `7b65f0f66f096ff66589d2587a52059915fc03353d4be8ed667c1ee471b33624` |
| `watch-316a5cc0116747f4868ba76fee07f5cb-trace.json` | `88219be8bccf219fb0e1fb7cbf003470bd33313a797b6a75979f6260affae7b8` |
| `b05-post-restore-80312b7fd6ce4699b86724dfd1759921.json` | `3e53d058759fa303e155fa4b1e4d0ef22e7dcc539b977ac1c09d1e6e7db7b275` |

## Local regression checks

Run from the repository root in Windows PowerShell 5.1.

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/test-terminal-t1-3e-state-check.ps1
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/test-terminal-t1-3e-launch-check.ps1
```

The state check uses fresh files and synthetic CIM objects for atomic journal replacement, repeated updates, and the raw Full predicate. The launch check uses simulated NetSecurity errors and a local child for absent-rule rollback, primary error preservation, exit status and byte-exact stdout/stderr. Both write fresh fixtures under the ignored `.tmp/terminal-t1-3e/` directory.
