$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"

function Assert-PlainPath([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    while ($item) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Unexpected reparse point' }
        $item = if ($item -is [IO.FileInfo]) { $item.Directory } else { $item.Parent }
    }
}

function Publish-Json([string]$Path, $Value) {
    $temporary = "$Path.$([guid]::NewGuid().ToString('N')).pending"
    $bytes = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 5 -Compress) + "`n")
    $file = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew)
    try {
        try { $file.Write($bytes, 0, $bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
        [IO.File]::Move($temporary, $Path)
    } finally { [IO.File]::Delete($temporary) }
}

function Read-Request([string]$Path, [string]$Nonce) {
    Assert-PlainPath $Path
    $expected = '{"v":1,"op":"setup_smoke","nonce":"' + $Nonce + '"}' + "`n"
    $file = [IO.File]::OpenRead($Path)
    try {
        if ($file.Length -ne $expected.Length) { return 'malformed_request' }
        $reader = [IO.BinaryReader]::new($file)
        $bytes = $reader.ReadBytes($expected.Length)
        $text = [Text.Encoding]::ASCII.GetString($bytes)
    } finally { $file.Dispose() }
    if ($text -ceq $expected) { return $null }
    if ($text -cmatch '\A\{"v":1,"op":"setup_smoke","nonce":"[a-f0-9]{32}"\}\n\z') { return 'stale_nonce' }
    return 'malformed_request'
}

function Invoke-FixedSetup($Images, [string]$Setup, [string]$Folder, $Receipt) {
    $job = $null
    $child = $null
    $inputStream = $null
    $filtered = $null
    $phase = 'job_start'
    try {
        . ([scriptblock]::Create([IO.File]::ReadAllText($Images['deadline.ps1'])))
        $job = [KilogramS5Deadline]::new(90, 110, [KilogramS5Deadline]::TickNow())
        $phase = 'bridge_start'
        Add-Type -TypeDefinition ([IO.File]::ReadAllText($Images['setup-pipe.cs'])) `
            -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'
        $job.CheckWork()

        $phase = 'child_start'
        $info = [Diagnostics.ProcessStartInfo]::new()
        $info.FileName = $Images['kilogram-cli.exe']
        $info.Arguments = 'setup --jsonl --dir "' + $Setup + '"'
        $info.WorkingDirectory = $Folder
        [KilogramSetupPipe]::Redirect($info)
        $child = [Diagnostics.Process]::Start($info)
        $Receipt.child_started = $true
        $job.VerifyChild($child)
        $Receipt.job_member = $true

        $phase = 'protected_exchange'
        $ops = @('begin','create','status','quit')
        $commands = foreach ($name in $ops) { '{"v":1,"id":"' + $name + '","op":"' + $name + '","args":{}}' }
        $inputStream = [IO.MemoryStream]::new([Text.Encoding]::UTF8.GetBytes(($commands -join "`n") + "`n"))
        $filtered = [IO.StringWriter]::new()
        $exchange = [KilogramSetupPipe]::Run($child, $inputStream, $filtered, $Setup)
        while (!$exchange.IsCompleted -or !$child.HasExited) {
            $job.CheckWork()
            if ($exchange.IsFaulted) { throw 'Protected exchange rejected' }
            Start-Sleep -Milliseconds 30
        }
        $job.CheckWork()
        $exchange.GetAwaiter().GetResult()
        $Receipt.filtered_eof = $true
        $Receipt.child_exit = $child.ExitCode

        $phase = 'public_result'
        $frames = @($filtered.ToString().TrimEnd() -split '\r?\n' | ForEach-Object { ConvertFrom-Json -InputObject $_ })
        if ($frames.Count -ne 5 -or $frames[0].type -cne 'ready') { throw 'Unexpected Setup response count' }
        for ($i = 0; $i -lt $ops.Count; $i++) {
            if ($frames[$i + 1].id -cne $ops[$i]) { throw 'Unexpected Setup response tag' }
        }
        $begin, $create, $status, $quit = $frames[1..4]
        $Receipt.begin_ok = $begin.ok -and $begin.value.state -ceq 'prepared'
        $Receipt.quit_ok = $quit.ok -and $quit.value.closed -eq $true
        if (!$create.ok) {
            $Receipt.create_code = if ($create.code -cin @('helper_unavailable','helper_failed','helper_timeout',
                    'cleanup_unconfirmed','cancelled','setup_invalid','invalid_state','not_open','invalid_request')) {
                $create.code
            } else { 'other_public_error' }
        } else {
            $identity = $create.value
            $Receipt.create_ok = $identity.state -ceq 'recovery_available' -and
                $identity.account_id -cmatch '\A[0-9a-f]{64}\z' -and $identity.device_id -cmatch '\A[0-9a-f]{64}\z' -and
                @($identity.contexts).Count -eq 2
            if ($status.ok -and $Receipt.create_ok) {
                $current = $status.value
                $Receipt.status_ok = $current.state -ceq $identity.state -and
                    $current.account_id -ceq $identity.account_id -and $current.device_id -ceq $identity.device_id -and
                    ($current.contexts -join ',') -ceq ($identity.contexts -join ',')
            }
        }
        if (!$Receipt.begin_ok -or !$Receipt.create_ok -or !$Receipt.status_ok -or !$Receipt.quit_ok -or
            $Receipt.child_exit -ne 0) { throw 'Setup smoke did not complete' }
    } catch {
        $Receipt.error = $phase
    } finally {
        if ($job) {
            try {
                $grace = [Diagnostics.Stopwatch]::StartNew()
                while (@($job.Children()).Count -and !$job.WorkExpired -and $grace.ElapsedMilliseconds -lt 1500) {
                    Start-Sleep -Milliseconds 30
                }
                $Receipt.work_expired = $job.WorkExpired
                $Receipt.forced_cleanup = $Receipt.work_expired -or @($job.Children()).Count -ne 0
                if (@($job.Children()).Count) { $job.CleanupChildren(6000) }
                $Receipt.job_empty = @($job.Children()).Count -eq 0
                if ($child -and $child.HasExited) { $Receipt.child_exit = $child.ExitCode }
                $job.Finish()
            } catch { $Receipt.error = 'job_cleanup' }
        }
        if ($child) { $child.Dispose() }
        if ($inputStream) { $inputStream.Dispose() }
        if ($filtered) { $filtered.Dispose() }
    }
    if ($Receipt.forced_cleanup -and !$Receipt.error) { $Receipt.error = 'forced_cleanup' }
}

function Invoke-InteractiveSmoke {
    $folder = [IO.Path]::GetDirectoryName($env:KILOGRAM_SETUP_SMOKE_ENTRY)
    $expectedFolder = 'C:\Users\test\KilogramLab\terminal-setup-interactive-smoke-01'
    $who = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        if ($folder -ine $expectedFolder -or [Environment]::MachineName -cne 'PAUL_HP_OMEN' -or
            $who.User.Value -cne 'S-1-5-21-2671144489-3236106012-1032288181-1006' -or $env:SSH_CONNECTION -or
            ([Security.Principal.WindowsPrincipal]::new($who)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'Unexpected user session'
        }
        Assert-PlainPath $folder
        $pins = [ordered]@{
            'kilogram-cli.exe' = '736827d50c3cfb4f34282b6651dccc82f25131c4c86a8594105771a38cda1ac2'
            'kilogram-bootstrap.exe' = 'dac789937cdb1743db49face9a5df23ddad89c33e649658ae33ad95ce45a9e7a'
            'setup-pipe.cs' = 'a738983b332563b186a91e65a0880303989d4d2048568c94ef006a94e8a6183c'
            'deadline.ps1' = '4cbac92301c2420ee83b64f41db66f751dc00cf80d4ec7bf98e091a76d8642b5'
            'dpapi-context.ps1' = 'dedc8bb4d57fe1572bfa5620492d778b783769c66aa70a26c57d17c19a987a1d'
        }
        $images = @{}
        $imageLocks = @()
        try {
            foreach ($name in $pins.Keys) {
                $path = Join-Path (Join-Path $folder 'bin') $name
                Assert-PlainPath $path
                $lock = [IO.File]::OpenRead($path)
                $imageLocks += ,$lock
                if ((Get-FileHash -InputStream $lock).Hash -ine $pins[$name]) { throw 'Pinned file SHA differs' }
                $images[$name] = $path
            }
            # Compile the pinned probe's logon helper. Its file/DPAPI entry point is never run.
            $source = [IO.File]::ReadAllText($images['dpapi-context.ps1'])
            $start = $source.IndexOf("using System;")
            $end = $source.IndexOf("'@", $start)
            Add-Type -TypeDefinition ($source.Substring($start, $end - $start))
            $logon = [TerminalT1Dpapi]::ReadLogonContext()
            if ($logon[0] -ne 2 -or $logon[1] -eq 0 -or
                !(Test-Path -LiteralPath "Registry::HKEY_USERS\$($who.User.Value)")) { throw 'Interactive user required' }

            $setup = Join-Path $folder 'setup'
            $ready = Join-Path $folder 'ready.json'
            $request = Join-Path $folder 'request.json'
            $accepted = Join-Path $folder 'request.claimed.json'
            $result = Join-Path $folder 'result.json'
            $claim = Join-Path $folder 'launcher.claim'
            try { ([IO.File]::Open($claim, [IO.FileMode]::CreateNew)).Dispose() }
            catch {
                if (!([IO.File]::Exists($claim))) { throw }
                [Console]::WriteLine('This Setup smoke was already opened. Its files were kept.')
                return 2
            }
            $nonce = [guid]::NewGuid().ToString('N')
            $receipt = [ordered]@{
                v=1; nonce=$nonce; outcome='failed'; error=$null; create_code=$null
                begin_ok=$false; create_ok=$false; status_ok=$false; quit_ok=$false
                child_started=$false; child_exit=$null; job_member=$false; job_empty=$false
                filtered_eof=$false; work_expired=$false; forced_cleanup=$false
            }
            try {
                if (@($setup,$ready,$request,$accepted,$result | Where-Object { Test-Path -LiteralPath $_ }).Count) {
                    $receipt.error = 'stale_files'
                } else {
                    $wait = [Diagnostics.Stopwatch]::StartNew()
                    Publish-Json $ready ([ordered]@{
                        v=1; nonce=$nonce; host=[Environment]::MachineName; sid_verified=$true; logon_type=2
                        session_zero=$false; administrative=$false; profile_hive_loaded=$true; pins=$pins
                    })
                    [Console]::WriteLine('Setup smoke is ready. Leave this window open for the controller.')
                    while (![IO.File]::Exists($request) -and $wait.ElapsedMilliseconds -lt 300000) {
                        Start-Sleep -Milliseconds 100
                    }
                    if ($wait.ElapsedMilliseconds -ge 300000) {
                        $receipt.error = 'mailbox_timeout'
                    } else {
                        Assert-PlainPath $request
                        [IO.File]::Move($request, $accepted)
                        $receipt.error = Read-Request $accepted $nonce
                        if (!$receipt.error) {
                            if (Test-Path -LiteralPath $setup) { $receipt.error = 'setup_exists' }
                            else { Invoke-FixedSetup $images $setup $folder $receipt }
                        }
                    }
                }
            } catch { if (!$receipt.error) { $receipt.error = 'mailbox_rejected' } }
            if (!$receipt.error -and $receipt.job_empty -and $receipt.quit_ok -and !$receipt.forced_cleanup) {
                $receipt.outcome = 'passed'
            }
            Publish-Json $result $receipt
            [Console]::WriteLine("Setup smoke $($receipt.outcome). The controller will collect its result.")
            if ($receipt.outcome -ceq 'passed') { return 0 }
            return 1
        } finally { foreach ($lock in $imageLocks) { $lock.Dispose() } }
    } finally { $who.Dispose() }
}

try { return (Invoke-InteractiveSmoke) }
catch { [Console]::WriteLine('Setup smoke could not start or save its result.'); return 3 }
