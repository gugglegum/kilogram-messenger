param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
[void][IO.Directory]::CreateDirectory($Evidence)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-native.cs"
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Reject([scriptblock]$Body){try{&$Body}catch{return};throw 'CHECK:expected_native_rejection'}
$p=[Diagnostics.Process]::GetCurrentProcess();$id=[Security.Principal.WindowsIdentity]::GetCurrent()
try{
    $sid=$id.User.Value;$path=$p.MainModule.FileName;$creation=$p.StartTime.ToFileTimeUtc()
    $actor=[KilogramLabNative]::OpenActor($p.Id,$creation,$path,$sid,$false)
    try{
        $user="O:LSD:(A;;CC;;;$sid)";$other='O:LSD:(A;;CC;;;S-1-5-21-1-2-3-9999)'
        Require ($actor.LocalUser($user) -eq 1 -and $actor.LocalUser($other) -eq -1) 'native_user_match_nonmatch'
        $group=@($actor.groups|Where-Object {($_.attributes -band 4) -and !($_.attributes -band 16) -and $_.sid -cne $sid})[0].sid
        Require ([bool]$group) 'enabled_group_available'
        $groupSddl="O:LSD:(A;;CC;;;$group)"
        Require ($actor.LocalUser($groupSddl) -eq 1) 'enabled_group_matches'
        Require ($actor.RestrictedLocalUser($groupSddl,'',$group) -eq -1) 'deny_only_group_nonmatch'
        Require ($actor.RestrictedLocalUser($user,'S-1-5-21-1-2-3-9999','') -eq -1) 'restricting_sid_intersection'
        Require ($actor.LocalUser("O:LSD:(A;;GA;;;$sid)") -eq 0) 'unsupported_descriptor_unknown'
        Reject {[KilogramLabNative]::OpenActor($p.Id,($creation+1),$path,$sid,$false)}
        Reject {[KilogramLabNative]::OpenActor($p.Id,$creation,'C:\wrong.exe',$sid,$false)}
        Reject {[KilogramLabNative]::OpenActor($p.Id,$creation,$path,'S-1-5-18',$false)}
        Reject {[KilogramLabNative]::OpenServerActor($p.Id,$path,$sid,$true)}
        Reject {[KilogramLabNative]::BindServerActors(@($actor),$p.Id,$path,$sid)}
        $actor.Dispose();Require (!$actor.Alive()) 'disposed_incarnation_rejected'
    }finally{$actor.Dispose()}
    Lab-Write (Join-Path $Evidence 'checks.json') @{count=12;native=$true;scope='actual current ordinary token; enabled/deny-only/restricted AccessCheck, role/image/incarnation rejection; no server/System access claim'}
    @{passed=12;host_mutations=0}|ConvertTo-Json -Compress
}finally{$id.Dispose();$p.Dispose()}
