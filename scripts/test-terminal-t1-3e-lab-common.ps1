# Shared plain-file JSON protocol. This file never changes firewall or Task Scheduler.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
function Lab-Plain([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    while ($item) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'LAB_REPARSE' }
        $item = if ($item -is [IO.FileInfo]) { $item.Directory } else { $item.Parent }
    }
}
function Lab-Read([string]$Path) {
    if (![IO.File]::Exists($Path)) { return $null }
    Lab-Plain $Path
    $stream = [IO.File]::Open($Path,'Open','Read',('ReadWrite, Delete'))
    try {
        if ($stream.Length -lt 2 -or $stream.Length -gt 131072) { throw 'LAB_JSON_SIZE' }
        $reader = [IO.StreamReader]::new($stream,[Text.UTF8Encoding]::new($false,$true))
        try { return ($reader.ReadToEnd() | ConvertFrom-Json) } finally { $reader.Dispose() }
    } finally { $stream.Dispose() }
}
function Lab-Write([string]$Path,$Value) {
    Lab-Plain ([IO.Path]::GetDirectoryName($Path))
    if ([IO.File]::Exists($Path)) { Lab-Plain $Path }
    $temp = "$Path.$([guid]::NewGuid().ToString('N')).pending"
    $bytes = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 18 -Compress)+[char]10)
    $f = [IO.File]::Open($temp,'CreateNew','Write','None')
    try { $f.Write($bytes,0,$bytes.Length); $f.Flush($true) } finally { $f.Dispose() }
    try {
        if ([IO.File]::Exists($Path)) { [IO.File]::Replace($temp,$Path,[NullString]::Value) }
        else { [IO.File]::Move($temp,$Path) }
    } finally { [IO.File]::Delete($temp) }
}
function Lab-Keys($Value,[string[]]$Keys) {
    if ($Value -isnot [pscustomobject] -or
        (@($Value.PSObject.Properties | ForEach-Object Name | Sort-Object) -join '|') -cne (@($Keys | Sort-Object) -join '|')) {
        throw 'LAB_SCHEMA'
    }
}
function Lab-Hex($Value,[int]$Length=32) {
    if ($Value -isnot [string] -or $Value -cnotmatch "\A[0-9a-f]{$Length}\z") { throw 'LAB_HEX' }
}
function Lab-Id($Value) {
    if ($Value -isnot [string] -or $Value -cnotmatch '\A[a-z0-9][a-z0-9-]{0,30}\z') { throw 'LAB_ID' }
}
function Lab-Hash([string]$Path) { Lab-Plain $Path; (Get-FileHash -LiteralPath $Path).Hash.ToLowerInvariant() }
function Lab-Config([string]$Path) {
    $cfg = Lab-Read $Path
    Lab-Keys $cfg @('v','campaign','side','host','sid','base','ip','peer','interface_alias','interface_index','interface_guid','suite','max_work_seconds')
    if ($cfg.v -ne 1 -or $cfg.side -cnotin @('A','B') -or $cfg.suite -cnotin @('field','synthetic') -or
        $cfg.sid -notmatch '^S-1-5-21-(\d+-){3}\d+$' -or $cfg.max_work_seconds -lt 10 -or
        $cfg.max_work_seconds -gt 1180 -or $cfg.base -match '["\r\n]' -or $cfg.base.EndsWith('\')) { throw 'LAB_CONFIG' }
    Lab-Id $cfg.campaign
    Lab-Plain $cfg.base
    $who = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        if ([Environment]::MachineName -cne $cfg.host -or $who.User.Value -cne $cfg.sid -or
            ([Security.Principal.WindowsPrincipal]::new($who)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'LAB_TOKEN'
        }
    } finally { $who.Dispose() }
    return $cfg
}
function Lab-PinSet($Pins,[string]$Suite) {
    $names = @('host.ps1','deadline.ps1','mailbox.cs','setup-pipe.cs','kilogram-cli.exe','kilogram-bootstrap.exe')
    if ($Suite -ceq 'synthetic') { $names += 'mailbox-fake.ps1' }
    Lab-Keys $Pins $names
    foreach ($p in $Pins.PSObject.Properties) { Lab-Hex $p.Value 64 }
    return $names
}
function Stop-LabIdleAgent([string]$Base,$Handles) {
    $agentControl=Join-Path $Base 'ordinary-agent/control'
    if(!(Test-Path -LiteralPath $agentControl)){return}
    Lab-Plain $agentControl
    $state=Lab-Read (Join-Path $agentControl 'state.json')
    if($state){
        Lab-Hex $state.session
        if($state.active){throw 'LAB_STOP_ATTEMPT_FIRST'}
        $locked=$false
        try{$lock=[IO.File]::Open((Join-Path $agentControl 'agent.lock'),'OpenOrCreate','Write','None');$lock.Dispose()}catch{$locked=$true}
        if($locked){
            $id=[guid]::NewGuid().ToString('N');$queue=Join-Path $agentControl $state.session
            Lab-Write (Join-Path $queue "$id.request.json") @{v=1;session=$state.session;id=$id;op='stop';args=@{}}
            $wait=[Diagnostics.Stopwatch]::StartNew();$reply=$null
            do{$reply=Lab-Read (Join-Path $queue "$id.response.json");if($reply){break};Start-Sleep -Milliseconds 80}while($wait.Elapsed.TotalSeconds -lt 6)
            if(!$reply -or !$reply.ok){throw 'LAB_AGENT_STOP_UNCONFIRMED'}
            do{
                $locked=$false
                try{$lock=[IO.File]::Open((Join-Path $agentControl 'agent.lock'),'OpenOrCreate','Write','None');$lock.Dispose()}catch{$locked=$true}
                if(!$locked){break};Start-Sleep -Milliseconds 80
            }while($wait.Elapsed.TotalSeconds -lt 8)
            if($locked){throw 'LAB_AGENT_STOP_UNCONFIRMED'}
        }
    }
    # Held until the caller completes teardown, so a logon trigger cannot race a new agent/update.
    $Handles.agent=[IO.File]::Open((Join-Path $agentControl 'agent.lock'),'OpenOrCreate','Write','None')
    $Handles.images=[IO.File]::Open((Join-Path $agentControl 'images.lock'),'OpenOrCreate','Write','None')
    if($state -and @([KilogramLabJob]::Inspect("Local\Kilogram-Lab-$($state.session)")).Count){throw 'LAB_AGENT_STILL_RUNNING'}
}
