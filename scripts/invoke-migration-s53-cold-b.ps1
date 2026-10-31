[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$HarnessSha256,
 [Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$AssemblySha256,
 [Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$ComposedSha256,
 [Parameter(Mandatory)][ValidatePattern('\A[a-f0-9]{64}\z')][string]$DecisionSha256)
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$e="$w\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-b-01-attempt-01"
$exe="$w\target\verification\deps\kilogram-cli-test-migration_s53_cold-tests.exe"
$dll="$w\target\verification\s53-cold-worker\worker.dll"
$cs="$w\target\verification\s53-cold-worker\worker.cs"
if([Environment]::ProcessPath -cne 'C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'){throw 'Fixed shell'}
foreach($p in @(@($exe,$HarnessSha256),@($dll,$AssemblySha256),@($cs,$ComposedSha256))){if((Get-FileHash -LiteralPath $p[0] -Algorithm SHA256).Hash.ToLowerInvariant() -cne $p[1]){throw 'Reviewed image mismatch'}}
# This source is submitted now; root separately activates its future F run.
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. "$w\scripts\migration-s5-l1-deadline.ps1"
$ticks=[Diagnostics.Stopwatch]::GetTimestamp()+210L*[Diagnostics.Stopwatch]::Frequency
$utc=[DateTimeOffset]::UtcNow.AddSeconds(210).ToUnixTimeMilliseconds()
$guard=[KilogramS5Deadline]::new(210,235)
$code=1;$capture=$null;$child=$null;$created=$false;$phase='bootstrap';$output=$null;$errorLog=$null
try {
 [Reflection.Assembly]::LoadFrom($dll)|Out-Null
 . "$w\scripts\migration-s53-root-copy.ps1"
 # Existing parent only; never change its ACL or create a substitute parent.
 $phase='new-e';$parent=[S53RootNativeR38]::DirectoryHandle([IO.Path]::GetDirectoryName($e))
 try{New-S53RootDirectory $e {$script:created=$true}}finally{$parent.Dispose()}
 Assert-S53RootPrivateAcl $e $true
 $env:KILOGRAM_S53_COLD_ATTEMPT='P2-s53-exhausted-cold-b-01-attempt-01'
 $env:KILOGRAM_S53_COLD_SOURCE_SHA256=$ComposedSha256;$env:KILOGRAM_S53_COLD_ASSEMBLY_SHA256=$AssemblySha256
 $env:KILOGRAM_S53_COLD_DECISION_SHA256=$DecisionSha256;$env:KILOGRAM_S53_COLD_DEADLINE_TICKS=[string]$ticks;$env:KILOGRAM_S53_COLD_UNTIL_UNIX_MS=[string]$utc
 $start=[Diagnostics.ProcessStartInfo]::new($exe);$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
 foreach($name in @('KILOGRAM_TEST_OD1_SCHEDULE','KILOGRAM_TEST_OD1_CLOCK_FILE','KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE','KILOGRAM_TEST_MIGRATION_PAUSE_FILE')){[void]$start.Environment.Remove($name)}
 foreach($arg in @('session::cold_b_private_one_session','--exact','--nocapture','--test-threads=1')){$start.ArgumentList.Add($arg)}
 $output=[S53RootNativeR38]::NewFile("$e\harness.stdout.private.log");$errorLog=[S53RootNativeR38]::NewFile("$e\harness.stderr.private.log")
 $phase='harness';$child=[Diagnostics.Process]::Start($start);$guard.VerifyChild($child)
 $capture=[S53RootCaptureR38]::new($child,$output,$errorLog)
 while(-not $child.WaitForExit(50)){$guard.CheckWork();if($capture.Faulted){throw 'Bounded capture failure'}}
 while(-not $capture.Completed){$guard.CheckWork();[Threading.Thread]::Sleep(10)}
 $null=$capture.CloseCompleted();$capture=$null;$output=$null;$errorLog=$null;$child.Refresh();$code=$child.ExitCode
} catch {
 if($created){try{$reason=$_.Exception.ToString();if($reason.Length -gt 8192){$reason=$reason.Substring(0,8192)};Write-S53RootPrivate "$e\outer-failure.private.json" ([Text.UTF8Encoding]::new($false).GetBytes((@{phase=$phase;error=$reason;no_restore=$true}|ConvertTo-Json -Compress))) 65536}catch{}}
 Write-Output 'cold_b_attempt_failed_or_unknown=true';$code=1
}
finally {
 try{$guard.CleanupChildren(1000);$count=$guard.Children().Length;if($capture){if($capture.Completed){$null=$capture.CloseCompleted();$capture=$null;$output=$null;$errorLog=$null}else{$code=1}};if($output){$output.Dispose()};if($errorLog){$errorLog.Dispose()};if($child){$child.Dispose()};$guard.Finish();if($count -ne 0){$code=1};Write-Output ('owned_children_after='+$count)}catch{$code=1}
 if($created){try{Write-S53RootPrivate "$e\result.private.json" ([Text.UTF8Encoding]::new($false).GetBytes((@{attempt=$env:KILOGRAM_S53_COLD_ATTEMPT;exit_code=$code;work_seconds=210;whole_seconds=235;no_restore=$true}|ConvertTo-Json -Compress)))}catch{$code=1}}
}
Write-Output ('native_exit='+$code)
exit $code
