param([Parameter(Mandatory)][string]$Evidence)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
if(Test-Path -LiteralPath $Evidence){throw 'Fresh evidence required'}
[void][IO.Directory]::CreateDirectory($Evidence)
. "$PSScriptRoot/migration-s5-l1-deadline.ps1"
$outer=[KilogramS5Deadline]::new(100,115);$inner=$null;$owned=@();$targets=@();$cases=@()
function Child([switch]$Natural) {
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName="$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
    $body=if($Natural){'$null=[Console]::ReadLine();exit 0'}else{'Start-Sleep -Seconds 90'}
    $si.Arguments='-NoProfile -NonInteractive -EncodedCommand '+[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
    $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardInput=$true
    $p=[Diagnostics.Process]::Start($si);$p.PriorityClass='BelowNormal';$script:owned+=,$p
    $ready=[Diagnostics.Stopwatch]::StartNew()
    while([string]::IsNullOrEmpty($p.Path) -and !$p.HasExited -and $ready.ElapsedMilliseconds -lt 5000){$p.Refresh();Start-Sleep -Milliseconds 20}
    if([string]::IsNullOrEmpty($p.Path)){throw 'Synthetic child image unavailable'}
    return $p
}
function Refuse([string]$Name,[scriptblock]$Action,$Survivor) {
    $rejected=$false;try{$null=& $Action}catch{$rejected=$true}
    if(!$rejected -or ($Survivor -and $Survivor.HasExited)){throw "Negative case failed: $Name"}
    $script:cases+=,@{name=$Name;rejected=$true;survivor_live=[bool]$Survivor}
}
$passed=$false
try {
    $outside=Child
    $inner=[KilogramS5Deadline]::new(85,95)
    Refuse 'foreign_job' {$inner.RetainCrashTarget($outside,$outside.StartTime.ToFileTimeUtc(),$outside.Path,$PID)} $outside
    $p=Child
    Refuse 'frontend' {$inner.RetainCrashTarget($p,$p.StartTime.ToFileTimeUtc(),$p.Path,$p.Id)} $p
    Refuse 'creation_at_retain' {$inner.RetainCrashTarget($p,1,$p.Path,$PID)} $p
    $target=$inner.RetainCrashTarget($p,$p.StartTime.ToFileTimeUtc(),$p.Path,$PID);$targets+=,$target
    $digest=('a'*64)
    Refuse 'wrong_pid' {$target.Execute($digest,$p.Id+1,$target.Started,$target.Image)} $p
    Refuse 'wrong_creation' {$target.Execute($digest,$p.Id,$target.Started+1,$target.Image)} $p
    Refuse 'wrong_image' {$target.Execute($digest,$p.Id,$target.Started,'wrong.exe')} $p
    Refuse 'bad_digest' {$target.Execute('bad',$p.Id,$target.Started,$target.Image)} $p
    $code=$target.Execute($digest,$p.Id,$target.Started,$target.Image)
    if($code -ne 137 -or !$p.WaitForExit(1000) -or $p.ExitCode -ne 137){throw 'Exact fault not observed'}
    $replacement=Child
    if($target.Execute($digest,$target.Pid,$target.Started,$target.Image) -ne 137 -or $replacement.HasExited){throw 'Lost reply replay killed replacement'}
    Refuse 'different_request_after_kill' {$target.Execute(('b'*64),$target.Pid,$target.Started,$target.Image)} $replacement
    Refuse 'replacement_binding_after_kill' {$target.Execute($digest,$replacement.Id,$replacement.StartTime.ToFileTimeUtc(),$replacement.Path)} $replacement
    $natural=Child -Natural
    $n=$inner.RetainCrashTarget($natural,$natural.StartTime.ToFileTimeUtc(),$natural.Path,$PID);$targets+=,$n
    $natural.StandardInput.Close();if(!$natural.WaitForExit(10000) -or $natural.ExitCode -ne 0){throw 'Natural exit fixture'}
    Refuse 'natural_exit_is_not_fault' {$n.Execute($digest,$n.Pid,$n.Started,$n.Image)} $replacement
    $target.Dispose();$target.Dispose()
    Refuse 'disposed_handle' {$target.Execute($digest,$target.Pid,$target.Started,$target.Image)} $replacement
    $cases+=,@{name='actual_exit137_and_lost_reply';passed=$true;pid=$p.Id;started=$target.Started;replacement_pid=$replacement.Id;replacement_live=$true}
    $passed=$true
} finally {
    foreach($target in $targets){$target.Dispose()}
    if($inner){$inner.CleanupChildren(6000);$inner.Finish()}
    $outer.CleanupChildren(6000);$empty=@($outer.Children()).Count -eq 0;$outer.Finish()
    foreach($p in $owned){$p.Dispose()}
    @{passed=$passed;cases=$cases;empty_owned_job=$empty;installed_agents_touched=$false;network=$false}|ConvertTo-Json -Depth 7|
        Set-Content -LiteralPath (Join-Path $Evidence 'result.json') -Encoding UTF8
}
if(!$passed){throw 'Native fault checks failed'}
@{passed=$true;negative_cases=$cases.Count-1;empty_owned_job=$empty}|ConvertTo-Json -Compress
