# Offline ratchet/sync audit probe

Component reproducer for review of commit
`3e82e6380c30d0f9443b6692ea34faccaacddef7`.

It uses real Kilogram `SyncServer`, `EventStore`, signed events and persisted
ratchets. It compares 256 messages decrypted chronologically with the same
ciphertexts selected by the real sync planner in hash-ordered pages of 64.
Within each page it uses the same author/device sequence ordering as the CLI.
It also checks rejection of an expired prekey pool despite an existing session.

This is an offline component probe, not a UI/network/end-to-end test. It creates
fresh synthetic keys and temporary stores; it opens no sockets. Expected success
means the existing defect was reproduced, not that the product is correct.
The random event IDs mean the exact failing sequence can differ between runs.

From the repository root on Windows:

```powershell
$env:TEMP = (Resolve-Path '.tmp').Path
$env:TMP = $env:TEMP
$env:CARGO_NET_OFFLINE = 'true'
& .\scripts\invoke-cargo-friendly.ps1 -CargoCommand build -CargoJobs 2 `
  -CargoArguments @('--locked', '--offline', '--manifest-path', `
    'ai-docs/reviews/repro-2026-10-22/Cargo.toml', '--target-dir', 'target')
if ($LASTEXITCODE -ne 0) { throw 'Audit probe build failed' }
& .\target\debug\kilogram-audit-probe.exe
if ($LASTEXITCODE -ne 0) { throw 'Audit probe did not reproduce the expected scenario' }
```

The initial audit built the identical source from `.tmp/astra-audit-probe`;
only relative dependency paths differ in this retained copy. Main source SHA-256:
`65678ffdd99df29952e563b12e9278dc297058d376ba47a3fbc913798e62714c`.
Keep the executable name/path stable for future runs. No direct Cargo test
harness invocation is needed.
