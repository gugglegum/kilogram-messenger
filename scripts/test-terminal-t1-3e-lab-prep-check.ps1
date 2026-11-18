param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK_EVIDENCE_EXISTS'}
[void][IO.Directory]::CreateDirectory($Evidence)
trap{Lab-Write (Join-Path $Evidence 'failure.json') @{message=$_.Exception.Message;stack=$_.ScriptStackTrace};exit 1}
function Require([bool]$Value,[string]$Reason){if(!$Value){throw "CHECK:$Reason"}}
function Reject([scriptblock]$Body,[string]$Expected){try{&$Body}catch{if($_.Exception.Message.Contains($Expected)){return};throw};throw "CHECK:not_rejected_$Expected"}
Lab-Write (Join-Path $Evidence 'source.json') @{installer_sha256=(Lab-Hash "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1")}
$cfg=Get-LabConfig A
$rules=@(Lab-Rules $cfg | ForEach-Object {[pscustomobject]$_})
foreach($rule in $rules){$rule|Add-Member -NotePropertyName InstanceID -NotePropertyValue $rule.Name}
$kinds=[ordered]@{Application=@('Program','Package');Service=@('Service');Port=@('Protocol','LocalPort','RemotePort','DynamicTransport');
    Address=@('LocalAddress','RemoteAddress');Interface=@('InterfaceAlias');InterfaceType=@('InterfaceType');
    Security=@('Authentication','Encryption','OverrideBlockRules','LocalUser','RemoteUser','RemoteMachine')}
