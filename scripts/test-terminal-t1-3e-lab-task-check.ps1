param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
$script:caseName='initial'
function Reject([scriptblock]$Body){try{&$Body}catch{return $_.Exception.Message};throw "CHECK:tamper_accepted:$script:caseName"}
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
[void][IO.Directory]::CreateDirectory($Evidence)
$fixturePath=Join-Path $PSScriptRoot 'fixtures/terminal-t1-3e-task-A-normalized.json'
$captured=Lab-Read $fixturePath
$spec=Lab-TaskSpec (Get-LabConfig A)
function MakeTask([string]$Xml,[string]$Dacl){
    $o=[pscustomobject]@{Xml=$Xml;descriptor=$Dacl}
    $o|Add-Member -MemberType ScriptMethod -Name GetSecurityDescriptor -Value {param($flags) $this.descriptor}
    return $o
}
$task=MakeTask $captured.xml $captured.sddl
Assert-LabTask $task $spec
$cases=[ordered]@{captured_actual_scheduler_xml_defaults_alias_and_mapped_dacl=$true}
Assert-LabTask (MakeTask $spec.xml $spec.sddl) $spec
$cases.explicit_generated_task_still_accepted=$true
$task.Xml=$spec.xml.Replace('<UserId>'+$spec.sid+'</UserId>','<UserId>DESKTOP-PAUL-RY\Paul</UserId>')
Assert-LabTask $task $spec
$cases.principal_and_trigger_account_alias_resolve_exact_sid=$true
$mutations=@(
 @('run-level','/t:Task/t:Principals/t:Principal','RunLevel','HighestAvailable'),
 @('principal-user','/t:Task/t:Principals/t:Principal','UserId','S-1-5-18'),
 @('trigger-user','/t:Task/t:Triggers/t:LogonTrigger','UserId','S-1-1-0'),
 @('trigger-disabled','/t:Task/t:Triggers/t:LogonTrigger','Enabled','false'),
 @('settings-disabled','/t:Task/t:Settings','Enabled','false'),
 @('demand-disabled','/t:Task/t:Settings','AllowStartOnDemand','false'),
 @('logon-type','/t:Task/t:Principals/t:Principal','LogonType','S4U'),
 @('action-path','/t:Task/t:Actions/t:Exec','Command','C:\wrong.exe'),
 @('action-args','/t:Task/t:Actions/t:Exec','Arguments','wrong'),
 @('action-directory','/t:Task/t:Actions/t:Exec','WorkingDirectory','C:\wrong'),
 @('hard-terminate','/t:Task/t:Settings','AllowHardTerminate','true')
)
foreach($m in $mutations){
    [xml]$x=$captured.xml;$n=[Xml.XmlNamespaceManager]::new($x.NameTable);$n.AddNamespace('t',$x.DocumentElement.NamespaceURI)
    $parent=$x.SelectSingleNode($m[1],$n);$value=$parent.SelectSingleNode('t:'+$m[2],$n)
    if(!$value){$value=$x.CreateElement($m[2],$x.DocumentElement.NamespaceURI);$null=$parent.AppendChild($value)}
    $value.InnerText=$m[3]
    $cases[$m[0]]=Reject {Assert-LabTask (MakeTask $x.OuterXml $captured.sddl) $spec}
}
foreach($kind in @('duplicate-default','duplicate-principal','duplicate-trigger','duplicate-action','missing-context','wrong-context','missing-user','group-id','required-privileges')){
    $script:caseName=$kind
    [xml]$x=$captured.xml;$n=[Xml.XmlNamespaceManager]::new($x.NameTable);$n.AddNamespace('t',$x.DocumentElement.NamespaceURI)
    switch($kind){
        'duplicate-default'{$p=$x.SelectSingleNode('/t:Task/t:Settings',$n);foreach($v in 1..2){$e=$x.CreateElement('Enabled',$x.DocumentElement.NamespaceURI);$e.InnerText='true';$null=$p.AppendChild($e)}}
        'duplicate-principal'{$p=$x.SelectSingleNode('/t:Task/t:Principals',$n);$null=$p.AppendChild($p.FirstChild.CloneNode($true))}
        'duplicate-trigger'{$p=$x.SelectSingleNode('/t:Task/t:Triggers',$n);$null=$p.AppendChild($p.FirstChild.CloneNode($true))}
        'duplicate-action'{$p=$x.SelectSingleNode('/t:Task/t:Actions',$n);$null=$p.AppendChild($p.FirstChild.CloneNode($true))}
        'missing-context'{$x.SelectSingleNode('/t:Task/t:Actions',$n).RemoveAttribute('Context')}
        'wrong-context'{$x.SelectSingleNode('/t:Task/t:Actions',$n).SetAttribute('Context','Other')}
        'missing-user'{$p=$x.SelectSingleNode('/t:Task/t:Principals/t:Principal',$n);$null=$p.RemoveChild($p.SelectSingleNode('t:UserId',$n))}
        'group-id'{$p=$x.SelectSingleNode('/t:Task/t:Principals/t:Principal',$n);$e=$x.CreateElement('GroupId',$x.DocumentElement.NamespaceURI);$e.InnerText='S-1-5-32-544';$null=$p.AppendChild($e)}
        'required-privileges'{$p=$x.SelectSingleNode('/t:Task/t:Principals/t:Principal',$n);$e=$x.CreateElement('RequiredPrivileges',$x.DocumentElement.NamespaceURI);$null=$p.AppendChild($e)}
    }
    $cases[$kind]=Reject {Assert-LabTask (MakeTask $x.OuterXml $captured.sddl) $spec}
}
$daclCases=[ordered]@{widened=$spec.sddl.Replace('GRGX','FA');missing_execute=$spec.sddl.Replace('GRGX','GR');unprotected=$spec.sddl.Replace('D:P','D:');
    foreign=($spec.sddl+'(A;;GR;;;WD)');duplicate=($spec.sddl+'(A;;GRGX;;;'+$spec.sid+')')}
