[CmdletBinding()]
param(
    [ValidateSet('Preflight', 'Smoke', 'Sustained')]
    [string]$Mode = 'Preflight',
    [string]$NightlyToolchain = 'nightly',
    [int]$SmokeSeconds = 60,
    [switch]$GenerateCoverage,
    [switch]$AcknowledgeSustainedResourceUse
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json'
$toolchainManifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-TOOLCHAIN.json'
$preparePath = Join-Path $PSScriptRoot 'prepare-kilogram-canonical-sync-v2-fuzz-corpus.ps1'
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$toolchainManifest = Get-Content -LiteralPath $toolchainManifestPath -Raw | ConvertFrom-Json
$isWindowsHost = $env:OS -ceq 'Windows_NT'
$target = [string]$manifest.target
$corpusDirectory = Join-Path $workspace ".tmp\fuzz\corpus\$target"
$artifactDirectory = Join-Path $workspace ".tmp\fuzz\artifacts\$target"
$evidenceDirectory = Join-Path $workspace '.tmp\fuzz\evidence'
$campaignLockDirectory = Join-Path $workspace '.tmp\fuzz'
$campaignLockPath = Join-Path $campaignLockDirectory 'canonical_sync_v2_decoders.campaign.lock'
$campaignLockStream = $null
[System.IO.Directory]::CreateDirectory($campaignLockDirectory) | Out-Null
try {
    $campaignLockStream = [System.IO.File]::Open(
        $campaignLockPath,
        [System.IO.FileMode]::OpenOrCreate,
        [System.IO.FileAccess]::ReadWrite,
        [System.IO.FileShare]::None
    )
}
catch [System.IO.IOException] {
    throw 'Another canonical sync v2 campaign is already active.'
}
Write-Output 'campaign_lock_acquired=true'

try {

$seedOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass `
    -File $preparePath -OutputDirectory $corpusDirectory -ResetOutputDirectory 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_sync_v2_fuzz_seed_corpus=verified' -notin @($seedOutput | ForEach-Object { [string]$_ })) {
    throw "Fuzz seed materialization failed.`n$($seedOutput -join [Environment]::NewLine)"
}
$seedOutput | Write-Output

$previousErrorActionPreference = $ErrorActionPreference
try {
    $ErrorActionPreference = 'Continue'
    $installedToolchains = @(& rustup toolchain list 2>&1 | ForEach-Object { [string]$_ })
    $rustupExitCode = $LASTEXITCODE
}
finally {
    $ErrorActionPreference = $previousErrorActionPreference
}
if ($rustupExitCode -ne 0) {
    throw "rustup toolchain list failed with exit code $rustupExitCode"
}
$toolchainInstalled = @($installedToolchains | Where-Object {
    $_ -match ('^' + [regex]::Escape($NightlyToolchain) + '(?:-|\s)')
}).Count -gt 0
$rustcRelease = ''
$rustcCommitHash = ''
$rustcCommitDate = ''
$llvmVersion = ''
if ($toolchainInstalled) {
    $rustcDetails = @(& rustc "+$NightlyToolchain" -vV 2>&1 | ForEach-Object { [string]$_ })
    if ($LASTEXITCODE -ne 0) {
        throw "rustc +$NightlyToolchain -vV failed with exit code $LASTEXITCODE"
    }
    foreach ($line in $rustcDetails) {
        if ($line -match '^release:\s*(.+)$') { $rustcRelease = $Matches[1].Trim() }
        if ($line -match '^commit-hash:\s*(.+)$') { $rustcCommitHash = $Matches[1].Trim() }
        if ($line -match '^commit-date:\s*(.+)$') { $rustcCommitDate = $Matches[1].Trim() }
        if ($line -match '^LLVM version:\s*(.+)$') { $llvmVersion = $Matches[1].Trim() }
    }
}
$cargoFuzz = Get-Command cargo-fuzz -ErrorAction SilentlyContinue
$cargoFuzzInstalled = $null -ne $cargoFuzz
$cargoFuzzVersion = ''
if ($cargoFuzzInstalled) {
    $cargoFuzzVersionOutput = [string](& $cargoFuzz.Source --version 2>&1)
    if ($LASTEXITCODE -ne 0 -or $cargoFuzzVersionOutput -cnotmatch '^cargo-fuzz\s+(.+)$') {
        throw 'Could not resolve the installed cargo-fuzz version.'
    }
    $cargoFuzzVersion = $Matches[1].Trim()
}
$asanInstalled = $false
$asanRuntimePath = $null
$asanImportLibraryPath = $null
if ($isWindowsHost) {
    $asanRuntimeCandidates = @(
        Get-ChildItem -Path @(
            'C:\Program Files\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx64\x64\clang_rt.asan_dynamic-x86_64.dll',
            'C:\Program Files\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx86\x64\clang_rt.asan_dynamic-x86_64.dll',
            'C:\Program Files (x86)\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx64\x64\clang_rt.asan_dynamic-x86_64.dll',
            'C:\Program Files (x86)\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx86\x64\clang_rt.asan_dynamic-x86_64.dll'
        ) -File -ErrorAction SilentlyContinue
    )
    $asanImportLibraryCandidates = @(
        Get-ChildItem -Path @(
            'C:\Program Files\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\lib\x64\clang_rt.asan_dynamic_runtime_thunk-x86_64.lib',
            'C:\Program Files (x86)\Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\lib\x64\clang_rt.asan_dynamic_runtime_thunk-x86_64.lib'
        ) -File -ErrorAction SilentlyContinue
    )
    if ($asanRuntimeCandidates.Count -gt 0 -and $asanImportLibraryCandidates.Count -gt 0) {
        $asanRuntimePath = [string]$asanRuntimeCandidates[0].FullName
        $asanImportLibraryPath = [string]$asanImportLibraryCandidates[0].FullName
        $asanInstalled = $true
    }
}

$toolchainContractSatisfied = (
    $toolchainInstalled -and
    $rustcRelease -ceq [string]$toolchainManifest.toolchain.rustc_release -and
    $rustcCommitHash -ceq [string]$toolchainManifest.toolchain.rustc_commit_hash -and
    $rustcCommitDate -ceq [string]$toolchainManifest.toolchain.rustc_commit_date -and
    $llvmVersion -ceq [string]$toolchainManifest.toolchain.llvm_version
)
$cargoFuzzContractSatisfied = (
    $cargoFuzzInstalled -and
    $cargoFuzzVersion -ceq [string]$toolchainManifest.toolchain.cargo_fuzz_version
)

Write-Output "fuzz_toolchain_installed=$($toolchainInstalled.ToString().ToLowerInvariant())"
Write-Output "cargo_fuzz_installed=$($cargoFuzzInstalled.ToString().ToLowerInvariant())"
Write-Output "windows_msvc_asan_detected=$($asanInstalled.ToString().ToLowerInvariant())"
Write-Output "fuzz_rustc_release=$rustcRelease"
Write-Output "fuzz_rustc_commit_hash=$rustcCommitHash"
Write-Output "fuzz_rustc_commit_date=$rustcCommitDate"
Write-Output "fuzz_llvm_version=$llvmVersion"
Write-Output "cargo_fuzz_version=$cargoFuzzVersion"
Write-Output "fuzz_toolchain_contract_satisfied=$($toolchainContractSatisfied.ToString().ToLowerInvariant())"
Write-Output "cargo_fuzz_contract_satisfied=$($cargoFuzzContractSatisfied.ToString().ToLowerInvariant())"
if (-not $toolchainInstalled) {
    throw "Required Rust toolchain '$NightlyToolchain' is not installed."
}
if (-not $cargoFuzzInstalled) {
    throw 'cargo-fuzz is not installed.'
}
if (-not $toolchainContractSatisfied) {
    throw 'The installed Rust nightly does not satisfy the pinned fuzz toolchain contract.'
}
if (-not $cargoFuzzContractSatisfied) {
    throw 'The installed cargo-fuzz does not satisfy the pinned driver contract.'
}
if ($isWindowsHost -and -not $asanInstalled) {
    throw 'MSVC AddressSanitizer is not installed; use a prepared Windows host or Linux.'
}
if ($Mode -ceq 'Preflight') {
    Write-Output 'canonical_sync_v2_fuzz_preflight=complete'
    Write-Output 'coverage_guided_campaign_executed=false'
    return
}
if ($Mode -ceq 'Sustained' -and -not $AcknowledgeSustainedResourceUse) {
    throw 'Sustained mode requires -AcknowledgeSustainedResourceUse.'
}
if ($Mode -ceq 'Sustained' -and -not $GenerateCoverage) {
    throw 'Sustained mode requires -GenerateCoverage.'
}
if ($SmokeSeconds -lt 1 -or $SmokeSeconds -gt 900) {
    throw 'SmokeSeconds must be between 1 and 900.'
}

$sourceRevisionBefore = [string](& git rev-parse HEAD)
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($sourceRevisionBefore)) {
    throw 'Could not resolve the source revision before the fuzz campaign.'
}
$sourceRevisionBefore = $sourceRevisionBefore.Trim()
$sourceStatusBefore = @(& git status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the source worktree before the fuzz campaign.'
}
$sourceWorktreeCleanBefore = $sourceStatusBefore.Count -eq 0
Write-Output "source_revision=$sourceRevisionBefore"
Write-Output "source_worktree_clean=$($sourceWorktreeCleanBefore.ToString().ToLowerInvariant())"
if ($Mode -ceq 'Sustained' -and -not $sourceWorktreeCleanBefore) {
    throw 'Sustained mode requires a clean committed source worktree.'
}

