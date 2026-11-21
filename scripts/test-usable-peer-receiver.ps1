# Disposable, ordinary-user receiver for ONE causal round-trip probe. This is
# never used by normal chat and never opens the user's profile.
param([string]$Exe=(Join-Path $PSScriptRoot 'kilogram-peer.exe'),[string]$Invitation,[string]$Case=([guid]::NewGuid().ToString('N')))
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
if($Case -cnotmatch '^[a-z0-9-]{1,40}$'){throw 'Fresh test case required'}
$Exe=[IO.Path]::GetFullPath($Exe);$root=[IO.Path]::GetDirectoryName($Exe)
function Assert-Plain([string]$Path){
    while(!(Test-Path -LiteralPath $Path)){$Path=[IO.Path]::GetDirectoryName($Path);if(!$Path){throw 'Missing test parent'}}
    $item=Get-Item -LiteralPath $Path
    while($item){if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reparse test path'};$item=if($item -is [IO.FileInfo]){$item.Directory}else{$item.Parent}}
}
Assert-Plain $root
if(([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Use an ordinary user, not administrator'}
if(!$Invitation){[Console]::WriteLine('Только тестовый аккаунт. Вставьте приглашение из PC probe:');$Invitation=[Console]::ReadLine()}
if(!$Invitation -or $Invitation.Length -gt 16384 -or $Invitation -cnotmatch '^kilogram://peer/[a-zA-Z0-9_-]+$'){throw 'Invitation format'}
$out=Join-Path (Join-Path $root 'test-data') $Case
if(Test-Path -LiteralPath $out){throw 'Test evidence already exists; preserve it'}
Assert-Plain (Join-Path $root 'test-data')
[void][IO.Directory]::CreateDirectory($out)
$guardSource=Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1'
. ([scriptblock]::Create([IO.File]::ReadAllText($guardSource)))
$watch=[KilogramS5Deadline]::new(900,930,[KilogramS5Deadline]::TickNow())
$child=$null;$exit=$null;$primary=$null;$forced=$false;$empty=$false;$stdoutEof=$false;$quit=$false;$echoed=0;$seen=@{};$nonce=$null;$stopAt=$null
$log=[IO.StreamWriter]::new((Join-Path $out 'public.jsonl'),$false,[Text.UTF8Encoding]::new($false));$log.AutoFlush=$true
try {
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName=$Exe
    $si.Arguments='--jsonl --profile "'+(Join-Path $out 'profile')+'" --join "'+$Invitation+'"'
    $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardInput=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
    $child=[Diagnostics.Process]::Start($si);$watch.VerifyChild($child)
    $errorDrain=$child.StandardError.ReadToEndAsync();$read=$child.StandardOutput.ReadLineAsync();$frames=0;$end=[DateTime]::UtcNow.AddSeconds(870)
    [Console]::WriteLine('Тест запущен на отдельном аккаунте. Ожидаю PC probe; максимум 15 минут.')
    while(!$child.HasExited -or !$stdoutEof) {
        $watch.CheckWork()
        if(!$quit -and ([DateTime]::UtcNow -ge $end -or ($stopAt -and [DateTime]::UtcNow -ge $stopAt) -or [IO.File]::Exists((Join-Path $out 'stop')))){$child.StandardInput.WriteLine('/quit');$child.StandardInput.Flush();$quit=$true}
        if(!$read.IsCompleted){Start-Sleep -Milliseconds 5;continue}
        $line=$read.GetAwaiter().GetResult()
        if($null -eq $line){$stdoutEof=$true;break}
        if($line.Length -gt 262144 -or (++$frames) -gt 1024){throw 'Receiver output bound'}
        $event=$line|ConvertFrom-Json
        if($event.type -ceq 'recovery'){$log.WriteLine('{"type":"recovery","redacted":true}')}
        else{$log.WriteLine($line)}
        if($event.type -ceq 'confirm'){
            if($event.value.command -cnotin @('/accept','/saved')){throw 'Unexpected test consent'}
            $child.StandardInput.WriteLine([string]$event.value.command);$child.StandardInput.Flush()
        } elseif($event.type -ceq 'message'){
            $text=[string]$event.value.text
            if($text -cmatch '^KGPROBE/([0-9a-f]{32})/([0-9]{1,3})$'){
                if(!$nonce){$nonce=$Matches[1]}
                if($nonce -cne $Matches[1]){throw 'Probe peer changed'}
                $key=[string]$event.value.event_id
                if(!$seen.ContainsKey($key)){
                    if($seen.Count -ge 80){throw 'Probe count bound'}
                    $seen[$key]=$true;$echoed++
                    $child.StandardInput.WriteLine('KGPROBE-REPLY/'+$nonce+'/'+$Matches[2]);$child.StandardInput.Flush()
                }
            } elseif($nonce -and $text -ceq ('KGPROBE-STOP/'+$nonce)) {
                $child.StandardInput.WriteLine('KGPROBE-STOPPED/'+$nonce);$child.StandardInput.Flush();$stopAt=[DateTime]::UtcNow.AddSeconds(3)
            }
        } elseif($event.type -cin @('operation_error','history_gap')) {throw ('Product refused: '+$event.type)}
        elseif($event.type -ceq 'connection'){[Console]::WriteLine([string]$event.text)}
        $read=$child.StandardOutput.ReadLineAsync()
    }
    if(!$child.WaitForExit(20000)){throw 'Receiver ordinary close timeout'}
    $exit=$child.ExitCode
    if(!$errorDrain.Wait(2000)){throw 'Receiver stderr EOF'}
    $stderr=$errorDrain.GetAwaiter().GetResult();if($stderr.Length -gt 1048576){throw 'Receiver stderr bound'}
    [IO.File]::WriteAllText((Join-Path $out 'stderr.log'),$stderr,[Text.UTF8Encoding]::new($false))
    if($exit -ne 0 -or !$quit -or !$stdoutEof){throw 'Receiver ordinary outcome not confirmed'}
} catch {$primary=$_.Exception.Message}
finally {
    if($child -and !$child.HasExited){try{$child.StandardInput.WriteLine('/quit');$child.StandardInput.Flush();$null=$child.WaitForExit(15000)}catch{}}
    $stop=[DateTime]::UtcNow.AddSeconds(2)
    while(@($watch.Children()).Count -and [DateTime]::UtcNow -lt $stop){Start-Sleep -Milliseconds 20}
    $forced=@($watch.Children()).Count -gt 0
    if($forced){$watch.CleanupChildren(5000)}
    $empty=@($watch.Children()).Count -eq 0;$watch.Finish();$log.Dispose()
    $result=@{passed=(!$primary -and !$forced -and $empty -and $exit -eq 0 -and $stdoutEof -and $quit);error=$primary;exit=$exit;forced=$forced;empty_owned_job=$empty;stdout_eof=$stdoutEof;ordinary_quit=$quit;echoed=$echoed;host=[Environment]::MachineName;windows_session=[Diagnostics.Process]::GetCurrentProcess().SessionId;administrator=([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);personal_profile_used=$false;exe_sha256=(Get-FileHash -LiteralPath $Exe).Hash.ToLowerInvariant();work_seconds=900;whole_seconds=930}
    $result|ConvertTo-Json -Depth 5|Set-Content -LiteralPath (Join-Path $out 'result.json') -Encoding UTF8
    if($child){$child.Dispose()}
}
[Console]::WriteLine('Evidence: '+$out)
if(!$result.passed){throw ('Disposable receiver failed: '+$primary)}
