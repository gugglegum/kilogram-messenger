param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
function Require([bool]$Value,[string]$Message){if(!$Value){throw "CHECK:$Message"}}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
[void][IO.Directory]::CreateDirectory($Evidence)
$cases=[ordered]@{};$cfg=Get-LabConfig A;$want=@{rules=@(Lab-Rules $cfg)}
# New COM objects remain in memory. Nothing is added to INetFwPolicy2.Rules.
$views=@(foreach($spec in $want.rules){
    $r=New-Object -ComObject HNetCfg.FWRule
    $r.Name=$spec.DisplayName;$r.Description=$spec.Description;$r.Grouping=$spec.Group;$r.Enabled=$true
    $r.Profiles=2;$r.Direction=1;$r.Action=1;$r.EdgeTraversalOptions=0;$r.ApplicationName=$spec.Program
    $r.Protocol=17;$r.LocalPorts=$spec.LocalPort;$r.RemotePorts=$spec.RemotePort
    $r.LocalAddresses=$spec.LocalAddress
    $r.RemoteAddresses=$spec.RemoteAddress
    $r.InterfaceTypes='All'
    $r.Interfaces=[object[]]@($cfg.interface_alias)
    $view=Convert-LabComRule $r $spec.Name
    Require (Lab-Equal $view $spec) 'real_memory_COM_rule_exact_spec'
    $r.SecureFlags=1;Reject {Convert-LabComRule $r $spec.Name} 'LAB_COM_RULE_SECURITY_UNSUPPORTED'
    [ordered]@{name=$spec.Name;view=$view;persistent_sha256=('b'*64)}
})
$cases.real_uninstalled_COM_objects_exact_rule_projection_and_security_reject=$true
$auditPath=Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/prepare-autonomous-install-05/A-current-audit.json'
$ordinaryViewPath=Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/combined-setup-05/A-ordinary-state.json'
Require ((Lab-Hash $auditPath) -ceq 'ec9da44aea31b27139580636712519a8b382dcea74413879c127b1a9f82ad1f5') 'actual_audit_pin'
$audit=(Lab-Read $auditPath -MaxBytes 2097152).snapshot
$ordinaryView=(Lab-Read $ordinaryViewPath).state
$relevant=@{profiles=$audit.profiles;rules=@();network=$audit.network;adapter=$audit.adapter}
Assert-LabOrdinaryAgainstCim $cfg @{rules=@()} $ordinaryView $relevant
$cases.recorded_actual_A_COM_native_profiles_network_match_authoritative_CIM=$true
$ordinaryView.rules=$views
$relevant.rules=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($v in $views){@{store=$store;view=$v.view}}})
function Get-NetFirewallRule {param($PolicyStore,[switch]$TracePolicyStore,$Name) [pscustomobject]@{PolicyStoreSourceType=$script:sourceType}}
$script:sourceType='Local'
Assert-LabOrdinaryAgainstCim $cfg $want $ordinaryView $relevant
foreach($key in @($views[0].view.Keys)){
    $bad=Clone $relevant
    $bad.rules[0].view.$key='unsupported-or-drift'
    Reject {Assert-LabOrdinaryAgainstCim $cfg $want $ordinaryView $bad} 'LAB_COM_CIM_RULE_DIFFERS'
}
$cases.every_own_CIM_field_drift_rejected=$views[0].view.Count
$script:sourceType='GroupPolicy';Reject {Assert-LabOrdinaryAgainstCim $cfg $want $ordinaryView $relevant} 'LAB_COM_POLICY_SOURCE_UNSUPPORTED';$script:sourceType='Local'
$cases.nonlocal_policy_source_rejected=$true
foreach($key in @('enabled','inbound','outbound','allow_inbound','allow_local','disabled')){
    $bad=Clone $relevant
    $row=@($bad.profiles|Where-Object {$_.store -ceq 'ActiveStore' -and $_.name -ceq 'Private'})[0]
    $row.$key=if($key -ceq 'disabled'){@('different-interface')}else{'unsupported-or-drift'}
    Reject {Assert-LabOrdinaryAgainstCim $cfg $want $ordinaryView $bad} 'LAB_COM_CIM_PROFILE_DIFFERS'
}
$cases.effective_profile_drift_rejected=$true
$bad=Clone $ordinaryView;$bad.backend='unproved';Reject {Assert-LabOrdinaryAgainstCim $cfg $want $bad $relevant} 'LAB_COM_PROJECTION_SCHEMA'
$bad=Clone $ordinaryView;$bad.native_network.adapter.guid=[guid]::NewGuid().ToString();Reject {Assert-LabOrdinaryAgainstCim $cfg $want $bad $relevant} 'LAB_COM_CIM_NETWORK_DIFFERS'
$cases.backend_and_network_drift_rejected=$true
# The same production baseline verifier must read current ordinary state on each call.
$fixture=Join-Path $Evidence 'fixture';[void][IO.Directory]::CreateDirectory((Join-Path $fixture 'ordinary-agent'))
$cfg=Clone $cfg;$cfg.base=[IO.Path]::GetFullPath($fixture);$fixedTask=Lab-TaskSpec $cfg
$script:taskFixture=[pscustomobject]@{Xml=$fixedTask.xml;descriptor=$fixedTask.sddl}
$script:taskFixture|Add-Member -MemberType ScriptMethod -Name GetSecurityDescriptor -Value {param($flags) $this.descriptor}
function Find-LabTask($Name){$script:taskFixture}
$script:current=$ordinaryView
function Get-LabOrdinaryState($Cfg,$Want){$script:current}
$baselinePath=Join-Path $fixture 'ordinary-agent/campaign-baseline.json'
Lab-Write $baselinePath @{v=2;task=$fixedTask;initial_audit_sha256=('a'*64);relevant=$relevant;ordinary=$ordinaryView}
$pin=Lab-Hash $baselinePath;$null=Assert-LabReadOnly $cfg $pin
foreach($name in @('persistent_sha256','policy_inputs','profiles','native_network')){
    $script:current=Clone $ordinaryView
    switch($name){
        'persistent_sha256'{$script:current.rules[0].persistent_sha256='c'*64}
        'policy_inputs'{$script:current.policy_inputs.sha256='c'*64}
        'profiles'{$script:current.profiles[1].enabled='False-drift'}
        'native_network'{$script:current.native_network.network[0].network=[guid]::NewGuid().ToString()}
    }
    Reject {Assert-LabReadOnly $cfg $pin} 'LAB_BASELINE_DRIFT'
}
$cases.current_raw_rule_policy_profile_network_drift_detected_with_unchanged_baseline=$true
Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;groups=$cases.Count;host_mutations=0;scope='real in-memory COM rule getters and pinned prior A readbacks; mocked CIM/task boundaries, no installed-state claim'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
