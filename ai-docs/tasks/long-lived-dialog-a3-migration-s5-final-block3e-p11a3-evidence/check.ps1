param([Parameter(Mandatory)][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
Set-Location $workspace
$base=Join-Path $workspace '.tmp/s54-block3e-p11a3-01'
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$argsList=switch -Regex ($Step){
 '^compile-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'compile.ps1'),'-Capture',(Join-Path $base "$Step.capture.json"))}
 '^clippy-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'clippy.ps1'))}
 '^fmt-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'fmt.ps1'))}
 '^pure-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'pure.ps1'),'-Step',$Step)}
 default {throw 'Unknown immutable check step.'}
}
$receipt=Join-Path $base "$Step.json"
if(Test-Path -LiteralPath $receipt){throw 'Immutable step already exists.'}
$env:CARGO_NET_OFFLINE='true'
. (Join-Path $workspace 'scripts/migration-s5-l1-deadline.ps1')
$limit=if($Step.StartsWith('compile-')){1200}elseif($Step.StartsWith('clippy-')){600}else{240}
$deadline=[KilogramS5Deadline]::new($limit-10,$limit)
$watch=[Diagnostics.Stopwatch]::StartNew();$code=1;$errorText=$null;$childCode=$null;$cleanup=$false
$p=[Diagnostics.Process]::new()
try{
 $p.StartInfo.FileName=$shell;$p.StartInfo.UseShellExecute=$false;$p.StartInfo.CreateNoWindow=$true;$p.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
 $p.StartInfo.RedirectStandardOutput=$true;$p.StartInfo.RedirectStandardError=$true
 foreach($arg in $argsList){[void]$p.StartInfo.ArgumentList.Add($arg)}
 if(-not $p.Start()){throw 'Check child failed to start.'};$deadline.VerifyChild($p)
 $outTask=$p.StandardOutput.ReadToEndAsync();$errTask=$p.StandardError.ReadToEndAsync()
 while(-not $p.WaitForExit(100)){$deadline.CheckWork()}
 $childCode=$p.ExitCode
 [IO.File]::WriteAllText((Join-Path $base "$Step.stdout.log"),$outTask.GetAwaiter().GetResult())
 [IO.File]::WriteAllText((Join-Path $base "$Step.stderr.log"),$errTask.GetAwaiter().GetResult())
 if($childCode -ne 0){throw "Native check exit $childCode"};$code=0
}catch{$errorText=$_.Exception.Message;$code=1}
finally{try{$deadline.CleanupChildren(3000);$cleanup=@($deadline.Children()).Count -eq 0}catch{$errorText+=$_.Exception.Message;$code=1};$p.Dispose()}
$row=[ordered]@{step=$Step;executable=$shell;argv=$argsList;actual_child_exit_code=$childCode;exit_code=$code;elapsed_seconds=$watch.Elapsed.TotalSeconds;cleanup_verified=$cleanup;error=$errorText}
[IO.File]::WriteAllText($receipt,($row|ConvertTo-Json -Depth 6))
$deadline.Finish()
$row|ConvertTo-Json -Depth 6
exit $code
