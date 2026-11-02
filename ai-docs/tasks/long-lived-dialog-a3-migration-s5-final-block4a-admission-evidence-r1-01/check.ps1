param([Parameter(Mandatory)][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
Set-Location $workspace
$base=Join-Path $workspace '.tmp/s54-block4a-r1-01'
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$argsList=switch -Regex ($Step){
 '^probe-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'probe.ps1'))}
 '^compile-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'compile.ps1'),'-Capture',(Join-Path $base "$Step.capture.json"))}
 '^clippy-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'clippy.ps1'))}
 '^fmt-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'fmt.ps1'))}
 '^pure-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'pure.ps1'),'-Step',$Step)}
 '^powershell-[0-9]{2}$' {@('-NoProfile','-NonInteractive','-File',(Join-Path $base 'powershell-tests.ps1'),'-Step',$Step)}
 default {throw 'Unknown immutable check step.'}
}
$receipt=Join-Path $base "$Step.json"
if(Test-Path -LiteralPath $receipt){throw 'Immutable step already exists.'}
$env:CARGO_NET_OFFLINE='true'
. (Join-Path $workspace 'scripts/migration-s5-l1-deadline.ps1')
$limit=if($Step.StartsWith('compile-')){1200}elseif($Step.StartsWith('clippy-') -or $Step.StartsWith('powershell-')){600}else{240}
$jobStartedUtc=[DateTimeOffset]::UtcNow
$deadline=[KilogramS5Deadline]::new($limit-10,$limit)
$admission=Join-Path $base "$Step.admission"
$membership=[Collections.Generic.List[object]]::new()
if($Step.StartsWith('pure-')){
 New-Item -ItemType Directory -Path $admission -ErrorAction Stop|Out-Null
 $harness=Join-Path $workspace 'target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe'
 $harnessHash=(Get-FileHash -LiteralPath $harness).Hash.ToLowerInvariant()
 $wholeUtc=$jobStartedUtc.AddSeconds($limit).ToUnixTimeMilliseconds()
}
function Grant-CaptureMembership {
 if(-not $Step.StartsWith('pure-')){return}
 foreach($request in @(Get-ChildItem -LiteralPath $admission -Filter 'join-*.json')){
  $permit=Join-Path $admission ($request.Name.Replace('join-','permit-'))
  if(Test-Path -LiteralPath $permit){continue}
  try{$row=Get-Content -LiteralPath $request.FullName -Raw|ConvertFrom-Json}catch{continue}
  if($row.attempt -cne $Step -or $row.owner_pid -ne $PID -or $row.membership_verified){throw 'Capture join binding failed.'}
  $member=[Diagnostics.Process]::GetProcessById([int]$row.harness_pid)
  try{
   $deadline.VerifyChild($member)
   $actualPath=$member.MainModule.FileName
   if([IO.Path]::GetFullPath($actualPath) -ine [IO.Path]::GetFullPath($harness)){throw 'Capture member executable mismatch.'}
   $actualHash=(Get-FileHash -LiteralPath $actualPath).Hash.ToLowerInvariant()
   if($actualHash -cne $harnessHash){throw 'Capture member image changed.'}
   $membership.Add([ordered]@{request=$request.FullName;attempt=$Step;owner_pid=$PID;launcher_pid=$p.Id;harness_pid=$member.Id;harness_start_utc=$member.StartTime.ToUniversalTime().ToString('o');path=$actualPath;sha256=$actualHash;actual_handle_membership_verified=$true;nonce=$row.nonce})
   $row.membership_verified=$true
   $temporary=$permit+'.new'
   [IO.File]::WriteAllText($temporary,($row|ConvertTo-Json -Depth 4))
   [IO.File]::Move($temporary,$permit)
  }finally{$member.Dispose()}
 }
}
$watch=[Diagnostics.Stopwatch]::StartNew();$code=1;$errorText=$null;$childCode=$null;$cleanup=$false
$beforeCleanup=@();$afterCleanup=@();$launcherVerified=$false
$p=[Diagnostics.Process]::new()
try{
 $p.StartInfo.FileName=$shell;$p.StartInfo.UseShellExecute=$false;$p.StartInfo.CreateNoWindow=$true;$p.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
 $p.StartInfo.RedirectStandardOutput=$true;$p.StartInfo.RedirectStandardError=$true
 if($Step.StartsWith('pure-')){$p.StartInfo.Environment['KILOGRAM_S54_CAPTURE_ADMISSION']=(Join-Path $admission 'admission.json')}
 foreach($arg in $argsList){[void]$p.StartInfo.ArgumentList.Add($arg)}
 if(-not $p.Start()){throw 'Check child failed to start.'};$deadline.VerifyChild($p)
 $launcherVerified=$true
 if($Step.StartsWith('pure-')){
  $intent=[ordered]@{version=1;attempt=$Step;owner_pid=$PID;launcher_pid=$p.Id;harness_path=$harness;whole_utc_ms=$wholeUtc}
  [IO.File]::WriteAllText((Join-Path $admission 'admission.json'),($intent|ConvertTo-Json))
 }
 $outTask=$p.StandardOutput.ReadToEndAsync();$errTask=$p.StandardError.ReadToEndAsync()
 while(-not $p.WaitForExit(50)){$deadline.CheckWork();Grant-CaptureMembership}
 $deadline.CheckWork()
 $childCode=$p.ExitCode
 [IO.File]::WriteAllText((Join-Path $base "$Step.stdout.log"),$outTask.GetAwaiter().GetResult())
 [IO.File]::WriteAllText((Join-Path $base "$Step.stderr.log"),$errTask.GetAwaiter().GetResult())
 if($childCode -ne 0){throw "Native check exit $childCode"};$code=0
}catch{$errorText=$_.Exception.Message;$code=1}
finally{try{$beforeCleanup=@($deadline.Children());$deadline.CleanupChildren(3000);$afterCleanup=@($deadline.Children());$cleanup=$afterCleanup.Count -eq 0}catch{$errorText+=$_.Exception.Message;$code=1};$p.Dispose()}
$row=[ordered]@{step=$Step;executable=$shell;argv=$argsList;actual_child_exit_code=$childCode;exit_code=$code;elapsed_seconds=$watch.Elapsed.TotalSeconds;cleanup_verified=$cleanup;error=$errorText;outer_job_owner_pid=$PID;actual_launcher_membership_verified=$launcherVerified;work_expired=$deadline.WorkExpired;members_before_cleanup=$beforeCleanup;members_after_cleanup=$afterCleanup;capture_harness_membership=@($membership)}
[IO.File]::WriteAllText($receipt,($row|ConvertTo-Json -Depth 6))
$deadline.Finish()
$row|ConvertTo-Json -Depth 6
exit $code

