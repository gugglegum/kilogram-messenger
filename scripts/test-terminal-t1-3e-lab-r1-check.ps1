param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:EVIDENCE_EXISTS'}
[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Reason){if(!$Value){throw "CHECK:$Reason"}}
function Reject([scriptblock]$Body,[string]$Expected){
    try{&$Body}catch{if($_.Exception.Message.Contains($Expected)){return};throw}
    throw "CHECK:did_not_reject_$Expected"
}
function Clone($Object){$Object | ConvertTo-Json -Depth 18 -Compress | ConvertFrom-Json}
$cases=[ordered]@{}
$cfg=Get-LabConfig A
$server=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
$client=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/ssh.exe'
$allow=[ordered]@{Name='sshd-allow';Enabled='True';Profile='Private';Direction='Inbound';Action='Allow';
    Program=$server;Service='Any';Protocol='TCP';LocalAddress=$cfg.ip;RemoteAddress=$cfg.peer;LocalPort='22';RemotePort='Any';
    InterfaceAlias=$cfg.interface_alias;InterfaceType='Any';Package='Any';DynamicTransport='Any';
    LocalUser='Any';RemoteUser='Any';RemoteMachine='Any';Authentication='NotRequired';Encryption='NotRequired';OverrideBlockRules='False'}
$snapshot=[ordered]@{
    profiles=@(@{name='Private';store='ActiveStore';inbound='Block';outbound='Allow';allow_inbound='True';allow_local='True';disabled=@()})
    network=@(@{index=$cfg.interface_index;category='Private'})
    adapter=@{name=$cfg.interface_alias;guid=$cfg.interface_guid;status='Up';addresses=@("$($cfg.ip)/24")}
    dynamic_udp=@('49152','16384');policy=@($allow)
    ssh=@{service_path=$server;running=$true;connection="$($cfg.peer) 55001 $($cfg.ip) 22"}
}
Assert-LabNetwork $cfg (Clone $snapshot)
$cases.ssh_valid_current_and_reconnect=$true
foreach($pair in @(@('LocalAddress','192.0.2.99'),@('RemotePort','1'),@('RemotePort','55001'),
    @('RemotePort','49152-65535'),@('Program','C:\wrong\sshd.exe'),@('Service','other-service'),
    @('InterfaceAlias','other-interface'),@('RemoteAddress','192.0.2.0/24'),@('Profile','Public'))) {
    $s=Clone $snapshot;$s.policy[0].($pair[0])=$pair[1]
    Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_EFFECT_UNCONFIRMED'
    $cases["ssh_false_allow_$($pair[0])_$($pair[1])"]=$true
}
$s=Clone $snapshot;$s.policy[0].LocalAddress='192.0.2.99';$s.policy[0].RemotePort='1'
Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_EFFECT_UNCONFIRMED'
$cases.root_false_allow_combination=$true
$s=Clone $snapshot;$s.policy[0].LocalPort='1-21,22,23-65535';$s.policy[0].RemotePort='1-30000,30001-65535'
$s.policy[0].LocalAddress="$($cfg.ip)/32";$s.policy[0].RemoteAddress='192.168.0.0/255.255.255.0'
$s.policy[0].InterfaceAlias='other-interface,'+$cfg.interface_alias
Assert-LabNetwork $cfg $s
$s.ssh.connection="$($cfg.peer) 55002 $($cfg.ip) 22";Assert-LabNetwork $cfg $s
$cases.ssh_sets_cidr_masks_and_new_client_port=$true
$block=Clone $allow;$block.Action='Block';$block.Program='Any';$block.LocalPort='1-65535'
$s=Clone $snapshot;$s.policy+=@($block);Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_BLOCKED'
$cases.root_range_block=$true
foreach($ports in @('22-23','1-21,22,200-300')) {
    $s=Clone $snapshot;$b=Clone $block;$b.LocalPort=$ports;$b.RemotePort='55002-55003';$s.policy+=@($b)
    Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_BLOCKED'
}
$cases.ssh_range_blocks_include_reconnect=$true
$s=Clone $snapshot;$b=Clone $block;$b.LocalPort='23-65535';$s.policy+=@($b);Assert-LabNetwork $cfg $s
$s=Clone $snapshot;$b=Clone $block;$b.RemoteAddress='192.0.2.1-192.0.2.254';$s.policy+=@($b);Assert-LabNetwork $cfg $s
$cases.ssh_nonintersecting_blocks=$true
foreach($pair in @(@('Authentication','Required'),@('InterfaceType','Wireless'),@('LocalUser','unparsed-sddl'),@('RemoteAddress','Internet'),@('LocalPort','RPC'))) {
    $s=Clone $snapshot;$b=Clone $block;$b.($pair[0])=$pair[1];$s.policy+=@($b)
    Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_FILTER_UNCERTAIN'
}
$s=Clone $snapshot;$s.policy[0].Package='conditional-package';Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_FILTER_UNCERTAIN'
$cases.ssh_unsupported_applicable_filters_uncertain=$true
$s=Clone $snapshot;$b=Clone $block;$b.Direction='Outbound';$b.Program=$client;$b.LocalPort='1-65535';$b.RemotePort='22';$s.policy+=@($b)
Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_BLOCKED'
$s=Clone $snapshot;$s.ssh.service_path='C:\wrong\sshd.exe';Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_SERVICE_UNCERTAIN'
$s=Clone $snapshot;$s.ssh.connection='192.0.2.99 55001 '+$cfg.ip+' 22';Reject {Assert-LabNetwork $cfg $s} 'LAB_SSH_ROUTE_UNCERTAIN'
$cases.ssh_outbound_service_and_current_route=$true
function Get-CimInstance([string]$ClassName,[string]$Filter){[pscustomobject]@{PathName=$server;State='Running'}}
$oldConnection=$env:SSH_CONNECTION
try {
    $env:SSH_CONNECTION="$($cfg.peer) 55001 $($cfg.ip) 22";$first=Get-LabSshFacts
    $env:SSH_CONNECTION="$($cfg.peer) 55002 $($cfg.ip) 22";$second=Get-LabSshFacts
    Require (Lab-Equal $first $second) 'audit_policy_not_bound_to_ephemeral_source_port'
    Assert-LabNetwork $cfg (Clone $snapshot)
} finally {$env:SSH_CONNECTION=$oldConnection}
$cases.ssh_policy_digest_stable_across_reconnect=$true

