[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RunDirectory,[string]$EvidenceName='reopen')
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
$workspace=Split-Path -Parent $PSScriptRoot
$exe=(Resolve-Path -LiteralPath (Join-Path $workspace 'target/field-tests/ordered-dialog/kilogram-cli.exe')).Path
$run=(Resolve-Path -LiteralPath $RunDirectory).Path
$origin=Get-Content -LiteralPath (Join-Path $run 'result.json') -Raw | ConvertFrom-Json
$evidence=Join-Path $run $EvidenceName
if (Test-Path -LiteralPath $evidence) {throw 'Use a completed run without previous reopen evidence.'}
[void](New-Item -ItemType Directory -Path $evidence)
$watch=[Diagnostics.Stopwatch]::StartNew();$processes=@();$durations=[Collections.Generic.List[double]]::new()
$immutable=@{}
function Invoke-ReopenCli([string[]]$Arguments) {
    $output=& $exe @Arguments 2>&1;$code=$LASTEXITCODE
    $text=($output | ForEach-Object {$_.ToString()}) -join "`n"
    Add-Content -LiteralPath (Join-Path $evidence 'trace.log') -Value ("{0:N3}s exit={1} {2}`n{3}" -f $watch.Elapsed.TotalSeconds,$code,($Arguments -join ' '),$text)
    if ($code -ne 0) {throw "Reopen CLI failed: $text"}
    return $text
}
function Get-ReopenIpcHash([string]$Path) {
    try {
        $stream=[IO.File]::OpenRead($Path)
        try {return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream))}
        finally {$stream.Dispose()}
    } catch [IO.IOException] {return ''}
}
try {
    $oldIpc=@{}
    foreach($side in @('one','two')) {
        $ipc=Join-Path $run "$side.ipc"
        $oldIpc[$side]=Get-ReopenIpcHash $ipc
        $root=Join-Path $run "$side.state"
        foreach($file in Get-ChildItem -LiteralPath $root -Recurse -File) {
            $relative=[IO.Path]::GetRelativePath($root,$file.FullName).Replace('\','/')
            if($relative.StartsWith('events/od1/') -or $relative.StartsWith('local-messages/od1/') -or $relative.Contains('/controls/') -or $relative.Contains('/ready/') -or $relative.EndsWith('/initial-pair') -or $relative.EndsWith('/native')) {$immutable[$file.FullName]=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash}
        }
        $processes+=Start-Process -FilePath $exe -ArgumentList @('runtime-from-profile','--profile-file',('"{0}"' -f (Join-Path $run "$side.profile"))) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $evidence "$side.stdout.log") -RedirectStandardError (Join-Path $evidence "$side.stderr.log")
    }
    $deadline=[DateTime]::UtcNow.AddSeconds(45)
    foreach($side in @('one','two')) {
        $ipc=Join-Path $run "$side.ipc"
        do {Start-Sleep -Milliseconds 50;$hash=Get-ReopenIpcHash $ipc;$fresh=$hash -ne '' -and $hash -cne $oldIpc[$side]} while(-not $fresh -and [DateTime]::UtcNow -lt $deadline)
        if(-not $fresh){throw 'Full-history runtime failed to reopen.'}
    }
    $reopenSeconds=$watch.Elapsed.TotalSeconds
    $c=$origin.context
    [void](Invoke-ReopenCli @('ordered','locator-import','--ipc-file',(Join-Path $run 'one.ipc'),'--context',$c,'--locator-file',(Join-Path $run "two.locators/$c.locator")))
    [void](Invoke-ReopenCli @('ordered','locator-import','--ipc-file',(Join-Path $run 'two.ipc'),'--context',$c,'--locator-file',(Join-Path $run "one.locators/$c.locator")))
    foreach($side in @('one','two')) {
        $expected=@(Get-Content -LiteralPath (Join-Path $run "$side.history.json") -Raw | ConvertFrom-Json)
        $actual=@()
        for($offset=0;$offset -lt $origin.text_count;$offset+=8) {
            $started=$watch.Elapsed.TotalMilliseconds
            $page=Invoke-ReopenCli @('ordered','history','--ipc-file',(Join-Path $run "$side.ipc"),'--context',$c,'--offset',"$offset",'--limit','8') | ConvertFrom-Json
            $durations.Add($watch.Elapsed.TotalMilliseconds-$started)
            if($page.total -ne $origin.text_count){throw 'Readable history count changed on reopen.'}
            $actual+=$page.items
        }
        if($actual.Count -ne $expected.Count){throw 'Wrong reopened history count.'}
        for($i=0;$i -lt $actual.Count;$i++) {
            foreach($field in @('event_id','text','request_id','role','ordinal','author_sequence')) {if($actual[$i].$field -cne $expected[$i].$field){throw "Reopened $field differs at position $i"}}
        }
        $actual | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidence "$side.history.json")
        $status=Invoke-ReopenCli @('ordered','status','--ipc-file',(Join-Path $run "$side.ipc")) | ConvertFrom-Json
        $original=$origin."final_$side".contexts[0]
        if($status.contexts[0].session_id -cne $original.session_id -or $status.contexts[0].send_next -ne $original.send_next -or $status.contexts[0].receive_next -ne $original.receive_next){throw 'Reopen replaced or advanced the selected session.'}
    }
    foreach($path in $immutable.Keys) {if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $immutable[$path]){throw 'Reopen changed committed immutable/native bytes.'}}
    $sorted=@($durations | Sort-Object)
    $summary=[pscustomobject]@{status='PASS';text_count=$origin.text_count;source_exe_sha256=$origin.exe_sha256;reader_exe_sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant();reopen_seconds=$reopenSeconds;history_and_reauth_seconds=$watch.Elapsed.TotalSeconds-$reopenSeconds;total_seconds=$watch.Elapsed.TotalSeconds;history_page_p50_ms=$sorted[[int][Math]::Floor(($sorted.Count-1)*0.50)];history_page_p95_ms=$sorted[[int][Math]::Floor(($sorted.Count-1)*0.95)];pids=@($processes.Id);immutable_records_verified=$immutable.Count}
    $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'result.json');$summary | ConvertTo-Json
    foreach($side in @('one','two')) {[void](Invoke-ReopenCli @('ordered','shutdown','--ipc-file',(Join-Path $run "$side.ipc")))}
    foreach($p in $processes){if(-not $p.WaitForExit(15000)){throw 'OD1 graceful shutdown did not complete.'}}
} finally {foreach($p in $processes){if(-not $p.HasExited){Stop-Process -Id $p.Id;$p.WaitForExit()}};Write-Host "reopen_evidence_directory=$evidence"}
