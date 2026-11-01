[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputFile,[Parameter(Mandatory)][string]$ShellExecutable)
# Non-network dependency probe only. No runtime2_process test or fixture.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
Set-Location $workspace
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-runtime-dependencies.ps1')
. (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
$shell=Get-KilogramS53ShellDependency -Executable $ShellExecutable -RequireCurrentHost
$memory=Get-KilogramS53MemoryShellDependency $workspace
if(Test-Path -LiteralPath (Join-Path ([IO.Path]::GetDirectoryName($shell.path)) 'powershell.exe')){throw 'Probe application-directory shadow differs from harness search.'}
$deadline=[KilogramS5Deadline]::new(6,9)
$watch=[Diagnostics.Stopwatch]::StartNew();$p=[Diagnostics.Process]::new();$record=$null
try{
    $p.StartInfo.FileName='powershell.exe';$p.StartInfo.UseShellExecute=$false;$p.StartInfo.CreateNoWindow=$true;$p.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
    $p.StartInfo.WorkingDirectory=$workspace;$p.StartInfo.RedirectStandardOutput=$true;$p.StartInfo.RedirectStandardError=$true
    $code='[ordered]@{executable=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName;private_bytes=(Get-Process -Id '+$PID+').PrivateMemorySize64}|ConvertTo-Json -Compress'
    foreach($a in @('-NoProfile','-NonInteractive','-Command',$code)){[void]$p.StartInfo.ArgumentList.Add($a)}
    if(-not $p.Start()){throw 'Memory-shell probe start failed.'};$deadline.VerifyChild($p)
    $childId=$p.Id;$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
    if(-not $p.WaitForExit(4000)){throw 'Memory-shell probe deadline4s.'}
    if(-not [Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($stdout,$stderr)).Wait(500)){throw 'Memory-shell probe stream deadline0.5s.'}
    $raw=$stdout.GetAwaiter().GetResult();$err=$stderr.GetAwaiter().GetResult()
    if($p.ExitCode -ne 0 -or $raw.Length -gt 4096 -or $err.Length -gt 4096){throw 'Memory query output/exit invalid.'}
    $value=$raw|ConvertFrom-Json
    if($value.executable -cne $memory.path -or [long]$value.private_bytes -le 0){throw 'Actual memory-shell resolution/query differs.'}
    $record=[ordered]@{classification='bounded non-network G4 memory-query dependency probe';shell=$shell;memory_shell=$memory;actual_executable=$value.executable;queried_private_bytes=[long]$value.private_bytes;owned_probe_pid=$childId;actual_exit_code=$p.ExitCode;job_membership_verified=$true;elapsed_seconds=$watch.Elapsed.TotalSeconds;test_selectors_executed=0;coverage='Runtime query is synchronous without local timeout; existing runtime case/outer jobs bound and reap inherited child. Probe used the same bare executable and no-profile/noninteractive flags.'}
}finally{
    $deadline.CleanupChildren(1000);$remaining=@($deadline.Children()).Count
    if($null -ne $record){$record.cleanup_verified=$remaining -eq 0;$record.owned_children_after_cleanup=$remaining}
    $p.Dispose();$deadline.Finish()
}
. (Join-Path $PSScriptRoot 'cargo-frozen-artifacts.ps1')
Write-KilogramFrozenJson $OutputFile $record
$record|ConvertTo-Json -Depth 5
