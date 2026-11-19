param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
$cases=[ordered]@{}
$cfg=Get-LabConfig A;$want=@{task=(Lab-TaskSpec $cfg);rules=@(Lab-Rules $cfg)}
$initial=Clone @{task=$null;rules=@();policy=@();network=@(@{index=$cfg.interface_index;alias=$cfg.interface_alias;category='Private'});
    adapter=@{name=$cfg.interface_alias;guid=$cfg.interface_guid;status='Up';addresses=@("$($cfg.ip)/24")};dynamic_udp=@('49152','16384');
    profiles=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($name in @('Domain','Private','Public')){
        @{store=$store;name=$name;enabled=$(if($name -ceq 'Private'){'False'}else{'True'});inbound='Block';outbound='Allow';allow_inbound='True';allow_local='True';disabled=@()}
    }})}
$block=Clone $want.rules[1];$block.Name='foreign-block';$block.Action='Block'
foreach($direction in @('Inbound','Outbound')){
    $b=Clone $block;$b.Direction=$direction
    Reject {Assert-LabUdpPath $cfg @($b)} 'LAB_UDP_BLOCKED'
    $cases["udp_block_$direction"]=$true
}
foreach($key in @('Program','Protocol','RemoteAddress','InterfaceAlias','LocalPort','Profile','Enabled')){
    $b=Clone $block
    $b.$key=switch($key){'Protocol'{'TCP'};'RemoteAddress'{'192.0.2.1'};'LocalPort'{'80'};'Profile'{'Public'};'Enabled'{'False'};default{'other'}}
    # An unsupported security selector on a provably unrelated rule stays unrelated.
    $b.LocalUser='__unsupported__'
    Assert-LabUdpPath $cfg @($b)
    $cases["irrelevant_block_$key"]=$true
}
$b=Clone $block;$b.RemoteAddress='__unsupported__'
Reject {Assert-LabUdpPath $cfg @($b)} 'LAB_UDP_FILTER_UNCERTAIN'
$cases.udp_unknown_applicability=$true
$b=Clone $block;$b.Action='__unsupported__'
Require (Test-LabCurrentPolicyRelevant $cfg $b) 'unknown_action_kept_for_diagnostic'
Reject {Assert-LabUdpPath $cfg @($b)} 'LAB_UDP_FILTER_UNCERTAIN'
$b.Program='other.exe';Require (!(Test-LabCurrentPolicyRelevant $cfg $b)) 'irrelevant_unknown_action_excluded'
Assert-LabUdpPath $cfg @($b)
$cases.unknown_action_relevance=$true
$b=Clone $want.rules[0];$b.Action='Block';$b.Name='foreign-discovery'
Reject {Assert-LabUdpPath $cfg @($b)} 'LAB_UDP_BLOCKED'
$b.Direction='Outbound';$b.LocalAddress=$cfg.ip;$b.RemoteAddress='239.255.75.72'
Reject {Assert-LabUdpPath $cfg @($b)} 'LAB_UDP_BLOCKED'
$cases.discovery_both_directions=$true
$ssh=Clone $block;$ssh.Protocol='TCP';$ssh.Direction='Outbound';$ssh.Program=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/ssh.exe'
$ssh.LocalPort='Any';$ssh.RemotePort='22'
Reject {Assert-LabSshPath $cfg @{policy=@($ssh)} -PolicyOnly} 'LAB_SSH_BLOCKED'
$ssh.RemoteUser='__unsupported__';Reject {Assert-LabSshPath $cfg @{policy=@($ssh)} -PolicyOnly} 'LAB_SSH_FILTER_UNCERTAIN'
$cases.ssh_current_block_and_unknown=$true
# COM foreign conversion preserves unknown selectors, and the fixed-flow matcher
# decides relevance. An unrelated unsupported COM rule cannot fail enumeration.
$com=[pscustomobject]@{Name='foreign';Enabled=$true;Profiles=2;Direction=1;Action=0;ApplicationName='other.exe';Protocol=17;
    InterfaceTypes='All';Interfaces=@();LocalPorts='Any';RemotePorts='Any';LocalAddresses='*';RemoteAddresses='*';ServiceName='';
    SecureFlags=0;LocalUserOwner='';LocalAppPackageId='';LocalUserAuthorizedList='';RemoteUserAuthorizedList='';RemoteMachineAuthorizedList=''}
