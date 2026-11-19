param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
$cfg=Get-LabConfig A;$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
try{
    Require ([Environment]::MachineName -ceq $cfg.host -and $identity.User.Value -ceq $cfg.sid -and
        !([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) 'ordinary_A_only'
}finally{$identity.Dispose()}
Initialize-LabNative
$fixturePath=Join-Path $PSScriptRoot 'fixtures/terminal-t1-3e-A-foreign-sandbox-block.json';$fixture=Lab-Read $fixturePath
Require ($fixture.host -ceq $cfg.host -and $fixture.ordinary_sid -ceq $cfg.sid) 'captured_host_scope'
$cases=[ordered]@{}
$captured=$fixture.rule.LocalUser
Require ((Test-LabFixedLocalUser $captured $cfg) -eq -1) 'different_verified_local_user'
Require (!(Test-LabCurrentPolicyRelevant $cfg $fixture.rule)) 'actual_foreign_sandbox_rule_irrelevant'
Assert-LabUdpPath $cfg @($fixture.rule);Assert-LabSshPath $cfg @{policy=@($fixture.rule)} -PolicyOnly
$cases.captured_foreign_user_excluded=$true
$own='O:LSD:(A;;CC;;;'+$cfg.sid+')'
Require ((Test-LabFixedLocalUser $own $cfg) -eq 1) 'actual_intended_user'
$rule=$fixture.rule|ConvertTo-Json -Depth 12|ConvertFrom-Json;$rule.LocalUser=$own
Reject {Assert-LabUdpPath $cfg @($rule)} 'LAB_UDP_BLOCKED'
Reject {Assert-LabSshPath $cfg @{policy=@($rule)} -PolicyOnly} 'LAB_SSH_BLOCKED'
$cases.same_user_blocks_both_paths=$true
# Reproduce the review finding with a *resolved actual ordinary local user*.
# These are synthetic B rule/config values; no laptop/service/socket is accessed.
$serverCfg=Get-LabConfig B;$serverCfg.sid=$cfg.sid
$serverRule=@(Lab-Rules $serverCfg)[1]|ConvertTo-Json -Depth 12|ConvertFrom-Json
$serverRule.Name='synthetic-conditional-ssh';$serverRule.Program=Join-Path ([Environment]::GetFolderPath('System')) 'OpenSSH/sshd.exe'
$serverRule.Protocol='TCP';$serverRule.LocalPort='22';$serverRule.RemotePort='Any';$serverRule.Action='Allow'
$serverRule.LocalUser='O:SYD:(A;;CC;;;'+$cfg.sid+')'
Require ((Test-LabFixedLocalUser $serverRule.LocalUser $serverCfg) -eq 1) 'ordinary_subject_actually_matches_but_is_not_server_proof'
Require ((Test-LabSshRule $serverRule $serverCfg $false $true) -eq 0) 'exact_review_allow_without_server_actor_unknown'
Require (Test-LabCurrentPolicyRelevant $serverCfg $serverRule) 'conditional_allow_not_filtered'
Reject {Assert-LabSshPath $serverCfg @{policy=@($serverRule)} -PolicyOnly} 'LAB_SSH_FILTER_UNCERTAIN'
$cases.resolved_ordinary_only_server_allow_unknown=$true
$unconditional=$serverRule|ConvertTo-Json -Depth 12|ConvertFrom-Json;$unconditional.LocalUser='Any'
Assert-LabSshPath $serverCfg @{policy=@($unconditional)} -PolicyOnly
$serverRule.Action='Block';$serverRule.LocalUser=$captured
Require ((Test-LabFixedLocalUser $serverRule.LocalUser $serverCfg) -eq -1) 'different_user_exclusion_would_use_wrong_server_subject'
Require ((Test-LabSshRule $serverRule $serverCfg $false $false) -eq 0) 'server_block_without_identity_unknown'
Require (Test-LabCurrentPolicyRelevant $serverCfg $serverRule) 'other_user_server_block_not_discarded'
Reject {Assert-LabSshPath $serverCfg @{policy=@($unconditional,$serverRule)} -PolicyOnly} 'LAB_SSH_FILTER_UNCERTAIN'
$cases.resolved_other_user_server_block_retained=$true
$cases.unconditional_server_policy_only_still_supported=$true
$unknowns=[ordered]@{
    group='O:LSD:(A;;CC;;;BA)';world='O:LSD:(A;;CC;;;WD)';mask=('O:LSD:(A;;GR;;;'+$cfg.sid+')');
    deny=('O:LSD:(D;;CC;;;'+$cfg.sid+')');multi=('O:LSD:(A;;CC;;;'+$cfg.sid+')(A;;CC;;;WD)');
    inherited=('O:LSD:(A;ID;CC;;;'+$cfg.sid+')');ownerless=('D:(A;;CC;;;'+$cfg.sid+')');
    foreign_domain='O:LSD:(A;;CC;;;S-1-5-21-1-2-3-1001)';
    unresolved=('O:LSD:(A;;CC;;;'+($cfg.sid -replace '-1001$','-2147483000')+')');malformed='not-sddl'
}
foreach($entry in $unknowns.GetEnumerator()){
    Require ((Test-LabFixedLocalUser $entry.Value $cfg) -eq 0) ('unknown_'+$entry.Key)
    $rule.LocalUser=$entry.Value
    Reject {Assert-LabUdpPath $cfg @($rule)} 'LAB_UDP_FILTER_UNCERTAIN'
    $cases['unknown_'+$entry.Key]=$true
}
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;host_mutations=0;fixture_sha256=(Lab-Hash $fixturePath);
    scope='read-only native LookupAccountSid for exact local-user ACEs and captured A foreign block; no token substitution, policy/account/task change or B access'}
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
