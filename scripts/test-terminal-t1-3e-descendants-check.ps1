param([Parameter(Mandatory)][string]$Evidence)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
if(Test-Path -LiteralPath $Evidence){throw 'Fresh descendant evidence required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
# Reuse the existing strict expected-crash policy fixtures. Payloads are synthetic;
# the owner/leaf processes, retained handles, Job queries and production boundary are real.
. "$PSScriptRoot/test-terminal-t1-3e-sender-crash-policy.ps1" -Evidence $Evidence -FixtureLibrary
$tokens=$null;$errors=$null
$agentPath=Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-agent.ps1'
$ast=[Management.Automation.Language.Parser]::ParseFile($agentPath,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Agent syntax'}
foreach($name in @('Lab-OwnerClean','Assert-LabCrashDescendants','Get-LabOwnerExitVerdict')){
    $nodes=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name},$false))
    if($nodes.Count -ne 1){throw "Production function missing: $name"}
    . ([ScriptBlock]::Create($nodes[0].Extent.Text))
}
$exe=Join-Path $Evidence 'descendant-fixture.exe'
Add-Type -OutputAssembly $exe -OutputType ConsoleApplication -TypeDefinition @'
using System; using System.Diagnostics; using System.IO; using System.Threading; using System.Runtime.InteropServices;
public static class DescendantFixture {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Startup {
  public int cb;public string reserved,desktop,title;public int x,y,cx,cy,xc,yc,fill,flags;
  public short show,cbReserved;public IntPtr reserved2,input,output,error;
 }
 [StructLayout(LayoutKind.Sequential)] struct Info {public IntPtr process,thread;public int pid,tid;}
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcess(
  string app,System.Text.StringBuilder cmd,IntPtr pa,IntPtr ta,bool inherit,uint flags,IntPtr env,string cwd,ref Startup s,out Info p);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
 public static void Main(string[] a) {
  if(a[0]=="leaf") { File.WriteAllText(a[1],Process.GetCurrentProcess().Id.ToString());
   while(!File.Exists(a[2])) Thread.Sleep(10); Thread.Sleep(250); return; }
  if(a[0]=="owner") { var s=new Startup();s.cb=Marshal.SizeOf(s);Info p;
   var cmd=new System.Text.StringBuilder("\""+Process.GetCurrentProcess().MainModule.FileName+"\" leaf \""+a[1]+"\" \""+a[2]+"\"");
   if(!CreateProcess(null,cmd,IntPtr.Zero,IntPtr.Zero,false,0x08000000,IntPtr.Zero,null,ref s,out p)) throw new System.ComponentModel.Win32Exception();
   CloseHandle(p.thread);CloseHandle(p.process);
   while(!File.Exists(a[3])) Thread.Sleep(10); Environment.Exit(1); }
  Environment.Exit(Int32.Parse(a[0]));
 }
}
'@
$watch=[KilogramS5Deadline]::new(60,75,[KilogramS5Deadline]::TickNow())
$owned=@();$cases=@();$passed=$false
function Start-CaseOwner([string]$Name,[switch]$Leaf,[int]$Exit=1){
    $dir=Join-Path $Evidence ($Name+'-process');[void][IO.Directory]::CreateDirectory($dir)
    $ready=Join-Path $dir 'leaf.pid';$release=Join-Path $dir 'leaf.release';$stop=Join-Path $dir 'owner.stop'
    $si=[Diagnostics.ProcessStartInfo]::new();$si.FileName=$exe
    $si.Arguments=if($Leaf){'owner "'+$ready+'" "'+$release+'" "'+$stop+'"'}else{[string]$Exit}
    $si.UseShellExecute=$false;$si.CreateNoWindow=$true;$si.RedirectStandardOutput=$true;$si.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($si);$script:owned+=,$p;$watch.VerifyChild($p)
    $script:hostChild=$p;$script:hostStart=$p.StartTime.ToFileTimeUtc();$handle=$p.Handle
    $ot=$p.StandardOutput.ReadToEndAsync();$et=$p.StandardError.ReadToEndAsync();$leafPid=$null
    if($Leaf){
        $until=[KilogramS5Deadline]::TickNow()+3000
        while(!(Test-Path -LiteralPath $ready)){if([KilogramS5Deadline]::TickNow() -ge $until){throw 'Leaf readiness'};Start-Sleep -Milliseconds 10}
        $leafPid=[int][IO.File]::ReadAllText($ready)
        if($leafPid -notin @($watch.Children())){throw 'Leaf not in native Job'}
        [IO.File]::WriteAllText($stop,'natural owner exit')
    }
    if(!$p.WaitForExit(3000) -or !$ot.Wait(1000) -or !$et.Wait(1000) -or $et.Result){
        Lab-Write (Join-Path $dir 'failure.json') @{has_exited=$p.HasExited;stdout_complete=$ot.IsCompleted;stderr_complete=$et.IsCompleted;stderr=$(if($et.IsCompleted){$et.Result}else{$null})}
        throw 'Owner exit/output drain'
    }
    return @{p=$p;release=$release;leaf=$leafPid;retained_handle=$handle.ToInt64()}
}
function View-Watch([string]$Release='',[switch]$QueryError,[switch]$ShortBudget){
    $v=[pscustomobject]@{Native=$watch;WorkTick=$watch.WorkTick;Release=$Release;QueryError=[bool]$QueryError;Calls=0;ShortBudget=[bool]$ShortBudget}
    if($ShortBudget){$v.WorkTick=[KilogramS5Deadline]::TickNow()+100}
    $v|Add-Member ScriptMethod CheckWork {$this.Native.CheckWork();if($this.ShortBudget -and [KilogramS5Deadline]::TickNow() -ge $this.WorkTick){throw 'fixture remaining work cutoff'}}
    $v|Add-Member ScriptMethod Children {
        $this.Calls++
        if($this.QueryError){throw 'fixture native query failure'}
        $ids=@($this.Native.Children())
        if($this.Release -and $this.Calls -eq 1){[IO.File]::WriteAllText($this.Release,'natural leaf exit')}
        return $ids
    }
    return $v
}
function Invoke-Case($F,$V){$script:cfg=$F.cfg;Get-LabOwnerExitVerdict $V $F.owner $script:hostChild $F.run $F.entry 0}
function Drain-Leaf($P){
    if($P.leaf){[IO.File]::WriteAllText($P.release,'fixture teardown natural exit')}
    $until=[KilogramS5Deadline]::TickNow()+2000
    while(@($watch.Children()).Count){if([KilogramS5Deadline]::TickNow() -ge $until){throw 'Fixture natural drain failed'};Start-Sleep -Milliseconds 10}
}
try {
    $p=Start-CaseOwner 'empty';$f=Fixture 'empty';$v=View-Watch;$r=Invoke-Case $f $v
    $d=Lab-Read (Join-Path $f.run 'receipts/runtime-fault-descendants.json')
    if($r.clean -or !$r.expected -or $d.queries -ne 1 -or @($d.first.pids).Count -or @($d.last.pids).Count){throw 'Immediate empty verdict'}
    $cases+=@{name='immediate_empty_native_retained_handle';passed=$true;proof=$d};Drain-Leaf $p

    $p=Start-CaseOwner 'delayed' -Leaf;$f=Fixture 'delayed';$v=View-Watch -Release $p.release
    # Reproduce the former actual immediate boundary with a real nonempty owned Job.
    $oldRefused=$false;try{if(@($watch.Children()).Count){throw 'LAB_CRASH_DESCENDANTS'};$null=Assert-LabExpectedCrash $f.run $f.entry $f.owner $f.cfg $f.started $hostChild}catch{$oldRefused=$_.Exception.Message -ceq 'LAB_CRASH_DESCENDANTS'}
    $r=Invoke-Case $f $v;$d=Lab-Read (Join-Path $f.run 'receipts/runtime-fault-descendants.json')
    if(!$oldRefused -or !$r.expected -or $r.clean -or $p.leaf -notin @($d.first.pids) -or @($d.last.pids).Count -or $d.queries -lt 2 -or ($d.end_tick-$d.begin_tick) -gt 2100){throw 'Delayed natural native verdict'}
    $cases+=@{name='controlled_delayed_natural_exit_old_refuses_corrected_reconciles';passed=$true;proof=$d;field04_cause_proved=$false};Drain-Leaf $p

    foreach($name in @('persistent','query','budget')){
        $p=Start-CaseOwner $name -Leaf;$f=Fixture $name
        $v=if($name -ceq 'query'){View-Watch -QueryError}elseif($name -ceq 'budget'){View-Watch -ShortBudget}else{View-Watch}
        $reason=$null;try{$null=Invoke-Case $f $v}catch{$reason=$_.Exception.Message}
        $d=Lab-Read (Join-Path $f.run 'receipts/runtime-fault-descendants.json')
        if(!$reason -or $d.outcome -cne 'refused' -or (Test-Path -LiteralPath (Join-Path $f.run 'receipts/runtime-fault-reconciled.json')) -or $p.leaf -notin @($watch.Children())){throw "Unsafe refusal: $name"}
        if($name -ceq 'persistent' -and ($reason -cne 'LAB_CRASH_DESCENDANTS' -or $d.cutoff_tick-$d.begin_tick -ne 2000 -or $p.leaf -notin @($d.last.pids))){throw 'Persistent refusal bound'}
        if($name -ceq 'budget' -and ($d.cutoff_tick -ne $v.WorkTick -or $d.end_tick-$d.begin_tick -gt 500)){throw 'Existing remaining budget cap'}
        if($name -ceq 'query' -and ($d.queries -ne 0 -or $null -ne $d.first)){throw 'Query failure invented snapshot'}
        $cases+=@{name=$name;passed=$true;reason=$reason;proof=$d;leaf_still_live=$true};Drain-Leaf $p
    }
    # Empty Job is necessary, not sufficient: the original strict policy still refuses.
    $p=Start-CaseOwner 'wrong-session';$f=Fixture 'wrong-session';$f.entry.session='0'*32;$v=View-Watch
    $reason=$null;try{$null=Invoke-Case $f $v}catch{$reason=$_.Exception.Message}
    if(!$reason -or (Test-Path -LiteralPath (Join-Path $f.run 'receipts/runtime-fault-reconciled.json'))){throw 'Strict policy relaxed'}
    $cases+=@{name='strict_expected_policy_after_empty_refuses_wrong_session';passed=$true;reason=$reason}

    foreach($mode in @('ordinary-failed','ordinary-clean','ordinary-no-opt-in','already-consumed')){
        $p=Start-CaseOwner $mode -Exit $(if($mode -ceq 'ordinary-clean'){0}else{1});$f=Fixture $mode;$script:cfg=$f.cfg
        if($mode -cne 'already-consumed'){$f.owner.expected_runtime_crash=$false}
        if($mode -ceq 'ordinary-no-opt-in'){$f.owner.PSObject.Properties.Remove('expected_runtime_crash')}
        if($mode -ceq 'ordinary-clean'){
            $receipt=$f.receipt;$receipt.child_exit=0;$receipt.descriptor=$false;$receipt.error=$null;$receipt.mailbox_quit=$true
            Lab-Write (Join-Path $f.run 'receipts/owner-a-1.json') $receipt
        }
        $v=View-Watch -QueryError;$count=if($mode -ceq 'already-consumed'){1}else{0}
        $r=Get-LabOwnerExitVerdict $v $f.owner $hostChild $f.run $f.entry $count
        if($v.Calls -or $r.expected -or $r.clean -ne ($mode -ceq 'ordinary-clean') -or (Test-Path -LiteralPath (Join-Path $f.run 'receipts/runtime-fault-descendants.json'))){throw "Ordinary branch changed: $mode"}
        $cases+=@{name=$mode;passed=$true;queries=$v.Calls;clean=$r.clean}
    }
    # Actual native original WorkTick, not the injected short-budget view above.
    # Its existing watchdog remains the safety owner; the new passive function has
    # no cleanup authority. Do not attribute any watchdog expiry cleanup to drain.
    $outerWatch=$watch;$budgetWatch=$null
    try {
        $budgetWatch=[KilogramS5Deadline]::new(1.2,6,[KilogramS5Deadline]::TickNow());$watch=$budgetWatch
        $p=Start-CaseOwner 'native-budget' -Leaf;$f=Fixture 'native-budget';$reason=$null
        try{$null=Invoke-Case $f $watch}catch{$reason=$_.Exception.Message}
        $d=Lab-Read (Join-Path $f.run 'receipts/runtime-fault-descendants.json')
        if(!$reason -or $d.outcome -cne 'refused' -or $d.cutoff_tick -ne $watch.WorkTick -or
           $d.cutoff_tick-$d.begin_tick -ge 2000 -or (Test-Path -LiteralPath (Join-Path $f.run 'receipts/runtime-fault-reconciled.json'))){throw 'Native remaining work cutoff not preserved'}
        $cases+=@{name='native_original_work_budget_expiry';passed=$true;reason=$reason;proof=$d;native_watchdog_unchanged=$true}
    } finally {
        if($budgetWatch){$budgetWatch.CleanupChildren(2000);$budgetEmpty=@($budgetWatch.Children()).Count -eq 0;$budgetWatch.Finish();if(!$budgetEmpty){throw 'Budget fixture cleanup'}}
        $watch=$outerWatch
    }
    $passed=$true
} finally {
    $beforeCleanup=@($watch.Children());$watch.CleanupChildren(3000);$empty=@($watch.Children()).Count -eq 0;$watch.Finish()
    foreach($p in $owned){$p.Dispose()}
    Lab-Write (Join-Path $Evidence 'result.json') @{passed=$passed;cases=$cases;empty_job=$empty;fixture_cleanup_required=($beforeCleanup.Count -gt 0);
        source_sha256=(Lab-Hash $agentPath);field04_cause_proved=$false;product_recovery_test=$false}
}
if(!$passed -or !$empty){throw 'Descendants boundary checks failed'}
@{passed=$true;cases=$cases.Count;empty_job=$empty}|ConvertTo-Json -Compress
