[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory,[ValidateSet('Parent','Work','Whole','Descendant')][string]$Mode='Parent')
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$shell=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
if($Mode -eq 'Descendant'){
 $p=Start-Process -FilePath $shell -ArgumentList @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 60') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'leaf.stdout') -RedirectStandardError (Join-Path $OutputDirectory 'leaf.stderr')
 [ordered]@{pid=$p.Id;start_utc=$p.StartTime.ToUniversalTime().ToString('O')}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'leaf.json')
 Start-Sleep -Seconds 60
 exit 1
}
if($Mode -in @('Work','Whole')){
 . (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
 $guard=if($Mode -eq 'Work'){[KilogramS5Deadline]::new(3,8)}else{[KilogramS5Deadline]::new(1.8,2)}
 # Whole mode ignores cooperative checks and has a shorter independent whole
 # deadline than the deliberate blocking operation, while its work timer is later.
 $p=Start-Process -FilePath $shell -ArgumentList @('-NoProfile','-NonInteractive','-File',('"{0}"' -f $PSCommandPath),'-OutputDirectory',('"{0}"' -f $OutputDirectory),'-Mode','Descendant') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'child.stdout') -RedirectStandardError (Join-Path $OutputDirectory 'child.stderr')
 $guard.VerifyChild($p)
 [ordered]@{pid=$p.Id;start_utc=$p.StartTime.ToUniversalTime().ToString('O')}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'child.json')
 if($Mode -eq 'Whole'){Start-Sleep -Seconds 60;throw 'Whole deadline did not terminate the attempt owner.'}
 try {while($true){$guard.CheckWork();Start-Sleep -Milliseconds 20}}
 catch {if(-not $guard.WorkExpired){throw}}
 $guard.CleanupChildren(1500)
 if(@($guard.Children()).Count){throw 'Owned child remained live.'}
 $guard.Finish()
 'WORK_CUTOFF_PASS owned_children=0'
 exit 0
}
if(Test-Path -LiteralPath $OutputDirectory){throw 'Probe attempt directory is immutable.'}
[void][IO.Directory]::CreateDirectory($OutputDirectory)
$rows=@()
foreach($kind in @('Work','Whole')){
 $dir=Join-Path $OutputDirectory $kind
 [void][IO.Directory]::CreateDirectory($dir)
 $start=[Diagnostics.Stopwatch]::StartNew()
 $p=Start-Process -FilePath $shell -ArgumentList @('-NoProfile','-NonInteractive','-File',('"{0}"' -f $PSCommandPath),'-OutputDirectory',('"{0}"' -f $dir),'-Mode',$kind) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $dir 'worker.stdout') -RedirectStandardError (Join-Path $dir 'worker.stderr')
 try {if(-not $p.WaitForExit(15000)){throw 'Deadline probe worker did not exit.'}}
 finally {if(-not $p.HasExited){$p.Kill($true);$p.WaitForExit()}}
 $p.Refresh()
 $expected=if($kind -eq 'Work'){0}else{124}
 if($p.ExitCode -ne $expected){throw "Probe $kind exit=$($p.ExitCode): $(Get-Content -LiteralPath (Join-Path $dir 'worker.stderr') -Raw)"}
 foreach($name in @('child','leaf')){
  $saved=Get-Content -Raw -LiteralPath (Join-Path $dir "$name.json")|ConvertFrom-Json
  $live=Get-Process -Id $saved.pid -ErrorAction SilentlyContinue
  if($live -and $live.StartTime.ToUniversalTime().ToString('O') -ceq $saved.start_utc){throw 'Exact owned process survived job cleanup.'}
 }
 $rows+=,[ordered]@{mode=$kind;exit_code=$p.ExitCode;elapsed_seconds=$start.Elapsed.TotalSeconds;actual_child_and_descendant_stopped=$true}
}
[ordered]@{classification='real-owned-process-deadline-mechanism-probe-no-Native';status='PASS';cases=$rows}|ConvertTo-Json -Depth 6
