param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
$legacy=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/combined-package-04/package-A'))
function Get-LabLegacyRoot {return $legacy}
$oldPackage=Lab-Read (Join-Path $legacy 'package.json')
Require ((Lab-Hash (Join-Path $legacy 'package.json')) -ceq '5e7b721ccd38192e2dfbb7e674446ab84cb3e8c74de4ffbcb3fc7ab6ae8a39f4') 'original_package04_pin'
$captured=Lab-Read (Join-Path $PSScriptRoot 'fixtures/terminal-t1-3e-task-A-normalized.json')
function MakeTask($Xml){$obj=[pscustomobject]@{Xml=$Xml;descriptor=$captured.sddl};$obj|Add-Member ScriptMethod GetSecurityDescriptor {param($flags) $this.descriptor};return $obj}
function Get-LabSnapshot($Cfg,$Want){return Clone $script:os}
function Find-LabTask($Name){return $script:task}
# Production transaction Read/Write/CanRemove/Undo blocks; only SCM/firewall/agent OS
# boundaries below are synthetic. File cleanup and append-only journal writes are real.
$errors=$null;$tokens=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
$assignment=$tree.Find({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true)
$table=$assignment.Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
$production=@{}
foreach($pair in $table.KeyValuePairs){if($pair.Item1.Value -cin @('Read','Write','Snapshot','CanRemove','Undo')){
    $production[$pair.Item1.Value]=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()
}}
function Stop-LabIdleAgent($Base,$Handles) {}
function Get-NetFirewallRule {param($PolicyStore) @($script:os.rules|Where-Object store -ceq $PolicyStore|ForEach-Object view)}
function Get-LabRuleView($Rule,$Store){return $Rule}
function Remove-NetFirewallRule {param($PolicyStore,$Name) $script:mutations++;$script:os.rules=@($script:os.rules|Where-Object {$_.view.Name -cne $Name});$script:os.policy=@($script:os.policy|Where-Object Name -cne $Name)}
function Set-NetFirewallProfile {param($PolicyStore,$Name,$Enabled) $script:mutations++;foreach($p in $script:os.profiles){if($p.name -ceq $Name){$p.enabled=$Enabled}}}
$folder=[pscustomobject]@{};$folder|Add-Member ScriptMethod DeleteTask {param($Name,$Flags) $script:mutations++;$script:task=$null;$script:os.task=$null}
$service=[pscustomobject]@{};$service|Add-Member ScriptMethod Connect {};$service|Add-Member ScriptMethod GetFolder {param($Path) $folder}
function New-Object {param($ComObject) Require ($ComObject -ceq 'Schedule.Service') 'fixed_task_service';return $service}
$cases=[ordered]@{}
foreach($scenario in @('recovering','rollback_incomplete','owner','want','phase','pins','initial','task','policy','file','intent-order','history-after','initial-after','file-after')){
    $caseDir=Join-Path $Evidence $scenario;$cfg=Get-LabConfig A;$spec=Lab-TaskSpec $cfg;$cfg.base=Join-Path $caseDir 'host'
    $want=@{task=$spec;rules=@(Lab-Rules $cfg);config=$cfg}
    $stateDir=Join-Path $caseDir 'state';[void][IO.Directory]::CreateDirectory($stateDir)
    $agentDir=Join-Path $cfg.base 'ordinary-agent';[void][IO.Directory]::CreateDirectory($agentDir)
    $files=@($oldPackage.files.PSObject.Properties.Name)
    foreach($name in $files){[IO.File]::Copy((Join-Path $legacy $name),(Join-Path $agentDir $name))}
    $initial=@{task=$null;rules=@();policy=@(@{Name='foreign';value='unchanged'});network=@{fixed='network'};adapter=@{fixed='adapter'};dynamic_udp=@('49152','16384');
        profiles=@(@{store='PersistentStore';name='Private';enabled='False'},@{store='ActiveStore';name='Private';enabled='False'})}
    $script:os=Clone $initial;$script:task=MakeTask $captured.xml;$script:os.task=@{present=$true};$script:mutations=0
    $journalPath=Join-Path $stateDir 'initial-and-journal.json';$packagePins=$null;$setupMode=$false;$freshB=$false;$teardownHandles=@{agent='mock-held-idle-lock'}
    $j=[pscustomobject]@{phase=$(if($scenario -ceq 'rollback_incomplete'){'rollback_incomplete'}else{'recovering'});setup_session='setup-e4125cddbe3f4dba9d3017aec19af6cd';
        want=$want;initial=$initial;changes=@('files','task','rule0','rule1','private');rollback_errors=@('LAB_TASK_SCHEMA');file_pins=$oldPackage.files}
    Save-LabInstallJournal $stateDir $j $null
    $historyDir=Join-Path $stateDir 'journal-history';[void][IO.Directory]::CreateDirectory($historyDir)
    $historyPath=Join-Path $historyDir (('a'*32)+'.json');Lab-Write $historyPath @{original='must survive'}
    $initialHash=Lab-Hash (Join-Path $stateDir 'initial.json');$historyHash=Lab-Hash $historyPath
    $expected=$null
    switch($scenario){
        'owner'{$j.setup_session='other';$expected='LAB_OLD_RECOVERY_SCOPE'}
        'want'{$j.want=Clone $want;$j.want.config.side='B';$expected='LAB_OLD_RECOVERY_SCOPE'}
        'phase'{$j.phase='installed';$expected='LAB_OLD_RECOVERY_SCOPE'}
        'pins'{$j.file_pins=Clone $oldPackage.files;$j.file_pins.'lab.json'='0'*64;$expected='LAB_OLD_FILE_PIN_SCOPE'}
        'initial'{Lab-Write (Join-Path $stateDir 'initial.json') @{changed=$true};$expected='LAB_INITIAL_BASELINE_CHANGED'}
        'task'{$script:task=MakeTask ($spec.xml.Replace('LeastPrivilege','HighestAvailable'));$expected='LAB_TASK_DRIFT'}
        'policy'{$script:os.policy[0].value='changed';$expected='LAB_UNRELATED_POLICY_DRIFT'}
        'file'{[IO.File]::AppendAllText((Join-Path $agentDir 'lab.json'),'changed');$expected='LAB_FILE_DRIFT'}
        'intent-order'{$j.changes=@('task','files');$expected='LAB_OLD_CHANGE_SCOPE'}
    }
    Lab-Write $journalPath $j -MaxBytes 2097152
    if($expected){
        Reject {Assert-LabOldRecovery $cfg $want $stateDir} $expected
        Require ($script:mutations -eq 0 -and @(Get-ChildItem -LiteralPath $agentDir -File).Count -eq 11) 'admission_refusal_before_mutation'
    }else{
        $before=Assert-LabOldRecovery $cfg $want $stateDir
        $result=Invoke-LabTransaction $production $want 'Teardown'
        Require ($result.state -ceq 'rolled_back' -and !$script:task -and !@(Get-ChildItem -LiteralPath $agentDir -File).Count) 'actual_old11_cleanup_and_normalized_task'
        $proof=Assert-LabOldRecoveryCompleted $cfg $want $stateDir $before
        Require ($proof.verified -and (Lab-Hash (Join-Path $stateDir 'initial.json')) -ceq $initialHash -and (Lab-Hash $historyPath) -ceq $historyHash) 'immutable_initial_and_existing_history'
        $protectedPackage=Join-Path $caseDir 'new-package';[void][IO.Directory]::CreateDirectory($protectedPackage)
        Lab-Write (Join-Path $protectedPackage 'old-recovery-proof.json') @{before=$before;result=$proof} -MaxBytes 2097152
        if($scenario -ceq 'history-after'){
            Lab-Write $historyPath @{changed=$true};Reject {Assert-LabRecoveredSetup $cfg $want $stateDir $protectedPackage} 'LAB_HISTORY_CHANGED'
        }elseif($scenario -ceq 'file-after'){
            [IO.File]::WriteAllText((Join-Path $agentDir 'lab.json'),'intervening file');Reject {Assert-LabRecoveredSetup $cfg $want $stateDir $protectedPackage} 'LAB_OLD_RECOVERY_FILES_REMAIN'
        }elseif($scenario -ceq 'initial-after'){
            [IO.File]::AppendAllText((Join-Path $stateDir 'initial.json'),' ');Reject {Assert-LabRecoveredSetup $cfg $want $stateDir $protectedPackage} 'LAB_OLD_RECOVERY_NOT_CONFIRMED'
        }else{
            $null=Assert-LabRecoveredSetup $cfg $want $stateDir $protectedPackage
            $rolled=Read-LabJournalStrict $journalPath
            Require (Lab-Equal $rolled.file_pins $oldPackage.files) 'old11_pins_retained_through_terminal_proof'
            $newPins=Clone $oldPackage.files;$newPins|Add-Member NoteProperty 'new-fixed-helper.ps1' ('1'*64)
            $rolled.phase='installing';$rolled.changes=@();$rolled.setup_session='setup-'+('b'*32)
            Save-LabInstallJournal $stateDir $rolled $newPins
            Require ((Lab-Equal (Lab-Read $journalPath).file_pins $newPins) -and (Lab-Hash (Join-Path $stateDir 'initial.json')) -ceq $initialHash) 'new_pins_only_after_proof_original_baseline'
        }
    }
    $cases[$scenario]=$true
}
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;legacy_manifest_sha256=(Lab-Hash (Join-Path $legacy 'package.json'));
    scope='real legacy11 file cleanup/journal/history, production transaction/Undo with captured actual task; only OS boundaries mocked; no live installation'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
