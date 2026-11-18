param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
function Require([bool]$Value,[string]$Reason){if(!$Value){throw "CHECK:$Reason"}}
function Reject([scriptblock]$Body,[string]$Expected){
    try{&$Body}catch{if($_.Exception.Message.Contains($Expected)){return};throw}
    throw "CHECK:did_not_reject_$Expected"
}
function Fake-Backend {
    @{
        Read={if($fake.journal){$fake.journal | ConvertTo-Json -Depth 20 | ConvertFrom-Json}else{$null}}
        Write={param($j)
            if(!$fake.initial){$fake.initial=$j.initial | ConvertTo-Json -Depth 20 -Compress}
            Require (($j.initial | ConvertTo-Json -Depth 20 -Compress) -ceq $fake.initial) 'baseline_immutable'
            $fake.journal=$j | ConvertTo-Json -Depth 20 | ConvertFrom-Json
        }
        Snapshot={@{task=$fake.task;rules=@($fake.rules);private=$fake.private;files=$fake.files;unrelated=$fake.unrelated}}
        Preflight={param($now) Require ($now.unrelated -ceq 'vpn-public-ssh-original') 'preserve_external'}
        Apply={param($change,$w)
            $fake.calls+=@("apply:$change")
            switch($change){'files'{$fake.files=$true};'task'{$fake.task='owned'};'rule0'{$fake.rules+=@('owned0')};'rule1'{$fake.rules+=@('owned1')};'private'{$fake.private=$true}}
            if($fake.failure -ceq $change){throw "injected_$change"}
        }
        Verify={param($w)
            if($fake.failure -ceq 'verify'){throw 'injected_verify'}
            Require ($fake.task -ceq 'owned' -and ($fake.rules -join ',') -ceq 'owned0,owned1' -and $fake.private -and $fake.files) 'verify_exact'
            Require ($fake.unrelated -ceq 'vpn-public-ssh-original') 'unrelated_drift'
        }
        CanRemove={param($w,$initial,$phase) Require ($fake.task -ceq 'owned' -and $fake.unrelated -ceq 'vpn-public-ssh-original') 'drift_preflight'}
        Undo={param($change,$w,$initial)
            $fake.calls+=@("undo:$change")
            switch($change){
                'files'{$fake.files=$initial.files};'task'{Require ($fake.task -in @($null,'owned')) 'task_ownership';$fake.task=$initial.task}
                'rule0'{$fake.rules=@($fake.rules | Where-Object {$_ -cne 'owned0'})}
                'rule1'{$fake.rules=@($fake.rules | Where-Object {$_ -cne 'owned1'})}
                'private'{$fake.private=$initial.private}
            }
        }
    }
}
function New-Fake {
    @{journal=$null;initial=$null;task=$null;rules=@();private=$false;files=$false;unrelated='vpn-public-ssh-original';failure='';calls=@()}
}
$cases=[ordered]@{}
$want=@{campaign='test';task=@{sid='exact-user'};rules=@('owned0','owned1')}
$fake=New-Fake;$b=Fake-Backend
$null=Invoke-LabTransaction $b $want Provision
Require ($fake.journal.phase -ceq 'installed') 'installed'
$initial=$fake.initial;$calls=$fake.calls.Count
$null=Invoke-LabTransaction $b $want Provision
Require ($fake.calls.Count -eq $calls -and $fake.initial -ceq $initial) 'idempotent_exact'
$null=Invoke-LabTransaction $b $want Verify
$cases.idempotent_exact_and_original_baseline=$true
$fake.unrelated='changed-vpn'
Reject {Invoke-LabTransaction $b $want Teardown} 'drift_preflight'
Require ($fake.calls.Count -eq $calls) 'no_mutation_on_drift'
$fake.unrelated='vpn-public-ssh-original'
$null=Invoke-LabTransaction $b $want Teardown
Require (!$fake.private -and !$fake.task -and !$fake.rules.Count -and $fake.initial -ceq $initial) 'teardown_original'
$null=Invoke-LabTransaction $b $want Teardown
$cases.teardown_drift_then_owned_cleanup=$true
foreach($name in @('files','task','rule0','rule1','private','verify')){
    $fake=New-Fake;$fake.failure=$name;$b=Fake-Backend
    Reject {Invoke-LabTransaction $b $want Provision} "injected_$name"
    Require (!$fake.private -and !$fake.task -and !$fake.rules.Count -and !$fake.files -and $fake.journal.phase -ceq 'rolled_back') "rollback_$name"
    $baseline=$fake.initial;$fake.failure=''
    $null=Invoke-LabTransaction $b $want Provision
    Require ($fake.initial -ceq $baseline -and $fake.journal.phase -ceq 'installed') "retry_$name"
    $cases["partial_$name`_rollback_and_retry"]=$true
}
foreach($collision in @('task','rule')){
    $fake=New-Fake;if($collision -ceq 'task'){$fake.task='foreign'}else{$fake.rules=@('foreign')};$b=Fake-Backend
    Reject {Invoke-LabTransaction $b $want Provision} 'LAB_NAME_COLLISION'
    Require (!$fake.journal -and !$fake.calls.Count) 'collision_no_change'
}
$cases.name_collisions_no_mutation=$true
# Task XML and DACL checks call the production verifier, but the COM boundary is mocked.
$cfg=Get-LabConfig A;$spec=Lab-TaskSpec $cfg
$task=[pscustomobject]@{Xml=$spec.xml;Sddl=$spec.sddl}
$task | Add-Member -MemberType ScriptMethod -Name GetSecurityDescriptor -Value {param($flags) $this.Sddl}
Assert-LabTask $task $spec
$task.Xml=$spec.xml.Replace('LeastPrivilege','HighestAvailable');Reject {Assert-LabTask $task $spec} 'LAB_TASK_DRIFT'
$task.Xml=$spec.xml.Replace($spec.sid,'S-1-5-18');Reject {Assert-LabTask $task $spec} 'LAB_TASK_DRIFT'
$task.Xml=$spec.xml.Replace('</Triggers>','<BootTrigger><Enabled>true</Enabled></BootTrigger></Triggers>');Reject {Assert-LabTask $task $spec} 'LAB_TASK_SCHEMA'
$task.Xml=$spec.xml;$task.Sddl='D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GR;;;'+$spec.sid+')'
Reject {Assert-LabTask $task $spec} 'LAB_TASK_DACL'
$task.Sddl=$spec.sddl.Replace('GRGX','FA');Reject {Assert-LabTask $task $spec} 'LAB_TASK_DACL'
$cases.task_principal_action_acl_boundary=$true
$fixture=Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Evidence))) 'readonly-fixture'
[void][IO.Directory]::CreateDirectory((Join-Path $fixture 'ordinary-agent'))
$cfg=Get-LabConfig A;$cfg.base=$fixture;$spec=Lab-TaskSpec $cfg
$script:taskFixture=[pscustomobject]@{Xml=$spec.xml;Sddl=$spec.sddl}
$script:taskFixture | Add-Member -MemberType ScriptMethod -Name GetSecurityDescriptor -Value {param($flags) $this.Sddl}
$script:relevantFixture=[pscustomobject]@{profile='PrivateOn';peer='exact';rules=@('scoped-discovery','scoped-iroh')}
function Find-LabTask([string]$Name){$script:taskFixture}
function Get-LabOrdinaryState($Cfg,$Want){$script:relevantFixture}
$baseline=Join-Path $fixture 'ordinary-agent/campaign-baseline.json'
Lab-Write $baseline @{v=2;task=$spec;initial_audit_sha256=('a'*64);relevant=@{authoritative='installation-only'};ordinary=$script:relevantFixture}
$pin=Lab-Hash $baseline
$null=Assert-LabReadOnly $cfg $pin
Reject {Assert-LabReadOnly $cfg ('b'*64)} 'LAB_BASELINE_PIN'
$script:relevantFixture.peer='drift'
Reject {Assert-LabReadOnly $cfg $pin} 'LAB_BASELINE_DRIFT'
$cases.ordinary_baseline_pin_and_relevant_drift_boundary=$true
$initial=[pscustomobject]@{network=@('WiFiPrivate');adapter=@{guid='fixed'};dynamic_udp=@('49152','16384');
    profiles=@(@{store='PersistentStore';name='Private';enabled='False';outbound='Allow'},@{store='PersistentStore';name='Public';enabled='True';outbound='Allow'});
    policy=@(@{Name='foreign-ssh';port=22})}
