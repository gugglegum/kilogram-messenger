param([string]$PinnedCli, [string]$FreshSetup)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $PWD 'scripts/test-terminal-t1-2e-setup-pipe.cs'))) `
    -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'

$auto = '{"mode":"auto","adapter_id":"' + ('a' * 32) + '","network_id":"' + ('b' * 32) + '","prefix":24}'
$manual = '{"mode":"manual"}'
$routeArgs = '{"local_ipv4":"192.168.4.9","peer_ipv4":"192.168.4.10"}'
$fake = @'
$ErrorActionPreference = 'Stop'
[Console]::Error.WriteLine('private_marker')
[Console]::WriteLine('{"v":1,"type":"ready"}')
$line = [Console]::ReadLine()
if (__REJECT__) {
    if ($null -ne $line) { exit 2 }
    exit 0
}
$request = $line | ConvertFrom-Json
if ($request.v -ne 1 -or $request.id -cne 'route' -or $request.op -cne 'route_lan_auto' -or
    @($request.args.PSObject.Properties).Count -ne 2 -or
    $request.args.local_ipv4 -cne '192.168.4.9' -or $request.args.peer_ipv4 -cne '192.168.4.10') {
    exit 2
}
[Console]::WriteLine('{"v":1,"type":"result","id":"route","ok":true,"value":{"state":"ready","account_id":null,"device_id":null,"contexts":null,"private":"private_marker","connection":{"mode":"__ROUTE__","local_ipv4":"192.168.4.9","peer_ipv4":__PEER__,"profile_state":"current","private":"private_marker"__DISCOVERY__}}}')
$line = [Console]::ReadLine()
if ($null -eq $line) { exit 0 }
if (($line | ConvertFrom-Json).op -cne 'quit') { exit 2 }
[Console]::WriteLine('{"v":1,"type":"result","id":"quit","ok":true,"value":{"closed":true}}')
'@

function Invoke-BridgeCase($Name, $Info, $Op, $RequestArgs, [bool]$Accepted, $Discovery, $ExpectedError) {
    [KilogramSetupPipe]::Redirect($Info)
    $protocol = '{"v":1,"id":"route","op":"' + $Op + '","args":' + $RequestArgs + '}' + "`n" +
        '{"v":1,"id":"quit","op":"quit","args":{}}' + "`n"
    $bytes = [Text.Encoding]::UTF8.GetBytes($protocol)
    $inputStream = New-Object IO.MemoryStream -ArgumentList @(,$bytes)
    $captured = New-Object IO.StringWriter
    $child = [Diagnostics.Process]::Start($Info)
    try {
        $bridge = [KilogramSetupPipe]::Run($child,$inputStream,$captured,$PWD.ProviderPath)
        $fault = $false
        try { $ended = $bridge.Wait(8000) }
        catch { $ended = $true; $fault = $bridge.IsFaulted }
        if (!$ended -or ($fault -eq $Accepted)) { throw "Unexpected bridge outcome for $Name" }
        $child.StandardInput.Close()
        if (!$child.WaitForExit(3000) -or $child.ExitCode -ne 0) { throw "Unexpected child outcome for $Name" }
        $result = $captured.ToString()
        if ($result -match 'private_marker') { throw "Private fixture data escaped for $Name" }
        $frames = @($result.Trim() -split "`n")
        if ($frames[0] -notmatch '"type":"ready"') { throw "Missing ready frame for $Name" }
        if (!$Accepted) {
            if ($frames.Count -ne 1) { throw "Rejected frame escaped for $Name" }
        } else {
            if ($frames.Count -ne 3 -or $frames[2] -notmatch '"closed":true') { throw "Missing completion for $Name" }
            $reply = $frames[1] | ConvertFrom-Json
            if ($ExpectedError) {
                if ($reply.ok -or $reply.code -cne $ExpectedError) { throw "Unexpected public CLI response for $Name" }
            } else {
                if (!$reply.ok) { throw "Missing success for $Name" }
                $actual = $reply.value.connection.lan_discovery
                if (($Discovery -and ($actual | ConvertTo-Json -Compress) -cne $Discovery) -or (!$Discovery -and $null -ne $actual)) {
                    throw "Unexpected discovery projection for $Name"
                }
            }
        }
        Write-Output "protected_setup_auto=$Name passed"
    } finally {
        if (!$child.HasExited) { $child.Kill(); if (!$child.WaitForExit(3000)) { throw 'Fixture child exit was not observed' } }
        $child.Dispose()
        $inputStream.Dispose()
        $captured.Dispose()
    }
}

