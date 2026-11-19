param(
    [Parameter(Mandatory)][ValidateSet('Library','Package','Audit','Setup','SetupRecoveredA','SetupFreshB','RecoverOld','Provision','Verify','Teardown')][string]$Mode,
    [ValidateSet('A','B')][string]$Side,
    [string]$PackageRoot,
    [string]$PackageSha,
    [string]$AuditSha,
    [switch]$Ordinary,
    [string]$BaselineSha,
    [string]$Output,
    [string]$SetupSession
)
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-diagnostics.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-readiness.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-setup.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-server.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-recovery.ps1"

function Get-LabConfig([string]$Which) {
    if ($Which -cnotin @('A','B')) { throw 'LAB_SIDE' }
    $a=$Which -ceq 'A'
    [pscustomobject][ordered]@{
        v=1;campaign='t13e-lab';side=$Which;host=if($a){'DESKTOP-PAUL-RY'}else{'PAUL_HP_OMEN'}
        sid=if($a){'S-1-5-21-1776862868-2140151431-2099235586-1001'}else{'S-1-5-21-2671144489-3236106012-1032288181-1006'}
        base=if($a){'E:\AI-Projects\kilogram-p2p-messenger\.tmp\terminal-t11f'}else{'C:\Users\test\KilogramLab\terminal-t11f'}
        ip=if($a){'192.168.0.140'}else{'192.168.0.152'};peer=if($a){'192.168.0.152'}else{'192.168.0.140'}
        interface_alias=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($(if($a){'0JHQtdGB0L/RgNC+0LLQvtC00L3QsNGPINGB0LXRgtGMIDI='}else{'0JHQtdGB0L/RgNC+0LLQvtC00L3QsNGPINGB0LXRgtGM'})))
        interface_index=if($a){22}else{19};interface_guid=if($a){'b80a5d15-1daa-4d9e-8a23-d44643e3c46a'}else{'e70ed7fb-d0d9-4aba-88df-364117667047'}
        suite='field';max_work_seconds=1180
    }
}
function Lab-TaskSpec($Cfg) {
    $path=Join-Path $Cfg.base 'ordinary-agent'
    $escape={param($s) [Security.SecurityElement]::Escape([string]$s)}
    $sid=&$escape $Cfg.sid
    $args='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+(Join-Path $path 'test-terminal-t1-3e-lab-agent.ps1')+'" -Action Run -Config "'+(Join-Path $path 'lab.json')+'"'
    $command="$env:windir\System32\WindowsPowerShell\v1.0\powershell.exe"
    $xml='<Task xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task" version="1.2"><RegistrationInfo><Description>Kilogram ordinary lab '+$Cfg.campaign+'</Description></RegistrationInfo>'+
        '<Triggers><LogonTrigger><Enabled>true</Enabled><UserId>'+$sid+'</UserId></LogonTrigger></Triggers>'+
        '<Principals><Principal id="AgentUser"><UserId>'+$sid+'</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>'+
        '<Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>'+
        '<AllowHardTerminate>false</AllowHardTerminate><AllowStartOnDemand>true</AllowStartOnDemand><Enabled>true</Enabled><Hidden>true</Hidden><ExecutionTimeLimit>PT0S</ExecutionTimeLimit></Settings>'+
        '<Actions Context="AgentUser"><Exec><Command>'+(&$escape $command)+'</Command><Arguments>'+(&$escape $args)+'</Arguments><WorkingDirectory>'+(&$escape $Cfg.base)+'</WorkingDirectory></Exec></Actions></Task>'
    @{name="Kilogram-Lab-$($Cfg.campaign)-$($Cfg.side)";sid=$Cfg.sid;xml=$xml;command=$command;arguments=$args;directory=$Cfg.base;
        sddl="D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GRGX;;;$($Cfg.sid))"}
}
function Assert-LabTask($Task,$Want) {
    if (!$Task) { throw 'LAB_TASK_ABSENT' }
    [xml]$xml=$Task.Xml
    $ns=[Xml.XmlNamespaceManager]::new($xml.NameTable);$ns.AddNamespace('t',$xml.DocumentElement.NamespaceURI)
    $defaults=@{
        '/t:Task/t:Principals/t:Principal/t:RunLevel'='LeastPrivilege'
        '/t:Task/t:Triggers/t:LogonTrigger/t:Enabled'='true'
        '/t:Task/t:Settings/t:AllowStartOnDemand'='true'
        '/t:Task/t:Settings/t:Enabled'='true'
    }
    function TextAt([string]$Path) {
        $nodes=$xml.SelectNodes($Path,$ns)
        if(!$nodes.Count -and $defaults.ContainsKey($Path)){return $defaults[$Path]}
        if ($nodes.Count -ne 1) { throw 'LAB_TASK_SCHEMA' }
        $nodes[0].InnerText
    }
    function TaskSid([string]$Text) {
        try {
            if($Text -match '^S-1-'){return [Security.Principal.SecurityIdentifier]::new($Text).Value}
            if(!$Text){throw 'Empty task user'}
            return [Security.Principal.NTAccount]::new($Text).Translate([Security.Principal.SecurityIdentifier]).Value
        }catch{throw 'LAB_TASK_DRIFT'}
    }
    $principal=$xml.SelectNodes('/t:Task/t:Principals/t:Principal',$ns)
    $triggers=$xml.SelectNodes('/t:Task/t:Triggers/*',$ns)
    $actions=$xml.SelectNodes('/t:Task/t:Actions/*',$ns)
    if ($principal.Count -ne 1 -or $triggers.Count -ne 1 -or $triggers[0].LocalName -cne 'LogonTrigger' -or
        $actions.Count -ne 1 -or $actions[0].LocalName -cne 'Exec') { throw 'LAB_TASK_SCHEMA' }
    foreach($container in @('Principals','Triggers','Actions','Settings')){if($xml.SelectNodes("/t:Task/t:$container",$ns).Count -ne 1){throw 'LAB_TASK_SCHEMA'}}
    if($principal[0].GetAttribute('id') -cne 'AgentUser' -or $xml.DocumentElement.SelectSingleNode('t:Actions',$ns).GetAttribute('Context') -cne 'AgentUser' -or
        $principal[0].SelectNodes('t:GroupId|t:RequiredPrivileges|t:ProcessTokenSidType',$ns).Count){throw 'LAB_TASK_SCHEMA'}
    $checks=@{
        '/t:Task/t:Principals/t:Principal/t:UserId'=$Want.sid
        '/t:Task/t:Principals/t:Principal/t:LogonType'='InteractiveToken'
        '/t:Task/t:Principals/t:Principal/t:RunLevel'='LeastPrivilege'
        '/t:Task/t:Triggers/t:LogonTrigger/t:UserId'=$Want.sid
        '/t:Task/t:Triggers/t:LogonTrigger/t:Enabled'='true'
        '/t:Task/t:Settings/t:MultipleInstancesPolicy'='IgnoreNew'
        '/t:Task/t:Settings/t:ExecutionTimeLimit'='PT0S'
        '/t:Task/t:Settings/t:AllowStartOnDemand'='true'
        '/t:Task/t:Settings/t:Enabled'='true'
        '/t:Task/t:Settings/t:AllowHardTerminate'='false'
        '/t:Task/t:Settings/t:DisallowStartIfOnBatteries'='false'
        '/t:Task/t:Settings/t:StopIfGoingOnBatteries'='false'
        '/t:Task/t:Actions/t:Exec/t:Command'=$Want.command
        '/t:Task/t:Actions/t:Exec/t:Arguments'=$Want.arguments
        '/t:Task/t:Actions/t:Exec/t:WorkingDirectory'=$Want.directory
    }
    foreach ($key in $checks.Keys) {
        $value=TextAt $key
        if($key -cin @('/t:Task/t:Principals/t:Principal/t:UserId','/t:Task/t:Triggers/t:LogonTrigger/t:UserId')){$value=TaskSid $value}
        if ($value -cne $checks[$key]) { throw 'LAB_TASK_DRIFT' }
    }
    $acl=[Security.AccessControl.RawSecurityDescriptor]::new($Task.GetSecurityDescriptor(4))
    $seen=@{}
    foreach ($ace in $acl.DiscretionaryAcl) {
        $s=$ace.SecurityIdentifier.Value
        if ($ace -isnot [Security.AccessControl.CommonAce] -or $ace.IsCallback -or $ace.AceFlags -ne [Security.AccessControl.AceFlags]::None -or
            $ace.AceQualifier -ne 'AccessAllowed' -or $seen.ContainsKey($s)) { throw 'LAB_TASK_DACL' }
        $mask=$ace.AccessMask
        if ($s -ceq $Want.sid) {
            if ($mask -notin @(-1610612736,1179817)) { throw 'LAB_TASK_DACL' } # GR|GX or mapped ReadAndExecute.
        } elseif ($s -in @('S-1-5-18','S-1-5-32-544')) {
            if ($mask -notin @(2032127,268435456)) { throw 'LAB_TASK_DACL' }
        } else { throw 'LAB_TASK_DACL' }
        $seen[$s]=$true
    }
    if ($seen.Count -ne 3 -or !($acl.ControlFlags -band [Security.AccessControl.ControlFlags]::DiscretionaryAclProtected)) { throw 'LAB_TASK_DACL' }
}
function Lab-Rules($Cfg) {
    foreach ($kind in @('Discovery','Iroh')) {
        [ordered]@{Name="Kilogram-Lab-Allow$kind-$($Cfg.side)";DisplayName="Kilogram-Lab-Allow$kind-$($Cfg.side)";
            Description="Kilogram lab $($Cfg.campaign) $($Cfg.side)";Group="Kilogram-Lab-$($Cfg.campaign)";
            Enabled='True';Profile='Private';Direction='Inbound';Action='Allow';EdgeTraversalPolicy='Block';
            Program=(Join-Path $Cfg.base 'bin/kilogram-cli.exe');Protocol='UDP';
            LocalAddress=if($kind -ceq 'Discovery'){'239.255.75.72'}else{$Cfg.ip};RemoteAddress=$Cfg.peer;
            LocalPort=if($kind -ceq 'Discovery'){'50372'}else{'49152-65535'};
            RemotePort=if($kind -ceq 'Discovery'){'50372'}else{'49152-65535'};InterfaceAlias=$Cfg.interface_alias;
            InterfaceType='Any';Package='Any';Service='Any';Authentication='NotRequired';Encryption='NotRequired';
            OverrideBlockRules='False';LocalUser='Any';RemoteUser='Any';RemoteMachine='Any';DynamicTransport='Any'}
    }
}
function Lab-Equal($A,$B) {
    # Compare recursively without relying on JSON property order.
    if ($null -eq $A -or $null -eq $B) { return $null -eq $A -and $null -eq $B }
    if ($A -is [Collections.IDictionary]) { $A=[pscustomobject]$A }
    if ($B -is [Collections.IDictionary]) { $B=[pscustomobject]$B }
    if ($A -is [pscustomobject] -and $B -is [pscustomobject]) {
        $keys=@($A.PSObject.Properties.Name | Sort-Object)
        if (($keys -join '|') -cne (@($B.PSObject.Properties.Name | Sort-Object) -join '|')) { return $false }
        foreach ($key in $keys) { if (!(Lab-Equal $A.$key $B.$key)) { return $false } }
        return $true
    }
    if ($A -is [array] -and $B -is [array]) {
        if ($A.Count -ne $B.Count) { return $false }
        for ($i=0;$i -lt $A.Count;$i++) { if (!(Lab-Equal $A[$i] $B[$i])) { return $false } }
        return $true
    }
    return [string]$A -ceq [string]$B
}
function Invoke-LabTransaction($Backend,$Want,[string]$Operation) {
    # The test adapter implements the same finite mutations. Production uses only native fixed specs.
    $journal=& $Backend.Read
    $now=& $Backend.Snapshot
    Set-LabStage 'transaction-admission'
    if ($Operation -ceq 'Provision') {
        if ($journal) {
            if (!(Lab-Equal $journal.want $Want)) { throw 'LAB_BASELINE_SCOPE' }
            if ($journal.phase -ceq 'installed') {
                Set-LabStage 'verify-installed'
                & $Backend.Verify $Want
                return @{state='already_installed';baseline_preserved=$true}
            }
            if($journal.phase -cne 'rolled_back') { throw 'LAB_INCOMPLETE_INSTALL_REQUIRES_ROLLBACK' }
            if(!(Lab-Equal $journal.initial $now)){throw 'LAB_ROLLBACK_BASELINE_DRIFT'}
        }
        if ($now.task -or @($now.rules).Count) { throw 'LAB_NAME_COLLISION' }
        Set-LabStage 'transaction-preflight'; & $Backend.Preflight $now
        if(!$journal){$journal=[pscustomobject]@{phase='installing';want=$Want;initial=$now;changes=@();rollback_errors=@()}}
        else{$journal.phase='installing';$journal.changes=@();$journal.rollback_errors=@()}
        & $Backend.Write $journal
        try {
            foreach ($change in @('files','task','rule0','rule1','private')) {
                # Persist intent before mutation; rollback checks actual owned configuration.
                $journal.changes+=@($change); & $Backend.Write $journal
                Set-LabStage ('apply-'+$change)
                & $Backend.Apply $change $Want
            }
            Set-LabStage 'verify-installed'
            & $Backend.Verify $Want
            $journal.phase='installed'; & $Backend.Write $journal
            return @{state='installed';baseline_preserved=$true}
        } catch {
            $primary=$_.Exception.Message
            $script:LabPrimary=Get-LabFailureSummary $_ (Get-LabStage)
            $journal|Add-Member -NotePropertyName primary_failure -NotePropertyValue $script:LabPrimary -Force
            for ($i=$journal.changes.Count-1;$i -ge 0;$i--) {
                try { Set-LabStage ('rollback-'+$journal.changes[$i]); & $Backend.Undo $journal.changes[$i] $Want $journal.initial }
                catch { $journal.rollback_errors+=@($_.Exception.Message) }
            }
            $journal.phase=if($journal.rollback_errors.Count){'rollback_incomplete'}else{'rolled_back'}
            & $Backend.Write $journal
            throw "LAB_INSTALL_FAILED:$primary;rollback=$($journal.phase)"
        }
    }
    if (!$journal -or !(Lab-Equal $journal.want $Want)) { throw 'LAB_BASELINE_SCOPE' }
    if ($Operation -ceq 'Verify') {
        if ($journal.phase -cne 'installed') { throw 'LAB_NOT_INSTALLED' }
        & $Backend.Verify $Want
        return @{state='verified';baseline_preserved=$true}
    }
    if ($Operation -cne 'Teardown') { throw 'LAB_OPERATION' }
    if ($journal.phase -ceq 'removed' -or $journal.phase -ceq 'rolled_back') { return @{state=$journal.phase} }
    # Preflight all ownership/drift before any removal. Never overwrite a changed profile.
    Set-LabStage 'recovery-preflight'; & $Backend.CanRemove $Want $journal.initial $journal.phase
    $recovery=$journal.phase -cin @('installing','rollback_incomplete','recovering')
    if(!$recovery -and $journal.phase -cnotin @('installed','removing')){throw 'LAB_INSTALL_PHASE'}
    $journal.phase=if($recovery){'recovering'}else{'removing'}; & $Backend.Write $journal
    for ($i=$journal.changes.Count-1;$i -ge 0;$i--) { Set-LabStage ('recovery-'+$journal.changes[$i]); & $Backend.Undo $journal.changes[$i] $Want $journal.initial }
    $journal.phase=if($recovery){'rolled_back'}else{'removed'}
    if($recovery){$journal.rollback_errors=@()}
    & $Backend.Write $journal
    return @{state=$journal.phase;baseline_preserved=$true}
}
function Undo-LabInstallFiles([string]$Base,[string]$StateDirectory,[string[]]$Names) {
    $j=Lab-Read (Join-Path $StateDirectory 'initial-and-journal.json') -MaxBytes 2097152
    if($j.phase -cin @('removing','removed')){return} # Final campaign teardown preserves ordinary updates/evidence.
    if($j.phase -cnotin @('installing','rollback_incomplete','recovering')){throw 'LAB_INSTALL_PHASE'}
    $targets=@()
    $baseline=Join-Path $Base 'ordinary-agent/campaign-baseline.json'
    if(Test-Path -LiteralPath $baseline) {
        $intent=Lab-Read (Join-Path $StateDirectory 'public-baseline-intent.json')
        if(!$intent -or (Lab-Hash $baseline) -cne $intent.sha256){throw 'LAB_BASELINE_DRIFT'}
        $targets+=@($baseline)
    }
    foreach($name in $Names) {
        if($name -cnotmatch '\A[a-z0-9][a-z0-9.-]+\z'){throw 'LAB_FILE_SCOPE'}
        $target=Join-Path $Base "ordinary-agent/$name"
        if(Test-Path -LiteralPath $target) {
            Lab-Hex $j.file_pins.$name 64
            if((Lab-Hash $target) -cne $j.file_pins.$name){throw 'LAB_FILE_DRIFT'}
            $targets+=@($target)
        }
    }
    # Verify the complete owned set before removing any file.
    foreach($target in $targets){Remove-Item -LiteralPath $target}
}
function Save-LabInstallJournal([string]$Directory,$Journal,$PackageFiles) {
    $initial=Join-Path $Directory 'initial.json'
    if(Test-Path -LiteralPath $initial){if(!(Lab-Equal (Lab-Read $initial -MaxBytes 2097152) $Journal.initial)){throw 'LAB_INITIAL_BASELINE_CHANGED'}}
    else{Lab-Write $initial $Journal.initial -MaxBytes 2097152}
    $path=Join-Path $Directory 'initial-and-journal.json'
    $previous=Lab-Read $path -MaxBytes 2097152
    if($previous -and $previous.phase -cne $Journal.phase) {
        $history=Join-Path $Directory 'journal-history';[void][IO.Directory]::CreateDirectory($history);Lab-Plain $history
        Lab-Write (Join-Path $history "$([guid]::NewGuid().ToString('N')).json") $previous -MaxBytes 2097152
    }
    # A retried failed installation may use a corrected reviewed package. Its own copied bytes,
    # not the first failed package's bytes, are the rollback scope; the initial host baseline is immutable.
    if($Journal.phase -ceq 'installing' -and @($Journal.changes).Count -eq 0 -and $PackageFiles) {
        $Journal | Add-Member -NotePropertyName file_pins -NotePropertyValue $PackageFiles -Force
    }
    Lab-Write $path $Journal -MaxBytes 2097152
}

