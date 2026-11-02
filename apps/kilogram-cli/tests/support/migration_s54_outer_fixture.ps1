[CmdletBinding()]
param(
    [ValidateSet('Owner','Launcher','Native','Linger')][string]$Role='Owner',
    [ValidateSet('normal','tail','legacy','nonzero','delayed','timeout','eof','linger','cleanup','partial','overflow')][string]$Mode='normal',
    [ValidateSet('WholeP','FeasibilityF')][string]$Purpose='WholeP',
    [Parameter(Mandatory)][string]$Root
)
# Deliberately benign: pinned PowerShell, finite sleeps/public JSON/stdout only.
# This file is never selected by production Run or the frozen Rust selector.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$self=[IO.Path]::GetFullPath($PSCommandPath)
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
. (Join-Path $workspace 'scripts/invoke-migration-s54-whole.ps1') -Purpose $Purpose
$rootPath=[IO.Path]::GetFullPath($Root)
$owned=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/s54-block4b-r1-01/'))
if(-not $rootPath.StartsWith($owned,[StringComparison]::OrdinalIgnoreCase)){throw 'Only owned public benign fixture root.'}
$shell=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
if($Role -ceq 'Linger'){Start-Sleep -Seconds 30;exit 0}
if($Role -ceq 'Launcher'){
    $specPath=Join-Path $rootPath 'fixture-session.json';$specUntil=[Environment]::TickCount64+5000
    while(-not (Test-Path -LiteralPath $specPath)){if([Environment]::TickCount64 -ge $specUntil){throw 'bounded benign launcher handoff'};Start-Sleep -Milliseconds 5}
    $spec=ConvertFrom-S54Strict (Read-S54Public $specPath 16384) 16384
    if($spec.classification -cne 'public_benign_fixture_only'){throw 'No production fixture input.'}
    $native=Invoke-S54SingleExecutable (Get-S54Pin $shell 134217728) @('-NoProfile','-NonInteractive','-File',$self,'-Role','Native','-Mode',$Mode,'-Root',$rootPath,'-Purpose',$Purpose) ([long]$spec.controller_tick_ms) -ForwardOutput
    if($native.code -ne 0){exit $native.code}
    if($Mode -ceq 'tail'){Start-Sleep -Milliseconds 1800}
    if($Purpose -ceq 'FeasibilityF'){Write-Output ($script:S54PolicyPurpose.Wrapper+' '+(ConvertTo-Json -InputObject $spec.wrapper_marker -Compress))}else{Write-Output 'S54_BENIGN_WRAPPER public_fixture_only'}
    exit 0
}
if($Role -ceq 'Native'){
    $specPath=Join-Path $rootPath 'fixture-session.json';$specUntil=[Environment]::TickCount64+5000
    while(-not (Test-Path -LiteralPath $specPath)){if([Environment]::TickCount64 -ge $specUntil){throw 'bounded benign session handoff'};Start-Sleep -Milliseconds 5}
    $spec=ConvertFrom-S54Strict (Read-S54Public $specPath 16384) 16384
    if($spec.classification -cne 'public_benign_fixture_only'){throw 'No production fixture input.'}
    $claim=@{version=1;classification='s54_native_started';case=$spec.case;attempt=$spec.attempt;run_id=$spec.run_id;owner_pid=$spec.owner_pid;launcher_pid=$spec.launcher_pid;harness_pid=$PID;nonce=$spec.nonce;inventory_sha256=$spec.inventory_sha256;launch_sha256=$spec.launch_sha256}
    $null=Write-S54New (Join-Path $rootPath 'native-started.json') $claim 16384
    $claim.classification='s54_clock_request';$before=[Environment]::TickCount64
    $request=Write-S54New (Join-Path $rootPath 'clock-request.json') $claim 16384
    $replyPath=Join-Path $rootPath 'clock-reply.json';$end=$before+5000
    while(-not (Test-Path -LiteralPath $replyPath)){if([Environment]::TickCount64 -ge $end){throw 'benign clock reply timeout'};Start-Sleep -Milliseconds 5}
    $reply=ConvertFrom-S54Strict (Read-S54Public $replyPath 16384) 16384;Assert-S54Wire $reply 'ClockReply'
    if($reply.request_sha256 -cne $request.sha256 -or $reply.nonce -cne $spec.nonce -or $before+$reply.remaining_controller_ms -gt $spec.controller_tick_ms){throw 'benign conservative clock request binding'}
    $join=@{attempt=$spec.attempt;owner_pid=$spec.owner_pid;harness_pid=$PID;nonce="benign-$PID";membership_verified=$false}
    # Deliberately hold a partial public join under the same exclusive Windows
    # sharing used by original S54. The owner may retry sharing errors only;
    # it must never parse the half-written JSON or grant admission early.
    $joinBytes=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $join -Compress))
    $joinFile=[IO.File]::Open((Join-Path $rootPath "join-$PID.json"),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    try{$half=[int]($joinBytes.Length/2);$joinFile.Write($joinBytes,0,$half);$joinFile.Flush($true);Start-Sleep -Milliseconds 200;$joinFile.Write($joinBytes,$half,$joinBytes.Length-$half);$joinFile.Flush($true)}finally{$joinFile.Dispose()}
    $permit=Join-Path $rootPath "permit-$PID.json"
    while(-not (Test-Path -LiteralPath $permit)){if([Environment]::TickCount64 -ge $end){throw 'benign actual job permit timeout'};Start-Sleep -Milliseconds 5}
    $permitValue=ConvertFrom-S54Strict (Read-S54Public $permit 4096) 4096
    if(-not $permitValue.membership_verified -or $permitValue.harness_pid -ne $PID -or $permitValue.nonce -cne $join.nonce){throw 'benign actual permit binding'}
    if($Mode -ceq 'nonzero'){Write-Output 'running 1 test';Write-Output 'test public_fixture::benign ... FAILED';[Console]::Error.WriteLine('public diagnostic secret-nonzero-must-be-redacted');exit 7}
    if($Mode -ceq 'partial'){Write-Output 'running 1 test';[Console]::Out.Write('test public_fixture::benign ... private partial diagnostic');Start-Sleep -Seconds 30;exit 0}
    if($Mode -ceq 'overflow'){[Console]::Out.Write(('x'*1100000));exit 0}
    if($Mode -cin @('timeout','eof','partial')){Write-Output 'public fixture keeps output open until bounded timeout';Start-Sleep -Seconds 10;exit 0}
    if($Mode -cin @('linger','cleanup')){
        $child=[Diagnostics.Process]::new();$child.StartInfo.FileName=$shell;$child.StartInfo.UseShellExecute=$false;$child.StartInfo.RedirectStandardOutput=$true;$child.StartInfo.RedirectStandardError=$true;$child.StartInfo.CreateNoWindow=$true;$child.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
        foreach($a in @('-NoProfile','-NonInteractive','-File',$self,'-Role','Linger','-Mode',$Mode,'-Root',$rootPath,'-Purpose',$Purpose)){$null=$child.StartInfo.ArgumentList.Add($a)}
        if(-not $child.Start()){throw 'benign descendant start'};$child.Dispose()
    }
    Write-Output 'running 1 test'
    Write-Output ('test '+$(if($Purpose -ceq 'FeasibilityF'){$script:S54Selector}else{'public_fixture::benign'})+' ... diagnostic on the named line')
    Write-Output 'public diagnostic between name and native ok'
    if($Purpose -ceq 'FeasibilityF'){Write-Output ($script:S54PolicyPurpose.Native+' '+(ConvertTo-Json -InputObject $spec.native_marker -Compress))}else{Write-Output 'S54_BENIGN_FUNCTIONAL public_fixture_only'}
    Write-Output 'ok'
    Write-Output 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s'
    exit 0
}
# One fresh owner process/Job per table row; no name-based process selection.
$null=New-Item -ItemType Directory -Path $rootPath -ErrorAction Stop
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
. (Join-Path $workspace 'scripts/migration-s5-l1-deadline.ps1')
if($Mode -ceq 'delayed'){
    $origin=[KilogramS5Deadline]::TickNow();Start-Sleep -Milliseconds 1150
    $refused=$false
    try{$unexpected=[KilogramS5Deadline]::new(1,3,[long]$origin)}catch{
        $observed=$_.Exception.GetBaseException()
        if($observed -isnot [TimeoutException] -or $observed.Message -cne 'invalid/expired original deadline'){throw}
        $refused=$true
    }
    if(-not $refused){$unexpected.Finish();throw 'original constructor incorrectly reset delayed origin'}
    $null=Write-S54New (Join-Path $rootPath 'fixture-result.json') @{classification='public_benign_fixture_only';mode=$Mode;original_delayed_constructor_refused=$refused;elapsed_ms=([KilogramS5Deadline]::TickNow()-$origin);whole_attempt_verdict='not_established'}
    exit 0
}
$origin=[KilogramS5Deadline]::TickNow()
$work=if($Mode -cin @('timeout','eof','partial')){8}else{15};$whole=$work+4
$job=if($Mode -ceq 'legacy'){[KilogramS5Deadline]::new($work,$whole)}else{[KilogramS5Deadline]::new($work,$whole,[long]$origin)}
Initialize-S54Output
$session=$null;$launcher=$null;$actualPassed=$false;$failure=$null;$failureDetail=$null;$unknown=@();$eofAtFailure=$null;$closed=$null;$teardown=$false
try {
    $runId='b'*64;$nonce=[Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
    $fixtureInput=Write-S54New (Join-Path $rootPath 'public-input.json') @{classification='public_benign_fixture_only';mode=$Mode} 4096
    $fixtureCase=if($Purpose -ceq 'FeasibilityF'){$script:S54Case}else{'public-outer-fixture'}
    $fixtureAttempt=if($Purpose -ceq 'FeasibilityF'){$script:S54Attempt}else{"public-$Mode"}
    $fixtureSelector=if($Purpose -ceq 'FeasibilityF'){$script:S54Selector}else{'public_fixture::benign'}
    $started=@{version=1;classification='s54_exclusive_attempt';case=$fixtureCase;attempt=$fixtureAttempt;run_id=$runId;selector=$fixtureSelector;inventory=$fixtureInput;activation=$fixtureInput;owner_pid=$PID;nonce=$nonce}
    $null=Write-S54New (Join-Path $rootPath 'started.json') $started 16384
    $launcher=New-S54Process $shell @('-NoProfile','-NonInteractive','-File',$self,'-Role','Launcher','-Mode',$Mode,'-Root',$rootPath,'-Purpose',$Purpose)
    if(-not $launcher.Start()){throw 'benign launcher start'};$job.VerifyChild($launcher)
    $i=@{identity=@{run=$rootPath;case=$fixtureCase;attempt=$fixtureAttempt;run_id=$runId;selector=$fixtureSelector}}
    $session=@{Job=$job;I=$i;Inputs=@{inventory=$fixtureInput;launch=$fixtureInput};Started=$started;Launcher=$launcher;Harness=$null;ExpectedHarness=(Get-S54Pin $shell 134217728);Output=([KilogramS54Output]::new($launcher));OriginTick=$origin;ControllerTick=($origin+$work*1000);ClockSent=$false;JoinSent=$false;JoinSharingRetries=0;HandleVerified=$false;LauncherWaited=$false;HarnessWaited=$false;LauncherWaitedTick=$null;HarnessWaitedTick=$null;Transcript=$null;ClockRequest=$null;ClockReply=$null;NativeClaim=$null;Join=$null;Permit=$null}
    $fixtureAssertion=@{Pin=$fixtureInput;Value=@{provenance=@{blake3=('3'*64)}}}
    $nativeMarker=New-S54Marker $i $session.Inputs $fixtureAssertion $script:S54PolicyPurpose.FunctionalClass
    $wrapperMarker=New-S54Marker $i $session.Inputs $fixtureAssertion $script:S54PolicyPurpose.WrapperClass
    $null=Write-S54New (Join-Path $rootPath 'fixture-session.json') @{native_marker=$nativeMarker;wrapper_marker=$wrapperMarker;classification='public_benign_fixture_only';case=$i.identity.case;attempt=$i.identity.attempt;run_id=$runId;owner_pid=$PID;launcher_pid=$launcher.Id;nonce=$nonce;inventory_sha256=$fixtureInput.sha256;launch_sha256=$fixtureInput.sha256;controller_tick_ms=$session.ControllerTick} 16384
    Wait-S54OwnedProcesses $session
    $layout=if($Purpose -ceq 'FeasibilityF'){Assert-S54NativeOutput $session.Output.Stdout $session.Output.Stderr $i $session.Inputs $fixtureAssertion}else{Get-S54Invocation $session.Output.Stdout 'public_fixture::benign' 'S54_BENIGN_FUNCTIONAL' 'S54_BENIGN_WRAPPER'}
    if($Mode -cnotin @('normal','tail','legacy')){throw 'unexpected benign failure mode passed'}
    $actualPassed=$true
}catch{
    $failureDetail=$_.Exception.Message;$failure=Get-S54Failure $session $job
    if($null -ne $session){
        $eofAtFailure=$session.Output.Eof
        foreach($idValue in @($job.Children()|Where-Object {$_ -ne $session.Launcher.Id -and ($null -eq $session.Harness -or $_ -ne $session.Harness.Id)})){
            $observed=$null
            try{$observed=[Diagnostics.Process]::GetProcessById($idValue);$job.VerifyChild($observed);$unknown+=@{pid=$observed.Id;image=$observed.MainModule.FileName;started_utc=$observed.StartTime.ToUniversalTime().ToString('o');exited=$observed.HasExited}}
            catch{$unknown+=@{pid=$idValue;observation_error=$_.Exception.Message}}
            finally{if($null -ne $observed){$observed.Dispose()}}
        }
    }
}
finally {
    $cleanupBudget=if($Mode -ceq 'cleanup'){0}else{3000}
    $closed=Close-S54Owned $job $cleanupBudget
    if($closed.forced -or -not $closed.verified){$actualPassed=$false}
    if($closed.failed){$failure='cleanup_failed'}
    # Intentional CleanupChildren(0) is an actual failed cleanup decision, not
    # a success flag. Test teardown then contains the still-live owned child.
    $contained=Close-S54Owned $job 3000;$teardown=$contained.verified
    $outcomes=if($null -ne $session){Get-S54KnownOutcomes $session}else{@{launcher=$null;harness=$null}}
    if($null -ne $session){$null=Write-S54Transcript $session;if($actualPassed -and -not $session.Transcript.complete){$actualPassed=$false;throw 'Benign success requires complete stored protocol.'}}
    if($null -ne $session){if($null -ne $session.Harness){$session.Harness.Dispose()}}
    if($null -ne $launcher){$launcher.Dispose()}
    $job.CheckWhole()
    $record=@{classification='public_benign_fixture_only';purpose=$Purpose;mode=$Mode;actual_outer_decision=if($actualPassed){'passed'}else{'failed'};failure=$failure;failure_detail=$failureDetail;unknown_owned_processes=$unknown;work_expired=$job.WorkExpired;clock_membership_served=($null -ne $session -and $session.ClockSent);a7_membership_served=($null -ne $session -and $session.JoinSent);exclusive_join_sharing_retries=if($null -ne $session){$session.JoinSharingRetries}else{0};launcher_waited=($null -ne $session -and $session.LauncherWaited);harness_waited=($null -ne $session -and $session.HarnessWaited);process_outcomes=$outcomes;transcript=if($null -ne $session){$session.Transcript}else{$null};known_wait_gap_ms=if($null -ne $session -and $null -ne $session.LauncherWaitedTick -and $null -ne $session.HarnessWaitedTick){$session.LauncherWaitedTick-$session.HarnessWaitedTick}else{$null};eof_at_failure=$eofAtFailure;cleanup=$closed;containment_teardown_verified=$teardown;elapsed_ms=([KilogramS5Deadline]::TickNow()-$origin);whole_attempt_verdict='not_established'}
    $null=Write-S54New (Join-Path $rootPath 'fixture-result.json') $record 65536
    if($teardown){$job.Finish()}
}
if(-not $teardown){throw 'benign fixture containment incomplete'}
if(($Mode -cin @('normal','tail','legacy')) -ne $actualPassed){throw 'benign expected actual outcome mismatch'}
if($Mode -ceq 'cleanup' -and ($closed.verified -or -not $closed.failed)){throw 'real cleanup failure must remain failure'}
if($Mode -cin @('timeout','eof','partial') -and -not $job.WorkExpired){throw 'actual original timeout not observed'}
Write-Output "benign_case=$Mode;fixture_only=true;expected_outcome_verified=true"