$durationSeconds = if ($Mode -ceq 'Sustained') {
    [int]$manifest.campaign_contract.minimum_wall_clock_seconds
}
else {
    $SmokeSeconds
}
[System.IO.Directory]::CreateDirectory($artifactDirectory) | Out-Null
[System.IO.Directory]::CreateDirectory($evidenceDirectory) | Out-Null
$artifactDirectoryInfo = Get-Item -LiteralPath $artifactDirectory -Force
if (($artifactDirectoryInfo.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
    @(Get-ChildItem -LiteralPath $artifactDirectory -Directory -Force).Count -ne 0) {
    throw 'Refusing to reset an artifact directory with a reparse point or subdirectories.'
}
Get-ChildItem -LiteralPath $artifactDirectory -File -Force | Remove-Item -Force
$runId = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
$logPath = Join-Path $evidenceDirectory "$runId-$target.log"
$recordPath = Join-Path $evidenceDirectory "$runId-$target.json"
$started = [DateTime]::UtcNow
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$previousJobs = $env:CARGO_BUILD_JOBS
$previousPath = $env:Path
$previousPriority = $null
$runExitCode = -1
$currentProcess = [System.Diagnostics.Process]::GetCurrentProcess()
try {
    $previousPriority = $currentProcess.PriorityClass
    $currentProcess.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::BelowNormal
}
catch {
    Write-Output "priority_adjustment_warning=$($_.Exception.Message)"
}

try {
    $env:CARGO_BUILD_JOBS = '1'
    if ($isWindowsHost) {
        $env:Path = "$(Split-Path -Parent $asanRuntimePath);$previousPath"
    }
    Push-Location $workspace
    try {
        $cargoArguments = @(
            "+$NightlyToolchain", 'fuzz', 'run', '--jobs', '1',
            $target, $corpusDirectory,
            '--', "-max_total_time=$durationSeconds",
            "-max_len=$([int]$manifest.campaign_contract.maximum_input_bytes)",
            "-timeout=$([int]$manifest.campaign_contract.per_input_timeout_seconds)",
            "-artifact_prefix=$artifactDirectory\"
        )
        # Windows PowerShell 5.1 wraps ordinary native stderr (including
        # Cargo progress) in non-terminating NativeCommandError records. Keep
        # streaming both channels, but decide success only from the native
        # process exit code.
        $nativeErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            & cargo @cargoArguments 2>&1 |
                Tee-Object -FilePath $logPath |
                Out-Null
            $runExitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $nativeErrorActionPreference
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    $stopwatch.Stop()
    $env:CARGO_BUILD_JOBS = $previousJobs
    $env:Path = $previousPath
    if ($null -ne $previousPriority) {
        try { $currentProcess.PriorityClass = $previousPriority } catch { }
    }
}

$coverageGenerated = $false
if ($runExitCode -eq 0 -and $GenerateCoverage) {
    Push-Location $workspace
    try {
        if ($isWindowsHost) {
            $env:Path = "$(Split-Path -Parent $asanRuntimePath);$previousPath"
        }
        $coverageErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            & cargo "+$NightlyToolchain" fuzz coverage $target $corpusDirectory 2>&1 |
                Tee-Object -FilePath (Join-Path $evidenceDirectory "$runId-$target-coverage.log") |
                Out-Null
            $coverageExitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $coverageErrorActionPreference
        }
        if ($coverageExitCode -ne 0) {
            throw "cargo fuzz coverage failed with exit code $coverageExitCode"
        }
    }
    finally {
        $env:Path = $previousPath
        Pop-Location
    }
    $coverageProfile = Join-Path $workspace "fuzz\coverage\$target\coverage.profdata"
    $coverageGenerated = Test-Path -LiteralPath $coverageProfile -PathType Leaf
}

$executions = 0L
if (Test-Path -LiteralPath $logPath -PathType Leaf) {
    foreach ($match in (Select-String -LiteralPath $logPath -Pattern '#([0-9]+)' -AllMatches)) {
        foreach ($value in $match.Matches) {
            $candidate = [long]$value.Groups[1].Value
            if ($candidate -gt $executions) { $executions = $candidate }
        }
    }
}
$elapsedSeconds = [long][Math]::Floor($stopwatch.Elapsed.TotalSeconds)
$finalCorpusFiles = @(Get-ChildItem -LiteralPath $corpusDirectory -File -Force).Count
$artifactFiles = @(Get-ChildItem -LiteralPath $artifactDirectory -File -Force).Count
$sourceRevisionAfter = [string](& git rev-parse HEAD)
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($sourceRevisionAfter)) {
    throw 'Could not resolve the source revision after the fuzz campaign.'
}
$sourceRevisionAfter = $sourceRevisionAfter.Trim()
$sourceStatusAfter = @(& git status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the source worktree after the fuzz campaign.'
}
$sourceWorktreeCleanAfter = $sourceStatusAfter.Count -eq 0
$sustainedSatisfied = (
    $Mode -ceq 'Sustained' -and
    $runExitCode -eq 0 -and
    $sourceWorktreeCleanBefore -and
    $sourceWorktreeCleanAfter -and
    $sourceRevisionBefore -ceq $sourceRevisionAfter -and
    $elapsedSeconds -ge [long]$manifest.campaign_contract.minimum_wall_clock_seconds -and
    $executions -ge [long]$manifest.campaign_contract.minimum_executions -and
    $coverageGenerated -and
    $artifactFiles -eq 0
)
$record = [ordered]@{
    schema_version = 1
    source_revision = $sourceRevisionBefore
    source_revision_after = $sourceRevisionAfter
    source_worktree_clean_before = $sourceWorktreeCleanBefore
    source_worktree_clean_after = $sourceWorktreeCleanAfter
    target = $target
    mode = $Mode.ToLowerInvariant()
    rustc_release = $rustcRelease
    rustc_commit_hash = $rustcCommitHash
    rustc_commit_date = $rustcCommitDate
    llvm_version = $llvmVersion
    cargo_fuzz_version = $cargoFuzzVersion
    sanitizer = 'address'
    asan_runtime_file = if ($null -eq $asanRuntimePath) { $null } else { [System.IO.Path]::GetFileName($asanRuntimePath) }
    asan_import_library_file = if ($null -eq $asanImportLibraryPath) { $null } else { [System.IO.Path]::GetFileName($asanImportLibraryPath) }
    started_at_utc = $started.ToString('o')
    elapsed_seconds = $elapsedSeconds
    requested_seconds = $durationSeconds
    executions = $executions
    initial_seed_count = [int]$manifest.seed_count
    final_corpus_files = $finalCorpusFiles
    artifact_files = $artifactFiles
    exit_code = $runExitCode
    workers = 1
    jobs = 1
    coverage_generated = $coverageGenerated
    sustained_gate_satisfied = $sustainedSatisfied
    log_file = [System.IO.Path]::GetFileName($logPath)
}
[System.IO.File]::WriteAllText(
    $recordPath,
    (($record | ConvertTo-Json -Depth 5) + [Environment]::NewLine),
    [System.Text.UTF8Encoding]::new($false)
)
if ($runExitCode -ne 0) {
    throw "cargo fuzz run failed with exit code $runExitCode; evidence=$recordPath"
}
if ($Mode -ceq 'Sustained' -and -not $sustainedSatisfied) {
    throw "Sustained fuzz campaign did not satisfy the closed evidence contract; evidence=$recordPath"
}
Write-Output "fuzz_run_record=$recordPath"
Write-Output "fuzz_executions=$executions"
Write-Output "fuzz_elapsed_seconds=$elapsedSeconds"
Write-Output "coverage_generated=$($coverageGenerated.ToString().ToLowerInvariant())"
Write-Output "sustained_gate_satisfied=$($sustainedSatisfied.ToString().ToLowerInvariant())"
}
finally {
    if ($null -ne $campaignLockStream) {
        $campaignLockStream.Dispose()
    }
}