function New-LabFilterIndex([object[]]$Rules,[string]$Store) {
    $wanted=@{};foreach($rule in $Rules){if(!$rule.InstanceID -or $wanted.ContainsKey($rule.InstanceID)){throw 'LAB_RULE_ID_AMBIGUOUS'};$wanted[$rule.InstanceID]=$true}
    $index=@{}
    foreach($kind in @('Application','Service','Port','Address','Interface','InterfaceType','Security')) {
        $rows=@{}
        if($wanted.Count) {
            foreach($filter in ($Rules | & "Get-NetFirewall${kind}Filter" -PolicyStore $Store -ThrottleLimit 4)) {
                if($wanted.ContainsKey($filter.InstanceID)) {
                    if($rows.ContainsKey($filter.InstanceID)){throw 'LAB_FILTER_AMBIGUOUS'}
                    $rows[$filter.InstanceID]=$filter
                }
            }
        }
        if($rows.Count -ne $wanted.Count){throw 'LAB_FILTER_MISSING'}
        $index[$kind]=$rows
    }
    return $index
}
function Get-LabRuleView($Rule,[string]$Store,$Index=$null) {
    $view=[ordered]@{}
    foreach($key in @('Name','DisplayName','Description','Group','Enabled','Profile','Direction','Action','EdgeTraversalPolicy')) { $view[$key]=[string]$Rule.$key }
    $kinds=[ordered]@{Application=@('Program','Package');Service=@('Service');Port=@('Protocol','LocalPort','RemotePort','DynamicTransport');
        Address=@('LocalAddress','RemoteAddress');Interface=@('InterfaceAlias');InterfaceType=@('InterfaceType');
        Security=@('Authentication','Encryption','OverrideBlockRules','LocalUser','RemoteUser','RemoteMachine')}
    foreach($kind in $kinds.Keys) {
        $filter=@(if($null -eq $Index){& "Get-NetFirewall${kind}Filter" -AssociatedNetFirewallRule $Rule -PolicyStore $Store}else{$Index[$kind][$Rule.InstanceID]})
        if($filter.Count -ne 1) { throw 'LAB_FILTER_AMBIGUOUS' }
        foreach($key in $kinds[$kind]) {
            $value=@($filter[0].$key | ForEach-Object {[string]$_} | Sort-Object) -join ','
            if (!$value -and $key -cin @('Package','Service','LocalUser','RemoteUser','RemoteMachine','DynamicTransport')) { $value='Any' }
            if ($key -cin @('LocalAddress','RemoteAddress')) { $value=$value -replace '/(32|255\.255\.255\.255)$','' }
            $view[$key]=$value
        }
    }
    if($view.Protocol -ceq '17'){$view.Protocol='UDP'}elseif($view.Protocol -ceq '6'){$view.Protocol='TCP'}elseif($view.Protocol -ceq '256'){$view.Protocol='Any'}
    return $view
}
function Get-LabSnapshot($Cfg,$Want) {
    $profiles=@(foreach($store in @('PersistentStore','ActiveStore')) {
        foreach($p in @(Get-NetFirewallProfile -PolicyStore $store | Sort-Object Name)) {
            [ordered]@{store=$store;name=[string]$p.Name;enabled=[string]$p.Enabled;inbound=[string]$p.DefaultInboundAction;
                outbound=[string]$p.DefaultOutboundAction;allow_inbound=[string]$p.AllowInboundRules;allow_local=[string]$p.AllowLocalFirewallRules;
                disabled=@($p.DisabledInterfaceAliases | Sort-Object)}
        }
    })
    $byStore=@{};foreach($store in @('PersistentStore','ActiveStore')){$byStore[$store]=@(Get-NetFirewallRule -PolicyStore $store | Sort-Object Name)}
    $rules=@(foreach($store in @('PersistentStore','ActiveStore')) {
        foreach($r in $byStore[$store]) {
            if ($r.Name -cin @($Want.rules | ForEach-Object Name)) { @{store=$store;view=(Get-LabRuleView $r $store)} }
        }
    })
    $selected=@($byStore['ActiveStore'] | Where-Object { [string]$_.Enabled -eq 'True' -and
        ([string]$_.Profile -eq 'Any' -or [string]$_.Profile -match '\bPrivate\b') })
    $index=New-LabFilterIndex $selected ActiveStore
    $policy=@($selected | ForEach-Object { Get-LabRuleView $_ ActiveStore $index })
    # One native association check guards the bulk identifier mapping without repeating all queries per rule.
    if($selected.Count -and !(Lab-Equal $policy[0] (Get-LabRuleView $selected[0] ActiveStore))){throw 'LAB_FILTER_INDEX_DIFFERS'}
    $net=@(Get-NetConnectionProfile | Sort-Object InterfaceIndex | ForEach-Object { [ordered]@{index=[int]$_.InterfaceIndex;alias=$_.InterfaceAlias;category=[string]$_.NetworkCategory} })
    $adapter=Get-NetAdapter -InterfaceIndex $Cfg.interface_index
    $addr=@(Get-NetIPAddress -InterfaceIndex $Cfg.interface_index -AddressFamily IPv4 | ForEach-Object { "$($_.IPAddress)/$($_.PrefixLength)" } | Sort-Object)
    $task=Find-LabTask $Want.task.name
    $taskView=if($task){@{xml=$task.Xml;sddl=$task.GetSecurityDescriptor(4)}}else{$null}
    $range=@(& "$env:windir/System32/netsh.exe" int ipv4 show dynamicport udp)
    if($LASTEXITCODE -ne 0) { throw 'LAB_UDP_RANGE' }
    [ordered]@{profiles=$profiles;rules=$rules;task=$taskView;policy=$policy;network=$net;
        adapter=@{name=$adapter.Name;guid=[string]$adapter.InterfaceGuid;status=[string]$adapter.Status;addresses=$addr};dynamic_udp=$range;
        ssh=(Get-LabSshFacts)}
}
function Get-LabRelevant($Cfg,$Want) {
    # Ordinary attempt checks avoid enumerating every unrelated firewall filter.
    $profiles=@(foreach($store in @('PersistentStore','ActiveStore')) {
        foreach($p in @(Get-NetFirewallProfile -PolicyStore $store | Sort-Object Name)) {
            [ordered]@{store=$store;name=[string]$p.Name;enabled=[string]$p.Enabled;inbound=[string]$p.DefaultInboundAction;
                outbound=[string]$p.DefaultOutboundAction;allow_inbound=[string]$p.AllowInboundRules;allow_local=[string]$p.AllowLocalFirewallRules;
                disabled=@($p.DisabledInterfaceAliases | Sort-Object)}
        }
    })
    $rules=@(foreach($store in @('PersistentStore','ActiveStore')) {
        foreach($rule in $Want.rules) {
            $rows=@(Get-NetFirewallRule -PolicyStore $store -Name $rule.Name)
            if($rows.Count -ne 1){throw 'LAB_RULE_DRIFT'}
            @{store=$store;view=(Get-LabRuleView $rows[0] $store)}
        }
    })
    $network=@(Get-NetConnectionProfile | Sort-Object InterfaceIndex | ForEach-Object { [ordered]@{index=[int]$_.InterfaceIndex;alias=$_.InterfaceAlias;category=[string]$_.NetworkCategory} })
    $adapter=Get-NetAdapter -InterfaceIndex $Cfg.interface_index
    $addresses=@(Get-NetIPAddress -InterfaceIndex $Cfg.interface_index -AddressFamily IPv4 | ForEach-Object {"$($_.IPAddress)/$($_.PrefixLength)"} | Sort-Object)
    $range=@(& "$env:windir/System32/netsh.exe" int ipv4 show dynamicport udp);if($LASTEXITCODE -ne 0){throw 'LAB_UDP_RANGE'}
    @{profiles=$profiles;rules=$rules;network=$network;adapter=@{name=$adapter.Name;guid=[string]$adapter.InterfaceGuid;status=[string]$adapter.Status;addresses=$addresses};dynamic_udp=$range}
}
function Assert-LabReadOnly($Cfg,[string]$Pin) {
    Lab-Hex $Pin 64
    $path=Join-Path $Cfg.base 'ordinary-agent/campaign-baseline.json'
    if((Lab-Hash $path) -cne $Pin){throw 'LAB_BASELINE_PIN'}
    $baseline=Lab-Read $path
    $want=@{task=(Lab-TaskSpec $Cfg);rules=@(Lab-Rules $Cfg)}
    if(!(Lab-Equal $baseline.task $want.task)){throw 'LAB_BASELINE_SCOPE'}
    Assert-LabTask (Find-LabTask $want.task.name) $want.task
    if($baseline.v -ne 2 -or !$baseline.ordinary){throw 'LAB_BASELINE_BACKEND'}
    if(!(Lab-Equal $baseline.ordinary (Get-LabOrdinaryState $Cfg $want))){throw 'LAB_BASELINE_DRIFT'}
    @{state='verified';baseline_sha256=$Pin;initial_audit_sha256=$baseline.initial_audit_sha256;read_only=$true}
}
function Find-LabTask([string]$Name) {
    $service=New-Object -ComObject Schedule.Service; $service.Connect()
    try { $service.GetFolder('\').GetTask($Name) }
    catch {
        $errorObject=$_.Exception;$missing=$false
        while($errorObject){if($errorObject.HResult -eq -2147024894){$missing=$true};$errorObject=$errorObject.InnerException}
        if(!$missing){throw};return $null
    }
}
function Get-LabSshFacts {
    $service=Get-CimInstance Win32_Service -Filter "Name='sshd'"
    # Source ports are transport observations, not immutable policy: Audit and Provision
    # can run through different SSH sessions (or interactive UAC). Validate the live route separately.
    @{service_path=if($service){[string]$service.PathName}else{''};running=($service -and $service.State -ceq 'Running');connection=$null}
}
function Lab-RuleField($Rule,[string]$Name) {
    if($Rule -is [Collections.IDictionary]){if($Rule.Contains($Name)){return [string]$Rule[$Name]}}
    elseif($Rule.PSObject.Properties[$Name]){return [string]$Rule.$Name}
    return '__unsupported__'
}
function Test-LabSet([string]$Text,[string]$Expected) {
    $unknown=$false
    foreach($token in $Text.Split(',')) {
        $token=$token.Trim()
        if($token -ieq 'Any' -or $token -ieq $Expected){return 1}
        if(!$token -or $token -eq '__unsupported__' -or $token -match '[*?]'){$unknown=$true}
    }
    if($unknown){return 0};return -1
}
function Test-LabPorts([string]$Text,[int]$First,[int]$Last,[bool]$NeedAll) {
    $ranges=@();$unknown=$false
    foreach($token in $Text.Split(',')) {
        $token=$token.Trim()
        if($token -ieq 'Any'){$ranges+=,@(1,65535)}
        elseif($token -match '^(\d{1,5})(?:-(\d{1,5}))?$') {
            $start=[int]$Matches[1];$end=if($Matches[2]){[int]$Matches[2]}else{$start}
            if($start -lt 1 -or $end -gt 65535 -or $start -gt $end){$unknown=$true}else{$ranges+=,@($start,$end)}
        }else{$unknown=$true}
    }
    $cursor=$First
    foreach($range in @($ranges | Sort-Object {$_[0]})) {
        if(!$NeedAll -and $range[0] -le $Last -and $range[1] -ge $First){return 1}
        if($range[0] -le $cursor -and $range[1] -ge $cursor){$cursor=[Math]::Max($cursor,$range[1]+1)}
    }
    if($NeedAll -and $cursor -gt $Last){return 1}
    if($unknown){return 0};return -1
}
function Lab-IPv4([string]$Text) {
    if($Text -notmatch '^\d{1,3}(\.\d{1,3}){3}$'){return $null}
    $value=[uint64]0
    foreach($part in $Text.Split('.')){if([int]$part -gt 255){return $null};$value=$value*256+[int]$part}
    return $value
}
function Test-LabAddress([string]$Text,[string]$IP,[string]$LocalIP) {
    $target=Lab-IPv4 $IP;$local=Lab-IPv4 $LocalIP;$unknown=$false
    foreach($token in $Text.Split(',')) {
        $token=$token.Trim()
        if($token -ieq 'Any'){return 1}
        if($token -iin @('LocalSubnet','LocalSubnet4')){if([Math]::Floor($target/256) -eq [Math]::Floor($local/256)){return 1};continue}
        $bounds=$token.Split('-')
        if($bounds.Count -eq 2) {
            $first=Lab-IPv4 $bounds[0];$last=Lab-IPv4 $bounds[1]
            if($null -eq $first -or $null -eq $last -or $first -gt $last){$unknown=$true}
            elseif($target -ge $first -and $target -le $last){return 1}
            continue
        }
        $cidr=$token.Split('/');$address=Lab-IPv4 $cidr[0]
        if($null -eq $address) {
            $parsed=$null
            if(![Net.IPAddress]::TryParse($token.Split('/')[0],[ref]$parsed) -or $parsed.AddressFamily -ne 'InterNetworkV6'){$unknown=$true}
            continue
        }
        $prefix=32
        if($cidr.Count -eq 2) {
            if($cidr[1] -match '^\d{1,2}$' -and [int]$cidr[1] -le 32){$prefix=[int]$cidr[1]}
            else {
                $mask=Lab-IPv4 $cidr[1]
                if($null -eq $mask){$unknown=$true;continue}
                $bits=[Convert]::ToString([long]$mask,2).PadLeft(32,'0')
                if($bits -notmatch '^1*0*$'){$unknown=$true;continue};$prefix=$bits.TrimEnd('0').Length
            }
        }elseif($cidr.Count -ne 1){$unknown=$true;continue}
        $size=[Math]::Pow(2,32-$prefix)
        if([Math]::Floor($target/$size) -eq [Math]::Floor($address/$size)){return 1}
    }
    if($unknown){return 0};return -1
}
function Test-LabSshRule($Rule,$Cfg,[bool]$Client,[bool]$Allow,[bool]$Reply=$false,$Actor=$null) {
    $system=[Environment]::GetFolderPath('System')
    $program=Join-Path $system $(if($Client){'OpenSSH/ssh.exe'}else{'OpenSSH/sshd.exe'})
    $app=[Environment]::ExpandEnvironmentVariables((Lab-RuleField $Rule Program)).Trim('"')
    $protocol=Lab-RuleField $Rule Protocol
    if($protocol -ceq '6'){$protocol='TCP'}elseif($protocol -ceq '256'){$protocol='Any'}
    $tests=@(
        (Test-LabSet (Lab-RuleField $Rule Enabled) 'True'),
        (Test-LabSet (Lab-RuleField $Rule Profile) 'Private'),
        (Test-LabSet (Lab-RuleField $Rule Direction) $(if($Client -xor $Reply){'Outbound'}else{'Inbound'})),
        (Test-LabSet $protocol 'TCP'),
        (Test-LabSet $app $program),
        (Test-LabSet (Lab-RuleField $Rule Service) $(if($Client){'__no_service__'}else{'sshd'})),
        (Test-LabSet (Lab-RuleField $Rule InterfaceAlias) $Cfg.interface_alias),
        (Test-LabAddress (Lab-RuleField $Rule LocalAddress) $Cfg.ip $Cfg.ip),
        (Test-LabAddress (Lab-RuleField $Rule RemoteAddress) $Cfg.peer $Cfg.ip),
        (Test-LabPorts (Lab-RuleField $Rule LocalPort) $(if($Client){1}else{22}) $(if($Client){65535}else{22}) $Allow),
        (Test-LabPorts (Lab-RuleField $Rule RemotePort) $(if($Client){22}else{1}) $(if($Client){22}else{65535}) $Allow)
    )
    # Constrained interface/security expressions are deliberately unsupported, never assumed to allow.
    foreach($key in @('Package','InterfaceType','DynamicTransport','RemoteUser','RemoteMachine')) {
        $tests+=if((Lab-RuleField $Rule $key) -ieq 'Any'){1}else{0}
    }
    $localUser=Lab-RuleField $Rule LocalUser
    if($localUser -ieq 'Any'){$tests+=1}
    elseif($Client -and $Actor -and $Actor -is [KilogramLabNative+Actor] -and $Actor.user -ceq $Cfg.sid -and !$Actor.elevated){$tests+=$Actor.LocalUser($localUser)}
    elseif(!$Client -and $Cfg.side -ceq 'B' -and $Actor -and $Actor -is [KilogramLabNative+ServerActors] -and $Actor.ordinary_sid -ceq $Cfg.sid -and $Actor.Alive()){$tests+=$Actor.LocalUser($localUser)}
    else{$tests+=0}
    foreach($pair in @(@('Authentication','NotRequired'),@('Encryption','NotRequired'),@('OverrideBlockRules','False'))) {
        $tests+=if((Lab-RuleField $Rule $pair[0]) -ieq $pair[1]){1}else{0}
    }
    if(-1 -in $tests){return -1};if(0 -in $tests){return 0};return 1
}
function Assert-LabSshPath($Cfg,$Snapshot,$Actor=$null) {
    $client=$Cfg.side -ceq 'A' # Actual route: A runs the controller/ssh client; B runs sshd.
    if(!$client -and $Actor){$null=Assert-LabServerActors $Cfg $Actor}
    $expected=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
    $hasSsh=if($Snapshot -is [Collections.IDictionary]){$Snapshot.Contains('ssh')}else{[bool]$Snapshot.PSObject.Properties['ssh']}
    if(!$client -and (!$hasSsh -or !$Snapshot.ssh -or !$Snapshot.ssh.running -or $Snapshot.ssh.service_path.Trim('"') -ine $expected)){throw 'LAB_SSH_SERVICE_UNCERTAIN'}
    $connection=$env:SSH_CONNECTION
    if(!$connection -and $hasSsh){$connection=$Snapshot.ssh.connection}
    if($connection) {
        if($client){throw 'LAB_SSH_ROUTE_UNCERTAIN'}
        $parts=$connection.Split(' ')
        if($parts.Count -ne 4 -or $parts[0] -cne $Cfg.peer -or $parts[2] -cne $Cfg.ip -or $parts[3] -cne '22' -or
            $parts[1] -notmatch '^\d{1,5}$' -or [int]$parts[1] -lt 1 -or [int]$parts[1] -gt 65535){throw 'LAB_SSH_ROUTE_UNCERTAIN'}
    }
    $allowed=$client;$uncertain=$false # Effective outbound default Allow is checked by Assert-LabNetwork.
    foreach($rule in $Snapshot.policy) {
        $action=Lab-RuleField $rule Action
        if($action -ieq 'Allow' -and !$client) {
            $match=Test-LabSshRule $rule $Cfg $false $true
            if($match -eq 1){$allowed=$true}elseif($match -eq 0){$uncertain=$true}
        }elseif($action -ieq 'Block') {
            foreach($reply in @($false,$true)) {
                $match=Test-LabSshRule $rule $Cfg $client $false $reply $Actor
                if($match -eq 1){throw 'LAB_SSH_BLOCKED'}elseif($match -eq 0){throw 'LAB_SSH_FILTER_UNCERTAIN'}
            }
        }elseif($action -ine 'Allow'){throw 'LAB_SSH_FILTER_UNCERTAIN'}
    }
    if(!$allowed){if($uncertain){throw 'LAB_SSH_FILTER_UNCERTAIN'};throw 'LAB_SSH_EFFECT_UNCONFIRMED'}
}
function Assert-LabNetwork($Cfg,$Snapshot,$Actor=$null,[switch]$SkipSsh) {
    $private=@($Snapshot.profiles | Where-Object { $_.name -ceq 'Private' -and $_.store -ceq 'ActiveStore' })
    if ($private.Count -ne 1 -or $private[0].inbound -cne 'Block' -or $private[0].outbound -cne 'Allow' -or
        $private[0].allow_inbound -cne 'True' -or $private[0].allow_local -cne 'True' -or $Cfg.interface_alias -in $private[0].disabled) { throw 'LAB_PRIVATE_POLICY' }
    $lan=@($Snapshot.network | Where-Object index -eq $Cfg.interface_index)
    if ($lan.Count -ne 1 -or $lan[0].category -cne 'Private' -or
        @($Snapshot.network | Where-Object { $_.category -ceq 'Private' -and $_.index -ne $Cfg.interface_index }).Count -or
        $Snapshot.adapter.name -cne $Cfg.interface_alias -or [guid]$Snapshot.adapter.guid -ne [guid]$Cfg.interface_guid -or
        $Snapshot.adapter.status -cne 'Up' -or "$($Cfg.ip)/24" -cnotin $Snapshot.adapter.addresses) { throw 'LAB_PROFILE_WIDE_NETWORK_EFFECT' }
    $numbers=[regex]::Matches(($Snapshot.dynamic_udp -join "`n"),'\d+') | ForEach-Object Value
    if (($numbers -join ',') -cne '49152,16384') { throw 'LAB_UDP_RANGE' }
    # Require one supported inbound allow covering every legal client source port, and no
    # possibly applicable inbound-server/outbound-client block. This covers reconnect too.
    if(!$SkipSsh){Assert-LabSshPath $Cfg $Snapshot $Actor}
}
function Assert-LabInstalled($Cfg,$Want,$Initial,$Actor=$null,[switch]$TeardownOnly) {
    $now=Get-LabSnapshot $Cfg $Want
    Assert-LabNetwork $Cfg $now $Actor -SkipSsh:$TeardownOnly
    Assert-LabTask (Find-LabTask $Want.task.name) $Want.task
    foreach($rule in $Want.rules) {
        foreach($store in @('PersistentStore','ActiveStore')) {
            $rows=@($now.rules | Where-Object { $_.store -ceq $store -and $_.view.Name -ceq $rule.Name })
            if($rows.Count -ne 1 -or !(Lab-Equal $rows[0].view $rule)) { throw 'LAB_RULE_DRIFT' }
        }
    }
    foreach($p in $now.profiles) {
        $original=@($Initial.profiles | Where-Object { $_.store -ceq $p.store -and $_.name -ceq $p.name })[0]
        $expected=$original | ConvertTo-Json -Depth 8 -Compress | ConvertFrom-Json
        if($p.name -ceq 'Private') { $expected.enabled='True' }
        if(!(Lab-Equal $p $expected)) { throw 'LAB_PROFILE_DRIFT' }
    }
    $before=@($Initial.policy | Where-Object { $_.Name -cnotin @($Want.rules | ForEach-Object Name) })
    $after=@($now.policy | Where-Object { $_.Name -cnotin @($Want.rules | ForEach-Object Name) })
    if(!(Lab-Equal $before $after) -or !(Lab-Equal $Initial.network $now.network)) { throw 'LAB_UNRELATED_POLICY_DRIFT' }
}
function Assert-LabRestoreScope($Now,$Initial,$Want) {
    # Also applies after an interrupted install/teardown, when owned rules/task may be partly absent.
    foreach($key in @('network','adapter','dynamic_udp')) {
        if(!(Lab-Equal $Now.$key $Initial.$key)){throw 'LAB_RESTORE_NETWORK_DRIFT'}
    }
    if(@($Now.profiles).Count -ne @($Initial.profiles).Count){throw 'LAB_PROFILE_DRIFT'}
    foreach($p in $Now.profiles) {
        $matches=@($Initial.profiles | Where-Object { $_.store -ceq $p.store -and $_.name -ceq $p.name })
        if($matches.Count -ne 1){throw 'LAB_PROFILE_DRIFT'}
        $expected=$matches[0] | ConvertTo-Json -Depth 8 -Compress | ConvertFrom-Json
        if($p.name -ceq 'Private' -and $p.enabled -ceq 'True'){$expected.enabled='True'}
        if(!(Lab-Equal $p $expected)){throw 'LAB_PROFILE_DRIFT'}
    }
    $names=@($Want.rules | ForEach-Object Name)
    $before=@($Initial.policy | Where-Object { $_.Name -cnotin $names })
    $after=@($Now.policy | Where-Object { $_.Name -cnotin $names })
    if(!(Lab-Equal $before $after)){throw 'LAB_UNRELATED_POLICY_DRIFT'}
}

if($Mode -ceq 'Library') { return }
$script:LabPrimary=$null;$script:LabOperation=$Mode;Set-LabStage 'configuration'
trap {
    $failure=$_
    # Fixed protected package only. Never write a privileged error artifact to an
    # ordinary caller-selected directory; preserve the original exception if reporting fails.
    try{
        if($script:LabOperation -cin @('Setup','SetupRecoveredA','SetupFreshB','RecoverOld','Provision','Verify','Teardown') -and $PackageRoot -and $SetupSession){
            $leaf=Split-Path -Leaf $SetupSession
            $expected=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) ('Kilogram-Lab/SetupPackages/'+$leaf)
            if($leaf -cmatch '^setup-[0-9a-f]{32}$' -and [IO.Path]::GetFullPath($PackageRoot) -ieq $expected){
                $terminal=Get-LabFailureSummary $failure (Get-LabStage)
                $first=if($script:LabPrimary){$script:LabPrimary}else{$terminal}
                $name=if($script:LabOperation -ceq 'RecoverOld'){'old-recovery-error.json'}elseif($script:LabOperation -ceq 'Teardown'){'recovery-error.json'}else{'main-error.json'}
                Lab-Write (Join-Path $PackageRoot $name) @{primary=$first;terminal=$terminal;operation=$script:LabOperation}
            }
        }
    }catch{}
    throw $failure
}
$cfg=Get-LabConfig $Side
$want=@{task=(Lab-TaskSpec $cfg);rules=@(Lab-Rules $cfg);config=$cfg}
$files=@('test-terminal-t1-3e-lab-agent.ps1','test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-job.cs','test-terminal-t1-3e-lab-install.ps1',
    'test-terminal-t1-3e-lab-native.cs','test-terminal-t1-3e-lab-readiness.ps1','test-terminal-t1-3e-lab-setup.ps1','test-terminal-t1-3e-lab-launch.ps1',
    'test-terminal-t1-3e-lab-diagnostics.ps1','test-terminal-t1-3e-lab-bootstrap.ps1','test-terminal-t1-3e-lab-server.ps1','test-terminal-t1-3e-lab-recovery.ps1','dpapi-context.ps1','lab.json','ssh-probe.json')
if($Mode -ceq 'Package') {
    if(Test-Path -LiteralPath $Output) { throw 'LAB_PACKAGE_EXISTS' }
    [void][IO.Directory]::CreateDirectory($Output); Lab-Plain $Output
    foreach($name in $files | Where-Object { $_ -cnotin @('lab.json','dpapi-context.ps1','ssh-probe.json') }) { [IO.File]::Copy((Join-Path $PSScriptRoot $name),(Join-Path $Output $name)) }
    [IO.File]::Copy((Join-Path $PSScriptRoot 'test-terminal-t1-dpapi.ps1'),(Join-Path $Output 'dpapi-context.ps1'))
    Lab-Write (Join-Path $Output 'lab.json') $cfg
    # Package creation happens on A. Only paths/public host-key pin travel, never the private key bytes.
    $ssh=@{host='192.168.0.152';source='192.168.0.140';user='test';identity='C:\Users\Paul\.ssh\kilogram-test-laptop-final-ed25519';
        known_hosts='E:\AI-Projects\kilogram-p2p-messenger\.tmp\remote-access\known_hosts';fingerprint='SHA256:vvR2wQIJmbHD5033l5xVMbrSGyUcucNCy711PYU4oyc'}
    $ssh.known_hosts_sha256=Lab-Hash $ssh.known_hosts
    Lab-Write (Join-Path $Output 'ssh-probe.json') $ssh
    $pins=[ordered]@{};foreach($name in $files){$pins[$name]=Lab-Hash (Join-Path $Output $name)}
    Lab-Write (Join-Path $Output 'package.json') @{v=2;side=$Side;files=$pins}
    & (Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-launch.ps1') -LaunchAction Build -LaunchSide $Side -LaunchRoot $Output -ManifestPin (Lab-Hash (Join-Path $Output 'package.json'))
    @{package=$Output;sha256=(Lab-Hash (Join-Path $Output 'package.json'));spec=$want} | ConvertTo-Json -Depth 14
    return
}
if([Environment]::MachineName -cne $cfg.host) { throw 'LAB_HOST' }
if($Mode -ceq 'Audit') {
    $snapshot=Get-LabSnapshot $cfg $want
    $json=ConvertTo-Json -InputObject $snapshot -Depth 18 -Compress
    $hash=[Security.Cryptography.SHA256]::Create()
    try{$sha=[BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($json))).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
    @{sha256=$sha;snapshot=$snapshot;spec=$want} | ConvertTo-Json -Depth 20
    return
}
if($Mode -ceq 'Verify' -and $Ordinary) {
    if(!(Lab-Equal (Lab-Config (Join-Path $cfg.base 'ordinary-agent/lab.json')) $cfg)){throw 'LAB_BASELINE_SCOPE'}
    Assert-LabReadOnly $cfg $BaselineSha | ConvertTo-Json -Depth 10
    return
}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
try { if(!([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'LAB_ADMIN_SETUP_ONLY'} }
finally{$identity.Dispose()}
$setupMode=$Mode -cin @('Setup','SetupRecoveredA','SetupFreshB');$freshB=$Mode -ceq 'SetupFreshB';$recoveredA=$Mode -ceq 'SetupRecoveredA';$oldRecovery=$Mode -ceq 'RecoverOld'
if($freshB -and $Side -cne 'B'){throw 'LAB_FRESH_B_SIDE'}
if($recoveredA -and $Side -cne 'A'){throw 'LAB_RECOVERED_A_SIDE'}
$setupActor=$null;$held=[Collections.Generic.List[IDisposable]]::new()
if($setupMode -or $oldRecovery){
    try {
        $null=Get-LabSetupSession $cfg $SetupSession
        Set-LabStage 'package-validation';$package=Assert-LabPackage $PackageRoot $PackageSha $Side $files $cfg $held
        if($setupMode){
            Set-LabStage 'actor-admission';$setupActor=Open-LabSetupActor $cfg $SetupSession
            $Mode='Provision'
        }
    }catch{if($setupActor){$setupActor.Dispose()};foreach($f in $held){$f.Dispose()};throw}
}
try {
$stateRoot=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab'
$stateDir=Join-Path $stateRoot "$($cfg.campaign)-$Side"
foreach($directory in @($stateRoot,$stateDir)) {
    $acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
    foreach($sid in @('S-1-5-18','S-1-5-32-544')) { $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid),'FullControl','ContainerInherit, ObjectInherit','None','Allow')) }
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
    if(!(Test-Path -LiteralPath $directory)){[void][IO.Directory]::CreateDirectory($directory,$acl)}
    Lab-Plain $directory;$actualAcl=Get-Acl -LiteralPath $directory
    if(!$actualAcl.AreAccessRulesProtected -or $actualAcl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin @('S-1-5-18','S-1-5-32-544')) {throw 'LAB_STATE_ACL'}
    foreach($ace in $actualAcl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) { if($ace.AccessControlType -ne 'Allow' -or $ace.IdentityReference.Value -notin @('S-1-5-18','S-1-5-32-544')){throw 'LAB_STATE_ACL'} }
}
Set-LabStage 'install-lock'
$installLock=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None')
$journalPath=Join-Path $stateDir 'initial-and-journal.json'
$teardownHandles=@{agent=$null;images=$null}
try {
    $oldContext=$null
    if($recoveredA){Set-LabStage 'old-recovery-recheck';$null=Assert-LabRecoveredSetup $cfg $want $stateDir $PackageRoot}
    if($oldRecovery){
        Set-LabStage 'old-recovery-admission'
        $oldContext=Assert-LabOldRecovery $cfg $want $stateDir
        $files=$oldContext.names;$Mode='Teardown'
    }
    if($Mode -ceq 'Provision' -and !$setupMode) {
        $package=Assert-LabPackage $PackageRoot $PackageSha $Side $files $cfg $held
    }
    $packagePins=if($Mode -ceq 'Provision'){$package.files}else{$null}
    $backend=@{
        Read={
            $saved=Read-LabJournalStrict $journalPath
            if($freshB -and $saved){throw 'LAB_B_INTENT_ALREADY_EXISTS'}
            $saved
        }
        Write={param($j)
            if($setupMode){$j|Add-Member -NotePropertyName setup_session -NotePropertyValue (Split-Path -Leaf $SetupSession) -Force}
            Save-LabInstallJournal $stateDir $j $packagePins
        }
        Snapshot={
            if($setupMode){
                $admitted=Get-LabSetupSnapshot $cfg $want $setupActor $installLock $PackageRoot
                Lab-Write (Join-Path $PackageRoot 'fresh-audit.json') $admitted -MaxBytes 2097152
                $admitted.snapshot
            }else{Get-LabSnapshot $cfg $want}
        }
        Preflight={param($now)
            Assert-LabNetwork $cfg $now $setupActor
            if(!$setupMode){
                Lab-Hex $AuditSha 64
                if((Lab-JsonHash $now) -cne $AuditSha){throw 'LAB_AUDIT_CHANGED'}
            }
            $agentDirectory=Join-Path $cfg.base 'ordinary-agent'
            foreach($name in $files){if(Test-Path -LiteralPath (Join-Path $agentDirectory $name)){throw 'LAB_FILE_COLLISION'}}
            if(Test-Path -LiteralPath (Join-Path $agentDirectory 'campaign-baseline.json')){throw 'LAB_FILE_COLLISION'}
        }
        Apply={param($change,$w)
            switch($change){
                'files' {
                    $agentDirectory=Join-Path $cfg.base 'ordinary-agent';[void][IO.Directory]::CreateDirectory($agentDirectory);Lab-Plain $agentDirectory
                    foreach($name in $files){[IO.File]::Copy((Join-Path $PackageRoot $name),(Join-Path $agentDirectory $name),$false)}
                    $acl=Get-Acl -LiteralPath $agentDirectory
                    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($cfg.sid),'Modify','ContainerInherit, ObjectInherit','None','Allow'))
                    Set-Acl -LiteralPath $agentDirectory -AclObject $acl
                }
                'task' {
                    $s=New-Object -ComObject Schedule.Service;$s.Connect()
                    # CREATE|DONT_ADD_PRINCIPAL_ACE|IGNORE_REGISTRATION_TRIGGERS. Exact same SID as XML; no password.
                    $null=$s.GetFolder('\').RegisterTask($w.task.name,$w.task.xml,50,$w.task.sid,$null,3,$w.task.sddl)
                }
                {$_ -in @('rule0','rule1')} {
                    $spec=$w.rules[[int]$change.Substring(4)]
                    $params=@{};foreach($key in @('Name','DisplayName','Description','Group','Enabled','Profile','Direction','Action','EdgeTraversalPolicy','Program','Protocol','LocalAddress','RemoteAddress','LocalPort','RemotePort','InterfaceAlias')){$params[$key]=$spec[$key]}
                    $null=New-NetFirewallRule @params -PolicyStore PersistentStore
                }
                'private' {Set-NetFirewallProfile -PolicyStore PersistentStore -Name Private -Enabled True}
            }
        }
        Verify={param($w)
            $j=Lab-Read $journalPath -MaxBytes 2097152;Assert-LabInstalled $cfg $w $j.initial $setupActor
            $path=Join-Path $cfg.base 'ordinary-agent/campaign-baseline.json'
            $intent=Join-Path $stateDir 'public-baseline-intent.json'
            if(!(Test-Path -LiteralPath $path)) {
                if($j.phase -cne 'installing'){throw 'LAB_BASELINE_ABSENT'}
                $json=ConvertTo-Json -InputObject $j.initial -Depth 18 -Compress;$hash=[Security.Cryptography.SHA256]::Create()
                try{$initialSha=[BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($json))).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
                $relevant=Get-LabRelevant $cfg $w;$ordinaryNow=Get-LabOrdinaryState $cfg $w
                Assert-LabOrdinaryAgainstCim $cfg $w $ordinaryNow $relevant
                $value=[ordered]@{v=2;task=$w.task;initial_audit_sha256=$initialSha;relevant=$relevant;ordinary=$ordinaryNow}
                $bytes=[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $value -Depth 18 -Compress)+[char]10)
                $hash=[Security.Cryptography.SHA256]::Create()
                try{$pin=[BitConverter]::ToString($hash.ComputeHash($bytes)).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
                Lab-Write $intent @{sha256=$pin}
                Lab-Write $path $value
            }
            $pin=(Lab-Read $intent).sha256
            $null=Assert-LabReadOnly $cfg $pin
            Set-LabStage 'ordinary-task-probe';$proof=Invoke-LabTaskProbe $cfg $w.task $pin
            Lab-Write (Join-Path $stateDir ('task-proof-'+[guid]::NewGuid().ToString('N')+'.json')) $proof
        }
        CanRemove={param($w,$initial,$phase)
            if($phase -ceq 'installed'){Assert-LabInstalled $cfg $w $initial -TeardownOnly}
            Assert-LabRestoreScope (Get-LabSnapshot $cfg $w) $initial $w
            if(!('KilogramLabJob' -as [type])){Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"}
            Stop-LabIdleAgent $cfg.base $teardownHandles
        }
        Undo={param($change,$w,$initial)
            switch($change){
                'private' {
                    $p=@($initial.profiles | Where-Object { $_.store -ceq 'PersistentStore' -and $_.name -ceq 'Private' })[0]
                    $snapshot=Get-LabSnapshot $cfg $w
                    Assert-LabRestoreScope $snapshot $initial $w
                    $now=@($snapshot.profiles | Where-Object { $_.store -ceq 'PersistentStore' -and $_.name -ceq 'Private' })[0]
                    $expected=$p | ConvertTo-Json -Depth 8 -Compress | ConvertFrom-Json;$expected.enabled='True'
                    if(!(Lab-Equal $now $p) -and !(Lab-Equal $now $expected)){throw 'LAB_PROFILE_DRIFT'}
                    if($now.enabled -cne $p.enabled){Set-NetFirewallProfile -PolicyStore PersistentStore -Name Private -Enabled $p.enabled}
                }
                {$_ -in @('rule0','rule1')} {
                    $spec=$w.rules[[int]$change.Substring(4)]
                    $actual=@(Get-NetFirewallRule -PolicyStore PersistentStore | Where-Object Name -ceq $spec.Name)
                    if($actual.Count){if($actual.Count -ne 1 -or !(Lab-Equal (Get-LabRuleView $actual[0] PersistentStore) $spec)){throw 'LAB_RULE_DRIFT'};Remove-NetFirewallRule -PolicyStore PersistentStore -Name $spec.Name}
                }
                'task' {
                    $task=Find-LabTask $w.task.name
                    if($task){
                        Assert-LabTask $task $w.task
                        if(!('KilogramLabJob' -as [type])){Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"}
                        if(!$teardownHandles.agent){Stop-LabIdleAgent $cfg.base $teardownHandles}
                        $s=New-Object -ComObject Schedule.Service;$s.Connect();$s.GetFolder('\').DeleteTask($w.task.name,0)
                    }
                }
                'files' {
                    Undo-LabInstallFiles $cfg.base $stateDir $files
                }
            }
        }
    }
    $result=Invoke-LabTransaction $backend $want $Mode
    if($oldRecovery){
        $result.old_recovery=Assert-LabOldRecoveryCompleted $cfg $want $stateDir $oldContext
        Lab-Write (Join-Path $PackageRoot 'old-recovery-proof.json') @{before=$oldContext;result=$result.old_recovery} -MaxBytes 2097152
    }
    if($Mode -cin @('Provision','Verify')) {
        $baselinePath=Join-Path $cfg.base 'ordinary-agent/campaign-baseline.json'
        $result.baseline_sha256=Lab-Hash $baselinePath
    }
    $result | ConvertTo-Json -Depth 14
} finally {foreach($f in $teardownHandles.Values){if($f){$f.Dispose()}};$installLock.Dispose()}
} finally {foreach($f in $held){$f.Dispose()};if($setupActor){$setupActor.Dispose()}}
