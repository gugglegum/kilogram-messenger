param(
    [Parameter(Mandatory)][string]$ProbeRoot,
    [Parameter(Mandatory)][string]$ImageRoot
)

# Invoke in a dedicated, noninteractive Windows PowerShell process.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$watch = [Diagnostics.Stopwatch]::StartNew()
$stage = 'preflight'
$report = [ordered]@{ ok = $false; stages = [Collections.Generic.List[object]]::new() }
$job = $null

function Read-Json([string]$Path) {
    if ((Get-Item -LiteralPath $Path).Length -gt 65536) { throw 'Fixture JSON exceeds bound' }
    [IO.File]::ReadAllText($Path) | ConvertFrom-Json
}

function To-Id([byte[]]$Bytes) {
    if ($Bytes.Length -ne 32) { throw 'Invalid public ID width' }
    [BitConverter]::ToString($Bytes).Replace('-','').ToLowerInvariant()
}

function Start-Owned([string]$File, [string[]]$Arguments, [switch]$Discard) {
    $job.CheckWork()
    if ($Arguments -match '[\s"]') { throw 'This fixture requires whitespace-free typed arguments' }
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $File
    $info.Arguments = $Arguments -join ' '
    $info.WorkingDirectory = $ProbeRoot
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($info)
    $job.VerifyChild($process)
    $process.StandardInput.Close()
    $output = if ($Discard) { $process.StandardOutput.BaseStream.CopyToAsync([IO.Stream]::Null) }
              else { $process.StandardOutput.ReadToEndAsync() }
    $errors = $process.StandardError.BaseStream.CopyToAsync([IO.Stream]::Null)
    [pscustomobject]@{ Process = $process; Output = $output; Errors = $errors }
}

function Wait-Owned($Child, [int]$Milliseconds) {
    $process = $Child.Process
    if (-not $process.WaitForExit($Milliseconds) -or
        -not $Child.Output.Wait(1500) -or -not $Child.Errors.Wait(1500)) {
        throw 'Owned process or EOF timeout'
    }
    $process.ExitCode
}

function Invoke-Owned([string]$File, [string[]]$Arguments, [int]$Timeout = 10000,
                      [switch]$Discard, [switch]$AllowFailure) {
    $child = Start-Owned $File $Arguments -Discard:$Discard
    try {
        $code = Wait-Owned $child $Timeout
        if ($code -ne 0 -and -not $AllowFailure) {
            $report.native_exit = $code
            throw 'Owned command failed'
        }
        [pscustomobject]@{ Code = $code; Text = if (-not $Discard) { $child.Output.Result } }
    } finally { $child.Process.Dispose() }
}

function Status($Node, [switch]$AllowFailure) {
    Invoke-Owned $cli @('ordered','status2','--ipc-file',$Node.Ipc) -Timeout 6000 -AllowFailure:$AllowFailure
}

function Check-Descriptor($Node) {
    $content = (Read-Json $Node.Ipc).content
    if ((To-Id $content.account_id) -cne $Node.Account -or (To-Id $content.device_id) -cne $Node.Device -or
        $content.address -cnotmatch '^127\.0\.0\.1:[1-9][0-9]{0,4}$') {
        throw 'Scoped descriptor binding'
    }
}

function Check-Status($Node, $Child, $Status) {
    if ($Status.version -ne 2 -or $Status.runtime_pid -ne $Child.Process.Id -or
        $Status.runtime_instance -cnotmatch '^[0-9a-f]{64}$' -or $Status.contexts.Count -ne 2 -or
        $Status.route.policy -cne 'direct-only' -or $Status.route.bind -cne '127.0.0.1:0' -or $Status.route.relay) {
        throw 'Scoped runtime status binding'
    }
    foreach ($slot in 0,1) {
        $row = $Status.contexts[$slot]
        if ($row.slot -ne $slot -or $row.context -cne $contexts[$slot] -or -not $row.details.current_consent) {
            throw 'Context or consent binding'
        }
    }
}