foreach($label in $daclCases.Keys){
    $script:caseName='dacl_'+$label
    $cases[$script:caseName]=Reject {Assert-LabTask (MakeTask $captured.xml $daclCases[$label]) $spec}
}
# Exercise the production task Undo branch with a captured task and a mocked COM delete
# boundary. No real Schedule.Service is created and no task/file/agent is touched.
$t=$null;$e=$null;$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$t,[ref]$e)
$assignment=$tree.Find({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true)
$table=$assignment.Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
$undo=$null;foreach($pair in $table.KeyValuePairs){if($pair.Item1.Value -ceq 'Undo'){$undo=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()}}
Require ($null -ne $undo) 'production_undo_found'
$script:deleted=0;$script:recoveryFixture=MakeTask $captured.xml $captured.sddl
$script:fakeFolder=[pscustomobject]@{}
$script:fakeFolder|Add-Member -MemberType ScriptMethod -Name DeleteTask -Value {param($name,$flags) if($name -cne 'Kilogram-Lab-t13e-lab-A' -or $flags -ne 0){throw 'CHECK:wrong_delete_target'};$script:deleted++}
$script:fakeService=[pscustomobject]@{}
$script:fakeService|Add-Member -MemberType ScriptMethod -Name Connect -Value {}
$script:fakeService|Add-Member -MemberType ScriptMethod -Name GetFolder -Value {param($path) if($path -cne '\'){throw 'CHECK:wrong_task_folder'};return $script:fakeFolder}
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
& {
    function Find-LabTask($Name){$script:recoveryFixture}
    function New-Object {param($ComObject) if($ComObject -cne 'Schedule.Service'){throw 'CHECK:unexpected_COM'};return $script:fakeService}
    $teardownHandles=@{agent='fixture-held-lock'}
    $null=& $undo 'task' @{task=$spec} $null
    Require ($script:deleted -eq 1) 'normalized_owned_task_recovery_reaches_mock_delete'
    $script:recoveryFixture=MakeTask ($spec.xml.Replace('LeastPrivilege','HighestAvailable')) $captured.sddl
    $script:caseName='recovery_wrong_privilege'
    $null=Reject {& $undo 'task' @{task=$spec} $null}
    Require ($script:deleted -eq 1) 'unsafe_task_rejected_before_mock_delete'
}
$cases.production_recovery_uses_semantics_and_rejects_unsafe_task=$true
Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;count=$cases.Count;fixture_sha256=(Lab-Hash $fixturePath);host_mutations=0;
    scope='captured actual XML/DACL with documented omitted defaults and exact account resolution; no live task run/delete/register'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