$view=Convert-LabComPolicyRule $com;Assert-LabUdpPath $cfg @($view)
$com.ApplicationName=Join-Path $cfg.base 'bin/kilogram-cli.exe'
$view=Convert-LabComPolicyRule $com;Reject {Assert-LabUdpPath $cfg @($view)} 'LAB_UDP_BLOCKED'
$com.PSObject.Properties.Remove('SecureFlags');$view=Convert-LabComPolicyRule $com
Reject {Assert-LabUdpPath $cfg @($view)} 'LAB_UDP_FILTER_UNCERTAIN'
$com.ApplicationName='other.exe';Assert-LabUdpPath $cfg @((Convert-LabComPolicyRule $com))
$cases.com_foreign_projection=$true
# Ordinary baseline retains exact owned rule/persistent-byte pins and relevant
# network/profile fields, while tolerating historical foreign registry digests.
$before=Clone @{backend='com-native-v1';rules=@(foreach($r in $want.rules){@{name=$r.Name;view=$r;persistent_sha256=('a'*64)}});
    profiles=@(@{name='Private';enabled='True'},@{name='Public';enabled='True'});policy_inputs=@{sha256=('a'*64)};
    native_network=@{adapter=@{guid=$cfg.interface_guid};udp=@(49152,65535);network=@(@{index=$cfg.interface_index;category='Private'})};policy=@()}
