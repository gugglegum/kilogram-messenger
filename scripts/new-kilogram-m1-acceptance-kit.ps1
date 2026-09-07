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
    if ($LASTEXITCODE -ne 0) {
        throw 'git status failed.'
    }
    if ($dirty) {
        throw 'Refusing to create an acceptance kit from a dirty worktree. Commit the exact source first.'
    }
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $revision -cnotmatch '^[0-9a-f]{40}$') {
        throw 'could not resolve an exact clean Git revision.'
    }

    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $OutputDirectory = ".tmp\m1-acceptance\kilogram-m1-acceptance-$($revision.Substring(0, 12))-$stamp"
    }
    $output = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
        [IO.Path]::GetFullPath($OutputDirectory)
    }
    else {
        [IO.Path]::GetFullPath((Join-Path $workspace $OutputDirectory))
    }
    if (Test-Path -LiteralPath $output) {
        throw "OutputDirectory already exists and will not be overwritten: $output"
    }

    $checks = @(
        'verify-kilogram-mailbox-boundary.ps1',
        'verify-kilogram-runtime-mailbox-flow.ps1',
        'verify-kilogram-mailbox-capability-lifecycle.ps1',
        'verify-kilogram-mailbox-capability-convergence.ps1',
        'verify-kilogram-mailbox-desktop-control.ps1',
        'verify-kilogram-mailbox-field-test-boundary.ps1',
        'verify-kilogram-m1-acceptance-kit-boundary.ps1'
    )
    $boundaryLines = [Collections.Generic.List[string]]::new()
    foreach ($check in $checks) {
        $checkPath = Join-Path $PSScriptRoot $check
        $checkOutput = & powershell -NoProfile -ExecutionPolicy Bypass -File $checkPath 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "$check failed.`n$($checkOutput -join [Environment]::NewLine)"
        }
        foreach ($line in $checkOutput) {
            $boundaryLines.Add([string]$line)
        }
    }

    & cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-windows
    if ($LASTEXITCODE -ne 0) {
        throw 'debug acceptance binaries failed to build.'
    }
    $dirtyAfterBuild = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0 -or $dirtyAfterBuild) {
        throw 'the worktree changed while the acceptance kit was being built.'
    }

    $sourceCli = Join-Path $workspace 'target\debug\kilogram-cli.exe'
    $sourceWindows = Join-Path $workspace 'target\debug\kilogram-windows.exe'
    foreach ($path in @($sourceCli, $sourceWindows)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "expected debug artifact is missing: $path"
        }
    }

    $binDirectory = Join-Path $output 'bin'
    $scriptDirectory = Join-Path $output 'scripts'
    $docDirectory = Join-Path $output 'docs'
    New-Item -ItemType Directory -Path $binDirectory -Force | Out-Null
    New-Item -ItemType Directory -Path $scriptDirectory -Force | Out-Null
    New-Item -ItemType Directory -Path $docDirectory -Force | Out-Null
    Copy-Item -LiteralPath $sourceCli -Destination (Join-Path $binDirectory 'kilogram-cli.exe')
    Copy-Item -LiteralPath $sourceWindows -Destination (Join-Path $binDirectory 'kilogram-windows.exe')

    foreach ($name in @(
        'invoke-kilogram-m1-acceptance-step.ps1',
        'test-kilogram-mailbox-store-preflight.ps1',
        'invoke-kilogram-mailbox-field-runtime.ps1',
        'capture-kilogram-mailbox-field-status.ps1',
        'verify-kilogram-mailbox-field-evidence.ps1'
    )) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $scriptDirectory $name)
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'docs\M0.9.60-M1-ACCEPTANCE-RU.md') `
        -Destination (Join-Path $docDirectory 'M0.9.60-M1-ACCEPTANCE-RU.md')
    Copy-Item -LiteralPath (Join-Path $workspace 'docs\M0.9.58-MAILBOX-LIFECYCLE-FIELD-TEST-RU.md') `
        -Destination (Join-Path $docDirectory 'M0.9.58-MAILBOX-LIFECYCLE-FIELD-TEST-RU.md')
    [IO.File]::WriteAllLines(
        (Join-Path $output 'BOUNDARIES.log'),
        $boundaryLines,
        [Text.UTF8Encoding]::new($false)
    )

    $artifacts = foreach ($relativeName in @('bin/kilogram-cli.exe', 'bin/kilogram-windows.exe')) {
        $path = Join-Path $output ($relativeName.Replace('/', '\'))
        $item = Get-Item -LiteralPath $path
        [ordered]@{
            file = $relativeName
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
            bytes = [UInt64]$item.Length
        }
    }
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
        artifacts = @($artifacts)
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'BUILD-INFO.json'),
        (($buildInfo | ConvertTo-Json -Depth 5) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )

    [IO.File]::WriteAllLines(
        (Join-Path $output 'RUN.ps1'),
        @(
            "Set-StrictMode -Version Latest",
            "`$ErrorActionPreference = 'Stop'",
            "& (Join-Path `$PSScriptRoot 'scripts\invoke-kilogram-m1-acceptance-step.ps1') @args"
        ),
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllLines(
        (Join-Path $output 'alice.local.example.psd1'),
        @(
            '@{',
            "    Role = 'alice'",
            "    ProfileFile = 'C:\Kilogram\private\alice-runtime-profile.json'",
            "    IpcFile = 'C:\Kilogram\private\alice-runtime.ipc.json'",
            "    StoreStartupLog = ''",
            '}'
        ),
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllLines(
        (Join-Path $output 'bob.local.example.psd1'),
        @(
            '@{',
            "    Role = 'bob'",
            "    ProfileFile = 'C:\Kilogram\private\bob-runtime-profile.json'",
            "    IpcFile = 'C:\Kilogram\private\bob-runtime.ipc.json'",
            "    StoreStartupLog = 'C:\Kilogram\shared-staging\mailbox-store-startup.log'",
            '}'
        ),
        [Text.UTF8Encoding]::new($false)
    )
    $manifest = [ordered]@{
        schema = 1
        run_id = 'REPLACE_WITH_yyyyMMdd-HHmmss'
        build_commit = $revision
        conversation_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        alice_account_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        alice_device_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        bob_account_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        bob_device_id = 'REPLACE_WITH_64_LOWERCASE_HEX'
        mailbox_service_url = 'https://mailbox.example.test'
        mailbox_store_key = 'REPLACE_WITH_64_LOWERCASE_HEX'
        activation_route = 'direct'
        rotation_route = 'relay'
        revocation_route = 'relay'
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'manifest.example.json'),
        (($manifest | ConvertTo-Json) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )

    Write-Output "acceptance_kit_directory=$output"
    Write-Output "source_revision=$revision"
    Write-Output 'build_profile=debug'
    Write-Output 'archive_created=false'
    Write-Output 'network_executed=false'
    Write-Output 'status=kilogram-m1-acceptance-kit-created'
}
finally {
    Pop-Location
}
