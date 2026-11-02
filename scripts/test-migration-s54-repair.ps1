[CmdletBinding()]
param([Parameter(Mandatory)][string]$FixtureIndex,[Parameter(Mandatory)][string]$EvidenceDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1')
. (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Get-S54Path $EvidenceDirectory
if(-not $evidence.StartsWith((Join-Path $workspace '.tmp/s54-block4b-r1-01/'),[StringComparison]::OrdinalIgnoreCase)){throw 'Owned R1 evidence only.'}
$job=[KilogramS5Deadline]::new(900,910,[long][Environment]::TickCount64)
$results=[Collections.Generic.List[object]]::new()
function Require([bool]$Value,[string]$Why){if(-not $Value){throw $Why}}
function Refuses([string]$Name,[scriptblock]$Test,[string]$Pattern){
    $reason=$null;try{$null=& $Test}catch{$reason=$_.Exception.Message}
    Require ($null -ne $reason -and $reason -match $Pattern) "Expected $Name refusal, actual: $reason"
    $results.Add(@{name=$Name;expected_refusal=$true;reason=$reason})
}
function Reopen($Pin){
    $raw=[IO.File]::ReadAllBytes($Pin.path)
    Require ($raw.Length -eq $Pin.bytes -and (Get-S54Sha $raw) -ceq $Pin.sha256) 'Independent stored byte/hash match.'
    return [Text.UTF8Encoding]::new($false,$true).GetString($raw)
}
function New-PublicStageRoot([string]$Name){$root=Join-Path $evidence $Name;$null=New-Item -ItemType Directory -Path $root -ErrorAction Stop;return $root}
try {
    $index=ConvertFrom-S54Strict (Read-S54Public $FixtureIndex 16384) 16384
    Require ($index.classification -ceq 'public_schema_fixture_only') 'Public input only.'
    $policy=New-S54Policy -FixtureRoot $index.root
    $null=Test-S54Admission $index.inputs $policy
    $results.Add(@{name='changed public helper/source admission baseline';passed=$true})
    Initialize-S54Output
    $shell=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName;$shellPin=Get-S54Pin $shell 134217728
    $fixture=Join-Path $workspace 'apps/kilogram-cli/tests/support/migration_s54_outer_fixture.ps1';$fixturePin=Get-S54Pin $fixture 65536
    foreach($mode in @('normal','tail','nonzero','timeout','eof','linger','cleanup','partial','overflow')){
        $job.CheckWork();$null=Read-S54Pin $fixturePin 65536
        $root=Join-Path $evidence "benign-$mode"
        $run=Invoke-S54SingleExecutable $shellPin @('-NoProfile','-NonInteractive','-File',$fixture,'-Role','Owner','-Mode',$mode,'-Root',$root) ([Environment]::TickCount64+40000)
        if($run.code -ne 0){[IO.File]::WriteAllText((Join-Path $evidence "$mode.stderr.log"),$run.stderr);throw "Benign R1 owner exit $($run.code) for $mode"}
        $record=ConvertFrom-S54Strict (Read-S54Public (Join-Path $root 'fixture-result.json') 65536) 65536
        $positive=$mode -cin @('normal','tail')
        Require ($record.classification -ceq 'public_benign_fixture_only' -and $record.whole_attempt_verdict -ceq 'not_established' -and $record.containment_teardown_verified) 'Actual fixture containment and authority.'
        Require (($record.actual_outer_decision -ceq 'passed') -eq $positive) "Actual R1 expected owner decision/$mode"
        Require ($record.clock_membership_served -and $record.a7_membership_served) 'Actual retained handle admission.'
        $transcript=$record.transcript
        Require ($transcript.persistence_completed -and $transcript.classification -ceq 'bounded_protocol_transcript_nonprotocol_redacted') 'Actual production transcript writer completed.'
        $out=Reopen $transcript.stdout.pin;$err=Reopen $transcript.stderr.pin
        Require (($out+$err) -cnotmatch 'secret-nonzero-must-be-redacted|private partial diagnostic|public diagnostic between|diagnostic on the named line') 'Nonprotocol diagnostics are not durably dumped.'
        if($positive){
            Require ($transcript.complete -and -not $record.cleanup.forced -and $record.process_outcomes.harness.code -eq 0 -and $record.process_outcomes.launcher.code -eq 0 -and $record.harness_waited -and $record.launcher_waited) 'Waited success with complete reopened transcript.'
            $null=Get-S54Invocation $out 'public_fixture::benign' 'S54_BENIGN_FUNCTIONAL' 'S54_BENIGN_WRAPPER'
            Require ($transcript.stdout.redacted_named_tails -eq 1 -and $transcript.stdout.redacted_lines -gt 0) 'Explicit named-tail/nonprotocol redaction.'
            if($mode -ceq 'tail'){Require ($record.known_wait_gap_ms -ge 1500 -and -not $record.work_expired) 'Required launcher tail over1s succeeds under original bound.'}
        }else{
            Refuses "reopened failed transcript cannot supply complete invocation/$mode" {Get-S54Invocation $out 'public_fixture::benign' 'S54_BENIGN_FUNCTIONAL' 'S54_BENIGN_WRAPPER'} 'S54'
            if($mode -ceq 'nonzero'){Require ($record.process_outcomes.harness.code -eq 7 -and $record.process_outcomes.harness.waited -and $record.process_outcomes.launcher.waited) 'Actual native nonzero waits retained.'}
            if($mode -cin @('linger','cleanup')){Require ($record.process_outcomes.harness.code -eq 0 -and $record.cleanup.forced) 'Unexpected descendant cannot upgrade native0.'}
            if($mode -ceq 'cleanup'){Require ($record.failure -ceq 'cleanup_failed' -and -not $record.cleanup.verified) 'Failed cleanup decision retained after separate teardown.'}
            if($mode -ceq 'partial'){Require (-not $transcript.complete -and $transcript.stdout.partial_tail_bytes -gt 0) 'Partial line is explicitly incomplete, never completed by writer.'}
            if($mode -ceq 'overflow'){Require (-not $transcript.complete -and $transcript.native_capture_status -ceq 'observed' -and $transcript.native_capture.stdout_overflow) 'Actual native overflow explicitly retained.'}
        }
        $results.Add(@{name="actual R1 owner and production transcript/$mode";passed=$true;actual_owner_exit=$run.code;record=$record})
    }
    # Seven small public files use the actual preparation writer and leases.
    $root=New-PublicStageRoot 'stage-interrupted';$paths=Get-S54MutablePaths $root
    for($n=0;$n -lt 7;$n+=2){$null=[IO.Directory]::CreateDirectory((Split-Path -Parent $paths[$n]));[IO.File]::WriteAllText($paths[$n],"public original $n")}
    $source=Join-Path $root 'public-new.bin';[IO.File]::WriteAllText($source,'public replacement bytes');$sourcePin=Get-S54Pin $source 4096
    $guard=New-S54ImageGuard $root (Join-Path $root 'saved')
    try {
        $pre=Read-S54Json $guard.PreimageManifest 65536
        Require ($pre.images.Count -eq 7 -and @($pre.images|Where-Object state -CEQ present).Count -eq 4 -and @($pre.images|Where-Object state -CEQ absent).Count -eq 3) 'Preimages record all present and absent roles before replacement.'
        foreach($row in $pre.images){if($row.state -ceq 'present'){$null=Reopen $row.saved;Require ($row.saved.sha256 -ceq $row.original.sha256) 'Actual preserved original bytes.'}else{Require (-not (Test-Path -LiteralPath $row.path)) 'Absent image not created by reservation.'}}
        Refuses 'concurrent owned preparation' {New-S54ImageGuard $root (Join-Path $root 'another')} 'concurrent owned preparation'
        $null=Copy-S54PreparedImage $guard $source $paths[0];$null=Copy-S54PreparedImage $guard $source $paths[1]
        Refuses 'interrupted stage missing source' {Copy-S54PreparedImage $guard (Join-Path $root 'absent-source.bin') $paths[2]} 'missing public path'
        Refuses 'guard cannot stage legacy/relay or arbitrary path' {Copy-S54PreparedImage $guard $source (Join-Path $root 'arbitrary.exe')} 'exact mutable destination'
    }finally{Close-S54ImageGuard $guard $false}
    $outcome=ConvertFrom-S54Strict (Read-S54Public (Join-Path $root 'saved/preparation-outcome.json') 65536) 65536
    Require (-not $outcome.preparation_succeeded -and -not $outcome.stage_completed -and -not $outcome.automatic_rollback -and $outcome.staged.Count -eq 2) 'Interrupted stage is retained without rollback/retry.'
    foreach($row in $pre.images){if($row.state -ceq 'present'){$null=Reopen $row.saved}}
    Require ((Get-S54Pin $paths[0] 4096).sha256 -ceq $sourcePin.sha256 -and (Get-S54Pin $paths[1] 4096).sha256 -ceq $sourcePin.sha256 -and (Get-S54Pin $paths[2] 4096).sha256 -ceq $pre.images[2].original.sha256) 'Actual interrupted destinations and untouched remainder.'
    $results.Add(@{name='actual seven-path preimages and interrupted stage';passed=$true;preimages=$pre;outcome=$outcome})
    $root=New-PublicStageRoot 'stage-complete';$paths=Get-S54MutablePaths $root;$source=Join-Path $root 'new.bin';[IO.File]::WriteAllText($source,'public seven image set')
    $guard=New-S54ImageGuard $root (Join-Path $root 'saved')
    try{foreach($path in $paths){$null=Copy-S54PreparedImage $guard $source $path};Complete-S54ImageStage $guard;Require $guard.StageCompleted 'All seven actual writes complete.'}finally{Close-S54ImageGuard $guard $true}
    $results.Add(@{name='actual seven-path complete stage and release';passed=$true;outcome=(ConvertFrom-S54Strict (Read-S54Public (Join-Path $root 'saved/preparation-outcome.json') 65536) 65536)})
    # A copied, pinned cmd.exe blocks only on its redirected public stdin. /d
    # disables AutoRun. It performs no networking or profile/script execution.
    $root=New-PublicStageRoot 'stage-live';$paths=Get-S54MutablePaths $root;$null=[IO.Directory]::CreateDirectory((Split-Path -Parent $paths[0]))
    $cmdPin=Get-S54Pin (Join-Path ([Environment]::GetFolderPath('System')) 'cmd.exe') 134217728
    [IO.File]::Copy($cmdPin.path,$paths[0],$false);Require ((Get-S54Pin $paths[0] 134217728).sha256 -ceq $cmdPin.sha256) 'Pinned benign executable copy.'
    $p=New-S54Process $paths[0] @('/d','/q','/c','set /p public_wait=');$p.StartInfo.RedirectStandardInput=$true
    try{
        if(-not $p.Start()){throw 'Benign fixed-path process start'};$job.VerifyChild($p)
        $capture=[KilogramS54Output]::new($p);Require (-not $p.WaitForExit(100)) 'Benign fixed path really live.'
        Refuses 'active exact-path image' {New-S54ImageGuard $root (Join-Path $root 'saved')} 'live exact image path'
        Require (-not (Test-Path -LiteralPath (Join-Path $root 'saved'))) 'Live image refused before preimage/staging mutation.'
    }finally{
        if(-not $p.HasExited){$p.StandardInput.WriteLine('public done');$p.StandardInput.Close()}
        $end=[Environment]::TickCount64+3000
        while(-not $p.WaitForExit(0) -or -not $capture.Eof){if([Environment]::TickCount64 -ge $end){$p.Kill();throw 'Benign fixed path cleanup bound'};Start-Sleep -Milliseconds 5}
        $p.Dispose()
    }
    $bad=@{OwnerPid=$PID;Closed=$true}
    Refuses 'stable preparation opt-in cannot accept inactive guard' {& (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -S54PreparationGuard $bad} 'active owned preparation guard'
    $job.CheckWork();Require (@($job.Children()).Count -eq 0) 'R1 final owned children empty.'
    $null=Write-S54New (Join-Path $evidence 'repair-public-results.json') @{classification='Block4B R1 affected public/benign boundaries only';count=$results.Count;checks=@($results);whole_attempt_verdict='not_established';production_preparation=$false} 524288
    Write-Output ('s54_r1_public_check_count='+$results.Count)
}finally{if(@($job.Children()).Count){$job.CleanupChildren([Math]::Min(3000,$job.RemainingWholeMilliseconds))};$job.Finish()}
