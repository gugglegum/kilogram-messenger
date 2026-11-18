param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return $_.Exception.Message};throw};throw "CHECK:expected_$Code"}
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
# Only the native token/socket boundary is synthetic here; the production admission
# loop, schema/path checks, one shared deadline and actor disposal are unchanged.
Add-Type @'
public static class KilogramLabNative {
 public static bool Live=true, BadIdentity=false;
 public static int Opens, Disposals;
 public sealed class Actor : System.IDisposable {
  public int pid=123;
  public bool Alive(){return Live;}
  public void Dispose(){Disposals++;}
 }
 public static Actor OpenActor(int pid,long started,string image,string sid,bool ordinary){
  Opens++;if(BadIdentity)throw new System.InvalidOperationException("fixture identity mismatch");return new Actor();
 }
}
'@
function Initialize-LabNative {}
$cfg=Get-LabConfig A;$cfg.base=Join-Path $Evidence 'actor-host'
$actorSession=Join-Path $cfg.base ('deploy/setup-'+('1'*32));[void][IO.Directory]::CreateDirectory($actorSession)
$actorPath=Join-Path $actorSession 'actor.json'
Lab-Write $actorPath @{pid=[int]123;started=[long]1;session=(Split-Path -Leaf $actorSession)}
$script:realRead=${function:Lab-Read}
function Lab-Read([string]$Path,[int]$MaxBytes=131072){
    if($Path -ceq $actorPath){
        $script:identityReads++
        if(($script:identityReads -lt 3 -and $script:scenario -ceq 'delayed') -or ($script:identityReads -lt 7 -and $script:scenario -ceq 'budget')){return $null}
    }
    & $script:realRead $Path -MaxBytes $MaxBytes
}
function Get-NetTCPConnection {
    param($ErrorAction)
    $script:socketReads++
    if($script:scenario -ceq 'exit'){[KilogramLabNative]::Live=$false;return}
    if($script:scenario -ceq 'delayed' -and $script:socketReads -ge 3){
        [pscustomobject]@{OwningProcess=123;State='Established';LocalAddress=$cfg.ip;RemoteAddress=$cfg.peer;RemotePort=22}
    }else{
        # A different process's established socket cannot admit our still-connecting actor.
        [pscustomobject]@{OwningProcess=124;State='Established';LocalAddress=$cfg.ip;RemoteAddress=$cfg.peer;RemotePort=22}
    }
}
$cases=[ordered]@{}
foreach($scenario in @('delayed','exit','never','budget','identity')){
    $script:scenario=$scenario;$script:identityReads=0;$script:socketReads=0
    [KilogramLabNative]::Live=$true;[KilogramLabNative]::BadIdentity=$scenario -ceq 'identity'
    [KilogramLabNative]::Opens=0;[KilogramLabNative]::Disposals=0
    $timer=[Diagnostics.Stopwatch]::StartNew();$actor=$null;$errorCode=$null
    if($scenario -ceq 'delayed'){
        try{$actor=Open-LabSetupActor $cfg $actorSession -AdmissionSeconds 2;Require ($socketReads -eq 3 -and $identityReads -eq 3) 'delayed_identity_and_socket_admitted'}finally{if($actor){$actor.Dispose()}}
    }else{
        $expected=switch($scenario){'exit'{'LAB_SSH_ACTOR_EXITED'};'never'{'LAB_SSH_ACTOR_TIMEOUT:phase=socket'};'budget'{'LAB_SSH_ACTOR_TIMEOUT:phase=socket'};'identity'{'fixture identity mismatch'}}
        $errorCode=Reject {Open-LabSetupActor $cfg $actorSession -AdmissionSeconds 1} $expected
    }
    Require ($timer.Elapsed.TotalSeconds -lt 2.6) 'shared_budget_not_restarted'
    Require ([KilogramLabNative]::Opens -eq 1) 'same_process_incarnation_handle'
    Require ([KilogramLabNative]::Disposals -eq $(if($scenario -ceq 'identity'){0}else{1})) 'owned_actor_disposed'
    if($scenario -ceq 'never'){Require ($timer.Elapsed.TotalSeconds -ge 1 -and $socketReads -gt 1) 'never_ready_bounded_poll'}
    if($scenario -ceq 'budget'){Require ($timer.Elapsed.TotalSeconds -ge 1 -and $timer.Elapsed.TotalSeconds -lt 1.5 -and $identityReads -eq 7 -and $socketReads -lt 7) 'file_wait_consumes_same_socket_deadline'}
    $cases["actor_$scenario"]=@{seconds=$timer.Elapsed.TotalSeconds;identity_reads=$identityReads;socket_reads=$socketReads;error=$errorCode}
}
# Use the exact production backend Snapshot/Preflight scriptblocks, not a copy of the
# old/new routing decision. Only CIM/COM observations and OS mutations are fixtures.
$errors=$null;$tokens=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
Require (!$errors.Count) 'installer_parse'
$assignment=@($tree.FindAll({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true))
Require ($assignment.Count -eq 1) 'one_production_backend'
$table=$assignment[0].Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
$production=@{}
foreach($pair in $table.KeyValuePairs){if($pair.Item1.Value -cin @('Snapshot','Preflight')){
    $expression=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true)
    $production[$pair.Item1.Value]=$expression.ScriptBlock.GetScriptBlock()
}}
Require ($production.Count -eq 2) 'exact_production_admission_blocks'
$cfg=Get-LabConfig B;$cfg.base=Join-Path $Evidence 'transaction-host'
$want=@{task=(Lab-TaskSpec $cfg);rules=@(Lab-Rules $cfg);config=$cfg}
$server=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
$allow=@{Name='fixture-ssh';Enabled='True';Profile='Private';Direction='Inbound';Action='Allow';Program=$server;Service='Any';Protocol='TCP';
    LocalAddress=$cfg.ip;RemoteAddress=$cfg.peer;LocalPort='22';RemotePort='Any';InterfaceAlias=$cfg.interface_alias;InterfaceType='Any';Package='Any';
    DynamicTransport='Any';LocalUser='Any';RemoteUser='Any';RemoteMachine='Any';Authentication='NotRequired';Encryption='NotRequired';OverrideBlockRules='False'}
