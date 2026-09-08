[CmdletBinding()]
param(
    [string] $OutputDirectory,
    [ValidateRange(1, 64)] [int] $CargoJobs = 4
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))

Push-Location $workspace
try {
    $dirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
    if ($dirty) { throw 'Refusing to create a field kit from a dirty worktree. Commit the exact source first.' }
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $revision -cnotmatch '^[0-9a-f]{40}$') {
        throw 'could not resolve an exact clean Git revision.'
    }
    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $OutputDirectory = ".tmp\m0967-field\kilogram-m0967-$($revision.Substring(0, 12))-$stamp"
    }
    $output = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
        [IO.Path]::GetFullPath($OutputDirectory)
    } else {
        [IO.Path]::GetFullPath((Join-Path $workspace $OutputDirectory))
    }
    if (Test-Path -LiteralPath $output) {
        throw "OutputDirectory already exists and will not be overwritten: $output"
    }

    $checks = @(
        'verify-kilogram-runtime-mailbox-flow.ps1',
        'verify-kilogram-volunteer-storage-boundary.ps1',
        'verify-kilogram-volunteer-iroh-boundary.ps1',
        'verify-kilogram-volunteer-provider-selection-boundary.ps1',
        'verify-kilogram-volunteer-provider-gossip-boundary.ps1',
        'verify-kilogram-volunteer-replication-boundary.ps1',
        'verify-kilogram-volunteer-retrieval-boundary.ps1',
        'verify-kilogram-volunteer-field-kit-boundary.ps1'
    )
    $boundaryLines = [Collections.Generic.List[string]]::new()
    foreach ($check in $checks) {
        $checkOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass `
            -File (Join-Path $PSScriptRoot $check) 2>&1)
        if ($LASTEXITCODE -ne 0) {
            throw "$check failed.`n$($checkOutput -join [Environment]::NewLine)"
        }
        foreach ($line in $checkOutput) { $boundaryLines.Add([string]$line) }
    }

    & cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli
    if ($LASTEXITCODE -ne 0) { throw 'debug field binary failed to build.' }
    if (& git status --porcelain --untracked-files=normal) {
        throw 'the worktree changed while the field kit was being built.'
    }
    $sourceCli = Join-Path $workspace 'target\debug\kilogram-cli.exe'
    if (-not (Test-Path -LiteralPath $sourceCli -PathType Leaf)) {
        throw "expected debug artifact is missing: $sourceCli"
    }

    $binDirectory = Join-Path $output 'bin'
    $scriptDirectory = Join-Path $output 'scripts'
    $docDirectory = Join-Path $output 'docs'
    New-Item -ItemType Directory -Path $binDirectory, $scriptDirectory, $docDirectory -Force | Out-Null
    Copy-Item -LiteralPath $sourceCli -Destination (Join-Path $binDirectory 'kilogram-cli.exe')
    $scriptNames = @(
        'new-kilogram-volunteer-field-provider.ps1',
        'invoke-kilogram-volunteer-field-runtime.ps1',
        'export-kilogram-volunteer-field-offer.ps1',
        'import-kilogram-volunteer-field-providers.ps1',
        'queue-kilogram-volunteer-field-message.ps1',
        'confirm-kilogram-volunteer-field-sender-offline.ps1',
        'capture-kilogram-volunteer-field-history.ps1',
        'verify-kilogram-volunteer-field-evidence.ps1'
    )
    foreach ($name in $scriptNames) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $scriptDirectory $name)
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'docs\M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md') `
        -Destination (Join-Path $docDirectory 'M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md')
    [IO.File]::WriteAllLines(
        (Join-Path $output 'BOUNDARIES.log'), $boundaryLines, [Text.UTF8Encoding]::new($false)
    )

    $cliItem = Get-Item -LiteralPath (Join-Path $binDirectory 'kilogram-cli.exe')
    $buildInfo = [ordered]@{
        schema = 1
        source_revision = $revision
        source_dirty = $false
        profile = 'debug'
        target = 'x86_64-pc-windows-msvc'
        cargo_jobs = $cargoJobsResolved
        created_utc = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
        archive = $false
        network_executed = $false
        artifacts = @([ordered]@{
            file = 'bin/kilogram-cli.exe'
            sha256 = (Get-FileHash -LiteralPath $cliItem.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            bytes = [UInt64]$cliItem.Length
        })
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'BUILD-INFO.json'),
        (($buildInfo | ConvertTo-Json -Depth 5) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    $manifest = [ordered]@{
        schema = 1
        run_id = 'REPLACE_WITH_yyyyMMdd-HHmmss'
        build_commit = $revision
        conversation_label = 'm0967-volunteer-field'
        alice_account_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        bob_account_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        message_marker = 'kilogram-m0967-REPLACE_WITH_UNIQUE_MARKER'
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'manifest.example.json'),
        (($manifest | ConvertTo-Json) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    foreach ($role in @('alice', 'bob')) {
        $config = @(
            '@{',
            "    Role = '$role'",
            "    ProfileFile = 'C:\Kilogram\private\$role\runtime-profile.json'",
            "    IpcFile = 'C:\Kilogram\private\$role\runtime.ipc.json'",
            "    StateDirectory = 'C:\Kilogram\private\$role\state'",
            '}'
        )
        [IO.File]::WriteAllLines(
            (Join-Path $output "$role.local.example.psd1"), $config, [Text.UTF8Encoding]::new($false)
        )
    }
    [IO.File]::WriteAllLines(
        (Join-Path $output 'RUN-ORDER.txt'),
        @(
            'Read docs/M0.9.67-VOLUNTEER-MAILBOX-FIELD-TEST-RU.md first.',
            '01 providers: bootstrap two private profiles, start both, export fresh offers.',
            '02 clients: start Alice and Bob, import both offers, then stop both.',
            '03 sender: keep Bob stopped, start Alice, queue marker, wait for 2/2, stop Alice, confirm offline.',
            '04 recipient: keep Alice stopped, start Bob, wait for two volunteer commits and deletes, stop Bob.',
            '05 restart: start and stop Bob once more, then capture history.',
            '06 evidence: copy BOUNDARIES.log and run the fail-closed verifier.'
        ),
        [Text.UTF8Encoding]::new($false)
    )

    Write-Output "volunteer_field_kit_directory=$output"
    Write-Output "source_revision=$revision"
    Write-Output 'build_profile=debug'
    Write-Output 'archive_created=false'
    Write-Output 'network_executed=false'
    Write-Output 'status=kilogram-m0967-volunteer-field-kit-created'
}
finally {
    Pop-Location
}
