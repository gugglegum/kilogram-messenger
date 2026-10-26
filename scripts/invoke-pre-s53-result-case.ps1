[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('C-result-lifetime-01')][string]$Case,
    [ValidateSet('NativeToken')][string]$ExecutionMode = 'NativeToken'
)
$timer = [Diagnostics.Stopwatch]::StartNew()
$startedUtc = [DateTime]::UtcNow.ToString('O')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$evidence = Join-Path $workspace 'ai-docs/tasks/pre-s53-refactor-rb-evidence'
$baseline = '9016b578bed8b8b5e43dd0e2e94edc376804e4f0'
$policy = @{set='C';work=45;whole=60;selector='pre_s53_attempt_result_'}
$expectedNames = @('pre_s53_attempt_result_persists_before_failing_tail','pre_s53_attempt_result_refuses_overwrite_and_skips_tail_on_write_error','pre_s53_attempt_result_success_keeps_separate_whole_outcome')
function Hash([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Write-NewJson([string]$Path, $Value) {
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 30))
    $file = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try { $file.Write($bytes,0,$bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
}
function Replace-Json([string]$Path, $Value) {
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 30))
    $file = [IO.File]::Open($Path, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try { $file.Write($bytes,0,$bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
}
function Get-CaseElapsedTotal([object[]]$Rows, [ValidateSet('C','F')][string]$Set) {
    $sum = 0.0
    foreach ($row in $Rows) {
        $item = if ($row -is [Collections.IDictionary]) { [pscustomobject]$row } else { $row }
        if ($null -eq $item) { throw 'Null case row cannot be accounted.' }
        $properties = @($item.PSObject.Properties.Name)
        foreach ($required in @('case','set','elapsed_seconds')) {
            if ($required -notin $properties) { throw "Case row lacks $required." }
        }
        if ([string]::IsNullOrWhiteSpace($item.case) -or $item.set -cnotin @('C','F')) { throw 'Invalid case identity/set.' }
        $value = $item.elapsed_seconds
        if ($null -eq $value -or $value -isnot [ValueType] -or $value -is [bool]) { throw 'Case elapsed_seconds must be numeric.' }
        $seconds = [double]$value
        if ([double]::IsNaN($seconds) -or [double]::IsInfinity($seconds) -or $seconds -lt 0) { throw 'Case elapsed_seconds must be finite and nonnegative.' }
        if ($item.set -ceq $Set) { $sum += $seconds }
    }
    if ([double]::IsNaN($sum) -or [double]::IsInfinity($sum)) { throw 'Case elapsed total overflow.' }
    return $sum
}
function Set-TemporaryProcessEnvironment([string]$Name, $Value) {
    $path = 'Env:'+$Name
    if ($null -eq $Value) {
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -ErrorAction Stop }
    } else {
        [Environment]::SetEnvironmentVariable($Name,[string]$Value,'Process')
    }
    $actual = [Environment]::GetEnvironmentVariable($Name,'Process')
    if ($null -eq $Value) {
        if ($null -ne $actual) { throw "Process environment deletion failed: $Name" }
    } elseif ($null -eq $actual -or $actual -cne [string]$Value) {
        throw "Process environment restoration failed: $Name"
    }
}
function Verify-Rows($Rows) {
    foreach ($row in $Rows) {
        $path = [IO.Path]::GetFullPath((Join-Path $workspace $row.path))
        if (-not $path.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Binding path escapes workspace.' }
        if ((Get-Item -LiteralPath $path).Length -ne $row.bytes -or (Hash $path) -cne $row.sha256) { throw "Binding changed: $($row.path)" }
    }
}
$bindingPath = Join-Path $evidence 'R-B-pre-run-binding.json'
$ledgerPath = Join-Path $evidence 'contributing-cases.json'
$recordPath = Join-Path $evidence "$Case.json"
$startPath = Join-Path $evidence "$Case.started.json"
$inputPath = Join-Path $evidence "$Case.inputs.json"
$logPath = Join-Path $evidence "$Case.log"
if ((Test-Path -LiteralPath $recordPath) -or (Test-Path -LiteralPath $startPath)) { throw 'One attempt only; unfinished starts require root reconciliation.' }
if ((& git -C $workspace rev-parse HEAD) -cne $baseline) { throw 'R-B baseline changed.' }
$binding = Get-Content -LiteralPath $bindingPath -Raw | ConvertFrom-Json
if ($binding.baseline -cne $baseline -or $binding.classification -cne 'R-B static-ready source/image binding; single C unstarted') { throw 'Wrong R-B binding.' }
$ledgerLock = [IO.File]::Open((Join-Path $workspace '.tmp/pre-s53-result-ledger.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$ledger = Get-Content -LiteralPath $ledgerPath -Raw | ConvertFrom-Json
if ($ledger.baseline -cne $baseline -or $ledger.active_limits_seconds.C -ne 60 -or $ledger.active_limits_seconds.F -ne 0 -or $ledger.active_limits_seconds.P -ne 0) { throw 'Independent R-B budget changed.' }
foreach ($prior in $ledger.cases) {
    if ($prior.case -cne $Case) { throw 'Unexpected R-B case.' }
    if (Test-Path -LiteralPath (Join-Path $evidence "$($prior.case).terminal-failure.json")) { throw 'Prior terminal failure requires root decision.' }
}
if (@($ledger.cases | Where-Object { $_.state -cne 'complete' -or $_.exit_code -ne 0 }).Count) { throw 'Prior failed/incomplete case requires root decision.' }
if (60.0-[double]$ledger.charged_elapsed_seconds.C -lt $policy.whole) { throw 'Remaining R-B allowance does not cover whole60.' }
Write-NewJson $startPath ([ordered]@{case=$Case;set='C';started_utc=$startedUtc;owner_pid=$PID;work_seconds=45;whole_seconds=60;baseline=$baseline;binding_sha256=(Hash $bindingPath);classification='attempt-start-not-final-result'})
$ledger.cases = @($ledger.cases) + @([pscustomobject]@{case=$Case;state='started';exit_code=$null;elapsed_seconds=$null;set='C'})
Replace-Json $ledgerPath $ledger
$deadline = $null
$code = 1
$failure = $null
$matched = $null
$matchesByName = $null
$summaries = $null
$completed = $null
$wrapperExit = $null
$wrapperReturnedNormally = $false
$wrapperException = $null
$logParsed = $false
$logParseError = $null
$cleanupVerified = $false
$ownedChildrenAfterCleanup = $null
$sourceVerifiedAfter = $false
$inventoryVerifiedAfter = $false
$inputHash = $null
$inventoryIdentity = 'R-B/'+$Case+'/'+(Hash $bindingPath)
$environment = @{}
foreach ($name in @('CARGO_NET_OFFLINE','KILOGRAM_RB_CASE','KILOGRAM_RB_INVENTORY','KILOGRAM_RB_INVENTORY_ID','KILOGRAM_TEST_OD1_SCHEDULE','KILOGRAM_TEST_OD1_CLOCK_FILE','KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE','KILOGRAM_TEST_MIGRATION_PAUSE_FILE')) { $environment[$name]=[Environment]::GetEnvironmentVariable($name,'Process') }
try {
    . (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
    $deadline = [KilogramS5Deadline]::new($policy.work-$timer.Elapsed.TotalSeconds,$policy.whole-$timer.Elapsed.TotalSeconds)
    . (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
    $null = Set-KilogramCargoResourcePolicy -RequestedJobs 12
    Verify-Rows $binding.inputs
    Verify-Rows $binding.images
    Write-NewJson $inputPath ([ordered]@{case=$Case;baseline=$baseline;binding_sha256=(Hash $bindingPath);classification='bound_pre_run_inventory';inventory_identity=$inventoryIdentity;sources=$binding.inputs;images=$binding.images;execution_mode_declared=$ExecutionMode;token_introspection=$false})
    $inputHash = Hash $inputPath
    if (@(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -like 'kilogram-*' }).Count) { throw 'Live Kilogram process prevents stable image recopy.' }
    foreach ($name in $environment.Keys) { Set-TemporaryProcessEnvironment $name $null }
    $env:CARGO_NET_OFFLINE = 'true'
    $env:KILOGRAM_RB_CASE = $Case
    $env:KILOGRAM_RB_INVENTORY = $inputPath
    $env:KILOGRAM_RB_INVENTORY_ID = $inventoryIdentity
    $deadline.CheckWork()
    try {
        & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package kilogram-cli -CargoJobs 12 -Profile verification -Filter $policy.selector -Run -NoCapture *> $logPath
        $wrapperExit = $LASTEXITCODE
        $wrapperReturnedNormally = $true
    } catch { $wrapperException = $_.Exception.Message }
    try {
        $text = Get-Content -LiteralPath $logPath -Raw
        if ($null -eq $text) { throw 'Test log empty.' }
        $matchesByName = @($expectedNames | ForEach-Object { [pscustomobject]@{name=$_;count=[regex]::Matches($text,('(?m)^test (?:[^\s]*::)?'+[regex]::Escape($_)+' \.\.\.')).Count} })
        $matched = 0
        foreach ($item in $matchesByName) { $matched += $item.count }
        $prefixMatches = [regex]::Matches($text,'(?m)^test [^\s]*pre_s53_attempt_result_[^\s]* \.\.\.').Count
        $summaries = [regex]::Matches($text,'(?m)^test result: ok\. 3 passed; 0 failed; 0 ignored;').Count
        $completed = [regex]::Matches($text,'(?m)^status=passed\r?$').Count
        $logParsed = $true
    } catch { $logParseError = $_.Exception.Message }
    if (-not $wrapperReturnedNormally -or $wrapperExit -ne 0) { throw "Stable wrapper failed, including finalization: $wrapperException" }
    if (-not $logParsed) { throw "Counts unknown: $logParseError" }
    if ($matched -ne 3 -or $prefixMatches -ne 3 -or @($matchesByName | Where-Object {$_.count -ne 1}).Count -ne 0 -or $summaries -ne 1 -or $completed -ne 1) { throw 'Expected the exact three lifecycle tests, one3/0 summary and wrapper completion.' }
    $deadline.CheckWork()
    $code = 0
} catch {
    $code = 1
    $failure = $_.Exception.Message
    try { $_ | Out-String | Add-Content -LiteralPath $logPath }
    catch { $failure += '; Error log write failed: '+$_.Exception.Message }
} finally {
    try { foreach ($name in $environment.Keys) { Set-TemporaryProcessEnvironment $name $environment[$name] } }
    catch { $code=1; $failure=(@($failure,'Environment restore: '+$_.Exception.Message)|Where-Object {$_}) -join '; ' }
    try {
        if ($deadline) {
            $deadline.CleanupChildren(5000)
            $actualChildren = @($deadline.Children())
            if ($actualChildren.Count -ne 0) { throw 'Owned children remain after cleanup.' }
            $ownedChildrenAfterCleanup = $actualChildren.Count
            $cleanupVerified = $true
        }
    } catch { $code=1; $failure=(@($failure,$_.Exception.Message)|Where-Object {$_}) -join '; ' }
    try {
        if ($null -eq $inputHash -or (Hash $inputPath) -cne $inputHash) { throw 'Actual pre-run inventory missing or changed.' }
        $inventoryVerifiedAfter = $true
        Verify-Rows $binding.inputs
        Verify-Rows $binding.images
        $sourceVerifiedAfter = $true
        if ((& git -C $workspace rev-parse HEAD) -cne $baseline) { throw 'Baseline changed during C.' }
    } catch { $code=1; $failure=(@($failure,$_.Exception.Message)|Where-Object {$_}) -join '; ' }
}
function Work-Expired { (-not $deadline) -or $deadline.WorkExpired -or $timer.Elapsed.TotalSeconds -ge $policy.work }
$workExpired = Work-Expired
if ($workExpired -or -not $cleanupVerified) { $code=1; $failure=(@($failure,'Work deadline expired or cleanup unverified after final validation.')|Where-Object {$_}) -join '; ' }
$record = [pscustomobject][ordered]@{case=$Case;set='C';state='sealing';classification='whole-contributing-runner';baseline=$baseline;selector=$policy.selector;expected_names=$expectedNames;exit_code=$code;error=$failure;elapsed_seconds=$timer.Elapsed.TotalSeconds;matched_tests=$matched;matches_by_name=$matchesByName;three_test_pass_summaries=$summaries;wrapper_completion_markers=$completed;wrapper_returned_normally=$wrapperReturnedNormally;wrapper_exit_code=$wrapperExit;wrapper_exception=$wrapperException;log_parsed=$logParsed;log_parse_error=$logParseError;source_images_verified_after=$sourceVerifiedAfter;pre_run_inventory_verified_after=$inventoryVerifiedAfter;inventory_identity=$inventoryIdentity;inventory_sha256=$inputHash;inputs_file="$Case.inputs.json";binding_sha256=(Hash $bindingPath);deadline=@{work_seconds=45;whole_seconds=60;work_expired=$workExpired;cleanup_verified=$cleanupVerified;owned_children_after_cleanup=$ownedChildrenAfterCleanup;whole_watchdog='unchanged owned job armed through case/ledger writes'};execution_mode_declared=$ExecutionMode;no_product_fixture=$true;rule='Includes setup/files, cached compile/stage, waits, stable finalization, failures and cleanup. A kernel or snapshot alone is not whole PASS.'}
try {
    Write-NewJson $recordPath $record
    $ledger.cases = @($ledger.cases | Where-Object {$_.case -cne $Case}) + @($record)
    if (-not [object]::ReferenceEquals($record,$ledger.cases[-1])) { throw 'Ledger lost current record reference.' }
    $ledger.charged_elapsed_seconds.C = Get-CaseElapsedTotal -Rows $ledger.cases -Set C
    Replace-Json $ledgerPath $ledger
    $record.elapsed_seconds = $timer.Elapsed.TotalSeconds
    $ledger.charged_elapsed_seconds.C = Get-CaseElapsedTotal -Rows $ledger.cases -Set C
    $record.deadline.work_expired = Work-Expired
    if ($record.deadline.work_expired -or $record.elapsed_seconds -ge 60 -or $ledger.charged_elapsed_seconds.C -gt 60) { $record.exit_code=1; $record.error=(@($record.error,'Work/whole/cumulative ceiling exceeded at sealing.')|Where-Object {$_}) -join '; '; $code=1 }
    $record.state = 'complete'
    Replace-Json $recordPath $record
    Replace-Json $ledgerPath $ledger
    if ((Work-Expired) -and $record.exit_code -eq 0) {
        $code=1
        $record.exit_code=1
        $record.deadline.work_expired=$true
        $record.error='Work deadline expired during terminal writes.'
        $record.elapsed_seconds=$timer.Elapsed.TotalSeconds
        $ledger.charged_elapsed_seconds.C=Get-CaseElapsedTotal -Rows $ledger.cases -Set C
        Replace-Json $recordPath $record
        Replace-Json $ledgerPath $ledger
    }
    if ($deadline) { $deadline.Finish() }
} catch {
    $code=1
    $record.exit_code=1
    $record.state='terminal-failed'
    $terminalError=$_.Exception.Message
    $record.error=(@($record.error,'Terminal accounting/publication: '+$terminalError)|Where-Object {$_}) -join '; '
    $record.elapsed_seconds=$timer.Elapsed.TotalSeconds
    $terminalFailure=[ordered]@{case=$Case;whole_exit_code=1;classification='terminal failure, ledger may be incomplete';primary_error=$failure;terminal_error=$terminalError;elapsed_lower_bound_seconds=$record.elapsed_seconds;last_record=$record;ledger_is_final=$false}
    try { Write-NewJson (Join-Path $evidence "$Case.terminal-failure.json") $terminalFailure }
    catch { [Console]::Error.WriteLine((ConvertTo-Json -InputObject $terminalFailure -Depth 20)) }
} finally { $ledgerLock.Dispose(); $timer.Stop() }
[ordered]@{case=$Case;exit_code=$code;elapsed_seconds=$record.elapsed_seconds;matched_tests=$matched;cleanup_verified=$cleanupVerified;owned_children_after_cleanup=$ownedChildrenAfterCleanup}|ConvertTo-Json -Compress
exit $code
