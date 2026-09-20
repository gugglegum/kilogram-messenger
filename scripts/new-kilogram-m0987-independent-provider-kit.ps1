[CmdletBinding()]
param(
    [string] $OutputDirectory,
    [ValidateRange(1, 64)] [int] $CargoJobs = 2
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$legacyTemplate = Join-Path $PSScriptRoot 'm0969-exact'
$providerTemplate = Join-Path $PSScriptRoot 'm0987-independent'

Push-Location $workspace
try {
    $dirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
    if ($dirty) { throw 'Refusing to create the M0.9.87 kit from a dirty worktree.' }
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $revision -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Cannot resolve clean HEAD.'
    }
    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = ".tmp\m0987-independent-providers-$($revision.Substring(0, 12))"
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
        'verify-kilogram-volunteer-replica-locator-boundary.ps1',
        'verify-kilogram-m0969-exact-locator-kit-boundary.ps1',
        'verify-kilogram-mailbox-https-retirement-boundary.ps1',
        'verify-kilogram-m0972-no-https-kit-boundary.ps1',
        'verify-kilogram-runtime-cooperative-scheduling.ps1',
        'verify-kilogram-m0973-no-https-kit-boundary.ps1',
        'verify-kilogram-runtime-mailbox-replication-recovery.ps1',
        'verify-kilogram-m0974-no-https-kit-boundary.ps1',
        'verify-kilogram-service-free-mailbox-capability-v2.ps1',
        'verify-kilogram-m0976-service-free-v2-kit-boundary.ps1',
        'verify-kilogram-m0987-independent-provider-kit-boundary.ps1'
    )
    $boundaries = [Collections.Generic.List[string]]::new()
    foreach ($check in $checks) {
        $lines = @(& (Join-Path $PSScriptRoot $check) 2>&1)
        foreach ($line in $lines) { $boundaries.Add([string]$line) }
    }

    & cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli
    if ($LASTEXITCODE -ne 0) { throw 'M0.9.87 debug CLI failed to build.' }
    if (& git status --porcelain --untracked-files=normal) {
        throw 'Worktree changed during M0.9.87 kit build.'
    }

    foreach ($directory in @('1', '2', '3', '4')) {
        New-Item -ItemType Directory -Path (Join-Path $output $directory) | Out-Null
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'target\debug\kilogram-cli.exe') `
        -Destination (Join-Path $output 'kilogram-cli.exe')
    Copy-Item -LiteralPath (Join-Path $legacyTemplate 'common.ps1') `
        -Destination (Join-Path $output 'common.ps1')
    foreach ($scriptFile in Get-ChildItem -LiteralPath (Join-Path $legacyTemplate '1') -File -Filter '*.ps1') {
        Copy-Item -LiteralPath $scriptFile.FullName -Destination (Join-Path $output '1')
    }
    Copy-Item -LiteralPath (Join-Path $providerTemplate '2\01_START_PROVIDER1.ps1') `
        -Destination (Join-Path $output '2\01_START_PROVIDER1.ps1')
    Copy-Item -LiteralPath (Join-Path $providerTemplate '3\01_START_PROVIDER2.ps1') `
        -Destination (Join-Path $output '3\01_START_PROVIDER2.ps1')
    foreach ($scriptFile in Get-ChildItem -LiteralPath (Join-Path $legacyTemplate '3') -File -Filter '*.ps1') {
        Copy-Item -LiteralPath $scriptFile.FullName -Destination (Join-Path $output '4')
    }
    foreach ($verifier in @(
        'verify-kilogram-m0969-exact-locator-evidence.ps1',
        'verify-kilogram-m0987-independent-provider-evidence.ps1'
    )) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $verifier) -Destination (Join-Path $output $verifier)
    }
    [IO.File]::WriteAllLines(
        (Join-Path $output 'BOUNDARIES.log'),
        $boundaries,
        [Text.UTF8Encoding]::new($false)
    )
    Copy-Item -LiteralPath (Join-Path $workspace 'docs\M0.9.87-INDEPENDENT-PROVIDER-FIELD-TEST-RU.md') `
        -Destination (Join-Path $output 'README-RU.md')

    $artifactNames = @(
        'kilogram-cli.exe',
        'common.ps1',
        'verify-kilogram-m0969-exact-locator-evidence.ps1',
        'verify-kilogram-m0987-independent-provider-evidence.ps1',
        'BOUNDARIES.log',
        'README-RU.md',
        '1/01_PREPARE_ALICE.ps1',
        '1/02_SEND_ALICE.ps1',
        '1/03_VERIFY.ps1',
        '2/01_START_PROVIDER1.ps1',
        '3/01_START_PROVIDER2.ps1',
        '4/01_PREPARE_BOB.ps1',
        '4/02_RECEIVE_BOB.ps1'
    )
    $artifacts = @()
    foreach ($name in $artifactNames) {
        $item = Get-Item -LiteralPath (Join-Path $output $name.Replace('/', '\'))
        $artifacts += [ordered]@{
            file = $name
            sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            bytes = [UInt64]$item.Length
        }
    }
    $buildInfo = [ordered]@{
        schema = 1
        milestone = 'M0.9.87'
        source_revision = $revision
        source_dirty = $false
        profile = 'debug'
        cargo_jobs = $cargoJobsResolved
        created_utc = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
        archive = $false
        network_executed = $false
        field_route_policy = 'auto'
        field_relay_url = 'https://aps1-1.relay.n0.iroh.link./'
        https_fixture_included = $false
        mailbox_capability_format = 'v2-exact-volunteer'
        central_service_descriptor_present = $false
        field_topology = [ordered]@{
            alice_folder = '1'
            provider1_folder = '2'
            provider2_folder = '3'
            bob_folder = '4'
            minimum_physical_hosts = 3
            ideal_physical_hosts = 4
        }
        provider_independence_claim = 'controlled-self-attestation-not-protocol-proof'
        artifacts = $artifacts
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'BUILD-INFO.json'),
        (($buildInfo | ConvertTo-Json -Depth 6) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )

    Write-Output "independent_provider_kit_directory=$output"
    Write-Output "source_revision=$revision"
    Write-Output 'folders=1,2,3,4'
    Write-Output 'operator_launches=7'
    Write-Output 'minimum_physical_hosts=3'
    Write-Output 'ideal_physical_hosts=4'
    Write-Output 'profile=debug'
    Write-Output 'archive_created=false'
    Write-Output 'network_executed=false'
    Write-Output 'https_fixture_included=false'
    Write-Output 'central_service_descriptor_present=false'
    Write-Output 'provider_independence_claim=controlled-self-attestation-not-protocol-proof'
    Write-Output 'status=kilogram-m0987-independent-provider-kit-created'
}
finally { Pop-Location }
