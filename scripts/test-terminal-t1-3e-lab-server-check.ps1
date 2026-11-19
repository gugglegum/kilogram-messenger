param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
# Native handle/token acquisition is the explicit synthetic boundary in this fixture.
# Production service/socket admission and firewall predicates run unchanged.
Add-Type @'
using System;
public static class KilogramLabNative {
 public static bool Denied; public static int Disposals;
 public sealed class Actor : IDisposable {
  public int pid; public long started=1; public string image,user;
  public bool elevated,live=true; public object[] groups=new object[0],restricting=new object[0];
  public int verdict=-1;
  public bool Alive(){return live;} public void Dispose(){Disposals++;live=false;}
 }
 public sealed class ServerActors : IDisposable {
  public Actor[] actors;public int service_pid;public string image,ordinary_sid;
  public bool Alive(){foreach(var a in actors)if(!a.Alive())return false;return true;}
  public bool Contains(int id){foreach(var a in actors)if(a.pid==id)return true;return false;}
  public Actor[] Members(){return actors;}
  public int LocalUser(string s){bool unknown=false;foreach(var a in actors){if(a.verdict==1)return 1;if(a.verdict==0)unknown=true;}return unknown?0:-1;}
  public void Dispose(){foreach(var a in actors)a.Dispose();}
 }
 public static Actor OpenServerActor(int id,string image,string sid,bool service){
  if(Denied)throw new InvalidOperationException("fixture native access denied");
  return new Actor{pid=id,image=image,user=service?"S-1-5-18":sid};
 }
 public static ServerActors BindServerActors(Actor[] a,int pid,string image,string sid){return new ServerActors{actors=a,service_pid=pid,image=image,ordinary_sid=sid};}
}
'@
function Initialize-LabNative {}
$cfg=Get-LabConfig B;$image=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
$script:service=[pscustomobject]@{State='Running';ProcessId=100;StartName='LocalSystem';PathName=$image}
$script:tcp=@([pscustomobject]@{LocalPort=22;State='Listen';LocalAddress='0.0.0.0';OwningProcess=100},
    [pscustomobject]@{LocalPort=22;State='Established';LocalAddress=$cfg.ip;RemoteAddress=$cfg.peer;RemotePort=55001;OwningProcess=101})
function Get-CimInstance {param($ClassName,$Filter,$ErrorAction) Require ($ClassName -ceq 'Win32_Service' -and $Filter -ceq "Name='sshd'") 'exact_service_query';$script:service}
function Get-NetTCPConnection {param($ErrorAction) $script:tcp}
$cases=[ordered]@{}
$actor=Open-LabServerActors $cfg
try{
    Require ($actor.actors.Count -eq 2 -and $actor.actors[0].user -ceq 'S-1-5-18' -and $actor.actors[1].user -ceq $cfg.sid) 'actual_observed_owners_requested_with_explicit_roles'
    $allow=@{Name='fixture-allow';Enabled='True';Profile='Private';Direction='Inbound';Action='Allow';Program=$image;Service='sshd';Protocol='TCP';
        LocalAddress=$cfg.ip;RemoteAddress=$cfg.peer;LocalPort='22';RemotePort='Any';InterfaceAlias=$cfg.interface_alias;InterfaceType='Any';Package='Any';
        DynamicTransport='Any';LocalUser='Any';RemoteUser='Any';RemoteMachine='Any';Authentication='NotRequired';Encryption='NotRequired';OverrideBlockRules='False'}
    $block=$allow|ConvertTo-Json|ConvertFrom-Json;$block.Name='foreign-test';$block.Action='Block';$block.Direction='Outbound';$block.Program='Any';$block.Service='Any';$block.LocalUser='O:LSD:(A;;CC;;;S-1-5-21-1-2-3-4)'
    $snapshot=@{policy=@($allow,$block);ssh=@{running=$true;service_path=$image;connection="$($cfg.peer) 55001 $($cfg.ip) 22"}}
    $before=Lab-JsonHash $snapshot
    Assert-LabSshPath $cfg $snapshot $actor;$cases.all_supported_owners_nonmatch=$true
    $actor.actors[1].verdict=1;Reject {Assert-LabSshPath $cfg $snapshot $actor} 'LAB_SSH_BLOCKED';$cases.flow_owner_match_blocks=$true
    $actor.actors[1].verdict=0;Reject {Assert-LabSshPath $cfg $snapshot $actor} 'LAB_SSH_FILTER_UNCERTAIN';$cases.one_unknown_aborts=$true
    $actor.actors[0].verdict=1;Reject {Assert-LabSshPath $cfg $snapshot $actor} 'LAB_SSH_BLOCKED';$cases.match_dominates_unknown=$true
    $actor.actors[0].verdict=-1;$actor.actors[1].verdict=-1
    $proof=Get-LabSshEvidence $cfg $snapshot $actor
    Require ($proof.binding.service_pid -eq 100 -and $proof.actors.Count -eq 2 -and $proof.rules.Count -eq 1 -and $proof.rules[0].verdict -eq -1) 'relevant_authoritative_rule_and_token_metadata_captured'
    Lab-Write (Join-Path $Evidence 'ssh-proof.json') $proof
    Require ((Lab-JsonHash $snapshot) -ceq $before) 'foreign_rule_unchanged';$cases.relevant_proof_and_foreign_preservation=$true
    $actor.actors[1].live=$false;Reject {Assert-LabServerActors $cfg $actor} 'LAB_SERVER_ACTOR_BINDING';$actor.actors[1].live=$true;$cases.dead_incarnation_refused=$true
    $actor.image='C:\wrong\sshd.exe';Reject {Assert-LabServerActors $cfg $actor} 'LAB_SERVER_ACTOR_BINDING';$actor.image=$image;$cases.bound_image_refused=$true
    $actor.ordinary_sid='other';Reject {Assert-LabServerActors $cfg $actor} 'LAB_SERVER_ACTOR_BINDING';$actor.ordinary_sid=$cfg.sid;$cases.bound_account_refused=$true
    $script:tcp[1].OwningProcess=102;Reject {Assert-LabServerActors $cfg $actor} 'LAB_SERVER_OWNER_CHANGED';$script:tcp[1].OwningProcess=101;$cases.new_flow_owner_aborts=$true
    $script:service.ProcessId=102;$script:tcp[0].OwningProcess=102;Reject {Assert-LabServerActors $cfg $actor} 'LAB_SERVER_OWNER_CHANGED';$script:service.ProcessId=100;$script:tcp[0].OwningProcess=100;$cases.restarted_service_refused=$true
}finally{$actor.Dispose()}
foreach($field in @('StartName','PathName','State')){
    $prior=$script:service.$field;$script:service.$field='wrong'
    Reject {Get-LabServerFacts $cfg} 'LAB_SERVER_SERVICE_BINDING';$script:service.$field=$prior;$cases["service_$field"]=$true
}
$script:tcp[0].OwningProcess=999;Reject {Get-LabServerFacts $cfg} 'LAB_SERVER_LISTENER_BINDING';$script:tcp[0].OwningProcess=100;$cases.unsupported_listener_refused=$true
[KilogramLabNative]::Denied=$true
Reject {Open-LabServerActors $cfg} 'LAB_SERVER_TOKEN_UNAVAILABLE:pid=100;service=100';$cases.denied_token_actionable=$true
[KilogramLabNative]::Denied=$false
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;scope='production SCM/TCP/guard routing, synthetic native acquisition; live System token access NOT proved'}
@{passed=$cases.Count;host_mutations=0;live_server_token=$false}|ConvertTo-Json -Compress
