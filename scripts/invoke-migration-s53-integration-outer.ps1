[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('^[CF]-s53-[a-z0-9-]+-01$')][string]$Case,[Parameter(Mandatory)][ValidatePattern('^[CF]-s53-[a-z0-9-]+-01-attempt-[0-9]{2}$')][string]$Attempt,[Parameter(Mandatory)][string]$ActivationFile,[Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$ActivationSha256,[Parameter(Mandatory)][string]$ShellExecutable)
$watch=[Diagnostics.Stopwatch]::StartNew()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence'
function Hash([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
if((Hash $ActivationFile) -cne $ActivationSha256){throw 'Exact root activation required.'}
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
$bindingName=Get-KilogramS53BindingName $Case
$bindingPath=Join-Path $evidence $bindingName;$bindingHash=Hash $bindingPath
$binding=Get-Content -LiteralPath $bindingPath -Raw|ConvertFrom-Json
$activation=Get-Content -LiteralPath $ActivationFile -Raw|ConvertFrom-Json
$allocation=@($activation.cases|Where-Object {$_.case -ceq $Case -and $_.attempt -ceq $Attempt})
if($activation.activated -ne $true -or $activation.binding_sha256 -cne $bindingHash -or $allocation.Count -ne 1){throw 'Inactive exact outer allocation.'}
Assert-KilogramS53G2Policy $Case $binding
if($Case -ceq 'F-s53-capacity-recovery-r2-01'){
    $head=Read-KilogramS53CaseBaseline -Workspace $workspace -Case $Case -Binding $binding
    if($head.status -cne 'verified_match' -or $allocation[0].build_profile -cne 'verification-scale' -or $activation.expected_artifacts_sha256 -cne $binding.expected_artifacts.sha256){throw 'Current G2 HEAD and frozen scale outer allocation required.'}
}
. (Join-Path $PSScriptRoot 'migration-s53-runtime-dependencies.ps1')
if(@($binding.runtime_dependencies).Count -ne 1 -or @($activation.runtime_dependencies).Count -ne 1){throw 'Exactly one reviewed shell dependency required.'}
$shell=Get-KilogramS53ShellDependency -Executable $ShellExecutable -Expected $binding.runtime_dependencies[0] -RequireCurrentHost
Assert-KilogramS53ShellIdentity $shell $activation.runtime_dependencies[0]
$limit=$allocation[0].whole_seconds
$outerPath=Join-Path $evidence ($Attempt+'.outer.json')
if(Test-Path -LiteralPath $outerPath){throw 'Immutable outer attempt exists.'}
. (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
$deadline=[KilogramS5Deadline]::new($limit-5-$watch.Elapsed.TotalSeconds,$limit-$watch.Elapsed.TotalSeconds)
$code=1;$childCode=$null;$cleanup=$false;$errorText=$null;$caseHash=$null;$inputHash=$null
$child=[Diagnostics.Process]::new()
try{
    $child.StartInfo.FileName=$shell.path;$child.StartInfo.UseShellExecute=$false;$child.StartInfo.CreateNoWindow=$true;$child.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
    foreach($arg in @('-NoLogo','-NoProfile','-NonInteractive','-File',(Join-Path $PSScriptRoot 'invoke-migration-s53-integration-case.ps1'),'-Case',$Case,'-Attempt',$Attempt,'-ActivationFile',$ActivationFile,'-ActivationSha256',$ActivationSha256,'-ShellExecutable',$shell.path)){[void]$child.StartInfo.ArgumentList.Add($arg)}
    if(-not $child.Start()){throw 'Outer child start.'};$deadline.VerifyChild($child)
    while(-not $child.WaitForExit(100)){$deadline.CheckWork()}
    $childCode=$child.ExitCode
    $casePath=Join-Path $evidence ($Attempt+'.json');$inputPath=Join-Path $evidence ($Attempt+'.inputs.json')
    $caseHash=Hash $casePath;$inputHash=Hash $inputPath
    $record=Get-Content -LiteralPath $casePath -Raw|ConvertFrom-Json
    if($childCode -ne 0 -or $record.exit_code -ne 0 -or $record.case -cne $Case -or $record.attempt -cne $Attempt -or $record.binding_sha256 -cne $bindingHash -or $record.activation_sha256 -cne $ActivationSha256){throw 'Child case failed.'}
    $code=0
}catch{$errorText=$_.Exception.Message;$code=1}
finally{try{$deadline.CleanupChildren(3000);$cleanup=@($deadline.Children()).Count -eq 0;if(-not $cleanup){throw 'Outer children remain.'}}catch{$code=1;$errorText+=$_.Exception.Message};$child.Dispose()}
try{
    $deadline.CheckWork()
    $null=Get-KilogramS53ShellDependency -Executable $shell.path -Expected $shell -RequireCurrentHost
    $receipt=@{classification='independent outer process outcome; actual final tool return still required';case=$Case;attempt=$Attempt;binding_sha256=$bindingHash;activation_sha256=$ActivationSha256;runtime_dependencies=@($shell);child_runner_exit_code=$childCode;outer_exit_code=$code;case_record_sha256=$caseHash;input_inventory_sha256=$inputHash;cleanup_verified=$cleanup;owned_children_after_cleanup=if($cleanup){0}else{$null};elapsed_seconds=$watch.Elapsed.TotalSeconds;whole_seconds=$limit;error=$errorText}
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $receipt -Depth 8));$file=[IO.File]::Open($outerPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try{$file.Write($raw,0,$raw.Length);$file.Flush($true)}finally{$file.Dispose()}
    $deadline.CheckWork();$deadline.Finish()
}catch{$code=1;[Console]::Error.WriteLine($_.Exception.Message)}
@{case=$Case;attempt=$Attempt;outer_exit_code=$code;actual_final_tool_return_required=$true}|ConvertTo-Json
exit $code
