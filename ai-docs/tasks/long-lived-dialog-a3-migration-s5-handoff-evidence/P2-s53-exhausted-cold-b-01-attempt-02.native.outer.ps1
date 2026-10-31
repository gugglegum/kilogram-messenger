$ErrorActionPreference='Stop'
$watch=[Diagnostics.Stopwatch]::StartNew();$start=[DateTimeOffset]::UtcNow
$w='E:\AI-Projects\kilogram-p2p-messenger'
$base="$w\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence"
$prefix="$base\P2-s53-exhausted-cold-b-01-attempt-02.native"
$child=$null;$capture=$null;$out=$null;$err=$null;$exitCode=1;$timedOut=$false;$preflight=$false;$captureClosed=$false;$outerFailure=$null
function WriteNew([string]$path,[string]$text){$raw=[Text.UTF8Encoding]::new($false).GetBytes($text);if($raw.Length -gt 65536){throw 'Public receipt cap'};$f=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read);try{$f.Write($raw,0,$raw.Length);$f.Flush($true)}finally{$f.Dispose()}}
function Verify($p){$item=Get-Item -LiteralPath $p.path;if($item.Length -ne $p.bytes -or (Get-FileHash -LiteralPath $p.path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $p.sha256){throw 'Reviewed input/image mismatch'}}
try {
 foreach($suffix in @('.preflight.json','.stdout.log','.stderr.log','.result.json','.invocation.ps1')){if(Test-Path -LiteralPath ($prefix+$suffix)){throw 'Attempt evidence collision'}}
 Set-Location $w
 if((& git rev-parse HEAD).Trim() -cne 'de16f0f7608a0d5d1d9d0f84ad38687a653343b5' -or @(& git diff --cached --name-only).Count -ne 0){throw 'Accepted HEAD/index mismatch'}
 $activation="$base\Cold-B-private-root-activation-02.md"
 Verify ([pscustomobject]@{path=$activation;bytes=3754;sha256='0cb091bd7df90394581c1e7b3b126d143867954e9be4560272fb373cef66a85f'})
 $planPath="$base\Cold-B-B2-combined-private-invocation.json"
 Verify ([pscustomobject]@{path=$planPath;bytes=7748;sha256='50126078ce99996382b18937a721d0a1214bb909dd26d1130fd11098fc02fe3e'})
 $plan=Get-Content -LiteralPath $planPath -Raw|ConvertFrom-Json
 foreach($pin in @($plan.entry,$plan.shell,$plan.deadline_helper,$plan.stable_harness,$plan.composed,$plan.assembly,$plan.artifact_receipt)){Verify $pin}
 $beforePath="$base\C-s53-cold-b2-01-attempt-09.inputs.before.json";$afterPath="$base\C-s53-cold-b2-01-attempt-09.inputs.after.json"
 Verify ([pscustomobject]@{path=$beforePath;bytes=118114;sha256='3ce85def1fd0d10eef8802517182ce3667c10e732b64199a50f0c66dca1f8ca3'})
 Verify ([pscustomobject]@{path=$afterPath;bytes=118114;sha256='3ce85def1fd0d10eef8802517182ce3667c10e732b64199a50f0c66dca1f8ca3'})
 $inputs=Get-Content -LiteralPath $beforePath -Raw|ConvertFrom-Json
 if($inputs.Count -ne 743){throw 'Input count mismatch'}
 foreach($pin in $inputs){Verify $pin}
 $imagesPath="$base\C-s53-cold-b2-01-attempt-09.images.json"
 Verify ([pscustomobject]@{path=$imagesPath;bytes=972;sha256='dad288000a126c08c3d6ec81e0f30e7720321a0c201f37b9ea52be33532edf70'})
 $images=Get-Content -LiteralPath $imagesPath -Raw|ConvertFrom-Json
 if($images.Count -ne 5){throw 'Current image count mismatch'}
 foreach($pin in $images){Verify $pin}
 $artifact=Get-Content -LiteralPath $plan.artifact_receipt.path -Raw|ConvertFrom-Json
 foreach($pin in $artifact.sources){Verify $pin}
 $pre=[ordered]@{attempt=$plan.attempt;decision_sha256='0cb091bd7df90394581c1e7b3b126d143867954e9be4560272fb373cef66a85f';source_input_count=743;source_before_after_sha256='3ce85def1fd0d10eef8802517182ce3667c10e732b64199a50f0c66dca1f8ca3';reviewed_inputs_current=$true;composition_sources_current=$true;images=@($images);work_seconds=210;whole_seconds=235;native_outer_seconds=240;retries=0;network=$false;rebuild=$false}
 WriteNew ($prefix+'.preflight.json') ($pre|ConvertTo-Json -Depth 8)
 $arguments=@('-NoLogo','-NoProfile','-NonInteractive','-OutputFormat','Text','-File',"$w\scripts\invoke-migration-s53-cold-b.ps1",'-HarnessSha256',$plan.stable_harness.sha256,'-AssemblySha256',$plan.assembly.sha256,'-ComposedSha256',$plan.composed.sha256,'-DecisionSha256','0cb091bd7df90394581c1e7b3b126d143867954e9be4560272fb373cef66a85f')
 $invocation="# Exact reviewed child invocation; owned outer240s capture."+[Environment]::NewLine+"& '"+$plan.shell.path+"' "+(($arguments|ForEach-Object{"'"+$_.Replace("'","''")+"'"}) -join ' ')+[Environment]::NewLine
 WriteNew ($prefix+'.invocation.ps1') $invocation
 [Diagnostics.Process]::GetCurrentProcess().PriorityClass=[Diagnostics.ProcessPriorityClass]::BelowNormal
 [Reflection.Assembly]::LoadFrom((Join-Path $w $plan.assembly.path))|Out-Null
 $psi=[Diagnostics.ProcessStartInfo]::new($plan.shell.path);$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true
 foreach($a in $arguments){$psi.ArgumentList.Add($a)}
 $out=[IO.File]::Open($prefix+'.stdout.log',[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
 $err=[IO.File]::Open($prefix+'.stderr.log',[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
 $preflight=$true;$child=[Diagnostics.Process]::Start($psi)
 $capture=[S53RootCaptureR38]::new($child,$out,$err)
 $left=[Math]::Max(1,240000-[int]$watch.Elapsed.TotalMilliseconds)
 $timedOut=-not $child.WaitForExit($left)
 if($timedOut){$child.Kill($true);$null=$child.WaitForExit(1000)}
 while(-not $capture.Completed -and $watch.Elapsed.TotalMilliseconds -lt 240000){[Threading.Thread]::Sleep(10)}
 if($capture.Completed){$null=$capture.CloseCompleted();$captureClosed=$true;$out=$null;$err=$null}else{throw 'Bounded outer capture incomplete'}
 $child.Refresh();$exitCode=if($timedOut){124}else{$child.ExitCode}
} catch {
 $outerFailure=if($preflight){'owned_invocation_failed_or_unknown'}else{'public_preflight_failed'};$exitCode=1
} finally {
 if($child -and -not $child.HasExited){try{$child.Kill($true);$null=$child.WaitForExit(1000)}catch{$outerFailure='owned_cleanup_unknown';$exitCode=1}}
 if($out){$out.Dispose()};if($err){$err.Dispose()}
}
$ownedExited=if($child){$child.HasExited}else{$null}
$elapsed=$watch.Elapsed.TotalSeconds
$ownedChildren=$null
if($captureClosed){$safe=Get-Content -LiteralPath ($prefix+'.stdout.log') -Raw;$matches=[regex]::Matches($safe,'(?m)^owned_children_after=(\d+)\r?$');if($matches.Count -eq 1){$ownedChildren=[int]$matches[0].Groups[1].Value}}
$result=[ordered]@{attempt='P2-s53-exhausted-cold-b-01-attempt-02';start_utc=$start.ToString('O');end_utc=[DateTimeOffset]::UtcNow.ToString('O');whole_seconds=$elapsed;work_seconds=210;inner_whole_seconds=235;native_outer_limit_seconds=240;timed_out=$timedOut;preflight_passed=$preflight;owned_shell_pid=if($child){$child.Id}else{$null};owned_shell_exited=$ownedExited;outer_capture_closed=$captureClosed;inner_owned_children_after=$ownedChildren;exit_code=$exitCode;failure=$outerFailure;network=$false;rebuild=$false;retry=0;priority='BelowNormal';decision_sha256='0cb091bd7df90394581c1e7b3b126d143867954e9be4560272fb373cef66a85f'}
WriteNew ($prefix+'.result.json') ($result|ConvertTo-Json -Depth 6)
$result|ConvertTo-Json -Depth 6
if($child){$child.Dispose()}
exit $exitCode

