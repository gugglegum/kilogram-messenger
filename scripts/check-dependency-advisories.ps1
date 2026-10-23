[CmdletBinding()]
param(
    [string]$LockfilePath = 'Cargo.lock',
    [string]$ManifestPath = 'Cargo.toml',
    [string]$DatabasePath = '.tmp/transport-security-a2/advisory-db',
    [string]$DatabaseRevision,
    [string]$AuditExecutable = 'cargo-audit',
    [string]$OutputPath = '.tmp/dependency-advisory-gate.json'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$policy = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'dependency-advisory-policy.json') -Raw | ConvertFrom-Json
if (-not $DatabaseRevision) { $DatabaseRevision = $policy.reviewed_database_revision }
$summary = [ordered]@{
    format_version = 1
    checked_at_utc = [DateTime]::UtcNow.ToString('o')
    status = 'FAIL'
    source_advisory = $policy.source_advisory
    source_version_gate = 'FAIL'
    database_revision = $DatabaseRevision
    database_commit_date = $null
    source_advisory_in_rustsec = $null
    cargo_audit_version = $null
    cargo_audit_exit = $null
    vulnerability_count = $null
    warning_count = $null
    registry_checksums = @()
    findings = @()
    errors = @()
    # Yank status is a mutable registry property. Replays use the exact RustSec
    # git snapshot and lockfile; no advisory or warning is ignored.
    yanked_check = 'separate from this reproducible advisory snapshot'
}

function Resolve-InputPath([string]$Path) {
    if ([IO.Path]::IsPathRooted($Path)) { return (Resolve-Path -LiteralPath $Path).Path }
    return (Resolve-Path -LiteralPath (Join-Path $workspace $Path)).Path
}
function Lock-Field([string]$Block, [string]$Name) {
    return [regex]::Match($Block, ('(?m)^' + [regex]::Escape($Name) + ' = "([^"]+)"')).Groups[1].Value
}

