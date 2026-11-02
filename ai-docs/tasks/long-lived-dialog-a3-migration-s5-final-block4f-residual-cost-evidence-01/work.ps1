param([Parameter(Mandatory)][ValidateSet('profile-01','probe-01','seal-01')][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=(Get-Location).Path
. (Join-Path $root 'scripts/invoke-migration-s54-whole.ps1') -Purpose FeasibilityF
$base=Get-S54Path (Join-Path $root '.tmp/s54-block4f-residual-cost-01')
function Write-OwnedResidual([string]$Name,$Value){
    $path=Get-S54Path (Join-Path $base $Name) -AllowMissing
    if(-not $path.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $path)){throw 'Fresh owned diagnostic output required.'}
    Write-KilogramFrozenJson $path $Value
}
function Pin-History {
    $baseline=Read-S54Json (Get-S54Pin (Join-Path $base 'baseline.json') 16384) 16384
    if((& git rev-parse HEAD) -cne $baseline.head -or @(& git diff --cached --name-only).Count){throw 'Published HEAD/index preservation.'}
    foreach($p in $baseline.navigation){$null=Read-S54Pin $p 8388608}
    $paths=Get-S54SourcePaths $root
    Assert-S54Equal @($paths|Sort-Object) @($script:ResidualSources|ForEach-Object {$_.path}|Sort-Object)
    foreach($p in $script:ResidualSources){$null=Read-S54Pin $p 8388608}
    foreach($p in @($script:ResidualContext.normal,$script:ResidualContext.tests)+@($script:ResidualContext.test_origins_after)+@($script:ResidualContext.test_dep_info_after)+@($script:ResidualContext.normal_origins)+@($script:ResidualContext.normal_dep_info)){$null=Read-S54Pin $p 134217728}
    foreach($r in $script:ResidualContext.images){$null=Read-S54Pin $r.pin 134217728}
    $old=Read-S54Json (Get-S54Pin (Join-Path $root 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-final-block4d-f-preparation-report-01.pins.json') 65536) 65536
    foreach($p in @($old.administration)+@($old.public_artifacts)){$null=Read-S54Pin $p 134217728}
    foreach($path in @('.tmp/migration-s54-feasibility/F-s54-mixed-hooks-feasibility-01-attempt-02','.tmp/migration-s54-feasibility-public-evidence/F-s54-mixed-hooks-feasibility-01-attempt-02','.tmp/migration-s54-integrated/P-s54-whole-resumed-l1-01-attempt-01','.tmp/migration-s54-public-evidence/P-s54-whole-resumed-l1-01-attempt-01')){if(Test-Path -LiteralPath (Join-Path $root $path)){throw 'No production allocation authorized.'}}
}
$policy=New-S54Policy;$root=$policy.Root
$script:ResidualContext=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4d-preparation-order-repair-01/compile-02.result.json') 1048576) 1048576
$paths=Get-S54SourcePaths $root
$script:ResidualSources=@($paths|ForEach-Object {Get-S54Pin $_ 8388608})
if($Step -ceq 'profile-01'){
    Pin-History
    $normal=Read-S54Json $ResidualContext.normal 8388608;$tests=Read-S54Json $ResidualContext.tests 8388608
    $embedded=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4d-preparation-order-repair-01/compile-public-02/embedded-before.json') 65536) 65536
    $proof=@{source=@{files=$ResidualSources};images=$ResidualContext.images;runtime=@{environment=$normal.build_inputs_before.environment;configs=$normal.build_inputs_before.configs;resolutions=$policy.Resolutions};compiler=@{embedded=@{before=$embedded.inputs;after=$ResidualContext.embedded_after;dep_info=$ResidualContext.test_dep_info_after}}}
    Add-Type -Path (Join-Path $base 'ledger.cs')
    $script:ResidualMapping=[Collections.Generic.List[object]]::new()
    $source=Join-Path $root 'scripts/invoke-migration-s54-whole.ps1'
    $tokens=$null;$errors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($source,[ref]$tokens,[ref]$errors)
    if($errors.Count){throw 'Published source parse.'}
    function Timed-ResidualStatement($Statement,[string]$Suffix,[string]$Level,[string]$Body){
        if(-not $Body){$Body=$Statement.Extent.Text}
        $id=$Suffix.Replace('.','_');$previous='$s54ResidualPrevious_'+$id;$tick='$s54ResidualTick_'+$id;$key='$s54ResidualKey_'+$id
        $script:ResidualMapping.Add(@{suffix=$Suffix;level=$Level;source=$source;start_line=$Statement.Extent.StartLineNumber;end_line=$Statement.Extent.EndLineNumber;original_statement_sha256=(Get-S54Sha ([Text.Encoding]::UTF8.GetBytes($Statement.Extent.Text)));first_line=($Statement.Extent.Text -split '\r?\n')[0]})
        return ($key+" = `$script:ResidualRole + '/$Suffix'`n"+$previous+' = [S54ResidualLedger]::Current'+"`n"+$tick+' = [S54ResidualLedger]::Begin('+$key+")`ntry {`n"+$Body+"`n} finally { [S54ResidualLedger]::End("+$key+','+$previous+','+$tick+") }`n")
    }
    function New-ResidualFunction([string]$Name,[string]$Label){
        $fn=@($ast.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $Name},$false))[0]
        if($fn.Body.BeginBlock -or $fn.Body.ProcessBlock -or $fn.Body.DynamicParamBlock){throw 'End-block-only diagnostic copy required.'}
        $body='param('+(@($fn.Parameters|ForEach-Object {$_.Extent.Text})-join ',')+")`n"
        $ordinal=0
        foreach($stmt in $fn.Body.EndBlock.Statements){
            $ordinal++;$suffix=$Label+('.s{0:d2}' -f $ordinal);$replacement=$null
            if($Name -ceq 'Assert-S54Compiler' -and $stmt -is [Management.Automation.Language.ForEachStatementAst] -and $stmt.Extent.Text.StartsWith('foreach($line in $Capture.raw_compiler_artifact_json)',[StringComparison]::Ordinal)){
                $inner='';$child=0
                foreach($item in $stmt.Body.Statements){$child++;$inner+=Timed-ResidualStatement $item ($suffix+('.raw{0:d2}' -f $child)) 'raw-child' ''}
                $prefix=$stmt.Extent.Text.Substring(0,$stmt.Body.Extent.StartOffset-$stmt.Extent.StartOffset)
                $replacement=$prefix+"{`n"+$inner+"`n}"
            }
            $body+=Timed-ResidualStatement $stmt $suffix 'top' $replacement
        }
        return $body
    }
    $compilerText=New-ResidualFunction 'Assert-S54Compiler' 'compiler'
    $embeddedText=New-ResidualFunction 'Assert-S54Embedded' 'embedded'
    Write-OwnedResidual 'statement-map.json' @($ResidualMapping)
    $copyPath=Get-S54Path (Join-Path $base 'instrumented-functions.ps1.txt') -AllowMissing
    if(-not $copyPath.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path $copyPath)){throw 'Owned fresh function-copy destination.'}
    [IO.File]::WriteAllText($copyPath,("# Process-local diagnostic bodies only; source file untouched.`n# Assert-S54Compiler`n"+$compilerText+"`n# Assert-S54Embedded`n"+$embeddedText),[Text.UTF8Encoding]::new($false))
    $script:ResidualOriginalCompiler=(Get-Item Function:Assert-S54Compiler).ScriptBlock
    $script:ResidualOriginalEmbedded=(Get-Item Function:Assert-S54Embedded).ScriptBlock
    $script:ResidualOriginalPath=(Get-Item Function:Get-S54Path).ScriptBlock
    $script:ResidualOriginalRead=(Get-Item Function:Read-S54Public).ScriptBlock
    $script:ResidualOriginalSha=(Get-Item Function:Get-S54Sha).ScriptBlock
    function Get-S54Path([string]$Path,[switch]$AllowMissing){
        $stamp=[Diagnostics.Stopwatch]::GetTimestamp()
        try{& $script:ResidualOriginalPath $Path -AllowMissing:$AllowMissing}finally{[S54ResidualLedger]::Path($stamp)}
    }
    function Read-S54Public([string]$Path,[long]$Cap){
        $stamp=[Diagnostics.Stopwatch]::GetTimestamp();$value=$null
        [S54ResidualLedger]::EnterRead()
        try{$value=& $script:ResidualOriginalRead $Path $Cap;return ,$value}finally{[S54ResidualLedger]::Read($stamp,$(if($null -eq $value){0}else{$value.Length}))}
    }
    function Get-S54Sha([byte[]]$Bytes){
        $stamp=[Diagnostics.Stopwatch]::GetTimestamp()
        try{& $script:ResidualOriginalSha $Bytes}finally{[S54ResidualLedger]::Sha($stamp,$Bytes.Length)}
    }
    $smokePath=Get-S54Path (Join-Path $base 'carrier-smoke.bin') -AllowMissing
    if(-not $smokePath.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $smokePath)){throw 'Owned fresh carrier smoke destination.'}
    $payload=[Text.Encoding]::UTF8.GetBytes('public carrier smoke only')
    [IO.File]::WriteAllBytes($smokePath,$payload)
    $smoke=Read-S54Public $smokePath 4096
    if($smoke -isnot [byte[]] -or $smoke.Length -ne $payload.Length -or (Get-S54Sha $smoke) -cne (& $script:ResidualOriginalSha $payload)){throw 'Diagnostic carrier equivalence failed before compiler measurement.'}
    Write-OwnedResidual 'carrier-smoke.json' @{byte_array_carrier_preserved=$true;hash_carrier_preserved=$true;bytes=$payload.Length}
    Set-Item Function:Assert-S54Compiler ([ScriptBlock]::Create($compilerText))
    Set-Item Function:Assert-S54Embedded ([ScriptBlock]::Create($embeddedText))
    $observations=[Collections.Generic.List[object]]::new()
    try {
        foreach($role in @('normal','tests','embedded')){
            $script:ResidualRole=$role;$timer=[Diagnostics.Stopwatch]::StartNew();$passed=$false
            try{if($role -ceq 'embedded'){Assert-S54Embedded $proof $policy $tests}else{Assert-S54Compiler $proof $policy $(if($role -ceq 'normal'){$normal}else{$tests}) ($role -ceq 'normal')};$passed=$true}
            finally{$timer.Stop();$row=@{role=$role;seconds=$timer.Elapsed.TotalSeconds;passed=$passed;instrumented=$true};$observations.Add($row);Write-OwnedResidual ('profile-01.'+$role+'.json') $row}
        }
    }finally{
        Set-Item Function:Assert-S54Compiler $script:ResidualOriginalCompiler
        Set-Item Function:Assert-S54Embedded $script:ResidualOriginalEmbedded
        Set-Item Function:Get-S54Path $script:ResidualOriginalPath
        Set-Item Function:Read-S54Public $script:ResidualOriginalRead
        Set-Item Function:Get-S54Sha $script:ResidualOriginalSha
    }
    $frequency=[double][Diagnostics.Stopwatch]::Frequency
    $metrics=@([S54ResidualLedger]::Snapshot()|ForEach-Object {@{key=$_.Key;calls=$_.Calls;inclusive_seconds=$_.Ticks/$frequency;path_calls=$_.PathCalls;path_seconds=$_.PathTicks/$frequency;direct_path_seconds=$_.DirectPathTicks/$frequency;path_inside_public_read_seconds=$_.ReadPathTicks/$frequency;public_read_calls=$_.ReadCalls;public_read_inclusive_seconds=$_.ReadTicks/$frequency;public_read_bytes=$_.ReadBytes;sha_calls=$_.ShaCalls;sha_seconds=$_.ShaTicks/$frequency;sha_bytes=$_.ShaBytes}})
    Write-OwnedResidual 'profile-01.metrics.json' @{classification='aggregated process-local statement/path/read/hash attribution; nested times overlap';frequency=$frequency;max_public_read_depth=[S54ResidualLedger]::MaxReadDepth;observations=@($observations);metrics=$metrics}
    $sample=@($ResidualSources|Select-Object -First 128)
    $timer=[Diagnostics.Stopwatch]::StartNew();foreach($p in $sample){$null=Get-S54Path $p.path};$timer.Stop();$beforeSeconds=$timer.Elapsed.TotalSeconds
    function Get-S54Path([string]$Path,[switch]$AllowMissing){$stamp=[Diagnostics.Stopwatch]::GetTimestamp();try{& $script:ResidualOriginalPath $Path -AllowMissing:$AllowMissing}finally{[S54ResidualLedger]::Path($stamp)}}
    [S54ResidualLedger]::Current='calibration'
    $timer=[Diagnostics.Stopwatch]::StartNew();foreach($p in $sample){$null=Get-S54Path $p.path};$timer.Stop();$wrappedSeconds=$timer.Elapsed.TotalSeconds
    Set-Item Function:Get-S54Path $script:ResidualOriginalPath
    $timer=[Diagnostics.Stopwatch]::StartNew();foreach($p in $sample){$null=Get-S54Path $p.path};$timer.Stop();$afterSeconds=$timer.Elapsed.TotalSeconds
    Write-OwnedResidual 'profile-01.calibration.json' @{classification='small warm-path wrapper calibration; not an exact overhead correction';paths=$sample.Count;original_before_seconds=$beforeSeconds;wrapped_seconds=$wrappedSeconds;original_after_seconds=$afterSeconds;note='Statement timers/loop overhead not isolated; instrumented times remain attribution only.'}
    Pin-History
    $rows=@(Get-CimInstance Win32_Process -Property ProcessId,ExecutablePath -OperationTimeoutSec 3 -ErrorAction Stop)
    $images=@($ResidualContext.images.pin.path);$active=@($rows|Where-Object {$_.ExecutablePath -and $_.ExecutablePath -in $images}|ForEach-Object {@{pid=$_.ProcessId;path=$_.ExecutablePath}})
    if($active.Count){throw 'Unexpected exact image process; no process stopped.'}
    Write-OwnedResidual 'profile-01.preservation.json' @{head=(& git rev-parse HEAD);source_count=$ResidualSources.Count;source_pins=$ResidualSources;normal=$ResidualContext.normal;tests=$ResidualContext.tests;images=$ResidualContext.images;source_capture_images_and_failed_F01_pins_preserved=$true;historical_fixtures_written=$false;native_process_inventory_succeeded=$true;active_exact_images=$active;production_F02_P01_directories_absent=$true;accepted_source_unchanged=$true;runtime='not_established'}
    Write-Output 'residual_instrumented_reference_pair_and_embedded=passed;source_and_history_read_only=true'
    exit 0
}
if($Step -ceq 'probe-01'){. (Join-Path $base 'probe.ps1');exit 0}
if($Step -ceq 'seal-01'){. (Join-Path $base 'seal.ps1');exit 0}
throw 'No follow-up diagnostic selected yet.'
