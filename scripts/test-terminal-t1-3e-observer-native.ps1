param([Parameter(Mandatory=$true)][string]$Evidence)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
if(Test-Path -LiteralPath $Evidence){throw 'Fresh observer evidence required'}
[void][IO.Directory]::CreateDirectory($Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-sender-crash-policy.ps1" -Evidence $Evidence -FixtureLibrary
$guard=[KilogramS5Deadline]::new(100,115);$owned=@();$passed=$false
try{
 $hostChild=Start-QuietFixture "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe" '-NoProfile -NonInteractive -Command "exit 1"';$owned+=,$hostChild;$guard.VerifyChild($hostChild);$hostStart=$hostChild.StartTime.ToFileTimeUtc()
 if(!$hostChild.WaitForExit(5000) -or $hostChild.ExitCode -ne 1){throw 'Isolated host exit fixture'}
 $f=Fixture 'observer';$v=Assert-LabExpectedCrash $f.run $f.entry $f.owner $f.cfg $f.started $hostChild
 Write-Case (Join-Path $f.run 'owners/2.done.json') @{state='expected_crash_reconciled';tag=$f.owner.tag;generation=$f.owner.generation;worker_pid=$hostChild.Id;worker_start=$hostStart;exit=1}
 Write-Case (Join-Path $f.run 'receipts/runtime-fault-reconciled.json') $v
 $base=$f.cfg.base;[void][IO.Directory]::CreateDirectory((Join-Path $base 'ordinary-agent/control'));[void][IO.Directory]::CreateDirectory((Join-Path $base 'bin'))
 [IO.File]::WriteAllBytes((Join-Path $base 'ordinary-agent/test-terminal-t1-3e-lab-common.ps1'),[IO.File]::ReadAllBytes("$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"))
 Write-Case (Join-Path $base 'ordinary-agent/lab.json') $f.cfg
 Write-Case (Join-Path $base 'ordinary-agent/control/ready.json') @{session=$f.entry.session}
 Write-Case (Join-Path $base 'ordinary-agent/control/state.json') @{session=$f.entry.session;active='one';blocked=$null}
 $exe=Join-Path $base 'bin/kilogram-cli.exe'
 Add-Type -TypeDefinition 'public static class ObserverFixture {public static void Main(string[] a){if(a.Length>1 && a[0]=="ordered"){System.Console.WriteLine(System.IO.File.ReadAllText(System.IO.Path.Combine(System.AppDomain.CurrentDomain.BaseDirectory,"status-fixture.json")));return;}System.Threading.Thread.Sleep(90000);}}' -OutputAssembly $exe -OutputType ConsoleApplication
 $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$exe;$info.Arguments='chat "'+(Join-Path $f.run 'setup')+'"';$info.UseShellExecute=$false;$info.CreateNoWindow=$true
 $info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 $chat=[Diagnostics.Process]::Start($info);$owned+=,$chat;$guard.VerifyChild($chat);$null=$chat.Handle;$chat.PriorityClass='BelowNormal'
 Write-Case (Join-Path $Evidence 'fixture.json') @{base=$base;host=[Environment]::MachineName;chat_pid=$chat.Id;chat_started=$chat.StartTime.ToFileTimeUtc();intent=$f.intent;cfg=$f.cfg;exe_sha256=(Lab-Hash $exe)}
 function Invoke-Checks([string]$Phase){
  $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName='C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
  $si.Arguments='-X utf8 -B "'+$PSScriptRoot+'/test-terminal-t1-3e-observer-check.py" "'+$Evidence+'" '+$Phase;$si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
  $child=[Diagnostics.Process]::Start($si);$script:owned+=,$child;$guard.VerifyChild($child);$child.PriorityClass='BelowNormal'
  $o=$child.StandardOutput.ReadToEndAsync();$e=$child.StandardError.ReadToEndAsync()
  while(!$child.WaitForExit(100)){$guard.CheckWork()};$guard.CheckWork()
  [IO.File]::WriteAllText((Join-Path $Evidence ($Phase+'.stdout.log')),$o.GetAwaiter().GetResult());[IO.File]::WriteAllText((Join-Path $Evidence ($Phase+'.stderr.log')),$e.GetAwaiter().GetResult())
  if($child.ExitCode -ne 0){throw ('Observer '+$Phase+' failed')}
 }
 Invoke-Checks 'preconnect'
 $chat.Kill();$chat.WaitForExit();$chat.Dispose()
 $info.Arguments='runtime-from-profile "'+(Join-Path $f.run 'setup')+'"';$runtime=[Diagnostics.Process]::Start($info);$owned+=,$runtime;$guard.VerifyChild($runtime);$null=$runtime.Handle;$runtime.PriorityClass='BelowNormal'
 Write-Case (Join-Path $Evidence 'runtime.json') @{pid=$runtime.Id;started=$runtime.StartTime.ToFileTimeUtc();image=$exe}
 $slots=@();for($i=0;$i -lt 2;$i++){$slots+=@{slot=$i;context=$f.intent.contexts[$i];role=0;phase='ready';active=$true;live_admitted=$true;lease_expires=999999;selected_lan_peer='127.0.0.1';own_locator_revision=2;peer_locator_revision=2;publication_ready=$true;blocked=$false;policy_error=$null}}
 Write-Case (Join-Path $base 'bin/status-fixture.json') @{runtime_instance=$f.intent.runtime_instance;runtime_pid=$runtime.Id;contexts=$slots;endpoint='127.0.0.1:12345';lan_discovery=@{enabled=$true};route=@{mode='direct_lan'}}
 Invoke-Checks 'status'
 $passed=$true
}finally{
 $guard.CleanupChildren(5000);$empty=$guard.Children().Length -eq 0;$guard.Finish()
 foreach($p in $owned){$p.Dispose()}
 @{passed=$passed;empty_owned_job=$empty;runtime_reply_synthetic=$true;no_installed_actors=$true;work_seconds=100;whole_seconds=115}|ConvertTo-Json -Compress|Set-Content -LiteralPath (Join-Path $Evidence 'outer.json') -Encoding UTF8
}
if(!$passed){throw 'Observer checks failed'};@{passed=$true;empty_job=$empty}|ConvertTo-Json -Compress
