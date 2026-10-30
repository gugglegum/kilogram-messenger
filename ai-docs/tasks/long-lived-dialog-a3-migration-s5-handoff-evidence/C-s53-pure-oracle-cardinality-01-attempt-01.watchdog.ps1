# Pure public-data hang safeguard. No atomic containment or real-time guarantee.
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
$e=$workspace+'\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence'
$attempt='C-s53-pure-oracle-cardinality-01-attempt-01'
$check=$e+'\'+$attempt+'.check.ps1'
$cases=$e+'\'+$attempt+'.cases.json'
$shell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$child=$null;$out=$null;$err=$null;$started=$false;$startAttempted=$false
$reaped=$false;$exitCode=$null;$timedOut=$false;$killAttempted=$false;$cleanupUnknown=$false
$primary='not_started';$primaryType='';$primaryHr=0;$reportBytes=0;$reportHash='';$reportValid=$false
$processClosed=$false;$watch=[Diagnostics.Stopwatch]::new()
function Need([bool]$Condition,[string]$Code){
 if(-not $Condition){$x=[InvalidOperationException]::new('Pure oracle supervisor');$x.Data['code']=$Code;throw $x}
}
function Pin([string]$Path,[int]$Length,[string]$Hash) {
 $b=[IO.File]::ReadAllBytes($Path)
 Need ($b.Length -eq $Length -and [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b)).ToLowerInvariant() -ceq $Hash) 'source_pin'
}
function Drain([IO.StreamReader]$Reader) {
 $b=[char[]]::new(1024)
 return [pscustomobject]@{Reader=$Reader;Buffer=$b;Text=[Text.StringBuilder]::new(4096);Task=$Reader.ReadAsync($b,0,1024);Eof=$false;Cap=$false;Closed=$false;CloseAttempted=$false}
}
function Pump($D){
 if($null -eq $D -or $D.Eof -or $D.Cap -or -not $D.Task.IsCompleted){return}
 $n=$D.Task.GetAwaiter().GetResult()
 if($n -eq 0){$D.Eof=$true;return}
 if($D.Text.Length+$n -gt 4096){$D.Cap=$true;throw [InvalidOperationException]::new('pipe_cap')}
 $null=$D.Text.Append($D.Buffer,0,$n);$D.Task=$D.Reader.ReadAsync($D.Buffer,0,1024)
}
try {
 Need ($PSVersionTable.PSVersion.ToString() -ceq '7.6.5') 'shell_version'
 Pin $shell 301368 '362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139'
 Pin $check 24277 '74a0d3dcde7c218ee3d3b538a13ed5159b3431567f6cf4fc997b866de1db958a'
 foreach($suffix in @('cases.json','actual-invocation.json','native-return.json','native-history.json')){
  $path=$e+'\'+$attempt+'.'+$suffix
  Need (-not [IO.File]::Exists($path) -and -not [IO.Directory]::Exists($path)) 'output_collision'
 }
 $psi=[Diagnostics.ProcessStartInfo]::new($shell)
 $psi.UseShellExecute=$false;$psi.CreateNoWindow=$true;$psi.WorkingDirectory=$workspace
 $psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true
 foreach($arg in @('-NoLogo','-NoProfile','-NonInteractive','-File',$check)){$null=$psi.ArgumentList.Add($arg)}
 $child=[Diagnostics.Process]::new();$child.StartInfo=$psi
 $watch.Start();$startAttempted=$true;$started=$child.Start()
 Need $started 'start_failed'
 $out=Drain $child.StandardOutput;$err=Drain $child.StandardError
 while(-not $reaped -and $watch.Elapsed.TotalSeconds -lt 30){
  Pump $out;Pump $err;$reaped=$child.WaitForExit(10)
 }
 if(-not $reaped){$timedOut=$true;Need $false 'timeout'}
 $exitCode=$child.ExitCode
 $primary=if($exitCode -eq 0){'none'}else{'child_exit'}
}catch{
 $original=$_
 $primary=if($original.Exception.Data.Contains('code')){[string]$original.Exception.Data['code']}else{'unexpected'}
 $primaryType=$original.Exception.GetType().FullName;$primaryHr=[int]$original.Exception.HResult
}
finally{
 # One cleanup window, not a renewed test budget. Kill only this exact owned child.
 $cleanup=[Diagnostics.Stopwatch]::StartNew()
 if($child -and $startAttempted){
  try{
   if(-not $reaped){
    $killAttempted=$true
    if(-not $child.HasExited){$child.Kill()}
   }
   while(-not $reaped -and $cleanup.Elapsed.TotalSeconds -lt 5){
    if($out){Pump $out};if($err){Pump $err};$reaped=$child.WaitForExit(10)
   }
   if($reaped){$exitCode=$child.ExitCode}else{$cleanupUnknown=$true}
  }catch{$cleanupUnknown=$true}
  try{
   while($cleanup.Elapsed.TotalSeconds -lt 5 -and (($out -and -not $out.Eof -and -not $out.Cap) -or ($err -and -not $err.Eof -and -not $err.Cap))){
    if($out){Pump $out};if($err){Pump $err};[Threading.Thread]::Sleep(5)
   }
  }catch{$cleanupUnknown=$true}
  if(-not $out -or -not $err -or -not $out.Eof -or -not $err.Eof){$cleanupUnknown=$true}
 }
 foreach($d in @($out,$err)){
  if($d -and -not $d.CloseAttempted){
   $d.CloseAttempted=$true
   try{$d.Reader.Dispose();$d.Closed=$true}catch{$cleanupUnknown=$true}
  }
 }
 if($child){
  try{$child.Dispose();$processClosed=$true}catch{$cleanupUnknown=$true}
 }
}
# Reports remain evidence on failure; do not overwrite, repair or rerun.
if($started -and $reaped -and [IO.File]::Exists($cases)){
 try{
  $b=[IO.File]::ReadAllBytes($cases);$reportBytes=$b.Length
  Need ($b.Length -gt 0 -and $b.Length -le 49152) 'report_cap'
  $reportHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b)).ToLowerInvariant()
  $r=ConvertFrom-Json -InputObject ([Text.UTF8Encoding]::new($false,$true).GetString($b))
  Need ($r.attempt -ceq $attempt -and $r.result -ceq 'observed' -and $r.primary_code -ceq 'none' -and
   $r.evaluations -eq 92 -and $r.checker_calls -eq 68 -and $r.rows.Count -eq 92 -and $r.localizations.Count -eq 2 -and
   $r.input_returns -eq 3 -and $r.input_closed -eq 3 -and $r.input_unknown -eq $false -and
   $r.synthetic_directory_final -eq $true -and $r.all38_assertions_retained -eq $true -and $r.original_r73_error_reconstructed -eq $false) 'report'
  for($i=0;$i -lt 92;$i++){Need ($r.rows[$i].i -eq $i -and $r.rows[$i].ok -eq $true -and $r.rows[$i].bad -eq $false) 'row'}
  Need (@($r.rows|Where-Object g -CEQ 'S').Count -eq 24 -and @($r.rows|Where-Object g -CEQ 'B').Count -eq 16 -and @($r.rows|Where-Object g -CEQ 'N').Count -eq 52) 'matrix'
  for($i=0;$i -lt 2;$i++){Need ($r.localizations[$i].complete -eq $true -and $r.localizations[$i].bad -eq $false -and
   $r.localizations[$i].trunc -eq $false -and $r.localizations[$i].ml -eq @(1041,207)[$i]) 'localization'}
  $reportValid=$true
 }catch{
  if($primary -ceq 'none'){$primary='report_failed';$primaryType=$_.Exception.GetType().FullName;$primaryHr=[int]$_.Exception.HResult}
 }
}
$pipeOk=($out -and $err -and $out.Eof -and $err.Eof -and $out.Closed -and $err.Closed -and -not $out.Cap -and -not $err.Cap)
$ok=($primary -ceq 'none' -and $started -and $reaped -and $exitCode -eq 0 -and -not $timedOut -and -not $killAttempted -and
 -not $cleanupUnknown -and $processClosed -and $pipeOk -and $out.Text.Length -eq 0 -and $err.Text.Length -eq 0 -and $reportValid)
$status=[ordered]@{
 schema=1;attempt=$attempt;result=if($ok){'observed'}else{'failed_or_unknown'}
 primary=$primary;primary_type=$primaryType;primary_hresult=$primaryHr
 started=$started;child_exit=$exitCode;reaped=$reaped;timeout=$timedOut;kill_attempted=$killAttempted;cleanup_unknown=$cleanupUnknown
 process_closed=$processClosed;pipes_closed=[bool]$pipeOk;timeout_seconds=30;reap_seconds=5;elapsed_seconds=$watch.Elapsed.TotalSeconds
 stdout=if($out){$out.Text.ToString()}else{''};stderr=if($err){$err.Text.ToString()}else{''}
 worker_report_bytes=$reportBytes;worker_report_sha256=$reportHash;worker_report_valid=$reportValid
}
[Console]::Out.WriteLine((ConvertTo-Json -InputObject $status -Depth 8 -Compress))
if($ok){exit 0}
exit 1