$filterData=@{};$calls=@{}
foreach($kind in $kinds.Keys){
    $filterData[$kind]=@(foreach($rule in $rules){$row=@{InstanceID=$rule.InstanceID};foreach($key in $kinds[$kind]){$row[$key]=$rule.$key};[pscustomobject]$row})
    $extra=@{InstanceID='unrelated'};foreach($key in $kinds[$kind]){$extra[$key]='unused'}
    $filterData[$kind]+=@([pscustomobject]$extra);$calls[$kind]=0
}
function Fixture-Filter([string]$Kind,$Rule){if($Rule){$ids=@($Rule|ForEach-Object InstanceID);$filterData[$Kind]|Where-Object {$_.InstanceID -cin $ids}}else{$filterData[$Kind]}}
function Get-NetFirewallApplicationFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.Application++}process{Fixture-Filter Application $AssociatedNetFirewallRule}}
function Get-NetFirewallServiceFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.Service++}process{Fixture-Filter Service $AssociatedNetFirewallRule}}
function Get-NetFirewallPortFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.Port++}process{Fixture-Filter Port $AssociatedNetFirewallRule}}
function Get-NetFirewallAddressFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.Address++}process{Fixture-Filter Address $AssociatedNetFirewallRule}}
function Get-NetFirewallInterfaceFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.Interface++}process{Fixture-Filter Interface $AssociatedNetFirewallRule}}
function Get-NetFirewallInterfaceTypeFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.InterfaceType++}process{Fixture-Filter InterfaceType $AssociatedNetFirewallRule}}
function Get-NetFirewallSecurityFilter($PolicyStore,[Parameter(ValueFromPipeline)]$AssociatedNetFirewallRule,$ThrottleLimit){begin{$calls.Security++}process{Fixture-Filter Security $AssociatedNetFirewallRule}}
$index=New-LabFilterIndex $rules ActiveStore
foreach($kind in $kinds.Keys){Require ($calls[$kind] -eq 1) 'one_query_per_filter_kind'}
foreach($rule in $rules){Require (Lab-Equal (Get-LabRuleView $rule ActiveStore $index) (Get-LabRuleView $rule ActiveStore)) 'indexed_equals_native_association_projection'}
$original=$filterData.Application;$filterData.Application+=@($filterData.Application[0])
Reject {New-LabFilterIndex $rules ActiveStore} 'LAB_FILTER_AMBIGUOUS'
$filterData.Application=$original|Where-Object InstanceID -cne $rules[0].InstanceID
Reject {New-LabFilterIndex $rules ActiveStore} 'LAB_FILTER_MISSING'
$filterData.Application=$original
Reject {New-LabFilterIndex @($rules[0],$rules[0]) ActiveStore} 'LAB_RULE_ID_AMBIGUOUS'
$before=@{};foreach($key in $calls.Keys){$before[$key]=$calls[$key]}
$empty=New-LabFilterIndex @() ActiveStore
foreach($key in $calls.Keys){Require ($calls[$key] -eq $before[$key] -and $empty[$key].Count -eq 0) 'empty_policy_no_filter_queries'}
$jsonCases=@()
foreach($limit in @(131072,2097152)) {
    $path=Join-Path $Evidence "bound-$limit.json"
    $overhead=[Text.Encoding]::UTF8.GetByteCount((@{text=''}|ConvertTo-Json -Compress)+[char]10)
    $value=@{text=('x'*($limit-$overhead))}
    if($limit -eq 131072){Lab-Write $path $value;$read=Lab-Read $path}else{Lab-Write $path $value -MaxBytes $limit;$read=Lab-Read $path -MaxBytes $limit}
    Require ((Get-Item -LiteralPath $path).Length -eq $limit -and $read.text -ceq $value.text) 'exact_reader_writer_bound'
    $beforePin=Lab-Hash $path;$tooBig=@{text=($value.text+'x')}
    if($limit -eq 131072){Reject {Lab-Write $path $tooBig} 'LAB_JSON_SIZE'}else{Reject {Lab-Write $path $tooBig -MaxBytes $limit} 'LAB_JSON_SIZE'}
    Require ((Lab-Hash $path) -ceq $beforePin) 'oversize_write_preserves_old_file'
    $stream=[IO.File]::Open($path,'Append','Write');try{$stream.WriteByte(32)}finally{$stream.Dispose()}
    if($limit -eq 131072){Reject {Lab-Read $path} 'LAB_JSON_SIZE'}else{Reject {Lab-Read $path -MaxBytes $limit} 'LAB_JSON_SIZE'}
    $jsonCases+=@{limit=$limit;exact_roundtrip=$true;write_plus_one_rejected=$true;read_plus_one_rejected=$true;prior_file_preserved=$true}
}
function Client-Snapshot($Cfg){
    @{profiles=@(@{name='Private';store='ActiveStore';inbound='Block';outbound='Allow';allow_inbound='True';allow_local='True';disabled=@()});
        network=@(@{index=$Cfg.interface_index;category='Private'});adapter=@{name=$Cfg.interface_alias;guid=('{'+$Cfg.interface_guid.ToUpperInvariant()+'}');status='Up';addresses=@("$($Cfg.ip)/24")};
        dynamic_udp=@('49152','16384');policy=@();ssh=@{service_path='';running=$false;connection=$null}}
}
$cfg=Get-LabConfig A;$s=Client-Snapshot $cfg;Assert-LabNetwork $cfg $s
$block=[pscustomobject](Lab-Rules $cfg|Select-Object -First 1)
$block.Action='Block';$block.Program=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/ssh.exe';$block.Protocol='TCP'
$block.LocalAddress=$cfg.ip;$block.RemoteAddress=$cfg.peer;$block.LocalPort='1-65535';$block.RemotePort='22';$block.InterfaceAlias='Any'
$s.policy=@($block);$block.Direction='Outbound';Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_BLOCKED'
$block.Direction='Inbound';Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_BLOCKED'
Lab-Write (Join-Path $Evidence 'checks.json') @{groups=9;passed=$true;scope='production batch projection/JSON budgets/actual SSH roles with OS boundary fixtures';host_mutations=0;
    json_boundaries=$jsonCases;client_without_sshd_accepted=$true;braced_guid_accepted=$true;client_request_and_reply_blocks_rejected=$true}
@{groups=9;passed=$true}|ConvertTo-Json -Compress
