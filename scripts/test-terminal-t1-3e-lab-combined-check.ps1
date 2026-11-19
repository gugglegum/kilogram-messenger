param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
$errors=$null;$tokens=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-launch.ps1'),[ref]$tokens,[ref]$errors)
Require (!$errors.Count) 'source_parse'
$names=@('Invoke-LabSetupSupervised','Invoke-LabRecoverySetupSupervised')
$nodes=@($tree.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -cin $names},$false))
Require ($nodes.Count -eq 2) 'production_routing_functions'
. ([scriptblock]::Create(($nodes|ForEach-Object {$_.Extent.Text}) -join "`n"))
$job=[pscustomobject]@{};$job|Add-Member ScriptMethod Children {return @()}
function Invoke-LabSetupChild($Operation,$Seconds,$SessionJob,$LogPrefix){
    $script:calls+=,@{operation=$Operation;seconds=$Seconds}
    [IO.File]::WriteAllText($LogPrefix+'.stdout.log',$Operation)
    [IO.File]::WriteAllText($LogPrefix+'.stderr.log','')
    if($Operation -ceq 'RecoverOld'){
        if($scenario -ceq 'old-exception'){throw 'fixture child launch failure'}
        if($scenario -cne 'old-no-proof'){
            Lab-Write (Join-Path $LaunchRoot 'old-recovery-proof.json') @{result=@{verified=$true;phase='rolled_back';
                owner=$(if($scenario -ceq 'old-wrong-owner'){'other'}else{'setup-e4125cddbe3f4dba9d3017aec19af6cd'});
                owned_absent=$true;history_preserved=$true;old_file_pins_preserved=$true}}
        }
        return @{exit=$(if($scenario -ceq 'old-failure'){1}else{0});timeout=($scenario -ceq 'old-timeout');capture_complete=($scenario -cne 'old-no-eof')}
    }
    if($scenario -ceq 'setup-failure-owned'){
        if($Operation -cne 'Teardown'){
            Lab-Write $JournalPath @{phase='rollback_incomplete';setup_session=(Split-Path -Leaf $InstallSession)}
            return @{exit=1;timeout=$false;capture_complete=$true}
        }
    }
    return @{exit=0;timeout=$false;capture_complete=$true}
}
$cases=[ordered]@{}
foreach($scenario in @('A-success','B-success','old-failure','old-timeout','old-no-eof','old-no-proof','old-wrong-owner','old-exception','B-existing-intent','B-malformed-intent','setup-failure-owned')){
    $LaunchRoot=Join-Path $Evidence $scenario;[void][IO.Directory]::CreateDirectory($LaunchRoot)
    $JournalPath=Join-Path $LaunchRoot 'initial-and-journal.json';$InstallSession='setup-'+('9'*32);$script:calls=@()
    $config=@{host='fixture';side=$(if($scenario.StartsWith('B-')){'B'}else{'A'})}
    if($scenario -ceq 'B-existing-intent'){Lab-Write $JournalPath @{phase='installing';setup_session='old'}}
    if($scenario -ceq 'B-malformed-intent'){[IO.File]::WriteAllText($JournalPath,'{invalid')}
    $result=Invoke-LabRecoverySetupSupervised $job $config $InstallSession $JournalPath
    $operations=@($calls|ForEach-Object operation)
    if($scenario -ceq 'A-success'){
        Require ($result.success -and ($operations -join ',') -ceq 'RecoverOld,SetupRecoveredA') 'old_cleanup_precedes_fresh_A'
        Require ($calls[0].seconds -eq 180 -and $calls[1].seconds -eq 450) 'fixed_phase_budgets'
    }elseif($scenario -ceq 'B-success'){
        Require ($result.success -and ($operations -join ',') -ceq 'SetupFreshB') 'B_no_old_cleanup'
    }elseif($scenario -ceq 'setup-failure-owned'){
        Require (!$result.success -and ($operations -join ',') -ceq 'RecoverOld,SetupRecoveredA,Teardown' -and $calls[2].seconds -eq 180) 'fallback_only_new_intent_owner'
    }else{
        Require (!$result.success -and !@($operations|Where-Object {$_ -cin @('SetupRecoveredA','SetupFreshB','Teardown')}).Count) 'uncertainty_prevents_setup_and_unowned_cleanup'
        Require ([bool]$result.combined_failure) 'readable_phase_failure'
    }
    Require ($result.bounds.active_seconds -eq 810 -and $result.bounds.overhead_seconds -eq 270 -and
        $result.bounds.actor_seconds -gt $result.bounds.wrapper_seconds) 'enclosing_budgets'
    if($config.side -ceq 'A'){Require $result.diagnostics.logs['old-recovery.stdout.log'].available 'old_phase_ordinary_export'}
    Lab-Write (Join-Path $LaunchRoot 'result.json') $result -MaxBytes 2097152
    $cases[$scenario]=@{success=$result.success;operations=$operations}
}
# Exercise the actual installer Read block: an intervening B intent is still rejected
# while the caller holds the exclusive install lock, before Snapshot or Write.
$installer=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
$assignment=@($installer.FindAll({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true))[0]
$table=$assignment.Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
$pair=@($table.KeyValuePairs|Where-Object {$_.Item1.Value -ceq 'Read'})[0]
$read=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()
$journalPath=Join-Path $Evidence 'B-race.json';Lab-Write $journalPath @{phase='installing'};$freshB=$true
$raceLock=[IO.File]::Open((Join-Path $Evidence 'B-race.lock'),'CreateNew','Write','None')
$rejected=$false
try{try{&$read}catch{if($_.Exception.Message -ceq 'LAB_B_INTENT_ALREADY_EXISTS'){$rejected=$true}else{throw}}}finally{$raceLock.Dispose()}
Require $rejected 'B_race_intent_refused_in_production_read'
$cases.B_intervening_intent=$true
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;scope='production routing; synthetic child/privileged observations, no UAC or task/policy writes'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