function Start-Runtime($Node) {
    if (Test-Path -LiteralPath $Node.Ipc) { throw 'Unexpected existing descriptor' }
    $child = Start-Owned $cli @('runtime-from-profile','--profile-file',$Node.Profile) -Discard
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    do {
        $job.CheckWork()
        if ($child.Process.HasExited) {
            $report.native_exit = $child.Process.ExitCode
            throw 'Runtime stopped before descriptor'
        }
        if (Test-Path -LiteralPath $Node.Ipc) {
            Check-Descriptor $Node
            $probe = Status $Node -AllowFailure
            if ($probe.Code -eq 0) {
                $value = ConvertFrom-Json $probe.Text
                Check-Status $Node $child $value
                return [pscustomobject]@{ Child=$child; Instance=$value.runtime_instance }
            }
        }
        Start-Sleep -Milliseconds 100
    } while ($deadline.ElapsedMilliseconds -lt 30000)
    throw 'Runtime readiness deadline'
}

function Read-Histories($Node) {
    foreach ($slot in 0,1) {
        $response = Invoke-Owned $cli @('ordered','history2','--ipc-file',$Node.Ipc,'--slot',"$slot",'--limit','4')
        $page = ConvertFrom-Json $response.Text
        if ($page.version -ne 2 -or -not $page.finished -or $page.items.Count -ne 0 -or
            $page.F.Count -ne 2 -or $page.view -cnotmatch '^[0-9a-f]{64}$') {
            throw 'Expected authenticated empty history'
        }
        $page.view
    }
}

function Stop-Runtime($Node, $Runtime) {
    $response = Invoke-Owned $cli @('ordered','shutdown','--ipc-file',$Node.Ipc)
    $code = Wait-Owned $Runtime.Child 15000
    if ($response.Text.Trim() -cne 'status=shutdown-accepted' -or $code -ne 0 -or
        (Test-Path -LiteralPath $Node.Ipc)) {
        $report.native_exit = $code
        throw 'Shutdown not confirmed'
    }
    $Runtime.Child.Process.Dispose()
    $report.stages.Add(@{ stage=$stage; ack=$true; exit=0; eof=$true; descriptor_removed=$true })
}

