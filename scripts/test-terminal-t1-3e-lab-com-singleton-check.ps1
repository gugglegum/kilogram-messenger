param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
$fixture=Join-Path $PSScriptRoot 'fixtures/terminal-t1-3e-B-com-singleton.json';$captured=Lab-Read $fixture -MaxBytes 2097152
$tokens=$null;$errors=$null
$oldPath=Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/B-rollback-retry-package-03/package-B/test-terminal-t1-3e-lab-readiness.ps1'
$tree=[Management.Automation.Language.Parser]::ParseFile($oldPath,[ref]$tokens,[ref]$errors)
$old=$tree.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Convert-LabComRule'},$false)
. ([scriptblock]::Create($old.Extent.Text.Replace('function Convert-LabComRule(','function Convert-LabComRuleBefore(')))
function RawCom($View){
    # Exact captured post-conversion values with the already-proven default COM
    # enum/security fields restored; no new registered rule or inferred interface mapping.
    [pscustomobject]@{Name=$View.DisplayName;Description=$View.Description;Grouping=$View.Group;Enabled=$true;Profiles=2;Direction=1;Action=1;EdgeTraversalOptions=0;
        ApplicationName=$View.Program;Protocol=17;InterfaceTypes='All';Interfaces=@($View.InterfaceAlias);SecureFlags=0;LocalUserOwner=$null;
        LocalPorts=$View.LocalPort;RemotePorts=$View.RemotePort;LocalAddresses=$View.LocalAddress;RemoteAddresses=$View.RemoteAddress;
        ServiceName=$null;LocalAppPackageId=$null;LocalUserAuthorizedList=$null;RemoteUserAuthorizedList=$null;RemoteMachineAuthorizedList=$null}
}
$cases=[ordered]@{}
foreach($row in $captured.rules){
    $raw=RawCom $row.ordinary;$before=Convert-LabComRuleBefore $raw $row.name
    Require (Lab-Equal $before $row.ordinary) 'exact_recorded_old_view_reproduced'
    Require (!(Lab-Equal $before $row.expected[0])) 'recorded_failure_reproduced_before_fix'
    $after=Convert-LabComRule $raw $row.name
    Require (Lab-Equal $after $row.expected[0]) 'singleton_only_correction_matches_spec'
    foreach($native in $row.native){Require (Lab-Equal $after $native.view) 'both_actual_CIM_stores_match'}
    $cases[$row.name]=@{before_matches_spec=$false;after_matches_spec=$true;observed_differences=$row.differences}
}
foreach($value in @('0.0.0.0','192.168.0.140','239.255.75.72','255.255.255.255')){
    Require ((Convert-LabComSingletonAddress ($value+'-'+$value)) -ceq $value) 'canonical_singleton_equivalence'
    $cases['singleton_'+$value]=$true
}
foreach($value in @('192.168.0.140-192.168.0.141','192.168.0.141-192.168.0.140','192.168.0.140-192.168.0.140,192.168.0.141',
    '192.168.0.140-192.168.0.140/32','192.168.0.140-192.168.0.140/24','256.1.2.3-256.1.2.3','192.168.00.140-192.168.00.140',
    '192.168.0.140 -192.168.0.140','::1-::1','192.168.0.140/32','Any','LocalSubnet','127.1-127.1')){
    Require ((Convert-LabComSingletonAddress $value) -ceq $value) 'no_other_range_or_syntax_normalization'
    $cases['unchanged_'+$value]=$true
}
# Production full comparison, including both CIM stores/source/network, on captured
# rule views. Only the effective-source cmdlet is a privileged observation fixture.
$cfg=Get-LabConfig B;$want=@{rules=@($captured.rules|ForEach-Object {$_.expected[0]})}
$profiles=@(foreach($name in @('Domain','Private','Public')){@{name=$name;enabled='False';inbound='Block';outbound='Allow';block_all=$false;disabled=@()}})
$projection=@{backend='com-native-v1';profiles=$profiles;rules=@($captured.rules|ForEach-Object {@{name=$_.name;view=(Convert-LabComRule (RawCom $_.ordinary) $_.name)}});
    native_network=@{network=@(@{index=19;alias=$cfg.interface_alias;category='Private'});adapter=@{guid=$cfg.interface_guid;name=$cfg.interface_alias;addresses=@('192.168.0.152/255.255.255.0')}}}
$relevant=@{profiles=@($profiles|ForEach-Object {@{name=$_.name;store='ActiveStore';enabled=$_.enabled;inbound=$_.inbound;outbound=$_.outbound;allow_local='True';allow_inbound='True';disabled=@()}});
    rules=@($captured.rules|ForEach-Object native);network=@(@{index=19;alias=$cfg.interface_alias;category='Private'});adapter=@{guid=$cfg.interface_guid;name=$cfg.interface_alias;status='Up';addresses=@('192.168.0.152/24')}}
function Get-NetFirewallRule {param($PolicyStore,[switch]$TracePolicyStore,$Name) [pscustomobject]@{PolicyStoreSourceType='Local'}}
Assert-LabOrdinaryAgainstCim $cfg $want $projection $relevant;$cases.production_full_comparison_accepts_actual_equivalent_view=$true
foreach($change in @('wider','different-singleton','interface','port','action','user','CIM')){
    $bad=Clone $projection;$native=Clone $relevant
    switch($change){
        'wider'{$raw=RawCom $captured.rules[0].ordinary;$raw.RemoteAddresses='192.168.0.140-192.168.0.141';$bad.rules[0].view=Convert-LabComRule $raw $bad.rules[0].name}
        'different-singleton'{$raw=RawCom $captured.rules[0].ordinary;$raw.RemoteAddresses='192.168.0.141-192.168.0.141';$bad.rules[0].view=Convert-LabComRule $raw $bad.rules[0].name}
        'interface'{$bad.rules[0].view.InterfaceAlias='other'}
        'port'{$bad.rules[0].view.RemotePort='Any'}
        'action'{$bad.rules[0].view.Action='Block'}
        'user'{$bad.rules[0].view.LocalUser='conditional'}
        'CIM'{$native.rules[0].view.RemoteAddress='192.168.0.141'}
    }
    Reject {Assert-LabOrdinaryAgainstCim $cfg $want $bad $native} $(if($change -ceq 'CIM'){'LAB_COM_CIM_RULE_DIFFERS'}else{'LAB_COM_SPEC_DIFFERS'})
    $cases['strict_'+$change]=$true
}
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;fixture_sha256=(Lab-Hash $fixture);host_mutations=0;
    scope='captured actual COM/spec/both CIM views; unchanged published converter reproduces mismatch, corrected converter preserves strict other fields; no rule registration'} -MaxBytes 2097152
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
