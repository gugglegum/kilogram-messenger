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
$templateRoot = Join-Path $PSScriptRoot 'm0967-simple'

Push-Location $workspace
try {
    $dirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
    if ($dirty) { throw 'Refusing to create the simple kit from a dirty worktree.' }
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $revision -cnotmatch '^[0-9a-f]{40}$') { throw 'Cannot resolve clean HEAD.' }
    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = ".tmp\m0967-simple-$($revision.Substring(0, 12))"
    }
    $output = if ([IO.Path]::IsPathRooted($OutputDirectory)) {
        [IO.Path]::GetFullPath($OutputDirectory)
    } else {
        [IO.Path]::GetFullPath((Join-Path $workspace $OutputDirectory))
    }
    if (Test-Path -LiteralPath $output) {
        if (@(Get-ChildItem -LiteralPath $output -Force).Count -ne 0) {
            throw "Output directory is not empty and will not be overwritten: $output"
        }
    } else {
        New-Item -ItemType Directory -Path $output | Out-Null
    }

    $checks = @(
        'verify-kilogram-runtime-mailbox-flow.ps1',
        'verify-kilogram-volunteer-storage-boundary.ps1',
        'verify-kilogram-volunteer-iroh-boundary.ps1',
        'verify-kilogram-volunteer-provider-selection-boundary.ps1',
        'verify-kilogram-volunteer-provider-gossip-boundary.ps1',
        'verify-kilogram-volunteer-replication-boundary.ps1',
        'verify-kilogram-volunteer-retrieval-boundary.ps1',
        'verify-kilogram-m0967-simple-kit-boundary.ps1'
    )
    $boundaries = [Collections.Generic.List[string]]::new()
    foreach ($check in $checks) {
        $lines = @(& (Join-Path $PSScriptRoot $check) 2>&1)
        foreach ($line in $lines) { $boundaries.Add([string]$line) }
    }

    & cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-ticket-store
    if ($LASTEXITCODE -ne 0) { throw 'M0.9.67 simple debug binaries failed to build.' }
    if (& git status --porcelain --untracked-files=normal) { throw 'Worktree changed during simple kit build.' }

    foreach ($directory in @('1', '2', '3')) {
        New-Item -ItemType Directory -Path (Join-Path $output $directory) | Out-Null
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'target\debug\kilogram-cli.exe') -Destination (Join-Path $output 'kilogram-cli.exe')
    Copy-Item -LiteralPath (Join-Path $workspace 'target\debug\kilogram-ticket-store.exe') -Destination (Join-Path $output 'kilogram-ticket-store.exe')
    Copy-Item -LiteralPath (Join-Path $templateRoot 'common.ps1') -Destination (Join-Path $output 'common.ps1')
    foreach ($role in @('1', '2', '3')) {
        foreach ($scriptFile in Get-ChildItem -LiteralPath (Join-Path $templateRoot $role) -File -Filter '*.ps1') {
            Copy-Item -LiteralPath $scriptFile.FullName -Destination (Join-Path $output $role)
        }
    }
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'verify-kilogram-volunteer-field-evidence.ps1') -Destination (Join-Path $output 'verify-kilogram-volunteer-field-evidence.ps1')
    [IO.File]::WriteAllLines((Join-Path $output 'BOUNDARIES.log'), $boundaries, [Text.UTF8Encoding]::new($false))

    $artifacts = @()
    foreach ($name in @('kilogram-cli.exe', 'kilogram-ticket-store.exe')) {
        $item = Get-Item -LiteralPath (Join-Path $output $name)
        $artifacts += [ordered]@{
            file = $name
            sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            bytes = [UInt64]$item.Length
        }
    }
    $buildInfo = [ordered]@{
        schema = 1
        source_revision = $revision
        source_dirty = $false
        profile = 'debug'
        cargo_jobs = $cargoJobsResolved
        created_utc = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
        archive = $false
        network_executed = $false
        artifacts = $artifacts
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'BUILD-INFO.json'),
        (($buildInfo | ConvertTo-Json -Depth 5) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
    $readme = @(
        'M0.9.67 - SIMPLE THREE-FOLDER TEST',
        '',
        'Do not copy state or edit IDs. Scripts coordinate through 1\shared.',
        'Private keys and state are created under %LOCALAPPDATA%\Kilogram\M0967.',
        '',
        'RUN ORDER:',
        '1. Desktop: run 1\01_PREPARE_ALICE.ps1 and leave it waiting.',
        '2. Laptop: run 3\01_PREPARE_BOB.ps1; wait until both preparation windows report success.',
        '3. Desktop: run 2\01_START_PROVIDERS.ps1 and leave that window open.',
        '4. Desktop: run 1\02_SEND_ALICE.ps1; wait for success.',
        '5. Laptop: run 3\02_RECEIVE_BOB.ps1; wait for success and Yandex synchronization.',
        '6. Desktop: run 1\03_VERIFY.ps1. It stops providers and verifies all evidence.',
        '',
        'RECOVERY ONLY: 2\02_RESTART_PROVIDERS_AFTER_FIX.ps1 reuses existing provider identities after a binary fix.',
        'RECOVERY ONLY: 3\06_RESUME_BOB_AFTER_POST_COMMIT_FIX.ps1 resumes the exact durable Bob commit after the fixed CLI arrives.',
        'A repeated 1\02_SEND_ALICE.ps1 resumes an exact incomplete durable queue item and never creates a duplicate.',
        '',
        'Always start scripts with:',
        'powershell -NoProfile -ExecutionPolicy Bypass -File .\SCRIPT_NAME.ps1',
        '',
        'The first network run may request Windows Firewall permission once for the stable kilogram-cli.exe path.'
    )
    [IO.File]::WriteAllLines((Join-Path $output 'README-RU.txt'), $readme, [Text.UTF8Encoding]::new($true))

    Write-Output "simple_kit_directory=$output"
    Write-Output "source_revision=$revision"
    Write-Output 'folders=1,2,3'
    Write-Output 'operator_launches=6'
    Write-Output 'archive_created=false'
    Write-Output 'network_executed=false'
    Write-Output 'status=kilogram-m0967-simple-kit-created'
}
finally { Pop-Location }