try {
    if ((Split-Path -Leaf $ProbeRoot) -cnotmatch '^terminal-t1-0b-[a-z0-9-]+$' -or
        -not (Test-Path -LiteralPath $ProbeRoot -PathType Container)) { throw 'Expected isolated probe root' }
    $cli = Join-Path $ImageRoot 'kilogram-cli.exe'
    $bootstrap = Join-Path $ImageRoot 'kilogram-bootstrap.exe'
    if ((Get-FileHash -LiteralPath $cli).Hash -ine '125838275f2ed3f7258e88a924a4c5499472b2efc3b935941a2cb386aae7e54f' -or
        (Get-FileHash -LiteralPath $bootstrap).Hash -ine 'baa3de3dec6b4fa53421c1dd3c54f429a44425240423c9bbbed521e9d3abecb4') {
        throw 'Unaccepted executable'
    }
    $report.stages.Add(@{ stage=$stage; image_hashes_match=$true })
    $stage = 'watchdog'
    $deadlineSource = Join-Path $ProbeRoot 'deadline.ps1'
    . ([scriptblock]::Create([IO.File]::ReadAllText($deadlineSource)))
    $job = [KilogramS5Deadline]::new(180,210)

    $stage = 'fixture'
    $fixture = Join-Path $ProbeRoot 'fixture'
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try {
        $contexts = @(foreach ($slot in 0,1) {
            $id = [byte[]]::new(32)
            $rng.GetBytes($id)
            To-Id $id
        })
    } finally { $rng.Dispose() }
    $nodes = @(foreach ($name in 'a','b') {
        $stage = "bootstrap-$name"
        $workspacePath = Join-Path $fixture $name
        $null = Invoke-Owned $bootstrap @('create-ordered2','--workspace-dir',$workspacePath,'--context',$contexts[0],$contexts[1]) -Timeout 25000 -Discard
        $ready = Read-Json (Join-Path $workspacePath 'bootstrap-ready.json')
        $public = $ready.public
        if ($ready.status -cne 'ready-runtime6' -or $public.root_key_protection -cne 'windows-dpapi-current-user' -or
            ($public.contexts -join ',') -cne ($contexts -join ',')) { throw 'Unprotected or unrelated bootstrap' }
        if ($public.claim -cnotmatch '^[0-9a-f]{64}$') { throw 'Invalid bootstrap claim binding' }
        [pscustomobject]@{
            Name=$name; Root=(Join-Path $workspacePath 'account-root'); State=(Join-Path $workspacePath 'device')
            Public=(Join-Path $workspacePath 'public'); Account=(To-Id $public.account_id)
            Device=(To-Id $public.device_id); Claim=$public.claim
            Profile=(Join-Path $workspacePath 'profile.json'); Ipc=(Join-Path $workspacePath 'runtime.ipc.json'); Locators=(Join-Path $workspacePath 'locators')
        }
        $report.stages.Add(@{ stage=$stage; root_protection='windows-dpapi-current-user'; exit=0 })
    })
    if ($nodes[0].Account -ceq $nodes[1].Account -or $nodes[0].Device -ceq $nodes[1].Device) {
        throw 'Fixture roles must be distinct'
    }
    if ([StringComparer]::Ordinal.Compare($nodes[0].Device,$nodes[1].Device) -lt 0) {
        $proposer,$signer = $nodes
    } else { $signer,$proposer = $nodes }
    $files = @('--credentials-file',(Join-Path $proposer.Public 'credential.bin'),(Join-Path $signer.Public 'credential.bin'),
               '--minimum-authority-file',(Join-Path $proposer.Public 'authority.snapshot'),(Join-Path $signer.Public 'authority.snapshot'))
    $clock = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()

    foreach ($slot in 0,1) {
        $context = $contexts[$slot]
        $proposal = Join-Path $fixture "slot$slot.proposal"
        $final = Join-Path $fixture "slot$slot.context"
        $stage = "propose-$slot"
        $null = Invoke-Owned $cli (@('ordered','context2-propose','--account-dir',$proposer.Root,
            '--expect-account',$proposer.Account,'--expect-peer-account',$signer.Account,
            '--expect-device',$proposer.Device,'--expect-peer-device',$signer.Device,'--context',$context,
            '--not-before',"$clock",'--valid-for-seconds','3600','--output-file',$proposal) + $files)
        $stage = "cosign-$slot"
        $null = Invoke-Owned $cli (@('ordered','context2-cosign','--account-dir',$signer.Root,
            '--expect-account',$signer.Account,'--expect-peer-account',$proposer.Account,
            '--expect-device',$signer.Device,'--expect-peer-device',$proposer.Device,'--context',$context,
            '--proposal-file',$proposal,'--output-file',$final) + $files)
        foreach ($node in $nodes) {
            $stage = "import-$($node.Name)-$slot"
            $null = Invoke-Owned $cli @('ordered','context2-import','--state-dir',$node.State,
                '--expect-account',$node.Account,'--expect-claim',$node.Claim,'--invitation-file',$final)
        }
        $report.stages.Add(@{ stage="consent-$slot"; two_installs=$true })
    }

    foreach ($node in $nodes) {
        $stage = "profile-$($node.Name)"
        $null = Invoke-Owned $cli @('runtime-profile-create','--profile-mode','ordered-dialog-v2',
            '--state-dir',$node.State,'--profile-file',$node.Profile,'--expect-account',$node.Account,
            '--expect-device',$node.Device,'--expect-claim',$node.Claim,'--ordered-context',$contexts[0],
            '--ordered-context',$contexts[1],'--ipc-file',$node.Ipc,'--locator-directory',$node.Locators,
            '--route-policy','direct-only','--ordered-bind','127.0.0.1:0')
    }

    $runs = @(foreach ($node in $nodes) {
        $stage = "start-$($node.Name)"
        Start-Runtime $node
        $report.stages.Add(@{ stage=$stage; authenticated=$true; descriptor_loopback=$true; two_slots=$true })
    })
    foreach ($index in 0,1) {
        $stage = "locators-$index"
        $node = $nodes[$index]
        $peer = $nodes[1-$index]
        foreach ($slot in 0,1) {
            $locator = Join-Path $peer.Locators "$($contexts[$slot]).locator"
            $null = Invoke-Owned $cli @('ordered','locator2-import','--ipc-file',$node.Ipc,
                '--slot',"$slot",'--locator-file',$locator)
        }
    }
    $stage = 'active'
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    do {
        $active = $true
        foreach ($index in 0,1) {
            $value = ConvertFrom-Json (Status $nodes[$index]).Text
            Check-Status $nodes[$index] $runs[$index].Child $value
            if (@($value.contexts | Where-Object { $_.phase -ne 3 -or -not $_.active }).Count) { $active = $false }
        }
        if (-not $active) { Start-Sleep -Milliseconds 250 }
    } while (-not $active -and $deadline.ElapsedMilliseconds -lt 30000)
    if (-not $active) { throw 'Active2 deadline' }
    $report.stages.Add(@{ stage=$stage; both_nodes_two_slots=$true })
    $stage = 'history'
    $before = @(Read-Histories $nodes[0])
    $null = Read-Histories $nodes[1]
    if ($before[0] -ceq $before[1]) { throw 'Expected distinct context views' }
    $report.stages.Add(@{ stage=$stage; complete_empty_pages=4; distinct_a_views=$true })
    foreach ($index in 0,1) {
        $stage = "shutdown-$index"
        Stop-Runtime $nodes[$index] $runs[$index]
    }

    $stage = 'missing-descriptor'
    $missing = Status $nodes[0] -AllowFailure
    if ($missing.Code -eq 0) { throw 'Stopped descriptor unexpectedly worked' }
    $report.stages.Add(@{ stage=$stage; rejected=$true; exit=$missing.Code })
    $stage = 'restart'
    $restart = Start-Runtime $nodes[0]
    if ($restart.Instance -ceq $runs[0].Instance) { throw 'Expected new runtime instance' }
    $after = @(Read-Histories $nodes[0])
    if (($before -join ',') -cne ($after -join ',')) { throw 'History binding changed on restart' }
    $report.stages.Add(@{ stage=$stage; fresh_instance=$true; history_views_retained=$true; complete_empty_pages=2 })
    $stage = 'shutdown-restart'
    Stop-Runtime $nodes[0] $restart
    $report.ok = $true
} catch {
    $report.failure_stage = $stage
    $report.error_type = $_.Exception.GetBaseException().GetType().Name
    $report.error_line = $_.InvocationInfo.ScriptLineNumber
    if ($_.Exception.GetBaseException() -is [ComponentModel.Win32Exception]) {
        $report.win32_error = $_.Exception.GetBaseException().NativeErrorCode
    }
} finally {
    if ($null -ne $job) {
        try {
            if ($report.ok) {
                $grace = [Diagnostics.Stopwatch]::StartNew()
                while ($job.Children().Length -and $grace.ElapsedMilliseconds -lt 2000) {
                    Start-Sleep -Milliseconds 25
                }
            }
            $report.work_expired = $job.WorkExpired
            $report.children_before_cleanup = $job.Children().Length
            if ($report.children_before_cleanup) {
                $report.ok = $false
                $job.CleanupChildren(15000)
            }
            $report.children_after_cleanup = $job.Children().Length
            $job.Finish()
        } catch {
            $report.ok = $false
            $report.cleanup_failed = $true
        }
    }
    $report.elapsed_ms = $watch.ElapsedMilliseconds
    [Console]::WriteLine(($report | ConvertTo-Json -Depth 6 -Compress))
}
if ($report.ok) { exit 0 }
exit 1