$guardWant=@{rules=@(@{Name='owned0'},@{Name='owned1'})}
$current=$initial | ConvertTo-Json -Depth 12 | ConvertFrom-Json
$current.profiles[0].enabled='True'
Assert-LabRestoreScope $current $initial $guardWant
$current.network+=@('new-private-vpn');Reject {Assert-LabRestoreScope $current $initial $guardWant} 'LAB_RESTORE_NETWORK_DRIFT'
$current=$initial | ConvertTo-Json -Depth 12 | ConvertFrom-Json
$current.profiles[1].enabled='False';Reject {Assert-LabRestoreScope $current $initial $guardWant} 'LAB_PROFILE_DRIFT'
$current=$initial | ConvertTo-Json -Depth 12 | ConvertFrom-Json
$current.policy+=@{Name='new-unrelated-rule';port=99};Reject {Assert-LabRestoreScope $current $initial $guardWant} 'LAB_UNRELATED_POLICY_DRIFT'
$current=$initial | ConvertTo-Json -Depth 12 | ConvertFrom-Json
$current.adapter.guid='different';Reject {Assert-LabRestoreScope $current $initial $guardWant} 'LAB_RESTORE_NETWORK_DRIFT'
$cases.rollback_and_teardown_profile_wide_drift_guard=$true
$journalDir=Join-Path ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Evidence))) 'journal-fixture'
if(Test-Path -LiteralPath $journalDir){throw 'CHECK:journal_fixture_exists'}
[void][IO.Directory]::CreateDirectory($journalDir)
$journal=[pscustomobject]@{phase='installing';initial=@{private='False';original='unchanged'};changes=@()}
Save-LabInstallJournal $journalDir $journal @{script='first-package'}
$initialPin=Lab-Hash (Join-Path $journalDir 'initial.json')
$journal.phase='rolled_back';$journal.changes=@('files');Save-LabInstallJournal $journalDir $journal $null
$journal.phase='installing';$journal.changes=@();Save-LabInstallJournal $journalDir $journal @{script='corrected-package'}
Require ((Lab-Hash (Join-Path $journalDir 'initial.json')) -ceq $initialPin) 'real_initial_pin_immutable'
Require ((Lab-Read (Join-Path $journalDir 'initial-and-journal.json')).file_pins.script -ceq 'corrected-package') 'current_package_rollback_pin'
Require (@(Get-ChildItem -LiteralPath (Join-Path $journalDir 'journal-history') -File).Count -eq 2) 'failed_journal_preserved'
$journal.initial.private='True';Reject {Save-LabInstallJournal $journalDir $journal $null} 'LAB_INITIAL_BASELINE_CHANGED'
$cases.real_file_journal_history_and_corrected_package_retry=$true
Lab-Write $Evidence @{scope='production transaction and XML/DACL verifier with mocked OS mutation boundary';cases=$cases;count=$cases.Count;host_mutations=0}
@{passed=$cases.Count;host_mutations=0} | ConvertTo-Json -Compress