# Real files and the exact production file-cleanup/journal/transaction functions.
# Only task/firewall/network mutation/check boundaries are replaced by this small in-memory OS state.
function Recovery-Backend {
    @{
        Read={Lab-Read (Join-Path $ctx.stateDir 'initial-and-journal.json')}
        Write={param($j) Save-LabInstallJournal $ctx.stateDir $j $ctx.pins}
        Snapshot={Clone $ctx.os}
        Preflight={param($now) foreach($name in $ctx.names){Require (!(Test-Path -LiteralPath (Join-Path $ctx.base "ordinary-agent/$name"))) 'retry_no_owned_file_collision'}}
        Apply={param($change,$want)
            switch($change){
                'files'{foreach($name in $ctx.names){[IO.File]::Copy((Join-Path $ctx.package $name),(Join-Path $ctx.base "ordinary-agent/$name"),$false)}}
                'task'{$ctx.os.task='owned'};'rule0'{$ctx.os.rules+=@('owned0')};'rule1'{$ctx.os.rules+=@('owned1')};'private'{$ctx.os.private=$true}
            }
        }
        Verify={param($want)
            Require ($ctx.os.task -ceq 'owned' -and $ctx.os.private -and ($ctx.os.rules -join ',') -ceq 'owned0,owned1') 'os_mock_installed'
            foreach($name in $ctx.names){Require ((Lab-Hash (Join-Path $ctx.base "ordinary-agent/$name")) -ceq $ctx.pins.$name) 'corrected_package_bytes'}
        }
        CanRemove={param($want,$initial,$phase) Require ($ctx.os.foreign -ceq 'untouched') 'foreign_os_state'}
        Undo={param($change,$want,$initial)
            switch($change){
                'files'{Undo-LabInstallFiles $ctx.base $ctx.stateDir $ctx.names}
                'task'{$ctx.os.task=$initial.task};'rule0'{$ctx.os.rules=@($ctx.os.rules | Where-Object {$_ -cne 'owned0'})}
                'rule1'{$ctx.os.rules=@($ctx.os.rules | Where-Object {$_ -cne 'owned1'})};'private'{$ctx.os.private=$initial.private}
            }
        }
    }
}
foreach($phase in @('installing','rollback_incomplete','recovering')) {
    $dir=Join-Path $Evidence "recovery-$phase"
    $ctx=@{base=(Join-Path $dir 'host');stateDir=(Join-Path $dir 'state');package=(Join-Path $dir 'package');
        names=@('agent.ps1','lab.json');pins=@{};os=@{task=$null;rules=@();private=$false;foreign='untouched'}}
    foreach($path in @((Join-Path $ctx.base 'ordinary-agent'),$ctx.stateDir,$ctx.package)){[void][IO.Directory]::CreateDirectory($path)}
    foreach($name in $ctx.names){[IO.File]::WriteAllText((Join-Path $ctx.package $name),'initial package '+$name);$ctx.pins[$name]=Lab-Hash (Join-Path $ctx.package $name)}
    $want=@{campaign='test-recovery';task='fixed';rules=@('owned0','owned1')}
    $j=[pscustomobject]@{phase='installing';want=$want;initial=(Clone $ctx.os);changes=@();rollback_errors=@()}
    Save-LabInstallJournal $ctx.stateDir $j $ctx.pins
    $initialPin=Lab-Hash (Join-Path $ctx.stateDir 'initial.json')
    $ctx.os.task='owned';$ctx.os.rules=@('owned0');$j.changes=@('files','task','rule0')
    foreach($name in $ctx.names){[IO.File]::Copy((Join-Path $ctx.package $name),(Join-Path $ctx.base "ordinary-agent/$name"))}
    $baseline=Join-Path $ctx.base 'ordinary-agent/campaign-baseline.json';Lab-Write $baseline @{owned='baseline'}
    Lab-Write (Join-Path $ctx.stateDir 'public-baseline-intent.json') @{sha256=(Lab-Hash $baseline)}
    $j.phase=$phase
    if($phase -ceq 'rollback_incomplete'){$j.rollback_errors=@('LAB_FILE_DRIFT')}
    Save-LabInstallJournal $ctx.stateDir $j $ctx.pins
    $b=Recovery-Backend
    $evidenceFile=Join-Path $ctx.base 'preserve-run-evidence.txt';[IO.File]::WriteAllText($evidenceFile,'preserve')
    if($phase -ceq 'rollback_incomplete') {
        [IO.File]::WriteAllText((Join-Path $ctx.base 'ordinary-agent/lab.json'),'drift')
        Reject {Invoke-LabTransaction $b $want Teardown} 'LAB_FILE_DRIFT'
        Require ((Lab-Read (Join-Path $ctx.stateDir 'initial-and-journal.json')).phase -ceq 'recovering') 'recovering_intent_durable'
        Require (Test-Path -LiteralPath $baseline) 'no_partial_file_cleanup_on_drift'
        [IO.File]::Copy((Join-Path $ctx.package 'lab.json'),(Join-Path $ctx.base 'ordinary-agent/lab.json'),$true)
    }
    if($phase -ceq 'recovering'){Remove-Item -LiteralPath (Join-Path $ctx.base 'ordinary-agent/agent.ps1')}
    $result=Invoke-LabTransaction $b $want Teardown
    Require ($result.state -ceq 'rolled_back') 'recovery_retryable_terminal'
    foreach($name in $ctx.names){Require (!(Test-Path -LiteralPath (Join-Path $ctx.base "ordinary-agent/$name"))) 'failed_install_files_removed'}
    Require (!(Test-Path -LiteralPath $baseline)) 'failed_baseline_removed'
    foreach($name in $ctx.names){[IO.File]::WriteAllText((Join-Path $ctx.package $name),'corrected package '+$name);$ctx.pins[$name]=Lab-Hash (Join-Path $ctx.package $name)}
    $result=Invoke-LabTransaction $b $want Provision
    Require ($result.state -ceq 'installed') 'corrected_package_retry'
    Require ((Lab-Hash (Join-Path $ctx.stateDir 'initial.json')) -ceq $initialPin) 'initial_baseline_preserved'
    [IO.File]::WriteAllText((Join-Path $ctx.base 'ordinary-agent/agent.ps1'),'legitimate ordinary update')
    $result=Invoke-LabTransaction $b $want Teardown
    Require ($result.state -ceq 'removed' -and (Test-Path -LiteralPath $evidenceFile)) 'final_teardown_evidence_preserved'
    Require ([IO.File]::ReadAllText((Join-Path $ctx.base 'ordinary-agent/agent.ps1')) -ceq 'legitimate ordinary update') 'final_teardown_updated_files_preserved'
    Require (@(Get-ChildItem -LiteralPath (Join-Path $ctx.stateDir 'journal-history') -File).Count -ge 3) 'recovery_history_preserved'
    $cases["real_files_$phase`_recovery_retry_and_final_preserve"]=$true
}
Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;count=$cases.Count;
    scope='actual SSH guard on synthetic policy, actual filesystem/production recovery cleanup, Task/firewall/network OS boundary mocked';host_mutations=0}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
