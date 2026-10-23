[CmdletBinding()]
param(
    [string]$Executable = 'target/field-tests/ordered-dialog/kilogram-cli.exe',
    [string]$RunDirectory,
    [ValidateSet('Duplex','Gap','Partial','BOnly','Crash','TwoContexts','Expiry','ConsentExpiry','Conflict','UnsupportedSuite','MissingAuth','CrossContext')][string]$Scenario = 'Duplex',
    [string]$KillPoint,
    [ValidateRange(1,128)][int]$MessagesPerLane = 128,
    [ValidateRange(1,16)][int]$Burst = 8
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
$workspace = Split-Path -Parent $PSScriptRoot
$exe = (Resolve-Path -LiteralPath (Join-Path $workspace $Executable)).Path
if (-not $RunDirectory) { $RunDirectory = Join-Path $workspace ('.tmp/ordered-dialog-process/' + [guid]::NewGuid().ToString('N')) }
if (Test-Path -LiteralPath $RunDirectory) { throw 'Process acceptance requires a new run directory.' }
[void](New-Item -ItemType Directory -Path $RunDirectory)
$run = (Resolve-Path -LiteralPath $RunDirectory).Path
$trace = Join-Path $run 'trace.log'
$watch = [Diagnostics.Stopwatch]::StartNew()
$processes = @()
$runtimeBySide = @{}
$immutableBefore = @{}
$queueTimes = [Collections.Generic.List[double]]::new()
function Invoke-Cli([string[]]$Arguments) {
    $output = & $exe @Arguments 2>&1
    $exitCode = $LASTEXITCODE
    $rendered = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    Add-Content -LiteralPath $trace -Value ("{0:N3}s exit={1} {2}`n{3}" -f $watch.Elapsed.TotalSeconds,$exitCode,($Arguments -join ' '),$rendered)
    if ($exitCode -ne 0) { throw "CLI exit ${exitCode}: $rendered" }
    return $rendered
}
function Path-In([string]$Name) { return (Join-Path $run $Name) }
function Get-Status([string]$Side) { return (Invoke-Cli @('ordered','status','--ipc-file',(Path-In "$Side.ipc")) | ConvertFrom-Json) }
function Wait-State([string]$Side,[scriptblock]$Predicate) {
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    do {
        foreach ($p in $processes) { if ($p.HasExited) { throw "Runtime PID $($p.Id) exited early." } }
        $status = Get-Status $Side
        if (& $Predicate $status.contexts[0]) { return $status }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    foreach ($otherSide in @('one','two')) { [void](Get-Status $otherSide) }
    throw "Runtime $Side did not reach expected state: $($status | ConvertTo-Json -Depth 7 -Compress)"
}
function Start-Runtime([string]$Side) {
    $oldSchedule = $env:KILOGRAM_TEST_OD1_SCHEDULE
    $oldPrimary = $env:KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE
    $oldClock=$env:KILOGRAM_TEST_OD1_CLOCK_FILE
    try {
        $env:KILOGRAM_TEST_OD1_SCHEDULE = Path-In "$Side.schedule.json"
        if ($Scenario -in @('Expiry','ConsentExpiry')) {$env:KILOGRAM_TEST_OD1_CLOCK_FILE=Path-In "$Side.clock"}
        if ($Scenario -eq 'Crash' -and $KillPoint -eq 'readable-primary' -and $Side -ceq $crashVictim) {
            $env:KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE=Path-In "$Side.primary"
        }
        $suffix = [guid]::NewGuid().ToString('N').Substring(0,8)
        $process = Start-Process -FilePath $exe -ArgumentList @('runtime-from-profile','--profile-file',('"{0}"' -f (Path-In "$Side.profile"))) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Path-In "$Side.$suffix.stdout.log") -RedirectStandardError (Path-In "$Side.$suffix.stderr.log")
        $runtimeBySide[$Side]=$process
        Add-Content -LiteralPath $trace -Value "runtime_start side=$Side pid=$($process.Id) log_suffix=$suffix"
        return $process
    } finally { $env:KILOGRAM_TEST_OD1_SCHEDULE = $oldSchedule; $env:KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE=$oldPrimary; $env:KILOGRAM_TEST_OD1_CLOCK_FILE=$oldClock }
}
function Set-Schedule([string]$Side,[hashtable]$Plan) {
    $path=Path-In "$Side.schedule.json"
    $temp="$path.new"
    [IO.File]::WriteAllText($temp,($Plan | ConvertTo-Json -Depth 5 -Compress))
    [IO.File]::Move($temp,$path,$true)
    Add-Content -LiteralPath $trace -Value "scheduler side=$Side $($Plan | ConvertTo-Json -Depth 5 -Compress)"
}
function Set-Clock([uint64]$Offset) {
    foreach ($side in @('one','two')) {
        $path=Path-In "$side.clock";$temp="$path.new"
        [IO.File]::WriteAllText($temp,"$Offset");[IO.File]::Move($temp,$path,$true)
    }
    Add-Content -LiteralPath $trace -Value "targeted_clock_offset_seconds=$Offset"
}
function Queue-Test($Sender,[int]$Index,[string]$ContextId=$context,[bool]$Track=$true) {
    $request=('{0:x16}{1:x16}' -f ([int]($Sender.side -ceq $a.side)+1),$Index)
    $text="OD1/$($Sender.side)/$Index/Привет"
    if ($Track) {$expected[$request]=$text}
    $started=$watch.Elapsed.TotalMilliseconds
    [void](Invoke-Cli @('ordered','queue','--ipc-file',(Path-In "$($Sender.side).ipc"),'--context',$ContextId,'--request-id',$request,'--text',$text))
    $queueTimes.Add($watch.Elapsed.TotalMilliseconds-$started)
}
function Restart-Side([string]$Side,[scriptblock]$BeforeStart) {
    $old=$runtimeBySide[$Side]
    Add-Content -LiteralPath $trace -Value "real_child_kill side=$Side pid=$($old.Id)"
    Stop-Process -Id $old.Id
    $old.WaitForExit()
    foreach ($file in Get-ChildItem -LiteralPath (Path-In "$Side.state") -Recurse -File) {
        $relative=[IO.Path]::GetRelativePath((Path-In "$Side.state"),$file.FullName).Replace('\','/')
        if ($relative.StartsWith('events/od1/') -or $relative.StartsWith('local-messages/od1/') -or $relative.Contains('/controls/') -or $relative.Contains('/ready/') -or $relative.EndsWith('/initial-pair')) {
            $immutableBefore[$file.FullName]=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        }
    }
    $script:processes=@($processes | Where-Object { $_.Id -ne $old.Id })
    if ($BeforeStart) { & $BeforeStart }
    $oldIpc=(Get-FileHash -LiteralPath (Path-In "$Side.ipc") -Algorithm SHA256).Hash
    $script:processes += Start-Runtime $Side
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100
        try {$changed=(Get-FileHash -LiteralPath (Path-In "$Side.ipc") -Algorithm SHA256 -ErrorAction Stop).Hash -cne $oldIpc}
        catch [IO.IOException] {$changed=$false}
    } while (-not $changed -and [DateTime]::UtcNow -lt $deadline)
    if (-not $changed) { throw 'Relaunch did not publish a fresh authenticated IPC endpoint.' }
    $other=if ($Side -ceq $a.side) {$b.side}else{$a.side}
    [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$other.ipc"),'--context',$context,'--locator-file',(Path-In "$Side.locators/$context.locator")))
}
function Wait-KillPoint([string]$Side) {
    $marker=if ($KillPoint -eq 'readable-primary') {Path-In "$Side.primary-hit"}else{Path-In "$Side.schedule.hit"}
    $deadline=[DateTime]::UtcNow.AddSeconds(25)
    while (-not (Test-Path -LiteralPath $marker)) {
        if ([DateTime]::UtcNow -ge $deadline) {throw "No crash marker for $KillPoint"}
        Start-Sleep -Milliseconds 20
    }
    Add-Content -LiteralPath $trace -Value "crash_point=$KillPoint marker=$marker real_pid=$($runtimeBySide[$Side].Id)"
}
try {
    $identities = @()
    foreach ($side in @('one','two')) {
        Set-Schedule $side @{}
        [IO.File]::WriteAllText((Path-In "$side.clock"),'0')
        $accountOutput = Invoke-Cli @('account-create','--account-dir',(Path-In "$side.account"))
        $account = [regex]::Match($accountOutput,'(?m)^account_id=(\S+)').Groups[1].Value
        $deviceOutput = Invoke-Cli @('device-enroll','--profile-mode','ordered-dialog-v1','--account-dir',(Path-In "$side.account"),'--state-dir',(Path-In "$side.state"))
        $device = [regex]::Match($deviceOutput,'(?m)^device_id=(\S+)').Groups[1].Value
        if (-not $account -or -not $device) { throw 'Missing real CLI identity.' }
        [void](Invoke-Cli @('ordered','credentials-export','--state-dir',(Path-In "$side.state"),'--output-file',(Path-In "$side.credentials")))
        $identities += [pscustomobject]@{ side=$side; account=$account; device=$device }
    }
    $ordered = @($identities | Sort-Object device)
    $a = $ordered[0]; $b = $ordered[1]
    if($Scenario -eq 'UnsupportedSuite'){Set-Schedule $a.side @{unsupported_suite=$true}}
    $earlyCrash=$Scenario -eq 'Crash' -and $KillPoint -match 'offer|grant|init|confirm|ready'
    $crashVictim=if ($KillPoint -in @('after-grant','before-confirm-commit','after-confirm-commit','after-ready','after-pending','before-readable-commit','after-readable-commit','readable-primary')) {$b.side}else{$a.side}
    if ($Scenario -eq 'Crash') {
        $allowed=@('after-offer','after-grant','before-init-commit','after-init-commit','before-confirm-commit','after-confirm-commit','after-receive-confirm','after-ready','after-queue','after-materialization','after-pending','before-readable-commit','after-readable-commit','readable-primary')
        if ($KillPoint -notin $allowed) {throw 'Select an explicit supported crash checkpoint.'}
        [IO.File]::WriteAllText((Path-In "$crashVictim.primary"),'off')
        if ($earlyCrash) {Set-Schedule $crashVictim @{pause_point=$KillPoint}}
    }
    $invitation = Invoke-Cli @('ordered','context-create','--state-dir',(Path-In "$($a.side).state"),'--account-dir',(Path-In "$($a.side).account"),'--peer-credentials-file',(Path-In "$($b.side).credentials"),'--expect-account',$b.account,'--output-file',(Path-In 'invitation'))
    $context = [regex]::Match($invitation,'(?m)^context=(\S+)').Groups[1].Value
    [void](Invoke-Cli @('ordered','context-import','--state-dir',(Path-In "$($b.side).state"),'--account-dir',(Path-In "$($b.side).account"),'--invitation-file',(Path-In 'invitation'),'--expect-account',$a.account,'--output-file',(Path-In 'countersigned')))
    [void](Invoke-Cli @('ordered','context-import','--state-dir',(Path-In "$($a.side).state"),'--invitation-file',(Path-In 'countersigned'),'--expect-account',$b.account))
    $contexts=@($context)
    if ($Scenario -eq 'TwoContexts') {
        $second=Invoke-Cli @('ordered','context-create','--state-dir',(Path-In "$($a.side).state"),'--account-dir',(Path-In "$($a.side).account"),'--peer-credentials-file',(Path-In "$($b.side).credentials"),'--expect-account',$b.account,'--output-file',(Path-In 'invitation2'))
        $context2=[regex]::Match($second,'(?m)^context=(\S+)').Groups[1].Value
        [void](Invoke-Cli @('ordered','context-import','--state-dir',(Path-In "$($b.side).state"),'--account-dir',(Path-In "$($b.side).account"),'--invitation-file',(Path-In 'invitation2'),'--expect-account',$a.account,'--output-file',(Path-In 'countersigned2')))
        [void](Invoke-Cli @('ordered','context-import','--state-dir',(Path-In "$($a.side).state"),'--invitation-file',(Path-In 'countersigned2'),'--expect-account',$b.account))
        $contexts += $context2
    }
    foreach ($side in @('one','two')) {
        $profileArguments=@('runtime-profile-create','--profile-mode','ordered-dialog-v1','--profile-file',(Path-In "$side.profile"),'--state-dir',(Path-In "$side.state"),'--ipc-file',(Path-In "$side.ipc"),'--locator-directory',(Path-In "$side.locators"))
        foreach ($c in $contexts) {$profileArguments+=@('--ordered-context',$c)}
        [void](Invoke-Cli $profileArguments)
        $processes += Start-Runtime $side
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (-not ((Test-Path -LiteralPath (Path-In 'one.ipc')) -and (Test-Path -LiteralPath (Path-In 'two.ipc')))) {
        if ([DateTime]::UtcNow -ge $deadline -or @($processes | Where-Object HasExited).Count) { throw 'Runtime IPC startup failed.' }
        Start-Sleep -Milliseconds 100
    }
    [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($a.side).ipc"),'--context',$context,'--locator-file',(Path-In "$($b.side).locators/$context.locator")))
    [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($b.side).ipc"),'--context',$context,'--locator-file',(Path-In "$($a.side).locators/$context.locator")))
    if ($contexts.Count -eq 2) {
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($a.side).ipc"),'--context',$context2,'--locator-file',(Path-In "$($b.side).locators/$context2.locator")))
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($b.side).ipc"),'--context',$context2,'--locator-file',(Path-In "$($a.side).locators/$context2.locator")))
    }
    if ($Scenario -eq 'UnsupportedSuite') {
        Start-Sleep -Seconds 2
        foreach($side in @('one','two')) {
            $status=Get-Status $side
            if($status.contexts[0].phase -ne 0 -or (Test-Path -LiteralPath (Path-In "$side.state/ratchet/od1-contexts/$context/native"))) {throw 'Unsupported suite created native state or silently downgraded.'}
        }
        [pscustomobject]@{status='PASS';scope='actual TLS and Device-signed unsupported suite; no Offer/Grant/native state';context=$context;pids=@($processes.Id);exe_sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()} | ConvertTo-Json | Set-Content -LiteralPath (Path-In 'result.json')
        Get-Content -LiteralPath (Path-In 'result.json')
        return
    }
    if ($earlyCrash) {
        Wait-KillPoint $crashVictim
        Restart-Side $crashVictim {
            Set-Schedule $crashVictim @{}
            $inspection=Invoke-Cli @('ordered','inspect','--state-dir',(Path-In "$crashVictim.state")) | ConvertFrom-Json
            $inspection | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Path-In 'crash-head-after-recovery.json')
            $stage=$inspection.contexts[0]
            $phase=if($KillPoint -in @('after-offer','before-init-commit')) {1}elseif($KillPoint -in @('after-grant','before-confirm-commit')) {2}else{3}
            if ($stage.phase -ne $phase) {throw 'Wrong durable crash phase.'}
            if ($phase -eq 1 -and ($null -ne $stage.d -or $null -ne $stage.e -or $null -ne $stage.sid)) {throw 'OfferedA depended on future IDs.'}
            if ($phase -eq 2 -and ($null -eq $stage.d -or $null -eq $stage.e -or $null -ne $stage.sid)) {throw 'GrantedB depended on a future SID.'}
        }
    }
    $activeA = Wait-State $a.side { param($s) $s.phase -eq 3 -and $s.receive_next -eq 1 }
    $activeB = Wait-State $b.side { param($s) $s.phase -eq 3 -and $s.receive_next -eq 1 }
    if ($activeA.contexts[0].session_id -cne $activeB.contexts[0].session_id) { throw 'SID mismatch.' }
    $setupSeconds = $watch.Elapsed.TotalSeconds
    $expected = @{}
    if($Scenario -in @('MissingAuth','CrossContext')) {
        $tamper=if($Scenario -eq 'MissingAuth'){'missing-auth'}else{'cross-context'}
        Set-Schedule $a.side @{context=$context;tamper_event=$tamper}
        Set-Schedule $b.side @{context=$context;suspend_prefix_requests=$true}
        Queue-Test $a 1
        $deadline=[DateTime]::UtcNow.AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 50
            $rejected=@(Get-ChildItem -LiteralPath $run -Filter "$($b.side).*.stderr.log" | Select-String -Pattern 'requested=1 saved=0 invalid=true').Count -gt 0
        } while(-not $rejected -and [DateTime]::UtcNow -lt $deadline)
        if(-not $rejected){throw 'Invalid signed data was not rejected by actual ingress.'}
        $sa=Get-Status $a.side;$sb=Get-Status $b.side
        if($sa.contexts[0].unacked -ne 1 -or $sb.contexts[0].receive_next -ne 1 -or $sb.contexts[0].pending -ne 0){throw 'Invalid signed data advanced readability/ACK/native state.'}
        Set-Schedule $a.side @{};Set-Schedule $b.side @{}
        [void](Wait-State $b.side {param($s) $s.receive_next -eq 2 -and $s.pending -eq 0})
        [void](Wait-State $a.side {param($s) $s.unacked -eq 0})
    } elseif ($Scenario -eq 'Conflict') {
        Queue-Test $a 1
        [void](Wait-State $b.side {param($s) $s.receive_next -eq 2})
        [void](Wait-State $a.side {param($s) $s.unacked -eq 0})
        $nativePath=Path-In "$($b.side).state/ratchet/od1-contexts/$context/native"
        $nativeHash=(Get-FileHash -LiteralPath $nativePath -Algorithm SHA256).Hash
        Set-Schedule $a.side @{context=$context;conflicting_offer=$true}
        [void](Invoke-Cli @('ordered','locator-export','--refresh','--ipc-file',(Path-In "$($a.side).ipc"),'--context',$context,'--output-file',(Path-In 'a.conflict-reconnect.locator')))
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($b.side).ipc"),'--context',$context,'--locator-file',(Path-In 'a.conflict-reconnect.locator')))
        $fork=Path-In "$($b.side).state/runtime/od1/contexts/$context/blocked-fork"
        $deadline=[DateTime]::UtcNow.AddSeconds(20)
        while(-not(Test-Path -LiteralPath $fork)){if([DateTime]::UtcNow -ge $deadline){throw 'Conflicting signed Offer did not leave durable fork evidence.'};Start-Sleep -Milliseconds 50}
        if((Get-FileHash -LiteralPath $nativePath -Algorithm SHA256).Hash -cne $nativeHash){throw 'Conflicting Offer changed native state.'}
        Restart-Side $b.side
        $denied=& $exe ordered queue --ipc-file (Path-In "$($b.side).ipc") --context $context --request-id '00000000000000010000000000000002' --text 'must-not-commit' 2>&1
        $exitCode=$LASTEXITCODE
        Add-Content -LiteralPath $trace -Value "fork_block_queue_exit=$exitCode $denied"
        if($exitCode -eq 0 -or "$denied" -notmatch 'signed fork'){throw 'Fork block did not survive restart.'}
    } elseif ($Scenario -eq 'ConsentExpiry') {
        Queue-Test $a 1
        [void](Wait-State $b.side {param($s) $s.receive_next -eq 2})
        [void](Wait-State $a.side {param($s) $s.unacked -eq 0})
        $nativePath=Path-In "$($a.side).state/ratchet/od1-contexts/$context/native"
        $nativeHash=(Get-FileHash -LiteralPath $nativePath -Algorithm SHA256).Hash
        Set-Clock 2592001
        $deniedOutput=& $exe ordered queue --ipc-file (Path-In "$($a.side).ipc") --context $context --request-id '00000000000000020000000000000002' --text 'must-not-commit' 2>&1
        $deniedExit=$LASTEXITCODE
        Add-Content -LiteralPath $trace -Value "expired_consent_queue_exit=$deniedExit $deniedOutput"
        if ($deniedExit -eq 0 -or "$deniedOutput" -notmatch 'consent interval') {throw 'Expired consent did not block the writer.'}
        if ((Get-FileHash -LiteralPath $nativePath -Algorithm SHA256).Hash -cne $nativeHash) {throw 'Expired consent advanced native state.'}
    } elseif ($Scenario -eq 'Expiry') {
        $initialPath=Path-In "$($a.side).state/runtime/od1/contexts/$context/initial-pair"
        $currentPath=Path-In "$($a.side).state/runtime/od1/contexts/$context/current-auth"
        $initialHash=(Get-FileHash -LiteralPath $initialPath -Algorithm SHA256).Hash
        $currentHash=(Get-FileHash -LiteralPath $currentPath -Algorithm SHA256).Hash
        Set-Clock 241
        $deadline=[DateTime]::UtcNow.AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 100
            $fresh=(Get-FileHash -LiteralPath $currentPath -Algorithm SHA256).Hash -cne $currentHash
        } while (-not $fresh -and [DateTime]::UtcNow -lt $deadline)
        if (-not $fresh) {throw 'A did not reauthenticate at its 240s lease timer.'}
        if ((Get-FileHash -LiteralPath $initialPath -Algorithm SHA256).Hash -cne $initialHash) {throw 'Fresh channel auth replaced the selected initial pair.'}
        Queue-Test $b 1
        $leaseCheck=Wait-State $a.side {param($s) $s.receive_next -eq 2 -and $s.send_next -eq 1}
        if ($leaseCheck.contexts[0].session_id -cne $activeA.contexts[0].session_id) {throw 'Reauth reset the native session.'}
        [void](Wait-State $b.side {param($s) $s.unacked -eq 0})
        Set-Schedule $b.side @{context=$context;hold_lane=1;hold_ordinals=@(2)}
        Queue-Test $b 2
        [void](Wait-State $b.side {param($s) $s.send_next -eq 3 -and $s.unacked -eq 1})
        Set-Clock 3601
        Start-Sleep -Milliseconds 500
        $blocked=Get-Status $a.side
        if ($blocked.contexts[0].receive_next -ne 2) {throw 'Expired locator dispatched held ciphertext.'}
        foreach ($side in @('one','two')) {
            [void](Invoke-Cli @('ordered','locator-export','--refresh','--ipc-file',(Path-In "$side.ipc"),'--context',$context,'--output-file',(Path-In "$side.refreshed.locator")))
        }
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($a.side).ipc"),'--context',$context,'--locator-file',(Path-In "$($b.side).refreshed.locator")))
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($b.side).ipc"),'--context',$context,'--locator-file',(Path-In "$($a.side).refreshed.locator")))
        Set-Schedule $b.side @{}
        [void](Wait-State $a.side {param($s) $s.receive_next -eq 3 -and $s.send_next -eq 1})
        [void](Wait-State $b.side {param($s) $s.unacked -eq 0})
        [void](Invoke-Cli @('device-revoke','--account-dir',(Path-In "$($b.side).account"),'--device-id',$b.device,'--revocation-file',(Path-In 'revocation')))
        [void](Invoke-Cli @('account-snapshot','--account-dir',(Path-In "$($b.side).account"),'--snapshot-file',(Path-In 'revoked.snapshot')))
        foreach ($side in @('one','two')) {[void](Invoke-Cli @('ordered','authority-import','--ipc-file',(Path-In "$side.ipc"),'--snapshot-file',(Path-In 'revoked.snapshot')))}
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($a.side).ipc"),'--context',$context,'--locator-file',(Path-In "$($b.side).refreshed.locator")))
        [void](Invoke-Cli @('ordered','locator-import','--ipc-file',(Path-In "$($b.side).ipc"),'--context',$context,'--locator-file',(Path-In "$($a.side).refreshed.locator")))
        $deniedOutput=& $exe ordered queue --ipc-file (Path-In "$($b.side).ipc") --context $context --request-id '00000000000000010000000000000003' --text 'must-not-commit' 2>&1
        $deniedExit=$LASTEXITCODE
        Add-Content -LiteralPath $trace -Value "revoked_queue_exit=$deniedExit $deniedOutput"
        if ($deniedExit -eq 0) {throw 'Revocation did not invalidate the cached policy.'}
        # Locators cannot remove revocation. Old readable history remains readable.
    } elseif ($Scenario -eq 'Crash') {
        if (-not $earlyCrash) {
            if ($KillPoint -eq 'readable-primary') {[IO.File]::WriteAllText((Path-In "$crashVictim.primary"),'readable-primary')}
            else {Set-Schedule $crashVictim @{pause_point=$KillPoint}}
            $request='00000000000000020000000000000001';$text="OD1/$($a.side)/1/Привет";$expected[$request]=$text
            $queueProcess=Start-Process -FilePath $exe -ArgumentList @('ordered','queue','--ipc-file',(Path-In "$($a.side).ipc"),'--context',$context,'--request-id',$request,'--text',$text) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Path-In 'crash-queue.stdout.log') -RedirectStandardError (Path-In 'crash-queue.stderr.log')
            Wait-KillPoint $crashVictim
            Restart-Side $crashVictim {Set-Schedule $crashVictim @{};[IO.File]::WriteAllText((Path-In "$crashVictim.primary"),'off')}
            $queueProcess.WaitForExit()
            Queue-Test $a 1
            [void](Wait-State $b.side {param($s) $s.receive_next -eq 2 -and $s.pending -eq 0})
            [void](Wait-State $a.side {param($s) $s.send_next -eq 2 -and $s.unacked -eq 0})
        } else {
            Queue-Test $a 1;Queue-Test $b 1
            [void](Wait-State $b.side {param($s) $s.receive_next -eq 2 -and $s.unacked -eq 0})
            [void](Wait-State $a.side {param($s) $s.receive_next -eq 2 -and $s.unacked -eq 0})
        }
    } elseif ($Scenario -eq 'TwoContexts') {
        Queue-Test $a 1
        [void](Wait-State $b.side {param($s) $s.receive_next -eq 2})
        Set-Schedule $a.side @{context=$context;hold_lane=0;hold_ordinals=@(2);delay_cursor_updates=$true}
        Set-Schedule $b.side @{context=$context;suspend_prefix_requests=$true}
        Queue-Test $a 2;Queue-Test $a 3
        [void](Wait-State $b.side {param($s) $s.pending -eq 1 -and $s.receive_next -eq 2})
        Queue-Test $a 101 $context2 $false;Queue-Test $b 101 $context2 $false
        $deadline=[DateTime]::UtcNow.AddSeconds(40)
        do {
            $sa=Get-Status $a.side;$sb=Get-Status $b.side
            if ($sa.contexts[1].receive_next -eq 2 -and $sb.contexts[1].receive_next -eq 2 -and $sa.contexts[1].unacked -eq 0 -and $sb.contexts[1].unacked -eq 0) {break}
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($sa.contexts[1].receive_next -ne 2 -or $sb.contexts[1].receive_next -ne 2) {throw 'C1 gap blocked independent C2.'}
        if ($sb.contexts[0].receive_next -ne 2 -or $sb.contexts[0].pending -ne 1) {throw 'C1 gap disappeared without its missing predecessor.'}
        if ($sa.contexts[0].session_id -ceq $sa.contexts[1].session_id) {throw 'Two contexts shared a native session.'}
        foreach ($side in @('one','two')) {
            $page=Invoke-Cli @('ordered','history','--ipc-file',(Path-In "$side.ipc"),'--context',$context2,'--offset','0','--limit','8') | ConvertFrom-Json
            if ($page.total -ne 2 -or @($page.items | Where-Object {$_.author_sequence -le 1}).Count) {throw 'Context2 confused device sequence with lane ordinal.'}
            $page | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Path-In "$side.context2.history.json")
        }
        Set-Schedule $a.side @{};Set-Schedule $b.side @{}
        [void](Wait-State $b.side {param($s) $s.receive_next -eq 4 -and $s.pending -eq 0})
        [void](Wait-State $a.side {param($s) $s.unacked -eq 0})
    } elseif ($Scenario -eq 'BOnly') {
        Queue-Test $b 1
        [void](Wait-State $a.side {param($s) $s.receive_next -eq 2 -and $s.send_next -eq 1})
        [void](Wait-State $b.side {param($s) $s.unacked -eq 0})
        Restart-Side $a.side {Queue-Test $b 2}
        $reopened=Wait-State $a.side {param($s) $s.receive_next -eq 3 -and $s.send_next -eq 1}
        if ($reopened.contexts[0].session_id -cne $activeA.contexts[0].session_id) {throw 'Relaunch replaced the selected epoch/session.'}
        [void](Wait-State $b.side {param($s) $s.unacked -eq 0 -and $s.send_next -eq 3})
    } elseif ($Scenario -ne 'Duplex') {
        Queue-Test $a 1
        [void](Wait-State $b.side {param($s) $s.receive_next -eq 2})
        Set-Schedule $a.side @{context=$context;hold_lane=0;hold_ordinals=@(2);delay_cursor_updates=$true;trace_rpc=$true}
        Set-Schedule $b.side @{context=$context;suspend_prefix_requests=$true}
        if ($Scenario -eq 'Gap') {
            Set-Schedule $b.side @{context=$context;hold_lane=1;hold_ordinals=@(1);suspend_prefix_requests=$true}
            Queue-Test $b 1
            [void](Wait-State $b.side {param($s) $s.send_next -eq 2})
            foreach ($i in 2..65) {Queue-Test $a $i}
            [void](Wait-State $b.side {param($s) $s.pending -eq 63 -and $s.receive_next -eq 2})
            Set-Schedule $b.side @{context=$context;suspend_prefix_requests=$true}
            [void](Wait-State $a.side {param($s) $s.receive_next -eq 2})
            Queue-Test $a 66
            $full=Wait-State $b.side {param($s) $s.pending -eq 64 -and $s.receive_next -eq 2}
            if ($full.contexts[0].peer_generation -ne 1) {throw 'Successor chain decrypted before the old tail.'}
            $target=67
        } else {
            foreach ($i in 2..49) {Queue-Test $a $i}
            [void](Wait-State $b.side {param($s) $s.pending -eq 47 -and $s.receive_next -eq 2})
            Set-Schedule $a.side @{context=$context;hold_lane=0;hold_ordinals=@(2)+(50..81);direct_batch=32;delay_cursor_updates=$true}
            foreach ($i in 50..81) {Queue-Test $a $i}
            [void](Wait-State $a.side {param($s) $s.send_next -eq 82})
            Set-Schedule $a.side @{context=$context;hold_lane=0;hold_ordinals=@(2);direct_batch=32;delay_cursor_updates=$true}
            $full=Wait-State $b.side {param($s) $s.pending -eq 64 -and $s.receive_next -eq 2}
            $target=82
        }
        $full | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Path-In 'full64-before-kill.json')
        Restart-Side $b.side
        [void](Wait-State $b.side {param($s) $s.pending -eq 64 -and $s.receive_next -eq 2})
        Set-Schedule $a.side @{trace_rpc=$true}
        Set-Schedule $b.side @{trace_rpc=$true}
        [void](Wait-State $b.side {param($s) $s.pending -eq 0 -and $s.receive_next -eq $target})
        [void](Wait-State $a.side {param($s) $s.unacked -eq 0})
    }
    if ($Scenario -eq 'Duplex') {
    for ($offset=0; $offset -lt $MessagesPerLane; $offset += $Burst) {
        foreach ($sender in @($a,$b)) {
            $receiver = if ($sender.side -ceq $a.side) { $b } else { $a }
            $end = [Math]::Min($offset+$Burst,$MessagesPerLane)
            for ($i=$offset; $i -lt $end; $i++) {
                Queue-Test $sender ($i+1)
            }
            [void](Wait-State $receiver.side { param($s) $s.receive_next -eq ($end+1) })
            [void](Wait-State $sender.side { param($s) $s.unacked -eq 0 -and $s.send_next -eq ($end+1) })
        }
    }
    }
    $exchangeSeconds=$watch.Elapsed.TotalSeconds-$setupSeconds
    $historyStarted=$watch.Elapsed.TotalSeconds
    $history = @{}
    foreach ($side in @('one','two')) {
        $items = @()
        for ($offset=0; $offset -lt $expected.Count; $offset+=8) {
            $page = Invoke-Cli @('ordered','history','--ipc-file',(Path-In "$side.ipc"),'--context',$context,'--offset',"$offset",'--limit','8') | ConvertFrom-Json
            if ($page.total -ne $expected.Count) { throw 'Wrong readable history count.' }
            $items += $page.items
        }
        foreach ($item in $items) { if ($expected[$item.request_id] -cne $item.text) { throw 'Wrong plaintext/request mapping.' } }
        if (@($items.request_id | Sort-Object -Unique).Count -ne $expected.Count) { throw 'Repeated or missing logical request.' }
        $history[$side] = $items
        $items | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Path-In "$side.history.json")
    }
    if (@(Compare-Object @($history.one.event_id | Sort-Object) @($history.two.event_id | Sort-Object)).Count) { throw 'Different immutable event history.' }
    foreach ($path in $immutableBefore.Keys) {
        if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $immutableBefore[$path]) {throw "Immutable bytes changed after process kill/reopen: $path"}
    }
    $timings = @($queueTimes | Sort-Object)
    $commitRows=@(Get-ChildItem -LiteralPath $run -Filter '*.stderr.log' | ForEach-Object {Get-Content -LiteralPath $_.FullName} | ForEach-Object {
        if ($_ -match 'od1_commit .*delta_bytes=(\d+) microseconds=(\d+) native=(true|false)') {
            [pscustomobject]@{delta_bytes=[uint64]$Matches[1];milliseconds=([double]$Matches[2]/1000);native=($Matches[3] -ceq 'true')}
        }
    })
    $commitTimes=@($commitRows.milliseconds | Sort-Object)
    $commitSummary=if ($commitTimes.Count) {@{count=$commitTimes.Count;native_count=@($commitRows | Where-Object native).Count;delta_total_bytes=($commitRows.delta_bytes | Measure-Object -Sum).Sum;delta_max_bytes=($commitRows.delta_bytes | Measure-Object -Maximum).Maximum;p50_ms=$commitTimes[[int][Math]::Floor(($commitTimes.Count-1)*0.50)];p95_ms=$commitTimes[[int][Math]::Floor(($commitTimes.Count-1)*0.95)]}}else{@{count=0}}
    $result = [pscustomobject]@{
        status='PASS'; scope="real CLI/runtime TLS $Scenario; scheduler delays only committed serialized records";
        exe=$exe; exe_sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant();
        pids=@($processes.Id); context=$context; elected_a=$a; elected_b=$b;
        text_count=$expected.Count; setup_seconds=$setupSeconds; total_seconds=$watch.Elapsed.TotalSeconds;
        exchange_seconds=$exchangeSeconds;history_seconds=$watch.Elapsed.TotalSeconds-$historyStarted;vault_commits=$commitSummary;
        vault_file_bytes=@((Get-Item -LiteralPath (Path-In 'one.state/state-vault.redb')).Length,(Get-Item -LiteralPath (Path-In 'two.state/state-vault.redb')).Length);
        crash_point=$KillPoint; immutable_reopen_records_checked=$immutableBefore.Count;
        queue_p50_ms=$timings[[int][Math]::Floor(($timings.Count-1)*0.50)];queue_p95_ms=$timings[[int][Math]::Floor(($timings.Count-1)*0.95)];
        final_one=(Get-Status 'one'); final_two=(Get-Status 'two')
    }
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Path-In 'result.json')
    $result | ConvertTo-Json -Depth 8
}
finally {
    foreach ($p in $processes) { if (-not $p.HasExited) { Stop-Process -Id $p.Id; $p.WaitForExit() } }
    Write-Host "process_evidence_directory=$run"
}
