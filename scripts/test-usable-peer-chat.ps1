param([Parameter(Mandatory)][string]$Exe,[Parameter(Mandatory)][string]$Evidence,[int]$Samples=10,[switch]$Recovery,[switch]$CheckpointRefusal,[switch]$RequireLatency,[switch]$LegacyMigration,[switch]$ConfirmationRegression,[switch]$Probe,[switch]$LocalReceiver,[ValidateSet('local-fixture','LAN','different-networks')][string]$Network='local-fixture')
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
$root=Split-Path $PSScriptRoot -Parent;$Evidence=[IO.Path]::GetFullPath($Evidence)
if(!$Evidence.StartsWith(([IO.Path]::GetFullPath((Join-Path $root '.tmp/usable-peer-chat'))+'\'),[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $Evidence)){throw 'Fresh own evidence path required'}
[void][IO.Directory]::CreateDirectory($Evidence)
[void][IO.Directory]::CreateDirectory((Join-Path $Evidence 'timings'))
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
$inputs=[ordered]@{}
foreach($path in @($PSCommandPath,(Join-Path $PSScriptRoot 'test-usable-peer-chat.py'),(Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1'),[IO.Path]::GetFullPath($Exe),(Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Exe))) 'kilogram-cli.exe'),(Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Exe))) 'kilogram-bootstrap.exe'))){$inputs[$path]=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}
if($Probe){foreach($name in @('test-usable-peer-probe.py','test-usable-peer-receiver.ps1')){$path=Join-Path $PSScriptRoot $name;$inputs[$path]=(Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()}}
$inputs|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $Evidence 'inputs.json') -Encoding UTF8
$env:KILOGRAM_TEST_OD2_TIMINGS='1'
$env:KILOGRAM_TEST_OD2_TIMING_DIRECTORY=(Join-Path $Evidence 'timings')
. "$PSScriptRoot/migration-s5-l1-deadline.ps1"
$guard=[KilogramS5Deadline]::new(900,930,[KilogramS5Deadline]::TickNow());$p=$null;$exit=$null;$errorText=$null;$forced=$false
try {
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName='C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
    $si.Arguments='-X utf8 -B "'+(Join-Path $PSScriptRoot 'test-usable-peer-chat.py')+'" --exe "'+[IO.Path]::GetFullPath($Exe)+'" --evidence "'+(Join-Path $Evidence 'case')+'" --samples '+$Samples+' --seconds 840'+$(if($Recovery){' --recovery'}else{''})+$(if($CheckpointRefusal){' --checkpoint-refusal'}else{''})+$(if($RequireLatency){' --require-latency'}else{''})+$(if($LegacyMigration){' --legacy-migration'}else{''})
    if($Probe){
        if($Recovery -or $CheckpointRefusal -or $RequireLatency -or $LegacyMigration){throw 'Separate functional and probe contracts'}
        $si.Arguments='-X utf8 -B "'+(Join-Path $PSScriptRoot 'test-usable-peer-probe.py')+'" --exe "'+[IO.Path]::GetFullPath($Exe)+'" --evidence "'+(Join-Path $Evidence 'case')+'" --samples '+$Samples+' --network '+$Network+$(if($LocalReceiver){' --local-receiver'}else{''})
    }
    if($ConfirmationRegression){if(!$LegacyMigration -or $Probe){throw 'Confirmation regression requires disposable legacy migration'};$si.Arguments+=' --confirmation-regression'}
    $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($si);$guard.VerifyChild($p)
    $out=[IO.File]::Create((Join-Path $Evidence 'stdout.log'));$err=[IO.File]::Create((Join-Path $Evidence 'stderr.log'))
    try{
        $ot=$p.StandardOutput.BaseStream.CopyToAsync($out);$et=$p.StandardError.BaseStream.CopyToAsync($err)
        while(!$p.WaitForExit(250)){$guard.CheckWork()}
        if(!$ot.Wait(2000) -or !$et.Wait(2000)){throw 'Product harness stream drain'}
        $exit=$p.ExitCode
    }finally{$out.Dispose();$err.Dispose()}
}catch{$errorText=$_.Exception.Message}
finally{
    $before=@($guard.Children());$forced=$before.Count -gt 0
    $guard.CleanupChildren(5000);$empty=@($guard.Children()).Count -eq 0;$guard.Finish();if($p){$p.Dispose()}
    @{passed=($exit -eq 0 -and !$errorText -and !$forced -and $empty);exit=$exit;error=$errorText;forced_cleanup=$forced;empty_owned_job=$empty;work_seconds=900;whole_seconds=930}|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $Evidence 'outer.json') -Encoding UTF8
}
if($exit -ne 0 -or $errorText -or $forced -or !$empty){throw "Product fixture failed: exit=$exit error=$errorText"}
@{passed=$true;evidence=$Evidence;empty_owned_job=$empty}|ConvertTo-Json -Compress
