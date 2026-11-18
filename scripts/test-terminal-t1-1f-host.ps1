param([Parameter(Mandatory)][string]$Request)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
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
if ($req.op -eq 'init') {
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
    if ([IO.Directory]::Exists($run)) { throw 'Run directory already exists' }
    [void][IO.Directory]::CreateDirectory((Join-Path $run 'in'))
    [void][IO.Directory]::CreateDirectory((Join-Path $run 'receipts'))
    @{host=$env:COMPUTERNAME; ip=$req.ip; pid=$PID} | ConvertTo-Json -Compress
    return
}

Assert-PlainPath $run
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
                    } finally { $runtime.Dispose() }
                    $socketPath = Join-Path (Join-Path $run 'receipts') "$($req.tag)-socket.json"
                    Write-NewJson "$socketPath.partial" $socket
                    [IO.File]::Move("$socketPath.partial", $socketPath)
                }
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
    if ($watchdog) { $watchdog.Finish() }
}
if (!$verified -or $forced -or $errorText -or $exitCode -ne 0) { exit 1 }
