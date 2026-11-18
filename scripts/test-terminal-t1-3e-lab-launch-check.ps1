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
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
$job=[KilogramLabJob]::new('Local\Kilogram-Lab-LaunchCheck-'+[guid]::NewGuid().ToString('N'))
$cases=[ordered]@{}
try {
    foreach($case in @('success','failure-previous','failure-ours','timeout-ours')){
        $LaunchRoot=Join-Path ([IO.Path]::GetFullPath($Evidence)) $case;[void][IO.Directory]::CreateDirectory($LaunchRoot)
        $LaunchSide='A';$ManifestPin='a'*64;$SessionPath=Join-Path $LaunchRoot ('setup-'+('1'*32))
        $child=@'
param($Mode,$Side,$PackageRoot,$PackageSha,$SetupSession)
if($Mode -eq 'Teardown'){[Console]::Out.Write('recovery');exit 0}
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'hang.flag')){Start-Sleep -Seconds 40}
[Console]::Out.Write('known stdout');[Console]::Error.Write('known stderr')
if(Test-Path -LiteralPath (Join-Path $PackageRoot 'fail.flag')){exit 1}
exit 0
'@
        [IO.File]::WriteAllText((Join-Path $LaunchRoot 'test-terminal-t1-3e-lab-install.ps1'),$child,[Text.UTF8Encoding]::new($false))
        if($case -like 'failure-*'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'fail.flag'),'fixture')}
        if($case -like 'timeout-*'){[IO.File]::WriteAllText((Join-Path $LaunchRoot 'hang.flag'),'fixture')}
        $journal=Join-Path $LaunchRoot 'initial-and-journal.json'
        Lab-Write $journal @{phase='installing';setup_session=$(if($case -ceq 'failure-previous'){'setup-'+('2'*32)}else{Split-Path -Leaf $SessionPath})}
        $before=Lab-Hash $journal
        $result=Invoke-LabSetupSupervised $job @{host='fixture'} $SessionPath $journal 'Setup' 2 4
        Require ((Lab-Hash $journal) -ceq $before) 'supervisor_preserves_journal'
        Require (!@($job.Children()).Count) 'empty_native_job_after_capture'
        if($case -ceq 'success'){Require ($result.success -and !$result.recovery) 'success_without_recovery'}
        elseif($case -ceq 'failure-previous'){Require (!$result.success -and !$result.recovery -and $result.main.exit -eq 1) 'no_recovery_of_previous_or_other_setup'}
        else{Require (!$result.success -and $result.recovery.exit -eq 0) 'owned_recovery_preserves_failure'}
        if($case -ceq 'timeout-ours'){Require $result.main.timeout 'timeout_native_guard'}
        else{
            Require ([IO.File]::ReadAllText($result.main.stdout) -ceq 'known stdout') 'actual_stdout_bytes'
            Require ([IO.File]::ReadAllText($result.main.stderr) -ceq 'known stderr') 'actual_stderr_bytes'
        }
        $cases[$case]=$result
    }
    Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;count=$cases.Count;host_mutations=0;scope='production supervisor/guard/capture, synthetic fixed child; no UAC/task/firewall installation'}
    @{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
}finally{$job.Dispose()}