try {
    $lock = Resolve-InputPath $LockfilePath
    $manifest = Resolve-InputPath $ManifestPath
    $summary.lockfile_sha256 = (Get-FileHash -LiteralPath $lock -Algorithm SHA256).Hash.ToLowerInvariant()
    $summary.manifest_sha256 = (Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash.ToLowerInvariant()
    $lockText = Get-Content -LiteralPath $lock -Raw
    $packages = @([regex]::Matches($lockText, '(?ms)^\[\[package\]\]\r?\n(.*?)(?=^\[\[package\]\]|\z)'))
    $manifestSection = [regex]::Match((Get-Content -LiteralPath $manifest -Raw), '(?ms)^\[workspace.dependencies\]\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
    $floor = [regex]::Match($manifestSection, '(?m)^iroh\s*=\s*"[=^~]?(\d+\.\d+\.\d+)"\s*$')
    $sourceErrors = @()
    if (-not $floor.Success -or [version]$floor.Groups[1].Value -lt [version]$policy.iroh_minimum) {
        $sourceErrors += 'Workspace Iroh requirement must have an explicit stable floor >= 1.1.0.'
    }
    $cargoRoot = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    foreach ($name in @('iroh', 'iroh-base', 'iroh-dns', 'iroh-relay')) {
        $lockedPackages = @($packages | Where-Object { (Lock-Field $_.Value 'name') -ceq $name })
        if ($lockedPackages.Count -ne 1) { $sourceErrors += "Expected exactly one locked $name package." }
        foreach ($package in $lockedPackages) {
            $version = Lock-Field $package.Value 'version'
            $source = Lock-Field $package.Value 'source'
            $checksum = Lock-Field $package.Value 'checksum'
            if ($version -notmatch '^\d+\.\d+\.\d+$' -or [version]$version -lt [version]$policy.iroh_minimum) {
                $sourceErrors += "Affected or unreviewed $name version: $version"
            }
            if ($source -cne 'registry+https://github.com/rust-lang/crates.io-index' -or $checksum -notmatch '^[a-f0-9]{64}$') {
                $sourceErrors += "Unreviewed source/checksum for $name $version"
            }
            $archives = @(Get-ChildItem -Path (Join-Path $cargoRoot "registry/cache/*/$name-$version.crate") -File)
            if ($archives.Count -eq 0) { $sourceErrors += "Missing registry archive for $name $version; run cargo fetch --locked." }
            foreach ($archive in $archives) {
                $actual = (Get-FileHash -LiteralPath $archive.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
                $summary.registry_checksums += @{package=$name;version=$version;expected=$checksum;actual=$actual;match=($actual -ceq $checksum)}
                if ($actual -cne $checksum) { $sourceErrors += "Registry checksum mismatch: $name $version" }
            }
        }
    }
    if ($sourceErrors.Count -eq 0) { $summary.source_version_gate = 'PASS' }
    $summary.errors += $sourceErrors

    if ($DatabaseRevision -notmatch '^[a-f0-9]{40}$') { throw 'DatabaseRevision must be an exact git commit, not a branch or tag.' }
    $database = Resolve-InputPath $DatabasePath
    $actualRevision = & git -C $database rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $actualRevision -cne $DatabaseRevision) { throw 'RustSec database HEAD differs from the requested revision.' }
    $dirty = @(& git -C $database status --porcelain --untracked-files=all)
    if ($LASTEXITCODE -ne 0 -or $dirty.Count -ne 0) { throw 'RustSec database must be a clean git checkout.' }
    $summary.database_commit_date = & git -C $database log -1 --format=%cI
    $summary.source_advisory_in_rustsec = @(Get-ChildItem -LiteralPath (Join-Path $database 'crates') -File -Recurse -Filter '*.md' |
        Select-String -SimpleMatch 'GHSA-7cq4-mhxw-xw78' -List).Count -gt 0
    $audit = (Get-Command $AuditExecutable -ErrorAction Stop).Source
    $summary.cargo_audit_version = (& $audit --version) -join ''
    if ($LASTEXITCODE -ne 0 -or $summary.cargo_audit_version -cne "cargo-audit $($policy.cargo_audit_version)") { throw 'Install the exact cargo-audit version from dependency-advisory-policy.json.' }
    $auditText = @(& $audit audit --db $database --no-fetch --no-yanked --deny warnings --file $lock --json)
    $summary.cargo_audit_exit = $LASTEXITCODE
    $report = ($auditText -join "`n") | ConvertFrom-Json
    $summary.vulnerability_count = [int]$report.vulnerabilities.count
    if ($report.vulnerabilities.count -gt 0) {
        foreach ($finding in @($report.vulnerabilities.list)) {
            $summary.findings += @{kind='vulnerability';id=$finding.advisory.id;package=$finding.package.name;version=$finding.package.version;patched=@($finding.versions.patched);url="https://rustsec.org/advisories/$($finding.advisory.id).html"}
        }
    }
    foreach ($warning in $report.warnings.PSObject.Properties) {
        foreach ($finding in @($warning.Value)) {
            $summary.findings += @{kind=$warning.Name;id=$finding.advisory.id;package=$finding.package.name;version=$finding.package.version;patched=@($finding.versions.patched);url="https://rustsec.org/advisories/$($finding.advisory.id).html"}
        }
    }
    $summary.warning_count = @($summary.findings | Where-Object { $_.kind -cne 'vulnerability' }).Count
    if ($summary.cargo_audit_exit -ne 0 -or $summary.findings.Count -ne 0) {
        $summary.errors += 'RustSec findings remain; inspect findings and classify reachability. No ignore list is used.'
    }
    if ($summary.errors.Count -eq 0) { $summary.status = 'PASS' }
} catch { $summary.errors += $_.Exception.Message }

$output = if ([IO.Path]::IsPathRooted($OutputPath)) { $OutputPath } else { Join-Path $workspace $OutputPath }
[void](New-Item -ItemType Directory -Path (Split-Path -Parent $output) -Force)
$json = $summary | ConvertTo-Json -Depth 10
$json | Set-Content -LiteralPath $output -Encoding utf8
Write-Output $json
if ($summary.status -cne 'PASS') { exit 1 }
exit 0
