param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
$cfg=Get-LabConfig B;$want=@{rules=@(Lab-Rules $cfg)}
$profiles=@(foreach($name in @('Domain','Private','Public')){@{name=$name;enabled='False';inbound='Block';outbound='Allow';block_all=$false;disabled=@()}})
$ordinaryFixture=@{backend='com-native-v1';profiles=$profiles;rules=@(foreach($spec in $want.rules){@{name=$spec.Name;view=(Clone $spec)}});
    native_network=@{network=@(@{index=19;alias='fixed';category='Private'});adapter=@{guid=$cfg.interface_guid;name=$cfg.interface_alias;addresses=@('192.168.0.152/255.255.255.0')}}}
$relevant=@{profiles=@(foreach($p in $profiles){@{name=$p.name;store='ActiveStore';enabled=$p.enabled;inbound=$p.inbound;outbound=$p.outbound;allow_local='True';allow_inbound='True';disabled=@()}});
    rules=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($spec in $want.rules){@{store=$store;view=(Clone $spec)}}});
    network=@(@{index=19;alias='fixed';category='Private'});adapter=@{guid=$cfg.interface_guid;name=$cfg.interface_alias;status='Up';addresses=@('192.168.0.152/24')}}
function Get-NetFirewallRule {param($PolicyStore,[switch]$TracePolicyStore,$Name) [pscustomobject]@{PolicyStoreSourceType='Local'}}
$cases=[ordered]@{}
Assert-LabOrdinaryAgainstCim $cfg $want $ordinaryFixture $relevant
$cases.unchanged_valid_projection=$true
$save=${function:Save-LabComRuleComparison}
# Retain production scope validation and real bounded writes, mapping only the
# protected CommonData boundary into this disposable fixture, as in supervisor tests.
$mappedCommon=Join-Path $Evidence 'CommonData'
$writer=$save.ToString().Replace("[Environment]::GetFolderPath('CommonApplicationData')",("'"+$mappedCommon.Replace("'","''")+"'"))
Set-Item Function:Save-LabComRuleComparison ([scriptblock]::Create($writer))
$PackageRoot=Join-Path $mappedCommon ('Kilogram-Lab/SetupPackages/setup-'+('a'*32));[void][IO.Directory]::CreateDirectory($PackageRoot)
$reportPath=Join-Path $PackageRoot 'com-rule-comparison.json'
try{
    foreach($field in @('InterfaceAlias','Program','LocalAddress','LocalPort','LocalUser','Action')){
        $bad=Clone $ordinaryFixture;$bad.rules[0].view.$field='changed'
        $beforeHash=Lab-JsonHash $bad
        Set-LabStage 'fixture-projection'
        Reject {Assert-LabOrdinaryAgainstCim $cfg $want $bad $relevant $PackageRoot} 'LAB_COM_SPEC_DIFFERS'
        $report=Lab-Read $reportPath -MaxBytes 2097152
        Require ($report.rules.Count -eq 2 -and $report.rules[0].differences.Count -eq 1 -and $report.rules[0].differences[0].field -ceq $field) 'exact_field_and_values'
        Require ($report.rules[0].differences[0].actual -ceq 'changed' -and $report.rules[0].native.Count -eq 2 -and $report.stage -ceq 'fixture-projection') 'ordinary_and_both_CIM_views'
        Require ((Lab-JsonHash $bad) -ceq $beforeHash) 'comparison_never_repairs_mismatch'
        $cases['record_'+$field]=$true
    }
    $bad=Clone $ordinaryFixture;$bad.rules[0].view.PSObject.Properties.Remove('Program')
    Reject {Assert-LabOrdinaryAgainstCim $cfg $want $bad $relevant $PackageRoot} 'LAB_COM_SPEC_DIFFERS'
    Require (!(Lab-Read $reportPath).rules[0].differences[0].actual_present) 'missing_property_explicit'
    $cases.missing_field=$true
    Reject {Save-LabComRuleComparison $cfg $want $ordinaryFixture $relevant $Evidence} 'LAB_COM_DIAGNOSTIC_SCOPE'
    Require (!(Test-Path -LiteralPath (Join-Path $Evidence 'com-rule-comparison.json'))) 'no_write_outside_protected_shape'
    $cases.fixed_diagnostic_scope=$true
    $bad=Clone $ordinaryFixture;$bad.rules[0].view.LocalPort='changed'
    $priorError=[Console]::Error;$capture=[IO.StringWriter]::new()
    try{
        [Console]::SetError($capture)
        Reject {Assert-LabOrdinaryAgainstCim $cfg $want $bad $relevant $Evidence} 'LAB_COM_SPEC_DIFFERS'
        Require ($capture.ToString().Contains('LAB_COM_DIAGNOSTIC_UNAVAILABLE:LAB_COM_DIAGNOSTIC_SCOPE')) 'diagnostic_failure_does_not_replace_primary'
    }finally{[Console]::SetError($priorError);$capture.Dispose()}
    $cases.diagnostic_failure_preserves_primary=$true
    $tooMany=Clone $ordinaryFixture;$tooMany.rules+=@($tooMany.rules[0])
    Reject {Get-LabComRuleComparison $cfg $want $tooMany $relevant} 'LAB_COM_COMPARISON_BOUND'
    $cases.rule_count_bound=$true
    # Production Apply/private block: only ordinary/CIM reads and the setter boundary
    # are fixtures. A mismatch must stop before the profile setter is called.
    $tokens=$null;$errors=$null
    $tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
    Require (!$errors.Count) 'installer_parse'
    $assignment=$tree.Find({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true)
    $table=$assignment.Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
    $pair=@($table.KeyValuePairs|Where-Object {$_.Item1.Value -ceq 'Apply'})[0]
    $apply=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()
    function Get-LabRelevant($Cfg,$Want){Clone $relevant}
    function Get-LabOrdinaryState($Cfg,$Want){Clone $script:observed}
    function Set-NetFirewallProfile {param($PolicyStore,$Name,$Enabled) $script:profileWrites++}
    $setupMode=$true;$script:profileWrites=0;$script:observed=Clone $ordinaryFixture;$script:observed.rules[0].view.InterfaceAlias='changed'
    Reject {&$apply 'private' $want} 'LAB_COM_SPEC_DIFFERS'
    Require ($script:profileWrites -eq 0 -and (Get-LabStage) -ceq 'pre-profile-comparison') 'reject_before_profile_change'
    $cases.production_pre_profile_rejection=$true
    $setupActor=$null;$stateDir=Join-Path $Evidence 'profile-state';[void][IO.Directory]::CreateDirectory($stateDir)
    $journalPath=Join-Path $stateDir 'initial-and-journal.json'
    $ssh=Clone $want.rules[1];$ssh.Program=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
    $ssh.Protocol='TCP';$ssh.LocalPort='22';$ssh.RemotePort='Any'
    $profileSnapshot=@{profiles=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($p in $relevant.profiles){$copy=Clone $p;$copy.store=$store;$copy}});
        rules=@();task=$null;policy=@($ssh);ssh=@{running=$true;service_path=$ssh.Program;connection=$null};
        network=@(@{index=$cfg.interface_index;alias=$cfg.interface_alias;category='Private'});adapter=$relevant.adapter;dynamic_udp=@('49152','16384')}
    function Get-LabSnapshot($Cfg,$Want){Clone $profileSnapshot}
    Save-LabInstallJournal $stateDir (Clone @{phase='installing';initial=$profileSnapshot;changes=@('private')}) $null
    $script:observed=Clone $ordinaryFixture;$null=&$apply 'private' $want (Lab-Read $journalPath -MaxBytes 2097152)
    Require ($script:profileWrites -eq 1) 'valid_comparison_reaches_single_setter'
    $cases.production_pre_profile_success=$true
    $diag=Get-LabSetupDiagnostics $PackageRoot (Join-Path $Evidence 'absent.json') ('setup-'+('a'*32))
    Require ($diag.com_rule_comparison.available -and !$diag.com_rule_comparison.truncated) 'ordinary_bounded_export'
    $payload=[Convert]::FromBase64String($diag.com_rule_comparison.chunks[0].data_base64)
    Require ([Convert]::ToBase64String($payload) -ceq [Convert]::ToBase64String([IO.File]::ReadAllBytes($reportPath))) 'exact_export_bytes'
    $cases.public_comparison_bytes=$true
    Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;host_mutations=0;
        scope='production strict comparison/pre-profile routing, real bounded diagnostic files with mapped CommonData; OS reads/setter mocked, no rule registration'}
    @{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
}finally{Set-Item Function:Save-LabComRuleComparison $save}
