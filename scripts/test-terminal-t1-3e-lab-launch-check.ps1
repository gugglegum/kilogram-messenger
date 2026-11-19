param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
$errors=$null;$tokens=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-launch.ps1'),[ref]$tokens,[ref]$errors)
Require (!$errors.Count) 'source_parse'
$names=@('Invoke-LabSetupChild','Invoke-LabSetupSupervised')
$functions=$tree.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -cin $names},$false)
Require ($functions.Count -eq $names.Count) 'exact_production_functions'
. ([scriptblock]::Create(($functions|ForEach-Object {$_.Extent.Text}) -join "`n"))
$installTree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
$traps=@($installTree.FindAll({param($n) $n -is [Management.Automation.Language.TrapStatementAst]},$false))
Require ($traps.Count -eq 1) 'one_production_error_trap'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
$job=[KilogramLabJob]::new('Local\Kilogram-Lab-LaunchCheck-'+[guid]::NewGuid().ToString('N'))
$cases=[ordered]@{}
try {
    foreach($case in @('success','failure-previous','failure-ours','timeout-ours','pre-intent','recovery-failure','truncated','huge')){
        $fixtureCommon=Join-Path ([IO.Path]::GetFullPath($Evidence)) ($case+'/CommonData')
        $LaunchRoot=Join-Path $fixtureCommon ('Kilogram-Lab/SetupPackages/setup-'+('1'*32));[void][IO.Directory]::CreateDirectory($LaunchRoot)
        $LaunchSide='A';$ManifestPin='a'*64;$SessionPath=Join-Path $LaunchRoot ('setup-'+('1'*32))
        $child=@'
param($Mode,$Side,$PackageRoot,$PackageSha,$SetupSession)
@ERROR_TRAP@
if($Mode -eq 'Teardown'){
 if(Test-Path -LiteralPath (Join-Path $PackageRoot 'recovery-fail.flag')){
  Set-LabStage 'rollback-task';throw 'fixture recovery failed'
 };[Console]::Out.Write('recovery');exit 0
}
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'hang.flag')){Start-Sleep -Seconds 40}
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'pre-intent.flag')){
 Set-LabStage 'policy-audit';throw 'fixture pre-intent failed'
}
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'large.flag')){[Console]::Out.Write(('x'*70000));exit 0}
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'huge.flag')){[Console]::Out.Write(('x'*2097153));exit 0}
[Console]::Out.Write('known stdout');[Console]::Error.Write('known stderr')
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'fail.flag')){exit 1}
exit 0
'@
        $trap=$traps[0].Extent.Text.Replace("[Environment]::GetFolderPath('CommonApplicationData')",("'"+$fixtureCommon.Replace("'","''")+"'"))
        $header=". '"+(Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-common.ps1').Replace("'","''")+"'`n. '"+(Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-diagnostics.ps1').Replace("'","''")+"'`n"+
            '$script:LabOperation=$Mode;$script:LabPrimary=$null;Set-LabStage ''fixture-start''' +"`n"+$trap
        $child=$child.Replace('@ERROR_TRAP@',$header)
        [IO.File]::WriteAllText((Join-Path $LaunchRoot 'test-terminal-t1-3e-lab-install.ps1'),$child,[Text.UTF8Encoding]::new($false))
        if($case -like 'failure-*'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'fail.flag'),'fixture')}
        if($case -like 'timeout-*'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'hang.flag'),'fixture')}
        if($case -ceq 'pre-intent'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'pre-intent.flag'),'fixture')}
        if($case -ceq 'recovery-failure'){foreach($flag in @('fail.flag','recovery-fail.flag')){[IO.File]::WriteAllText((Join-Path $LaunchRoot $flag),'fixture')}}
        if($case -ceq 'truncated'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'large.flag'),'fixture')}
        if($case -ceq 'huge'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'huge.flag'),'fixture')}
        $journal=Join-Path $LaunchRoot 'initial-and-journal.json'
        if($case -cne 'pre-intent'){
            Lab-Write $journal @{phase='installing';setup_session=$(if($case -ceq 'failure-previous'){'setup-'+('2'*32)}else{Split-Path -Leaf $SessionPath});
                initial=@{sentinel='DO_NOT_EXPORT_INITIAL'};want=@{sentinel='DO_NOT_EXPORT_WANT'};rollback_errors=@('fixture rollback detail')}
        }
        $before=if(Test-Path -LiteralPath $journal){Lab-Hash $journal}else{''}
        $result=Invoke-LabSetupSupervised $job @{host='fixture'} $SessionPath $journal 'Setup' 2 4
        if($before){Require ((Lab-Hash $journal) -ceq $before) 'supervisor_preserves_journal'}else{Require (!(Test-Path -LiteralPath $journal)) 'pre_intent_no_journal_creation'}
        Require (!@($job.Children()).Count) 'empty_native_job_after_capture'
        if($case -cin @('success','truncated','huge')){Require ($result.success -and !$result.recovery) 'success_without_recovery'}
        elseif($case -ceq 'failure-previous'){Require (!$result.success -and !$result.recovery -and $result.main.exit -eq 1) 'no_recovery_of_previous_or_other_setup'}
        elseif($case -ceq 'pre-intent'){Require (!$result.success -and !$result.recovery -and $result.diagnostics.failures['main-error.json'].primary.stage -ceq 'policy-audit') 'pre_intent_primary_published'}
        elseif($case -ceq 'recovery-failure'){Require (!$result.success -and $result.recovery.exit -eq 1 -and $result.diagnostics.failures['recovery-error.json'].primary.stage -ceq 'rollback-task') 'recovery_failure_published'}
        else{Require (!$result.success -and $result.recovery.exit -eq 0) 'owned_recovery_preserves_failure'}
        if($case -ceq 'timeout-ours'){Require $result.main.timeout 'timeout_native_guard'}
        elseif($case -cin @('truncated','huge')){
            $sample=$result.diagnostics.logs['main.stdout.log']
            Require ($sample.available -and $sample.truncated -and $sample.exported_bytes -eq 65536 -and $sample.chunks.Count -eq 2) 'bounded_head_tail_output'
            Require ($sample.full_hash_complete -eq ($case -ceq 'truncated')) 'bounded_full_source_hash'
        }
        elseif($case -cin @('pre-intent','recovery-failure')){Require $result.main.capture_complete 'failure_capture_complete'}
        else{
            Require ([IO.File]::ReadAllText($result.main.stdout) -ceq 'known stdout') 'actual_stdout_bytes'
            Require ([IO.File]::ReadAllText($result.main.stderr) -ceq 'known stderr') 'actual_stderr_bytes'
        }
        $public=ConvertTo-Json -InputObject $result -Depth 18 -Compress
        Require (!$public.Contains('DO_NOT_EXPORT_')) 'initial_and_want_never_exported'
        Require ([Text.Encoding]::UTF8.GetByteCount($public) -lt 2097152) 'public_explicit_size_bound'
        $cases[$case]=$result
    }
    $savedChild=${function:Invoke-LabSetupChild}
    try{
        function Invoke-LabSetupChild {param($Operation,$Seconds,$SessionJob,$LogPrefix) @{exit=0;timeout=$false;capture_complete=$false;error='fixture missing EOF'}}
        $missing=Invoke-LabSetupSupervised $job @{host='fixture'} 'setup-other' (Join-Path $Evidence 'absent-journal.json') 'Setup' 2 4
        Require (!$missing.success -and !$missing.recovery) 'zero_exit_without_capture_not_success'
        $cases.incomplete_capture_zero_exit_rejected=$true
    }finally{Set-Item Function:Invoke-LabSetupChild $savedChild}
    Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;count=$cases.Count;host_mutations=0;scope='production supervisor/guard/capture, synthetic fixed child; no UAC/task/firewall installation'} -MaxBytes 2097152
    @{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
}finally{$job.Dispose()}