$cases = [ordered]@{
    auto = @{ discovery = $auto }
    manual = @{ discovery = $manual }
    legacy = @{}
    extra_arg = @{ args = $routeArgs.Replace('}', ',"extra":"private_marker"}') }
    missing_arg = @{ args = '{"local_ipv4":"192.168.4.9"}' }
    wrong_arg = @{ args = $routeArgs.Replace('peer_ipv4','unknown') }
    ipv6_arg = @{ args = $routeArgs.Replace('192.168.4.10','::1') }
    ipv4_alias = @{ args = $routeArgs.Replace('192.168.4.10','192.168.4.010') }
    unknown_op = @{ op = 'route_lan_unknown' }
    string_prefix = @{ invalid = $auto.Replace(':24',':"24"') }
    float_prefix = @{ invalid = $auto.Replace(':24',':24.0') }
    zero_prefix = @{ invalid = $auto.Replace(':24',':0') }
    large_prefix = @{ invalid = $auto.Replace(':24',':31') }
    upper_id = @{ invalid = $auto.Replace(('a' * 32),('A' * 32)) }
    zero_id = @{ invalid = $auto.Replace(('b' * 32),('0' * 32)) }
    extra_auto = @{ invalid = $auto.Replace('}', ',"private":"private_marker"}') }
    extra_manual = @{ invalid = '{"mode":"manual","private":"private_marker"}' }
    invalid_mode = @{ invalid = '{"mode":"unknown"}' }
    null_discovery = @{ invalid = 'null' }
    wrong_route_mode = @{ invalid = $auto; mode = 'loopback' }
    missing_peer = @{ invalid = $auto; peer = 'null' }
}
foreach ($entry in $cases.GetEnumerator()) {
    $case = $entry.Value
    $reject = $case.ContainsKey('args') -or $case.ContainsKey('op')
    $op = if ($case.op) { $case.op } else { 'route_lan_auto' }
    $requestArgs = if ($case.args) { $case.args } else { $routeArgs }
    $route = if ($case.mode) { $case.mode } else { 'direct_lan' }
    $peer = if ($case.peer) { $case.peer } else { '"192.168.4.10"' }
    $discovery = if ($case.invalid) { $case.invalid } else { $case.discovery }
    $field = if ($discovery) { ',"lan_discovery":' + $discovery } else { '' }
    $source = $fake.Replace('__REJECT__',('$' + $reject.ToString())).Replace('__DISCOVERY__',$field).
        Replace('__ROUTE__',$route).Replace('__PEER__',$peer)
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
    $info.Arguments = "-NoProfile -NonInteractive -EncodedCommand $encoded"
    $info.EnvironmentVariables['PSModulePath'] = "$env:windir\System32\WindowsPowerShell\v1.0\Modules"
    Invoke-BridgeCase $entry.Key $info $op $requestArgs (!$reject -and !$case.invalid) $case.discovery $null
}

if ($PinnedCli -or $FreshSetup) {
    if (!$PinnedCli -or !$FreshSetup -or (Test-Path -LiteralPath $FreshSetup)) {
        throw 'The pinned CLI smoke requires a fresh setup directory'
    }
    $cli = (Resolve-Path -LiteralPath $PinnedCli).ProviderPath
    if ((Get-FileHash -LiteralPath $cli).Hash -cne '736827D50C3CFB4F34282B6651DCCC82F25131C4C86A8594105771A38CDA1AC2') {
        throw 'Unexpected CLI image'
    }
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = 'setup --jsonl --dir "' + [IO.Path]::GetFullPath($FreshSetup) + '"'
    Invoke-BridgeCase 'pinned_cli_no_setup' $info 'route_lan_auto' $routeArgs $true $null 'not_open'
}
