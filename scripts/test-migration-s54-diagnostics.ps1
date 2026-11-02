[CmdletBinding()]
param([Parameter(Mandatory)][string]$EvidenceDirectory,[Parameter(Mandatory)][string]$HarnessPath,[Parameter(Mandatory)][string]$HarnessSha256)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1') -Purpose FeasibilityF
. (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Get-S54Path $EvidenceDirectory
$owned=Get-S54Path (Join-Path $workspace '.tmp/s54-block4k-admission-diagnostics-01')
if(-not $evidence.StartsWith($owned+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Fresh owned K public diagnostics evidence only.'}
$harness=Get-S54Pin $HarnessPath 134217728
Assert-S54SamePath $harness.path (Join-Path $workspace 'target/s54-diagnostics-source-checks/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe')
if($harness.sha256 -cne $HarnessSha256){throw 'Exact nonproduction stable image pin required.'}
Initialize-S54Output
$shell=[Diagnostics.Process]::GetCurrentProcess().MainModule.FileName;$shellPin=Get-S54Pin $shell 134217728
$job=[KilogramS5Deadline]::new(540,570,[long][Environment]::TickCount64)
$checks=[Collections.Generic.List[object]]::new()
function Require([bool]$Condition,[string]$Name){if(-not $Condition){throw $Name}}
function New-OwnedText([string]$Path,[string]$Text){
    $p=Get-S54Path $Path -AllowMissing
    if(-not $p.StartsWith($evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh canonical public test file required.'}
    [IO.File]::WriteAllText($p,$Text,[Text.UTF8Encoding]::new($false))
}
function Quote-Public([string]$Text){"'"+$Text.Replace("'","''")+"'"}
function Save-Public($Pin){[Text.UTF8Encoding]::new($false,$true).GetString((Read-S54Pin $Pin 1048576))}
$p=Get-S54Purpose FeasibilityF
$i=@{identity=@{case=$p.Case;attempt=$p.Attempt;selector=$p.Selector;run_id=('a'*64)}}
$inputs=@{inventory=@{sha256=('b'*64)}}
$assertion=@{Pin=@{sha256=('c'*64);bytes=123};Value=@{provenance=@{blake3=('d'*64)}}}
$native=$p.Native+' '+(ConvertTo-Json -InputObject (New-S54Marker $i $inputs $assertion $p.FunctionalClass) -Compress)
$wrapper=$p.Wrapper+' '+(ConvertTo-Json -InputObject (New-S54Marker $i $inputs $assertion $p.WrapperClass) -Compress)
$success=(@('running 1 test',('test '+$p.Selector+' ...'),$native,'ok','test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s',$wrapper)-join "`n")+"`n"
$diagnostic='S54_ADMISSION_DIAGNOSTIC v1 stage=inputs reason=required_input'
$canary='SYNTHETIC_PRIVATE_CANARY_must_never_be_retained'
try{
    # Check the complete finite producer vocabulary against the actual filter.
    $rust=Save-Public (Get-S54Pin (Join-Path $workspace 'apps/kilogram-cli/tests/support/migration_s54_admission_diagnostic.rs') 65536)
    $matches=[regex]::Matches($rust,'=> \("([a-z_]+)", "([a-z_]+)"\)')
    Require ($matches.Count -eq 29) 'Finite producer code enumeration changed without protocol review.'
    foreach($m in $matches){
        $line='S54_ADMISSION_DIAGNOSTIC v1 stage='+$m.Groups[1].Value+' reason='+$m.Groups[2].Value
        $state=[KilogramS54Output+StreamState]::new();$state.Bytes=[Text.Encoding]::UTF8.GetBytes($line+"`n");$state.Eof=$true;$state.Stopped=$true
        $saved=[KilogramS54Output]::Protocol($state,[Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal))
        Require ($saved.Text -ceq ($line+"`n") -and $saved.AdmissionDiagnostics -eq 1 -and $saved.Malformed -eq 0) 'Typed producer/persistence vocabulary parity.'
    }
    $checks.Add(@{name='all29 typed producer codes pass actual closed protocol filter';passed=$true})
    $cases=@(
        @{name='success';stdout=$success;stderr='';code=0},
        @{name='missing-native-input';actual_native=$true;code=101},
        @{name='forged';stdout=("S54_FEASIBILITY_FUNCTIONAL $canary`n");stderr=($diagnostic+"`n"+"S54_ADMISSION_DIAGNOSTIC v1 stage=inputs reason=made_up`nS54_ADMISSION_DIAGNOSTIC v1 stage=private_stage reason=unknown`nS54_ADMISSION_DIAGNOSTIC v1 stage=inputs reason=unknown`n"+$diagnostic+" $canary`n"+$diagnostic+"`r`r`nS54_ADMISSION_DIAGNOSTIC v1 stage=inputs`nreason=required_input $canary`n$canary`n");code=7},
        @{name='count-bound';stdout='';stderr=(($diagnostic+"`n")*8);code=7},
        @{name='oversized';stdout='';stderr=($diagnostic+('x'*1024)+"$canary`n");code=7},
        @{name='partial';stdout='';stderr=$diagnostic;code=7},
        @{name='invalid-utf8';stdout='';stderr=$diagnostic+"`n";invalid_utf8=$true;code=7}
    )
    foreach($case in $cases){
        $job.CheckWork();$root=Join-Path $evidence $case.name
        $null=[IO.Directory]::CreateDirectory($root)
        $launcherPath=Join-Path $root 'forwarder.ps1'
        if($case.ContainsKey('actual_native')){
            Require (-not (Test-Path -LiteralPath (Join-Path $workspace ('.tmp/'+$p.Namespace+'/'+$p.Attempt)))) 'Actual F04 run directory must remain unallocated.'
            $exe=$harness
            $argv=@('--exact','live_l1::handoff::final_p5::admission::tests::s54_admission_diagnostics_missing_input_child','--ignored','--nocapture','--test-threads=1')
        }else{
            $outBytes=[Text.Encoding]::UTF8.GetBytes($case.stdout);$errBytes=[Text.Encoding]::UTF8.GetBytes($case.stderr)
            if($case.ContainsKey('invalid_utf8')){$errBytes=[byte[]](@(255)+@($errBytes))}
            $fixture=Join-Path $root 'benign-child.ps1'
            $body='$b=[Convert]::FromBase64String('+ (Quote-Public ([Convert]::ToBase64String($outBytes))) +');[Console]::OpenStandardOutput().Write($b,0,$b.Length);'+"`n"+'$b=[Convert]::FromBase64String('+(Quote-Public ([Convert]::ToBase64String($errBytes)))+');[Console]::OpenStandardError().Write($b,0,$b.Length);'+"`n"+'exit '+$case.code
            New-OwnedText $fixture $body
            $exe=$shellPin;$argv=@('-NoProfile','-NonInteractive','-File',$fixture)
        }
        # This is the published native leaf, including incremental ForwardNew
        # and native capture status. It returns the real nonzero child code.
        $body='. '+(Quote-Public (Join-Path $PSScriptRoot 'invoke-migration-s54-whole.ps1'))+' -Purpose FeasibilityF'+"`n"+'$pin=Get-S54Pin '+(Quote-Public $exe.path)+' 134217728'+"`n"+'if($pin.sha256 -cne '+(Quote-Public $exe.sha256)+'){throw ''source-check executable drift''}'+"`n"+'$leaf=Invoke-S54SingleExecutable $pin @('+ (($argv|ForEach-Object {Quote-Public $_}) -join ',') +') ([Environment]::TickCount64+30000) -ForwardOutput'+"`n"+'exit $leaf.code'
        New-OwnedText $launcherPath $body
        $process=New-S54Process $shell @('-NoProfile','-NonInteractive','-File',$launcherPath)
        foreach($key in @('KILOGRAM_S54_INVENTORY_SHA256','KILOGRAM_S54_ACTIVATION_SHA256','KILOGRAM_S54_LAUNCH_SHA256','KILOGRAM_S54_CAPTURE_SHA256')){$null=$process.StartInfo.Environment.Remove($key)}
        $capture=$null
        try{
            Require ($process.Start()) 'Benign forwarder start.';$job.VerifyChild($process)
            $capture=[KilogramS54Output]::new($process);$end=[Environment]::TickCount64+40000
            while(-not $process.WaitForExit(0) -or -not $capture.Eof){$job.CheckWork();if($capture.Failed -or [Environment]::TickCount64 -ge $end){throw 'Benign forwarder bounded wait/EOF.'};Start-Sleep -Milliseconds 5}
            Require ($process.ExitCode -eq $case.code) ('Real forwarded native exit/'+$case.name)
            $public=@{identity=@{case=$i.identity.case;attempt=$i.identity.attempt;selector=$i.identity.selector;run_id=$i.identity.run_id;run=$root}}
            $session=@{Job=$job;I=$public;Output=$capture;Transcript=$null}
            $transcript=Write-S54Transcript $session
            $out=Save-Public $transcript.stdout.pin;$err=Save-Public $transcript.stderr.pin
            Require (($out+$err) -cnotmatch ([regex]::Escape($canary))) 'Private synthetic canary never persisted.'
            Require ($transcript.stdout.source_eof -and $transcript.stderr.source_eof -and $capture.Eof -and -not $capture.Failed) 'Actual complete bounded capture/EOF.'
            Require ($transcript.stderr.admission_diagnostics.authority -ceq 'none' -and $transcript.whole_attempt_verdict -ceq 'not_established') 'Diagnostics have no success authority.'
            if($case.name -ceq 'success'){
                $layout=Assert-S54NativeOutput $out $err $i $inputs $assertion
                Require ($transcript.complete -and $layout.native_markers -eq 1 -and $layout.wrapper_markers -eq 1 -and $transcript.stderr.admission_diagnostics.count -eq 0) 'Successful protocol/cardinality unchanged.'
            }else{
                $refused=$false;try{$null=Assert-S54NativeOutput $out $err $i $inputs $assertion}catch{$refused=$true}
                Require $refused 'Advisory diagnostic cannot satisfy success contract.'
            }
            switch($case.name){
                'missing-native-input' {Require ($transcript.complete -and $err.Contains($diagnostic+"`n") -and $transcript.stderr.admission_diagnostics.count -eq 1 -and -not (Test-Path -LiteralPath (Join-Path $workspace ('.tmp/'+$p.Namespace+'/'+$p.Attempt)))) 'Actual load_for error retained before native claim and any F04 allocation.'}
                'forged' {Require ($transcript.stderr.admission_diagnostics.count -eq 1 -and $transcript.stderr.admission_diagnostics.dropped -ge 6 -and -not $transcript.complete -and -not $err.Contains('made_up') -and -not $err.Contains('private_stage')) 'Only exact closed code survives forged fields/tails/newline.'}
                'count-bound' {Require ($transcript.stderr.admission_diagnostics.count -eq 4 -and $transcript.stderr.admission_diagnostics.dropped -eq 4 -and $transcript.stderr.admission_diagnostics.bytes -le 512 -and -not $transcript.complete) 'Explicit four-line512-byte stream cap.'}
                'oversized' {Require ($transcript.stderr.admission_diagnostics.count -eq 0 -and $transcript.stderr.admission_diagnostics.dropped -eq 1 -and -not $transcript.complete) 'Oversized diagnostic is redacted/incomplete.'}
                'partial' {Require ($transcript.stderr.admission_diagnostics.count -eq 0 -and $transcript.stderr.admission_diagnostics.dropped -eq 1 -and -not $transcript.complete) 'Forwarder capture separator cannot complete a partial diagnostic.'}
                'invalid-utf8' {Require ($transcript.stderr.invalid_utf8 -and $transcript.stderr.admission_diagnostics.count -eq 0 -and -not $transcript.complete) 'Invalid encoding cannot become a diagnostic.'}
            }
            $checks.Add(@{name=$case.name;passed=$true;actual_exit=$process.ExitCode;actual_waited=$true;pid=$process.Id;actual_job_handle_verified=$true;bounded_eof=$capture.Eof;native_capture=$transcript.native_capture;transcript=$transcript;success_authority='none';runtime_attempt_started=$false})
        }finally{$process.Dispose()}
    }
    # Relevant existing owner/transcript regressions, using new public fixture
    # roots accepted by that helper. No historical fixture is cloned or rebound.
    $priorBase=Get-S54Path (Join-Path $workspace ('.tmp/s54-block4b-r1-01/block4k-diagnostics-'+[IO.Path]::GetFileName((Split-Path -Parent $evidence)))) -AllowMissing
    if(Test-Path -LiteralPath $priorBase){throw 'Existing transcript regression root cannot be reused.'}
    $null=[IO.Directory]::CreateDirectory($priorBase)
    $fixturePin=Get-S54Pin (Join-Path $workspace 'apps/kilogram-cli/tests/support/migration_s54_outer_fixture.ps1') 65536
    foreach($mode in @('normal','tail','nonzero','partial','overflow')){
        $job.CheckWork();$null=Read-S54Pin $fixturePin 65536
        $fixtureRoot=Join-Path $priorBase $mode
        $leaf=Invoke-S54SingleExecutable $shellPin @('-NoProfile','-NonInteractive','-File',$fixturePin.path,'-Role','Owner','-Mode',$mode,'-Root',$fixtureRoot) ([Environment]::TickCount64+40000)
        Require ($leaf.code -eq 0 -and $leaf.waited -and $leaf.eof) ('Existing benign owner actual return/'+$mode)
        $record=Read-S54Json (Get-S54Pin (Join-Path $fixtureRoot 'fixture-result.json') 65536) 65536
        $stored=$record.transcript;$out=Save-Public $stored.stdout.pin;$err=Save-Public $stored.stderr.pin
        Require ($record.containment_teardown_verified -and $record.clock_membership_served -and $record.a7_membership_served) 'Existing benign ownership/clock/join checks.'
        Require (($out+$err) -cnotmatch 'secret-nonzero-must-be-redacted|private partial diagnostic|public diagnostic between|diagnostic on the named line') 'Existing arbitrary diagnostic redaction unchanged.'
        if($mode -cin @('normal','tail')){
            Require ($record.actual_outer_decision -ceq 'passed' -and $stored.complete -and -not $record.cleanup.forced) 'Existing successful complete protocol unchanged.'
            $null=Get-S54Invocation $out 'public_fixture::benign' 'S54_BENIGN_FUNCTIONAL' 'S54_BENIGN_WRAPPER'
        }else{
            Require ($record.actual_outer_decision -ceq 'failed') 'Existing failed benign run remains failed.'
            $refused=$false;try{$null=Get-S54Invocation $out 'public_fixture::benign' 'S54_BENIGN_FUNCTIONAL' 'S54_BENIGN_WRAPPER'}catch{$refused=$true}
            Require $refused 'Existing failed protocol cannot provide success.'
            if($mode -ceq 'partial'){Require (-not $stored.complete -and $stored.stdout.partial_tail_bytes -gt 0) 'Existing partial native line stays incomplete.'}
            if($mode -ceq 'overflow'){Require (-not $stored.complete -and $stored.native_capture_status -ceq 'observed' -and $stored.native_capture.stdout_overflow) 'Existing native overflow status retained.'}
            if($mode -ceq 'nonzero'){Require ($record.process_outcomes.harness.code -eq 7 -and $record.process_outcomes.harness.waited) 'Existing native nonzero actual wait preserved.'}
        }
        $checks.Add(@{name=('existing-transcript/'+$mode);passed=$true;actual_owner_exit=$leaf.code;bounded_eof=$leaf.eof;result=(Get-S54Pin (Join-Path $fixtureRoot 'fixture-result.json') 65536);transcript=$stored;cleanup=$record.cleanup;production_Run=false})
    }
    $closed=Close-S54Owned $job 3000
    Require ($closed.verified -and -not $closed.failed -and -not $closed.forced) 'Clean nonforced owned cleanup.'
    $null=Write-S54New (Join-Path $evidence 'results.json') @{classification='focused public producer-forwarder-production-persistence diagnostics checks only';checks=@($checks);count=$checks.Count;cleanup=$closed;actual_Run_executed=$false;private_values_exported=$false;whole_attempt_verdict='not_established'} 524288
    Write-Output ('s54_admission_diagnostics_checks='+$checks.Count+';production_Run=false')
}finally{if(@($job.Children()).Count){$job.CleanupChildren([Math]::Min(3000,$job.RemainingWholeMilliseconds))};$job.Finish()}