$script:ordinaryFixture=(Lab-Read (Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/combined-current-01/B-current.json')).state
$profiles=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($p in $ordinaryFixture.profiles){@{store=$store;name=$p.name;enabled=$p.enabled;
    inbound=$p.inbound;outbound=$p.outbound;allow_inbound='True';allow_local='True';disabled=@($p.disabled)}}})
$prepared=@{profiles=$profiles;network=@(@{index=$cfg.interface_index;alias=$cfg.interface_alias;category='Private'});
    adapter=@{name=$cfg.interface_alias;guid=$cfg.interface_guid;status='Up';addresses=@("$($cfg.ip)/24")};dynamic_udp=@('49152','16384');policy=@($allow);
    ssh=@{service_path=$server;running=$true;connection="$($cfg.peer) 55001 $($cfg.ip) 22"};task=$null;rules=@()}
Assert-LabNetwork $cfg $prepared
function Get-LabSnapshot($Cfg,$Want){
    $script:auditReads++
    Require ($installLock -and $installLock.CanWrite) 'audit_requires_live_lock'
    $blocked=$false
    try{$other=[IO.File]::Open($installLock.Name,'Open','Read','ReadWrite');$other.Dispose()}catch [IO.IOException]{$blocked=$true}
    Require $blocked 'exclusive_lock_held_at_authoritative_read'
    Clone $script:observed
}
function Get-LabOrdinaryState($Cfg,$Want,[switch]$AllowMissing){Clone $script:ordinaryFixture}
$setupMode=$true;$setupActor=$null;$files=@('fixed-fixture.txt')
foreach($change in @('policy','task','admissible-profile')){
    $PackageRoot=Join-Path $Evidence $change;[void][IO.Directory]::CreateDirectory($PackageRoot)
    $script:observed=Clone $prepared;$script:ordinaryFixture=(Lab-Read (Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/combined-current-01/B-current.json')).state
    # A pre-lock observation is intentionally left in the old cache variable. Production
    # admission must ignore it and observe the changed state under its exclusive lock.
    $freshAudit=Clone $prepared
    switch($change){
        'policy'{@($script:observed.profiles|Where-Object {$_.store -ceq 'ActiveStore' -and $_.name -ceq 'Private'})[0].inbound='Allow'}
        'task'{$script:observed.task=@{xml='competing installed task'}}
        'admissible-profile'{foreach($p in $script:observed.profiles){if($p.name -ceq 'Public'){$p.enabled='False'}};$script:ordinaryFixture.profiles[2].enabled='False'}
    }
    $script:auditReads=0;$script:writes=0;$script:mutations=@();$script:journal=$null
    $installLock=[IO.File]::Open((Join-Path $PackageRoot 'install.lock'),'CreateNew','Write','None')
    try {
        $backend=@{Read={$null};Write={param($j) $script:writes++;$script:journal=Clone $j};Snapshot=$production.Snapshot;Preflight=$production.Preflight;
            Apply={param($step,$w) $script:mutations+=,$step};Verify={param($w)};Undo={param($step,$w,$initial) throw 'CHECK:unexpected_undo'}}
        if($change -ceq 'admissible-profile'){
            $null=Invoke-LabTransaction $backend $want Provision
            Require ($mutations.Count -eq 5 -and $journal.phase -ceq 'installed') 'admitted_current_state_used'
            Require (@($journal.initial.profiles|Where-Object {$_.name -ceq 'Public' -and $_.enabled -ceq 'False'}).Count -eq 2) 'fresh_rollback_baseline'
        }else{
            $expected=if($change -ceq 'policy'){'LAB_PRIVATE_POLICY'}else{'LAB_NAME_COLLISION'}
            $null=Reject {Invoke-LabTransaction $backend $want Provision} $expected
            Require (!$writes -and !$mutations.Count) 'changed_state_rejected_before_intent_or_mutation'
        }
        Require ($auditReads -eq 1) 'one_fresh_authoritative_read_inside_lock'
        $cases["interleave_$change"]=@{audit_reads=$auditReads;intent_writes=$writes;mock_mutations=$mutations.Count}
    }finally{$installLock.Dispose()}
}
Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;count=$cases.Count;host_mutations=0;scope='production actor admission and transaction routing/predicates; native token/socket/CIM/COM/OS boundary fixtures; real exclusive file lock'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
