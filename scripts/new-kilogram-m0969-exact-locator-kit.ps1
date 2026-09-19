[CmdletBinding()]
param(
    [string] $OutputDirectory,
    [ValidateRange(1, 64)] [int] $CargoJobs = 2,
    [ValidateSet('M0.9.69', 'M0.9.72')] [string] $Milestone = 'M0.9.69'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
$cargoJobsResolved = Set-KilogramCargoResourcePolicy -RequestedJobs $CargoJobs
$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$templateRoot = Join-Path $PSScriptRoot 'm0969-exact'
$noHttpsCompatibility = $Milestone -ceq 'M0.9.72'

Push-Location $workspace
try {
    $dirty = & git status --porcelain --untracked-files=normal
    if ($LASTEXITCODE -ne 0) { throw 'git status failed.' }
    if ($dirty) { throw "Refusing to create the $Milestone kit from a dirty worktree." }
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $revision -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Cannot resolve clean HEAD.'
    }
    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $kitName = if ($noHttpsCompatibility) { 'm0972-no-https' } else { 'm0969-exact' }
        $OutputDirectory = ".tmp\$kitName-$($revision.Substring(0, 12))"
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
        'verify-kilogram-m0969-exact-locator-kit-boundary.ps1'
    )
    if ($noHttpsCompatibility) {
        $checks += @(
            'verify-kilogram-mailbox-https-retirement-boundary.ps1',
            'verify-kilogram-m0972-no-https-kit-boundary.ps1'
        )
    }
    $boundaries = [Collections.Generic.List[string]]::new()
    foreach ($check in $checks) {
        $lines = @(& (Join-Path $PSScriptRoot $check) 2>&1)
        foreach ($line in $lines) { $boundaries.Add([string]$line) }
    }

    if ($noHttpsCompatibility) {
        & cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli
    } else {
        & cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-ticket-store
    }
    if ($LASTEXITCODE -ne 0) { throw "$Milestone debug binaries failed to build." }
    if (& git status --porcelain --untracked-files=normal) {
        throw "Worktree changed during $Milestone kit build."
    }

    foreach ($directory in @('1', '2', '3')) {
        New-Item -ItemType Directory -Path (Join-Path $output $directory) | Out-Null
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'target\debug\kilogram-cli.exe') `
        -Destination (Join-Path $output 'kilogram-cli.exe')
    if (-not $noHttpsCompatibility) {
        Copy-Item -LiteralPath (Join-Path $workspace 'target\debug\kilogram-ticket-store.exe') `
            -Destination (Join-Path $output 'kilogram-ticket-store.exe')
    }
    Copy-Item -LiteralPath (Join-Path $templateRoot 'common.ps1') `
        -Destination (Join-Path $output 'common.ps1')
    foreach ($role in @('1', '2', '3')) {
        foreach ($scriptFile in Get-ChildItem -LiteralPath (Join-Path $templateRoot $role) -File -Filter '*.ps1') {
            Copy-Item -LiteralPath $scriptFile.FullName -Destination (Join-Path $output $role)
        }
    }
    Copy-Item `
        -LiteralPath (Join-Path $PSScriptRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
        -Destination (Join-Path $output 'verify-kilogram-m0969-exact-locator-evidence.ps1')
    if ($noHttpsCompatibility) {
        Copy-Item `
            -LiteralPath (Join-Path $PSScriptRoot 'verify-kilogram-m0972-no-https-evidence.ps1') `
            -Destination (Join-Path $output 'verify-kilogram-m0972-no-https-evidence.ps1')
    }
    [IO.File]::WriteAllLines(
        (Join-Path $output 'BOUNDARIES.log'),
        $boundaries,
        [Text.UTF8Encoding]::new($false)
    )

    $artifactNames = @(
        'kilogram-cli.exe',
        'common.ps1',
        'verify-kilogram-m0969-exact-locator-evidence.ps1',
        'BOUNDARIES.log',
        '1/01_PREPARE_ALICE.ps1',
        '1/02_SEND_ALICE.ps1',
        '1/03_VERIFY.ps1',
        '2/01_START_PROVIDERS.ps1',
        '3/01_PREPARE_BOB.ps1',
        '3/02_RECEIVE_BOB.ps1'
    )
    if ($noHttpsCompatibility) {
        $artifactNames += 'verify-kilogram-m0972-no-https-evidence.ps1'
    } else {
        $artifactNames += 'kilogram-ticket-store.exe'
    }
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
        milestone = $Milestone
        source_revision = $revision
        source_dirty = $false
        profile = 'debug'
        cargo_jobs = $cargoJobsResolved
        created_utc = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
        archive = $false
        network_executed = $false
        field_route_policy = 'auto'
        field_relay_url = 'https://aps1-1.relay.n0.iroh.link./'
        https_fixture_included = (-not $noHttpsCompatibility)
        compatibility_endpoint = if ($noHttpsCompatibility) { 'http://127.0.0.1:18787' } else { 'http://127.0.0.1:8787' }
        artifacts = $artifacts
    }
    [IO.File]::WriteAllText(
        (Join-Path $output 'BUILD-INFO.json'),
        (($buildInfo | ConvertTo-Json -Depth 5) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )

    $readme = if ($noHttpsCompatibility) { @(
        'M0.9.72 - CLEAN VOLUNTEER DELIVERY WITH NO HTTPS MAILBOX FIXTURE',
        '',
        'Folders synchronize through 1\shared. Live Redb state stays under %LOCALAPPDATA%.',
        'Use one fresh generated kit for one run; clean evidence intentionally cannot be resumed.',
        '',
        'RUN ORDER:',
        '1. Desktop: run 1\01_PREPARE_ALICE.ps1 and leave it waiting.',
        '2. Desktop: run 2\01_START_PROVIDERS.ps1 and leave that window open.',
        '3. Laptop: run 3\01_PREPARE_BOB.ps1; wait until both preparation windows report success.',
        '4. Desktop: run 1\02_SEND_ALICE.ps1; wait for success.',
        '5. Laptop: run 3\02_RECEIVE_BOB.ps1; wait for success and Yandex synchronization.',
        '6. Desktop: run 1\03_VERIFY.ps1. It stops providers and verifies all evidence.',
        '',
        'Always start scripts with:',
        'powershell -NoProfile -ExecutionPolicy Bypass -File .\SCRIPT_NAME.ps1',
        '',
        'The kit contains no kilogram-ticket-store.exe and starts no HTTPS compatibility fixture.',
        'The legacy capability tuple points at an unreachable loopback endpoint and is never used.',
        'Success requires http_put=not-attempted, exact volunteer durability with two signed receipts,',
        'Alice offline before Bob retrieval, two commit-before-delete results, and no legacy fallback.',
        '',
        'This controlled field run pins auto-mode relay fallback to aps1; direct upgrade remains allowed.',
        'The generator creates no ZIP, uses no release build, and starts no network process.'
    ) } else { @(
        'M0.9.69 - CLEAN AUTHENTICATED EXACT-LOCATOR FIELD TEST',
        '',
        'Folders synchronize through 1\shared. Live Redb state stays under %LOCALAPPDATA%.',
        'Use one fresh generated kit for one run; clean evidence intentionally cannot be resumed.',
        '',
        'RUN ORDER:',
        '1. Desktop: run 1\01_PREPARE_ALICE.ps1 and leave it waiting.',
        '2. Desktop: run 2\01_START_PROVIDERS.ps1 and leave that window open.',
        '3. Laptop: run 3\01_PREPARE_BOB.ps1; wait until both preparation windows report success.',
        '4. Desktop: run 1\02_SEND_ALICE.ps1; wait for success.',
        '5. Laptop: run 3\02_RECEIVE_BOB.ps1; wait for success and Yandex synchronization.',
        '6. Desktop: run 1\03_VERIFY.ps1. It stops providers and verifies all evidence.',
        '',
        'Always start scripts with:',
        'powershell -NoProfile -ExecutionPolicy Bypass -File .\SCRIPT_NAME.ps1',
        '',
        'Success requires: providers before mailbox activation, one Device-signed exact commitment,',
        'fresh live endpoint tickets, the same commitment at sender and recipient, exact 2/2 lookup,',
        'two signed receipts,',
        'Alice offline before Bob retrieval, two commit-before-delete results, and no legacy fallback.',
        '',
        'The compatibility HTTP store is loopback-only on Alice and is stopped before Bob receives.',
        'This controlled field run pins auto-mode relay fallback to aps1; direct upgrade remains allowed.',
        'The generator creates no ZIP, uses no release build, and starts no network process.'
    ) }
    [IO.File]::WriteAllLines(
        (Join-Path $output 'README-RU.txt'),
        $readme,
        [Text.UTF8Encoding]::new($true)
    )

    Write-Output "exact_locator_kit_directory=$output"
    Write-Output "source_revision=$revision"
    Write-Output 'folders=1,2,3'
    Write-Output 'operator_launches=6'
    Write-Output 'profile=debug'
    Write-Output 'kit_script_integrity=sha256-length'
    Write-Output 'archive_created=false'
    Write-Output 'network_executed=false'
    if ($noHttpsCompatibility) {
        Write-Output 'https_fixture_included=false'
        Write-Output 'status=kilogram-m0972-no-https-kit-created'
    } else {
        Write-Output 'status=kilogram-m0969-exact-locator-kit-created'
    }
}
finally { Pop-Location }