foreach($change in @('add','remove','update')){
    $now=Clone $before;$now.policy_inputs.sha256='b'*64;$now.profiles[1].enabled='False'
    if($change -cne 'remove'){$now.policy=@((Clone $block));$now.policy[0].Program='other.exe';if($change -ceq 'update'){$now.policy[0].Description='updated'}}
    Assert-LabOrdinaryReady $cfg $want $before $now
    $cases["ordinary_foreign_$change"]=$true
}
foreach($change in @('owned-view','owned-bytes','private','network','udp-block','ssh-block','unknown')){
    $now=Clone $before;$code='LAB_BASELINE_DRIFT'
    switch($change){
        'owned-view'{$now.rules[0].view.RemoteAddress='other'}
        'owned-bytes'{$now.rules[0].persistent_sha256='b'*64}
        'private'{$now.profiles[0].enabled='False'}
        'network'{$now.native_network.adapter.guid='different'}
        'udp-block'{$now.policy=@((Clone $block));$code='LAB_UDP_BLOCKED'}
        'ssh-block'{$s=Clone $ssh;$s.RemoteUser='Any';$now.policy=@($s);$code='LAB_SSH_BLOCKED'}
        'unknown'{$s=Clone $block;$s.RemoteAddress='__unsupported__';$now.policy=@($s);$code='LAB_UDP_FILTER_UNCERTAIN'}
    }
    Reject {Assert-LabOrdinaryReady $cfg $want $before $now} $code
    $cases["ordinary_reject_$change"]=$true
}
# Planned shared profile writes: no-op ignores foreign fields; a real write
# requires the matching Private fields and affected network, not other profiles.
$now=Clone $initial;$now.profiles[0].enabled='False';$now.policy=@($block)
Require (Test-LabPrivateWrite $now $initial) 'enable_ignores_foreign_and_domain'
foreach($p in $now.profiles){if($p.name -ceq 'Private'){$p.enabled='True'}}
Require (!(Test-LabPrivateWrite $now $initial)) 'already_enabled_no_setter'
Reject {Test-LabPrivateWrite $now $initial -Restore} 'LAB_PROFILE_RESTORE_UNCONFIRMED'
Require (Test-LabPrivateWrite $now $initial -Restore -OwnedEnableConfirmed) 'confirmed_owned_enable_restorable'
$now.profiles[1].outbound='Block'
Reject {Test-LabPrivateWrite $now $initial -Restore -OwnedEnableConfirmed} 'LAB_PROFILE_DRIFT'
$cases.profile_specific_write_and_legacy_ambiguity=$true
$now=Clone $initial;$now.adapter.guid='changed';Require (!(Test-LabPrivateWrite $now $initial -Restore)) 'no_op_after_network_drift'
foreach($p in $now.profiles){if($p.name -ceq 'Private'){$p.enabled='True'}}
Reject {Test-LabPrivateWrite $now $initial -Restore -OwnedEnableConfirmed} 'LAB_RESTORE_NETWORK_DRIFT'
$cases.profile_write_network_conflict=$true
# Capture the real transaction/CanRemove/Undo/Write blocks. Only native OS
# boundaries are mocked; cleanup files, journal snapshots and history are real.
$tokens=$null;$errors=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
Require (!$errors.Count) 'installer_parses'
$assignment=$tree.Find({param($n)$n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true)
$table=$assignment.Right.Find({param($n)$n -is [Management.Automation.Language.HashtableAst]},$true);$production=@{}
foreach($pair in $table.KeyValuePairs){$production[$pair.Item1.Value]=$pair.Item2.Find({param($n)$n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()}
$snapshotReader=${function:Get-LabSnapshot}
function Get-LabSnapshot($Cfg,$Want,[switch]$Cleanup,[switch]$ProfileWrite){Clone $script:os}
function Find-LabTask($Name){$script:task}
function Stop-LabIdleAgent($Base,$Handles){}
function Get-NetFirewallRule {param($PolicyStore) @($script:os.rules|Where-Object store -ceq $PolicyStore|ForEach-Object view)}
function Get-LabRuleView($Rule,$Store){$Rule}
function Remove-NetFirewallRule {param($PolicyStore,$Name) $script:removes++;$script:os.rules=@($script:os.rules|Where-Object {$_.view.Name -cne $Name})}
function Set-NetFirewallProfile {param($PolicyStore,$Name,$Enabled) $script:profileWrites++;foreach($p in $script:os.profiles){if($p.name -ceq $Name){$p.enabled=$Enabled}}}
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
& {
    function Get-NetFirewallProfile {param($PolicyStore)
        foreach($p in @($initial.profiles|Where-Object store -ceq $PolicyStore)){
            [pscustomobject]@{Name=$p.name;Enabled=$p.enabled;DefaultInboundAction=$p.inbound;DefaultOutboundAction=$p.outbound;
                AllowInboundRules=$p.allow_inbound;AllowLocalFirewallRules=$p.allow_local;DisabledInterfaceAliases=$p.disabled}
        }
    }
    function Get-NetFirewallRule {param($PolicyStore) [pscustomobject]@{Name='foreign-unsupported'} }
    function New-LabFilterIndex {throw 'CHECK:foreign_filter_reader_called'}
    function Get-NetConnectionProfile {throw 'CHECK:network_reader_called'}
    $script:task=$null
    $observation=&$snapshotReader $cfg $want -Cleanup
    Require (!$observation.task -and !$observation.rules.Count -and $observation.profiles.Count -eq 6) 'owned_only_cleanup_snapshot'
    Reject {&$snapshotReader $cfg $want -Cleanup -ProfileWrite} 'CHECK:network_reader_called'
}
$cases.cleanup_reader_skips_foreign_filters_and_network=$true
$script:task=[pscustomobject]@{Xml=$want.task.xml;descriptor=$want.task.sddl}
$script:task|Add-Member ScriptMethod GetSecurityDescriptor {param($Flags) $this.descriptor}
$installed=Clone $initial
$installed.rules=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($rule in $want.rules){@{store=$store;view=(Clone $rule)}}})
foreach($p in $installed.profiles){if($p.name -ceq 'Private'){$p.enabled='True'}else{$p.enabled='False'}}
foreach($change in @('add','remove','update')){
    $script:os=Clone $installed
    if($change -cne 'remove'){$r=Clone $block;$r.Program='other.exe';if($change -ceq 'update'){$r.RemotePort='80'};$script:os.policy=@($r)}
    Assert-LabInstalled $cfg $want $initial
    $cases["installed_foreign_$change"]=$true
}
foreach($change in @('rule','block','profile','task')){
    $script:os=Clone $installed;$code='LAB_RULE_DRIFT'
    switch($change){
        'rule'{$script:os.rules[0].view.Action='Block'}
        'block'{$script:os.policy=@($block);$code='LAB_UDP_BLOCKED'}
        'profile'{@($script:os.profiles|Where-Object {$_.store -ceq 'ActiveStore' -and $_.name -ceq 'Private'})[0].inbound='Allow';$code='LAB_PRIVATE_POLICY'}
        'task'{$script:task.Xml=$want.task.xml.Replace('LeastPrivilege','HighestAvailable');$code='LAB_TASK_DRIFT'}
    }
    Reject {Assert-LabInstalled $cfg $want $initial} $code
    $cases["installed_reject_$change"]=$true
}
$script:task=$null
foreach($scenario in @('foreign-add','foreign-remove','foreign-update','owned-drift','profile-conflict','legacy-enabled-unknown','confirmed-enable')){
    $stateDir=Join-Path $Evidence $scenario;[void][IO.Directory]::CreateDirectory($stateDir)
    $cfg=Get-LabConfig A;$cfg.base=Join-Path $stateDir 'host';$agent=Join-Path $cfg.base 'ordinary-agent';[void][IO.Directory]::CreateDirectory($agent)
    $want=@{task=(Lab-TaskSpec $cfg);rules=@(Lab-Rules $cfg)};$files=@('payload.txt');$payload=Join-Path $agent $files[0];[IO.File]::WriteAllText($payload,'owned')
    $journalPath=Join-Path $stateDir 'initial-and-journal.json';$packagePins=$null;$setupMode=$false;$freshB=$false;$bRetry=$false;$teardownHandles=@{agent='fixture'}
    $script:os=Clone $initial;$script:os.rules=@(@{store='PersistentStore';view=(Clone $want.rules[0])});$script:profileWrites=0;$script:removes=0
    $j=Clone @{phase='recovering';want=$want;initial=$initial;changes=@('files','rule0','private');rollback_errors=@();file_pins=@{'payload.txt'=(Lab-Hash $payload)}}
    Save-LabInstallJournal $stateDir $j $null;$originalHash=Lab-Hash (Join-Path $stateDir 'initial.json')
    $foreign=Clone $block;$foreign.Program='other.exe';$script:os.policy=@($foreign)
    switch($scenario){
        'foreign-remove'{$script:os.policy=@()}
        'foreign-update'{$script:os.policy[0].LocalPort='80'}
        'owned-drift'{$script:os.rules[0].view.Action='Block'}
        'profile-conflict'{foreach($p in $script:os.profiles){if($p.name -ceq 'Private'){$p.enabled='True';$p.outbound='Block'}};$j|Add-Member NoteProperty private_write @{enabled='True';baseline_sha256=(Lab-JsonHash $initial)}}
        'legacy-enabled-unknown'{foreach($p in $script:os.profiles){if($p.name -ceq 'Private'){$p.enabled='True'}}}
        'confirmed-enable'{foreach($p in $script:os.profiles){if($p.name -ceq 'Private'){$p.enabled='True'}};$j|Add-Member NoteProperty private_write @{enabled='True';baseline_sha256=(Lab-JsonHash $initial)}}
    }
    Lab-Write $journalPath $j -MaxBytes 2097152;$foreignPin=Lab-JsonHash $script:os.policy
    if($scenario -ceq 'owned-drift'){
        Reject {Invoke-LabTransaction $production $want Teardown} 'LAB_RULE_DRIFT'
        Require ((Test-Path -LiteralPath $payload) -and !$script:removes -and !$script:profileWrites) 'owned_drift_before_delete'
    }else{
        if($scenario -cin @('profile-conflict','legacy-enabled-unknown')){
            $code=if($scenario -ceq 'profile-conflict'){'LAB_PROFILE_DRIFT'}else{'LAB_PROFILE_RESTORE_UNCONFIRMED'}
            Reject {Invoke-LabTransaction $production $want Teardown} $code
            Require ((Lab-Read $journalPath).phase -ceq 'rollback_incomplete' -and !$script:profileWrites) 'shared_profile_conflict_retained'
        }else{
            $result=Invoke-LabTransaction $production $want Teardown;Require ($result.state -ceq 'rolled_back') 'cleanup_completed'
            Require ($script:profileWrites -eq $(if($scenario -ceq 'confirmed-enable'){1}else{0})) 'only_confirmed_private_write'
        }
        Require (!(Test-Path -LiteralPath $payload) -and $script:removes -eq 1) 'owned_cleanup_even_on_shared_profile_conflict'
    }
    Require ((Lab-Hash (Join-Path $stateDir 'initial.json')) -ceq $originalHash -and (Lab-JsonHash $script:os.policy) -ceq $foreignPin) 'original_and_foreign_evidence_unchanged'
    $cases["transaction_$scenario"]=$true
}
# A fresh admitted retry has a separately persisted current checkpoint. Its
# rollback restores that checkpoint; original initial/history remain unchanged.
$stateDir=Join-Path $Evidence 'current-checkpoint';[void][IO.Directory]::CreateDirectory($stateDir)
$journalPath=Join-Path $stateDir 'initial-and-journal.json';$packagePins=$null;$setupMode=$false;$bRetry=$false
$j=Clone @{phase='rolled_back';want=$want;initial=$initial;changes=@();rollback_errors=@()};Save-LabInstallJournal $stateDir $j $null
$originalHash=Lab-Hash (Join-Path $stateDir 'initial.json');$script:os=Clone $initial
$script:os.policy=@(@{Name='foreign-new';value='preserve-current'});foreach($p in $script:os.profiles){if($p.name -ceq 'Private'){$p.enabled='True'}}
$expectedCheckpoint=Clone $script:os;$script:restored=$null
$backend=@{Read=$production.Read;Write=$production.Write;Snapshot={Clone $script:os};Preflight={param($Now) Assert-LabNetwork $cfg $Now -SkipSsh};
    Apply={param($Change,$W) if($Change -ceq 'files'){throw 'injected'} };Verify={};Undo={param($Change,$W,$Initial) $script:restored=Clone $Initial}}
Reject {Invoke-LabTransaction $backend $want Provision} 'injected'
$saved=Lab-Read $journalPath -MaxBytes 2097152
Require ((Lab-Equal $script:restored $expectedCheckpoint) -and (Lab-Equal (Get-LabRollbackInitial $saved) $expectedCheckpoint)) 'rollback_uses_current_checkpoint'
Require ((Lab-Hash (Join-Path $stateDir 'initial.json')) -ceq $originalHash -and (Lab-Equal $saved.initial $initial)) 'original_snapshot_not_rebased'
Require (@(Get-ChildItem -LiteralPath (Join-Path $stateDir 'journal-history') -File).Count -eq 2) 'old_terminal_and_failed_attempt_archived'
$saved.attempt_baseline.sha256='0'*64;Reject {Get-LabRollbackInitial $saved} 'LAB_ATTEMPT_BASELINE_DRIFT'
$cases.current_checkpoint_and_original_history=$true
# Exercise production private Apply with the real transaction's shared journal:
# a successful install must not overwrite the confirmed setter receipt at commit.
$stateDir=Join-Path $Evidence 'profile-receipt-lifecycle';[void][IO.Directory]::CreateDirectory($stateDir)
$journalPath=Join-Path $stateDir 'initial-and-journal.json';$script:os=Clone $initial;$script:profileWrites=0;$setupActor=$null
$backend=@{Read=$production.Read;Write=$production.Write;Snapshot={Clone $script:os};Preflight={};Verify={};CanRemove={};
    Apply={param($Change,$W,$TransactionJournal) if($Change -ceq 'private'){&$production.Apply $Change $W $TransactionJournal}};
    Undo={param($Change,$W,$Initial) if($Change -ceq 'private'){&$production.Undo $Change $W $Initial}}}
$null=Invoke-LabTransaction $backend $want Provision
$saved=Lab-Read $journalPath -MaxBytes 2097152
Require ($saved.phase -ceq 'installed' -and $saved.private_write.enabled -ceq 'True' -and $script:profileWrites -eq 1) 'confirmed_setter_receipt_survives_success'
$null=Invoke-LabTransaction $backend $want Teardown
Require ((Lab-Read $journalPath -MaxBytes 2097152).phase -ceq 'removed' -and $script:profileWrites -eq 2) 'normal_teardown_uses_own_receipt'
$cases.private_receipt_install_teardown_lifecycle=$true
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;host_mutations=0;
    scope='fixed UDP/SSH policy predicates and ordinary readiness; production transaction/cleanup with OS boundaries mocked, real journal/files; no privileged host changes'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
