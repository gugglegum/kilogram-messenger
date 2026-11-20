param([Parameter(Mandatory)][string]$Request)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$req = ConvertFrom-Json -InputObject $Request
$base = [IO.Path]::GetFullPath($req.base)
if ($req.attempt -cnotmatch '^[a-z0-9][a-z0-9-]{0,30}$') { throw 'Invalid attempt ID' }
$run = Join-Path (Join-Path $base 'runs') $req.attempt
$images = Join-Path $base 'bin'
$setup = Join-Path $run 'setup'

function Assert-PlainPath([string]$Path) {
    $item = Get-Item -LiteralPath $Path
    while ($item) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse point in pilot path' }
        $item = if ($item -is [IO.FileInfo]) { $item.Directory } else { $item.Parent }
    }
}

function Write-NewBytes([string]$Path, [byte[]]$Bytes) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew)
    try { $stream.Write($Bytes, 0, $Bytes.Length); $stream.Flush($true) }
    finally { $stream.Dispose() }
}

function Write-NewJson([string]$Path, $Value) {
    $json = ConvertTo-Json -InputObject $Value -Depth 12 -Compress
    Write-NewBytes $Path ([Text.Encoding]::UTF8.GetBytes($json))
}

function Read-Public([string]$Path) {
    Assert-PlainPath $Path
    if ((Get-Item -LiteralPath $Path).Length -gt 524288) { throw 'Public artifact size' }
    $bytes = [IO.File]::ReadAllBytes($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $hash = [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
    @{sha256=$hash; data=[Convert]::ToBase64String($bytes)} | ConvertTo-Json -Compress
}

function Check-Image([string]$Name) {
    $path = Join-Path $images $Name
    Assert-PlainPath $path
    $pin = $req.pins.PSObject.Properties[$Name].Value
    if ($pin -cnotmatch '^[0-9a-f]{64}$' -or (Get-FileHash -LiteralPath $path).Hash -ine $pin) {
        throw "SHA-256 mismatch for $Name"
    }
    return $path
}

# Create a run directory only after its own exclusive marker was created.
Assert-PlainPath $base
function Assert-LabAttempt([switch]$ReadCompleted) {
    # Existing ordinary-agent protocol only. Never create/adopt an arbitrary run.
    foreach($name in @('agent_session','agent_generation')){if($req.$name -cnotmatch '^[0-9a-f]{32}$'){throw 'Lab attempt identity'}}
    if($req.worker_pid -isnot [int] -or $req.worker_pid -le 0 -or $req.worker_started -isnot [long] -or $req.worker_started -le 0){throw 'Lab worker identity'}
    $control=Join-Path $base 'ordinary-agent/control'
    foreach($p in @((Join-Path $run 'attempt.json'),(Join-Path $run 'started.json'),(Join-Path $control 'ready.json'),(Join-Path $control 'state.json'))){Assert-PlainPath $p}
    $entry=Get-Content -LiteralPath (Join-Path $run 'attempt.json') -Raw|ConvertFrom-Json
    $started=Get-Content -LiteralPath (Join-Path $run 'started.json') -Raw|ConvertFrom-Json
    $ready=Get-Content -LiteralPath (Join-Path $control 'ready.json') -Raw|ConvertFrom-Json
    $state=Get-Content -LiteralPath (Join-Path $control 'state.json') -Raw|ConvertFrom-Json
    if($entry.attempt -cne $req.attempt -or $entry.generation -cne $req.agent_generation -or $entry.session -cne $req.agent_session -or
       $ready.session -cne $entry.session -or $state.session -cne $entry.session -or
       (!$ReadCompleted -and ($state.active -cne $req.attempt -or $state.blocked)) -or
       $started.generation -cne $entry.generation -or $started.pid -ne $req.worker_pid -or $started.started -ne $req.worker_started){throw 'Lab attempt binding'}
    $keys=@($entry.pins.PSObject.Properties.Name|Sort-Object)
    if(($keys -join '|') -cne (@($req.pins.PSObject.Properties.Name|Sort-Object)-join '|')){throw 'Lab attempt pins'}
    foreach($name in $keys){if($entry.pins.$name -cne $req.pins.$name){throw 'Lab attempt pins'}}
    return @{entry=$entry;started=$started}
}
if($req.op -ceq 'lab-clock'){
    $bound=Assert-LabAttempt
    $clockSource=Check-Image 'deadline.ps1';. ([scriptblock]::Create([IO.File]::ReadAllText($clockSource)))
    @{attempt=$req.attempt;generation=$bound.entry.generation;session=$bound.entry.session;
      worker_pid=$bound.started.pid;worker_started=$bound.started.started;start_tick=$bound.entry.start_tick;
      work_tick=$bound.started.work_tick;whole_tick=$bound.started.whole_tick;tick=[KilogramS5Deadline]::TickNow();utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json -Compress
    return
}
if ($req.op -in @('init','lab-init')) {
    $lab=$req.op -ceq 'lab-init'
    if($lab){$null=Assert-LabAttempt}
    $requiredImages = if ($req.pins.PSObject.Properties['mailbox-fake.ps1']) {
        @('mailbox-fake.ps1','mailbox.cs','setup-pipe.cs','deadline.ps1')
    } else {
        $onboarding = if ($req.pins.PSObject.Properties['setup-pipe.cs']) { 'setup-pipe.cs' } else { 't11f-fixture.exe' }
        @('kilogram-cli.exe','kilogram-bootstrap.exe',$onboarding,'deadline.ps1')
        if ($req.pins.PSObject.Properties['mailbox.cs']) { 'mailbox.cs' }
    }
    foreach ($name in $requiredImages) {
        $null = Check-Image $name
    }
    $actualIp = [Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces() |
        Where-Object OperationalStatus -eq 'Up' | ForEach-Object { $_.GetIPProperties().UnicastAddresses } |
        Where-Object { $_.Address.ToString() -eq $req.ip }
    if (@($actualIp).Count -ne 1 -or $env:COMPUTERNAME -ine $req.hostname) { throw 'Unexpected host or IPv4' }
    if ($req.ssh_peer) {
        $source, $sourcePort, $server, $serverPort = $env:SSH_CONNECTION -split '\s+'
        if ($source -ne $req.ssh_peer -or $server -ne $req.ip -or $serverPort -ne '22') {
            throw 'SSH source or target changed'
        }
    }
    $runs = [IO.Directory]::CreateDirectory((Join-Path $base 'runs')).FullName
    Assert-PlainPath $runs
    Write-NewJson (Join-Path $runs "$($req.attempt).claim.json") @{host=$env:COMPUTERNAME; pins=$req.pins}
    if(!$lab){
        if ([IO.Directory]::Exists($run)) { throw 'Run directory already exists' }
        [void][IO.Directory]::CreateDirectory((Join-Path $run 'in'))
        [void][IO.Directory]::CreateDirectory((Join-Path $run 'receipts'))
    }
    @{host=$env:COMPUTERNAME; ip=$req.ip; pid=$PID} | ConvertTo-Json -Compress
    return
}

Assert-PlainPath $run
if($req.op -ceq 'lab-diagnostic-enable'){
    $bound=Assert-LabAttempt
    # Setup begin/join rejects every pre-existing foreign entry. Enable only AFTER
    # the real Setup has completed and its terminal owner has closed.
    Assert-PlainPath (Join-Path $setup 'setup.json')
    if(Test-Path -LiteralPath (Join-Path $setup 'connection/ipc.json')){throw 'Diagnostic enable requires preconnect state'}
    $diag=Join-Path $setup 'lab-diagnostics-v1'
    if(Test-Path -LiteralPath $diag){throw 'Fresh diagnostic directory required'}
    [void][IO.Directory]::CreateDirectory($diag);Assert-PlainPath $diag
    Write-NewJson (Join-Path $diag 'enabled.json') @{schema=1;run_nonce=$bound.entry.generation}
    @{schema=1;run_nonce=$bound.entry.generation;enabled=$true}|ConvertTo-Json -Compress
    return
}
if($req.op -cin @('lab-crash-descriptor','lab-runtime-crash')) {
    $bound=Assert-LabAttempt -ReadCompleted
    $common=Join-Path $base 'ordinary-agent/test-terminal-t1-3e-lab-common.ps1'
    . $common
    $ipc=Join-Path $setup 'connection/ipc.json'
    if($req.op -ceq 'lab-crash-descriptor') {
        Assert-PlainPath $ipc
        if((Get-Item -LiteralPath $ipc).Length -gt 65536){throw 'Descriptor cap'}
        $profiles=@(Get-ChildItem -LiteralPath (Join-Path $setup 'connection') -Filter 'profile-*.json')
        if($profiles.Count -ne 1){throw 'LAB_CRASH_PROFILE'}
        @{sha256=Lab-Hash $ipc;profile_name=$profiles[0].Name;profile_sha256=Lab-Hash $profiles[0].FullName}|ConvertTo-Json -Compress;return
    }
    $owners=@(Get-ChildItem -LiteralPath (Join-Path $run 'owners') -Filter '*.request.json'|ForEach-Object {Lab-Read $_.FullName}|Where-Object tag -CEQ 'owner-a-1')
    if($owners.Count -ne 1){throw 'LAB_CRASH_OWNER'}
    Assert-LabCrashIntent $req.fault $bound.entry $owners[0] (Lab-Read (Join-Path $base 'ordinary-agent/lab.json')) $bound.started
    $requestPath=Join-Path $run 'receipts/runtime-fault-request.json'
    $prior=Lab-Read $requestPath
    if($prior) {
        if(($prior|ConvertTo-Json -Depth 12 -Compress) -cne ($req.fault|ConvertTo-Json -Depth 12 -Compress)){throw 'LAB_CRASH_REPLAY'}
    } else {
        $null=Assert-LabAttempt
    }
    $null=Set-LabCrashRequest $run $req.fault
    $limit=[Diagnostics.Stopwatch]::StartNew()
    do {
        $faultResult=Lab-Read (Join-Path $run 'receipts/runtime-fault-result.json')
        if($faultResult) {
            if($faultResult.digest -cne (Lab-Hash $requestPath)){throw 'LAB_CRASH_REPLAY'}
            $faultResult|ConvertTo-Json -Depth 8 -Compress;return
        }
        if(Test-Path -LiteralPath (Join-Path $run 'receipts/runtime-fault-error.json')){throw 'LAB_CRASH_OUTCOME_UNKNOWN'}
        Start-Sleep -Milliseconds 60
    }while($limit.ElapsedMilliseconds -lt 9000)
    throw 'LAB_CRASH_REPLY_UNKNOWN'
}
if($req.op -ceq 'lab-diagnostic-read'){
    $bound=Assert-LabAttempt -ReadCompleted
    $dir=Join-Path $setup 'lab-diagnostics-v1';Assert-PlainPath $dir
    Assert-PlainPath (Join-Path $dir 'enabled.json')
    if((Get-Item -LiteralPath (Join-Path $dir 'enabled.json')).Length -gt 256){throw 'Diagnostic marker cap'}
    $marker=Get-Content -LiteralPath (Join-Path $dir 'enabled.json') -Raw|ConvertFrom-Json
    if((@($marker.PSObject.Properties.Name|Sort-Object)-join '|') -cne 'run_nonce|schema' -or $marker.schema -ne 1 -or $marker.run_nonce -cne $bound.entry.generation){throw 'Diagnostic marker binding'}
    $startupKinds=@('startup_setup_open','startup_begin','startup_import_begin','startup_import_end','startup_profile_begin','startup_prepared','startup_spawned','startup_descriptor','startup_status','startup_failed')
    $recordNamePattern='^(runtime-stopping|runtime-network-stopped|runtime-final|connection-final|worker-final|startup-(setup-open|begin|import-begin|import-end|profile-begin|prepared|spawned|descriptor|status|failed))-[1-9][0-9]*\.json$'
    $allowed=@('schema','kind','run_nonce','pid','runtime_pid','runtime_instance','data','enabled','elapsed_ms','ok','cancelled','spawned','counters','last_os_error_by_stage','slots','slot','stage',
        'attempts','errors','timeouts','leases','fresh_ok','fresh_errors','network_ok','network_join_ok','ipc_ok','owner_join_ok',
        'client_present','was_running','child_query_failed','ack_attempted','ack','descriptor_read_ok','conflict','forced','exit_success','exit','eof','drain_count','drain_eof','drain_failed',
        'running_ok','saved_ok','stop_graceful','stop_forced','reaped',
        'MetadataStart','MetadataStartError','MetadataReady','MetadataError','MetadataLate','MetadataRevoked','MetadataCleanupTimeout','MetadataCleanupError',
        'SocketOpen','SocketOpenError','SendAttempt','SendOk','SendError','ReceiveError','ReceivePacket','ReceiveStale','PeerFilterRejected','QuotaDrop','InboxQueued','InboxDrop',
        'AnnounceAttempt','AnnounceOk','AnnounceError','PublicationQueued','PublicationDrop','CaptureWithoutObservation','LocatorAttempt','LocatorOpened','LocatorValidated','LocatorDuplicate','LocatorCommitted','LocatorError',
        'ListenerIncoming','ListenerPeerRejected','ListenerTlsError','ListenerPrerequisiteRejected','ListenerActualTlsError','ListenerDeadline','InboundAdmissionError','InboundLease','SocketCancelError','SocketCleanupTimeout','SocketJoinError','SocketWorkerError',
        'discovery_rejections','Unknown','GenerationFreshness','ValidationQuota','LocalScope','Observation','FrameSize','RecipientKey','Context','PeerScope','Envelope','Decryption','SignedBody','Signature','AnnouncementFuture','AnnouncementExpired','AnnouncementDuration','Consent','LocatorParse','LocatorVerification','LocatorInterval','Route','LocatorPair','PostLocatorVerification','PostRoute','Revision','Commit',
        'EndpointBindError','EndpointBound','EndpointAttachError','EndpointCleanupError','TransportCleanupError','OwnerDetachError','SupervisorError','MetadataMonitorError','MetadataMonitorJoinError','OutboundAdmissionError','OutboundAdmissionTimeout')
    function Assert-DiagnosticValue($Value,[string]$Key,[int]$Depth){
        if($Depth -gt 8){throw 'Diagnostic nesting cap'}
        if($null -eq $Value -or $Value -is [bool]){return}
        if($Value -is [string]){
            if(($Key -ceq 'run_nonce' -and $Value -cmatch '^[0-9a-f]{32}$') -or ($Key -ceq 'runtime_instance' -and $Value -cmatch '^[0-9a-f]{64}$') -or
               ($Key -ceq 'kind' -and ($Value -cin $startupKinds -or $Value -cin @('runtime_stopping','runtime_network_stopped','runtime_final','connection_final','worker_final'))) -or
               ($Key -ceq 'stage' -and $Value -cin @('idle','role_check','not_dialer','waiting_target','peer_selection','connect','route_policy','begin_admission','hello','proof','admitted','authenticated_lease','fresh_protocol','serving','ended','unknown'))){return}
            throw 'Diagnostic string outside typed schema'
        }
        if($Value -is [ValueType]){if([double]::IsNaN([double]$Value) -or [double]::IsInfinity([double]$Value)){throw 'Diagnostic numeric cap'};return}
        if($Value -is [array]){if($Value.Count -gt 64){throw 'Diagnostic array cap'};foreach($item in $Value){Assert-DiagnosticValue $item $Key ($Depth+1)};return}
        if(@($Value.PSObject.Properties).Count -gt 64){throw 'Diagnostic object cap'}
        foreach($field in $Value.PSObject.Properties){if($field.Name -cnotin $allowed){throw 'Diagnostic unknown field'};Assert-DiagnosticValue $field.Value $field.Name ($Depth+1)}
    }
    # One recipient restart: keep the previous files, validate their exact scope,
    # and return only the requested incarnation. Unknown/malformed files still fail.
    $previous=@();if($null -ne $req.diagnostic_previous){$previous=@($req.diagnostic_previous)}
    if(@($previous).Count -gt 1){throw 'Diagnostic previous incarnation cap'}
    $seenPids=@($req.diagnostic_chat_pid,$req.diagnostic_runtime_pid)
    foreach($prior in $previous){
        if((@($prior.PSObject.Properties.Name|Sort-Object)-join '|') -cne 'diagnostic_chat_pid|diagnostic_runtime_instance|diagnostic_runtime_pid' -or
           $prior.diagnostic_runtime_instance -cnotmatch '^[0-9a-f]{64}$'){throw 'Diagnostic previous scope'}
        foreach($idValue in @($prior.diagnostic_chat_pid,$prior.diagnostic_runtime_pid)){
            if(($idValue -isnot [int] -and $idValue -isnot [long]) -or $idValue -le 0 -or $idValue -in $seenPids){throw 'Diagnostic previous PID scope'}
            $seenPids+=,$idValue
        }
    }
    $files=@(Get-ChildItem -LiteralPath $dir -File -Force|Where-Object Name -cne 'enabled.json')
    if($files.Count -gt 16*(1+@($previous).Count)){throw 'Diagnostic file count cap'}
    $records=@();$rejected=@();$priorValidated=0
    foreach($file in $files){
        try{
            if($file.Name -cnotmatch $recordNamePattern -or $file.Length -gt 16384){throw 'Diagnostic file cap/name'}
            Assert-PlainPath $file.FullName
            $bytes=[IO.File]::ReadAllBytes($file.FullName);$value=[Text.Encoding]::UTF8.GetString($bytes)|ConvertFrom-Json
            Assert-DiagnosticValue $value '' 0
            if((@($value.PSObject.Properties.Name|Sort-Object)-join '|') -cne 'data|kind|pid|run_nonce|runtime_instance|runtime_pid|schema'){throw 'Diagnostic envelope keys'}
            if($value.schema -ne 1 -or $value.run_nonce -cne $bound.entry.generation){throw 'Diagnostic nonce/schema binding'}
            $stem=$value.kind.Replace('_','-');if($file.Name -cne ($stem+'-'+$value.pid+'.json')){throw 'Diagnostic filename binding'}
            $expected=$req;$isPrevious=$false
            foreach($prior in $previous){
                if(($value.kind -clike 'runtime_*' -and $value.pid -eq $prior.diagnostic_runtime_pid) -or
                   ($value.kind -cnotlike 'runtime_*' -and $value.pid -eq $prior.diagnostic_chat_pid)){$expected=$prior;$isPrevious=$true}
            }
            if($value.kind -clike 'runtime_*'){
                if($value.pid -ne $expected.diagnostic_runtime_pid -or $value.runtime_pid -ne $expected.diagnostic_runtime_pid -or $value.runtime_instance -cne $expected.diagnostic_runtime_instance){throw 'Diagnostic runtime binding'}
            }elseif($value.kind -cin $startupKinds){
                if($value.pid -ne $expected.diagnostic_chat_pid){throw 'Diagnostic startup Chat binding'}
                if($null -ne $value.runtime_pid -and (($value.runtime_pid -isnot [int] -and $value.runtime_pid -isnot [long]) -or $value.runtime_pid -le 0)){throw 'Diagnostic startup runtime PID type'}
                if($null -ne $value.runtime_instance -and ($null -eq $value.runtime_pid -or $value.kind -cnotin @('startup_status','startup_failed'))){throw 'Premature startup instance'}
                if($null -ne $value.runtime_pid -and $expected.diagnostic_runtime_pid -and $value.runtime_pid -ne $expected.diagnostic_runtime_pid){throw 'Startup runtime PID binding'}
                if($null -ne $value.runtime_instance -and $expected.diagnostic_runtime_instance -and $value.runtime_instance -cne $expected.diagnostic_runtime_instance){throw 'Startup runtime instance binding'}
            }else{
                if($value.pid -ne $expected.diagnostic_chat_pid){throw 'Diagnostic Chat binding'}
                if($expected.diagnostic_runtime_pid -and ($value.runtime_pid -ne $expected.diagnostic_runtime_pid -or $value.runtime_instance -cne $expected.diagnostic_runtime_instance)){throw 'Diagnostic Chat runtime binding'}
            }
            if($isPrevious){$priorValidated++;continue}
            $sha=[Security.Cryptography.SHA256]::Create();try{$hash=([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
            $records+=@{name=$file.Name;bytes=$bytes.Length;sha256=$hash;data=[Convert]::ToBase64String($bytes)}
        }catch{$rejected+=@{name=if($file.Name -cmatch $recordNamePattern){$file.Name}else{'invalid_name'};reason='schema_or_identity_rejected'}}
    }
    $reply=@{schema=1;records=$records;rejected=$rejected;run_nonce=$bound.entry.generation}
    if(@($previous).Count){$reply.prior_validated=$priorValidated}
    $reply|ConvertTo-Json -Depth 6 -Compress
    return
}
$mailboxMode = $req.PSObject.Properties['generation']
if ($mailboxMode) {
    if ($req.tag -cnotmatch '^[a-z0-9-]{1,40}$' -or $req.generation -cnotmatch '^[0-9a-f]{32}$') {
        throw 'Invalid mailbox tag or generation'
    }
    $receipts = Join-Path $run 'receipts'
    Assert-PlainPath $receipts
    $mailboxPath = Join-Path $receipts "mailbox-$($req.tag)"
    $mailboxSource = Check-Image 'mailbox.cs'
    Add-Type -TypeDefinition ([IO.File]::ReadAllText($mailboxSource)) -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'
    if ($req.op -eq 'mailbox') {
        $inputReader = if ($req.action -eq 'submit') { [Console]::In } else { [IO.TextReader]::Null }
        try {
            [Console]::WriteLine([KilogramTestMailbox]::Exchange($mailboxPath, $req.generation,
                $req.action, $req.sequence, $req.digest, $req.cursor, $inputReader))
        } catch { throw 'Mailbox exchange rejected' }
        return
    }
}
switch ($req.op) {
    'put' {
        if ($req.name -cnotmatch '^[a-z0-9-]{1,40}\.(json|kilogram)$') { throw 'Unexpected input filename' }
        Assert-PlainPath (Join-Path $run 'in')
        $line = [Console]::ReadLine()
        if (!$line -or $line.Length -gt 699052) { throw 'Public input size' }
        $path = Join-Path (Join-Path $run 'in') $req.name
        $bytes = [Convert]::FromBase64String($line)
        if ($bytes.Length -gt 524288) { throw 'Public input size' }
        Write-NewBytes $path $bytes
        $actual = (Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant()
        if ($actual -cne $req.sha256) { throw 'Received public artifact hash mismatch' }
        @{sha256=$actual; path=$path} | ConvertTo-Json -Compress
        return
    }
    'get' {
        if ($req.name -cmatch '^(invite|reply|proposal|final)\.kilogram$') {
            Read-Public (Join-Path (Join-Path $setup 'exchange') $req.name)
        } elseif ($req.name -cmatch '^public-[0-9a-f]{64}\.json$') {
            Read-Public (Join-Path (Join-Path $setup 'connection') $req.name)
        } else { throw 'Unexpected public export filename' }
        return
    }
    'receipt' {
        if ($req.tag -cnotmatch '^[a-z0-9-]{1,40}$') { throw 'Unexpected receipt name' }
        $path = Join-Path (Join-Path $run 'receipts') "$($req.tag).json"
        Assert-PlainPath $path
        [Console]::WriteLine([IO.File]::ReadAllText($path))
        return
    }
    'socket' {
        if ($req.tag -cnotmatch '^[a-z0-9-]{1,40}$') { throw 'Unexpected socket receipt name' }
        $path = Join-Path (Join-Path $run 'receipts') "$($req.tag)-socket.json"
        if (![IO.File]::Exists($path)) { '{"observed":false}'; return }
        Assert-PlainPath $path
        [Console]::WriteLine([IO.File]::ReadAllText($path))
        return
    }
    'descriptors' {
        @{descriptor=[IO.File]::Exists((Join-Path $setup 'connection/ipc.json'))} | ConvertTo-Json -Compress
        return
    }
}

if ($req.tag -cnotmatch '^[a-z0-9-]{1,40}$') { throw 'Unexpected process tag' }
$log = Join-Path (Join-Path $run 'receipts') "$($req.tag).json"
if (Test-Path -LiteralPath $log) { throw 'Process receipt already exists' }
Assert-PlainPath (Join-Path $run 'receipts')
$deadline = Check-Image 'deadline.ps1'
. ([scriptblock]::Create([IO.File]::ReadAllText($deadline)))
$start = [KilogramS5Deadline]::TickNow()
$watchdog = $null
$child = $null
$verified = $false
$forced = $false
$exitCode = $null
$errorText = $null
$before = @()
$after = @()
$socket = $null
$protectedSetup = $req.op -eq 'setup'
$filteredEof = $false
$mailbox = $null
$mailboxEof = $false
$mailboxReader = $null
$mailboxWriter = $null
$mailboxErrorReader = $null
$fake = $mailboxMode -and $req.op -ceq 'mailbox-fake'
$fakeProtected = $fake -and $req.fake_mode -cin @('valid','early','malformed','oversized','truncated','eof','timeout','wrong-id','wrong-scope','raw')
$protectedSetup = $protectedSetup -or $fakeProtected
$observeSocket = $req.op -eq 'chat' -and $null -ne $req.PSObject.Properties['observe_socket'] -and $req.observe_socket
$faultEnabled=$null -ne $req.PSObject.Properties['runtime_crash'] -and $req.runtime_crash -eq $true
$faultTarget=$null;$faultObserved=$false
if($faultEnabled) {
    if(!$mailboxMode -or $req.op -cne 'chat' -or $req.tag -cne 'owner-a-1'){throw 'LAB_CRASH_SCOPE'}
    . (Join-Path $base 'ordinary-agent/test-terminal-t1-3e-lab-common.ps1')
}
try {
    if ($fake) {
        if (!$fakeProtected -and $req.fake_mode -cnotin @('echo','disconnect')) { throw 'Unknown fake child case' }
        $fakeFile = Check-Image 'mailbox-fake.ps1'
        $exe = "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
        $literal = $fakeFile.Replace("'", "''")
        $source = "& ([scriptblock]::Create([IO.File]::ReadAllText('$literal'))) '$($req.fake_mode)'"
        $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
        $argv = @('-NoProfile','-NonInteractive','-EncodedCommand',$encoded)
        $work = if ($req.fake_mode -cin @('disconnect','timeout')) { 18 } else { 115 }
        $watchdog = [KilogramS5Deadline]::new($work, ($work + 12), $start)
        if ($fakeProtected) {
            $bridge = Check-Image 'setup-pipe.cs'
            Add-Type -TypeDefinition ([IO.File]::ReadAllText($bridge)) -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'
        }
    } elseif ($req.op -eq 'chat') {
        $exe = Check-Image 'kilogram-cli.exe'
        $argv = @('chat', '--jsonl', '--setup', $setup)
        $watchdog = [KilogramS5Deadline]::new(760, 790, $start)
    } elseif ($protectedSetup) {
        $exe = Check-Image 'kilogram-cli.exe'
        $null = Check-Image 'kilogram-bootstrap.exe'
        $bridge = Check-Image 'setup-pipe.cs'
        Add-Type -TypeDefinition ([IO.File]::ReadAllText($bridge)) -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'
        $argv = @('setup', '--jsonl', '--dir', $setup, '--allow-recovery-output')
        $watchdog = [KilogramS5Deadline]::new(450, 480, $start)
    } elseif ($req.op -eq 'fixture') {
        $exe = Check-Image 't11f-fixture.exe'
        $null = Check-Image 'kilogram-cli.exe'
        $null = Check-Image 'kilogram-bootstrap.exe'
        if ($req.action -cnotmatch '^(invite|join|reply|propose|cosign|import)$') { throw 'Unexpected action' }
        $argv = @($req.action, $setup, $images)
        if ($req.input) {
            if ($req.input -cnotmatch '^[a-z0-9-]{1,40}\.kilogram$') { throw 'Unexpected fixture input' }
            $path = Join-Path (Join-Path $run 'in') $req.input
            Assert-PlainPath $path
            $argv += $path
        }
        if ($req.action -eq 'import') { $argv += @($req.ip, $req.peer) }
        $watchdog = [KilogramS5Deadline]::new(105, 135, $start)
    } else { throw 'Unexpected process operation' }
    if ($argv | Where-Object { $_ -match '["\r\n]' -or $_.EndsWith('\') }) { throw 'Unsafe process argument' }
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $exe
    $info.Arguments = ($argv | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $info.WorkingDirectory = $run
    $info.UseShellExecute = $false
    if ($mailboxMode) {
        $mailbox = [KilogramTestMailbox]::new($mailboxPath, $req.generation, $watchdog.WorkTick)
    }
    if ($protectedSetup) {
        [KilogramSetupPipe]::Redirect($info)
    } elseif ($mailbox) {
        $info.RedirectStandardInput = $true
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        $info.CreateNoWindow = $true
    }
    $child = [Diagnostics.Process]::Start($info)
    $watchdog.VerifyChild($child)
    $verified = $true
    $filter = if ($protectedSetup -and $mailbox) {
        [KilogramSetupPipe]::Run($child, $mailbox.SetupInput(), $mailbox.SetupOutput(), $setup)
    } elseif ($protectedSetup) {
        [KilogramSetupPipe]::Run($child, [Console]::OpenStandardInput(), [Console]::Out, $setup)
    }
    if ($mailbox -and !$protectedSetup) {
        $mailboxReader = $mailbox.ForwardOutput($child.StandardOutput.BaseStream)
        $mailboxWriter = $mailbox.FeedInput($child.StandardInput.BaseStream, $child)
        $mailboxErrorReader = $mailbox.EmptyError($child.StandardError.BaseStream)
    }
    $socketWatch = [Diagnostics.Stopwatch]::StartNew()
    while (!$child.WaitForExit(200)) {
        $watchdog.CheckWork()
        if ($mailbox -and $mailbox.Stopped) { throw 'Mailbox stopped' }
        if (($mailboxReader -and $mailboxReader.IsFaulted) -or
            ($mailboxWriter -and $mailboxWriter.IsFaulted) -or
            ($mailboxErrorReader -and $mailboxErrorReader.IsFaulted)) { throw 'Mailbox stream failure' }
        if ($protectedSetup -and $filter.IsFaulted) { throw 'Protected Setup exchange failed' }
        if ($observeSocket -and !$socket -and $socketWatch.ElapsedMilliseconds -ge 1000) {
            $socketWatch.Restart()
            $owners = @($watchdog.Children() | Where-Object { $_ -ne $child.Id })
            if ($owners.Count) {
                $address = [regex]::Escape($req.ip)
                $pattern = "^\s*UDP\s+${address}:(\d+)\s+\S+\s+(\d+)\s*$"
                $ownedPorts = @(& "$env:windir/System32/netstat.exe" -ano -p udp | ForEach-Object {
                    if ($_ -match $pattern -and [int]$Matches[2] -in $owners) {
                        @{observed=$true; ip=$req.ip; port=[int]$Matches[1]; pid=[int]$Matches[2]}
                    }
                })
                if ($ownedPorts.Count -gt 1) { throw 'Multiple owned LAN UDP ports' }
                if ($ownedPorts.Count -eq 1) {
                    $socket = $ownedPorts[0]
                    $runtime = Get-Process -Id $socket.pid
                    try {
                        $watchdog.VerifyChild($runtime)
                        if ($runtime.Path -ine $exe) { throw 'Owned runtime image differs' }
                        $socket.started = $runtime.StartTime.ToFileTimeUtc()
                        $socket.image = $runtime.Path
                        if($faultEnabled){$faultTarget=$watchdog.RetainCrashTarget($runtime,$socket.started,$socket.image,$child.Id)}
                    } finally { $runtime.Dispose() }
                    $socketPath = Join-Path (Join-Path $run 'receipts') "$($req.tag)-socket.json"
                    Write-NewJson "$socketPath.partial" $socket
                    [IO.File]::Move("$socketPath.partial", $socketPath)
                }
            }
        }
        if($faultEnabled -and $faultTarget -and !$faultObserved -and (Test-Path -LiteralPath (Join-Path $run 'receipts/runtime-fault-request.json'))) {
            $faultObserved=$true
            try {
                $entry=Lab-Read (Join-Path $run 'attempt.json');$started=Lab-Read (Join-Path $run 'started.json')
                $ownerEntry=@(Get-ChildItem -LiteralPath (Join-Path $run 'owners') -Filter '*.request.json'|ForEach-Object {Lab-Read $_.FullName}|Where-Object tag -CEQ $req.tag)
                if($ownerEntry.Count -ne 1){throw 'LAB_CRASH_OWNER'}
                $b=Get-LabCrashEvidence $run $entry $ownerEntry[0] (Lab-Read (Join-Path $base 'ordinary-agent/lab.json')) $started
                $f=$b.intent;$ipc=Join-Path $setup 'connection/ipc.json'
                if((Lab-Hash $ipc) -cne $f.descriptor_sha256 -or $f.runtime_pid -ne $socket.pid -or $f.runtime_started -ne $socket.started){throw 'LAB_CRASH_BINDING'}
                if((Lab-Hash (Join-Path $setup ('connection/'+$f.profile_name))) -cne $f.profile_sha256){throw 'LAB_CRASH_PROFILE'}
                $status=(& $exe ordered status2 --ipc-file $ipc)|ConvertFrom-Json
                if($LASTEXITCODE -ne 0 -or $status.runtime_pid -ne $f.runtime_pid -or $status.runtime_instance -cne $f.runtime_instance -or
                    ($status.contexts.context -join '|') -cne ($f.contexts -join '|')){throw 'LAB_CRASH_STATUS'}
                if((Lab-Hash $ipc) -cne $f.descriptor_sha256){throw 'LAB_CRASH_DESCRIPTOR'}
                Write-NewJson (Join-Path $run 'receipts/runtime-fault-claim.json') @{digest=$b.digest;owner_pid=$PID;tick=[KilogramS5Deadline]::TickNow()}
                $code=$faultTarget.Execute($b.digest,$f.runtime_pid,$f.runtime_started,$f.runtime_image)
                $watchdog.CheckWork()
                if((Lab-Hash $ipc) -cne $f.descriptor_sha256){throw 'LAB_CRASH_DESCRIPTOR'}
                Write-NewJson (Join-Path $run 'receipts/runtime-fault-result.json') @{request=$f.request;digest=$b.digest;
                    owner_pid=$PID;owner_started=(Get-Process -Id $PID).StartTime.ToFileTimeUtc();chat_pid=$child.Id;
                    runtime_pid=$faultTarget.Pid;runtime_started=$faultTarget.Started;runtime_image=$faultTarget.Image;
                    runtime_instance=$f.runtime_instance;exit=$code;observed_tick=[KilogramS5Deadline]::TickNow()}
            }catch{
                Write-NewJson (Join-Path $run 'receipts/runtime-fault-error.json') @{error=$_.Exception.Message;outcome_unknown=$true}
                throw
            }
        }
    }
    $watchdog.CheckWork()
    if ($protectedSetup) {
        if (!$filter.Wait(2000)) { throw 'Protected Setup did not drain' }
        $filteredEof = $true
        $mailboxEof = [bool]$mailbox
    } elseif ($mailbox) {
        if (!$mailboxReader.Wait(2000) -or !$mailboxErrorReader.Wait(2000) -or !$mailboxWriter.Wait(2000) -or
            !$mailboxReader.Result -or !$mailboxErrorReader.Result) { throw 'Mailbox streams did not drain cleanly' }
        $mailboxEof = $true
    }
    $exitCode = $child.ExitCode
    if ($mailbox -and !$mailbox.QuitAcknowledged) { throw 'Mailbox child exited without acknowledged quit' }
} catch {
    $errorText = if ($protectedSetup) { 'Protected Setup launch or exchange failed' }
        elseif ($mailboxMode) { 'Mailbox launch, stop or exchange failed' } else { $_.Exception.Message }
} finally {
    if ($watchdog) {
        # Let the terminal finish its own shutdown before classifying retained Job children.
        $grace = [Diagnostics.Stopwatch]::StartNew()
        $before = @($watchdog.Children())
        while ($before.Count -and $grace.ElapsedMilliseconds -lt 2000 -and !$watchdog.WorkExpired) {
            Start-Sleep -Milliseconds 50
            $before = @($watchdog.Children())
        }
        $forced = [bool]$before.Count -or $watchdog.WorkExpired
        if ($before.Count) { $watchdog.CleanupChildren(6000) }
        $after = @($watchdog.Children())
    }
    if (!$mailboxMode -or $mailbox) {
        Write-NewJson $log @{
            tag=$req.tag; host=$env:COMPUTERNAME; owner=$PID; child=if($child){$child.Id}else{$null}
            child_exit=$exitCode; membership=$verified; forced=$forced; before=$before; after=$after
            descriptor=[IO.File]::Exists((Join-Path $setup 'connection/ipc.json')); error=$errorText
            filtered_setup_eof=if($protectedSetup){$filteredEof}else{$null}
            mailbox_eof=if($mailboxMode){$mailboxEof}else{$null}
            mailbox_quit=if($mailbox){$mailbox.QuitAcknowledged}else{$null}
            work_expired=if($watchdog){$watchdog.WorkExpired}else{$null}
        }
        if ($mailbox) { $mailbox.Finish($verified -and !$forced -and !$errorText -and $exitCode -eq 0 -and $mailboxEof) }
    }
    if ($faultTarget) { $faultTarget.Dispose() }
    if ($watchdog) { $watchdog.Finish() }
}
if (!$verified -or $forced -or $errorText -or $exitCode -ne 0) { exit 1 }
