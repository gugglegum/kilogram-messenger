$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $PWD 'scripts/test-terminal-t1-2e-setup-pipe.cs'))) `
    -ReferencedAssemblies 'System.dll','System.Core.dll','System.Web.Extensions.dll'

$fake = @'
$mode = '__MODE__'
$private = (@('blindword') * 24) -join ' '
[Console]::Error.WriteLine($private)
if ($mode -eq 'early') {
    [Console]::WriteLine($private)
    exit 1
}
[Console]::WriteLine('{"v":1,"type":"ready"}')
$null = [Console]::ReadLine()
[Console]::WriteLine('{"v":1,"type":"result","id":"make","ok":true,"value":{"state":"recovery_available","account_id":null,"device_id":null,"contexts":null}}')
$null = [Console]::ReadLine()
$good = '{"v":1,"type":"result","id":"host-recovery","ok":true,"value":{"recovery_scope":"root-key-only-authority-history-required","recovery_words":"' + $private + '"}}'
switch ($mode) {
    'malformed' { [Console]::WriteLine('{"' + $private); exit 1 }
    'oversized' { [Console]::WriteLine('x' * 20000 + $private); exit 1 }
    'truncated' { [Console]::Write($good); exit 1 }
    'eof' { exit 1 }
    'timeout' { [Threading.Thread]::Sleep(30000); exit 1 }
    'wrong-id' { [Console]::WriteLine($good.Replace('host-recovery','outer')); exit 1 }
    'wrong-scope' { [Console]::WriteLine($good.Replace('root-key-only','wrong')); exit 1 }
}
[Console]::WriteLine($good)
$null = [Console]::ReadLine()
[Console]::WriteLine('{"v":1,"type":"result","id":"save","ok":true,"value":{"state":"recovery_saved","account_id":null,"device_id":null,"contexts":null}}')
$null = [Console]::ReadLine()
[Console]::WriteLine('{"v":1,"type":"result","id":"quit","ok":true,"value":{"closed":true}}')
'@

foreach ($mode in 'valid','early','malformed','oversized','truncated','eof','timeout','wrong-id','wrong-scope','raw') {
    $source = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($fake.Replace('__MODE__',$mode)))
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe"
    $info.Arguments = "-NoProfile -NonInteractive -EncodedCommand $source"
    [KilogramSetupPipe]::Redirect($info)
    $second = if ($mode -eq 'raw') { 'recovery_show' } else { 'save_test_recovery' }
    $protocol = '{"v":1,"id":"make","op":"create","args":{}}' + "`n" +
        '{"v":1,"id":"save","op":"' + $second + '","args":{}}' + "`n" +
        '{"v":1,"id":"quit","op":"quit","args":{}}' + "`n"
    $bytes = [Text.Encoding]::UTF8.GetBytes($protocol)
    $inputStream = New-Object IO.MemoryStream -ArgumentList @(,$bytes)
    $captured = New-Object IO.StringWriter
    $child = [Diagnostics.Process]::Start($info)
    try {
        $bridge = [KilogramSetupPipe]::Run($child,$inputStream,$captured,$PWD.ProviderPath)
        $limit = if ($mode -eq 'timeout') { 1500 } else { 8000 }
        try { $success = $bridge.Wait($limit) }
        catch { $success = $false }
        $result = $captured.ToString()
        if ($result -match 'blindword|recovery_words|host-recovery|recovery_scope') {
            throw 'Synthetic private frame escaped its host'
        }
        if ($mode -eq 'valid') {
            if (!$success -or !$child.WaitForExit(2000) -or $child.ExitCode -ne 0 -or
                $result -notmatch '"recovery_saved"' -or $result -notmatch '"closed":true') {
                throw 'Expected sanitized success and EOF'
            }
        } elseif ($success -or $result -match '"id":"save"|"id":"quit"') {
            throw "Fault was not rejected: $mode"
        }
        Write-Output "protected_setup_synthetic=$mode passed"
    } finally {
        if (!$child.HasExited) { $child.Kill(); if (!$child.WaitForExit(3000)) { throw 'Synthetic child exit was not observed' } }
        $child.Dispose()
        $inputStream.Dispose()
        $captured.Dispose()
    }
}
