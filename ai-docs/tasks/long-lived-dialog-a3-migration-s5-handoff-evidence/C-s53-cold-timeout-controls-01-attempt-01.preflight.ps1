# SOURCE ONLY until root review and a separate C20/30 allocation.
# Three fixed public dummy scenarios; no Core/private entrypoint/network.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$lf=[string][char]10
$w='E:\AI-Projects\kilogram-p2p-messenger'
$taskRoot="$w\ai-docs\tasks";$e="$taskRoot\long-lived-dialog-a3-migration-s5-handoff-evidence"
$fixedShell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$entry="$w\scripts\invoke-migration-s53-cold-preservation.ps1"
$corePath="$w\scripts\migration-s53-cold-preservation.ps1";$outputPath="$w\scripts\migration-s53-cold-output.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1";$deadlinePath="$w\scripts\migration-s5-l1-deadline.ps1"
$prefix='C-s53-cold-timeout-controls-01-attempt-01'
$rootStart=[Diagnostics.Stopwatch]::GetTimestamp();$rootSelf=[Diagnostics.Process]::GetCurrentProcess()
$control=[hashtable]::Synchronized(@{Child=$null;Forced=$false;Unknown=$false})
$done=[Threading.ManualResetEvent]::new($false)
$guardian=$null;$ticket=$null;$cases=[Collections.Generic.List[object]]::new()
function Assert-Control([bool]$Value,[string]$Label){if(-not $Value){throw ('Timeout control failed: '+$Label)}}
function Hash-Text([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Write-Fresh([string]$Path,[string]$Text) {
    $b=[Text.UTF8Encoding]::new($false).GetBytes($Text);Assert-Control ($b.Length -le 65536) 'public fixture cap'
    $f=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try{$f.Write($b,0,$b.Length);$f.Flush($true)}finally{$f.Dispose()}
}
function Check-Total{Assert-Control (([Diagnostics.Stopwatch]::GetTimestamp()-$rootStart)/[double][Diagnostics.Stopwatch]::Frequency -lt 20) 'total work20'}
Assert-Control ($IsWindows -and [Environment]::ProcessPath -ceq $fixedShell -and [Threading.Thread]::CurrentThread.GetApartmentState().ToString() -ceq 'MTA') 'fixed host'
# Byte-verified copy of the actual guardian body arms BEFORE source reads/AST/bootstrap.
$bootstrapGuardian=@'

    param($Owner,$Control,$Done,[long]$Start,[bool]$Parent)
    try {
        while(-not $Done.WaitOne(20)){
            $age=([Diagnostics.Stopwatch]::GetTimestamp()-$Start)/[double][Diagnostics.Stopwatch]::Frequency
            if(-not $Parent -and $age -ge 20){
                $Control.Forced=$true;$p=$Control.Child
                if($p){try{if(-not $p.HasExited){$p.Kill()};$null=$p.WaitForExit(20)}catch{$Control.Unknown=$true}}
            }
            if($age -ge $(if($Parent){51}else{29})){$Owner.Kill();return}
        }
    }catch{try{$Owner.Kill()}catch{}}

'@
$guardian=[powershell]::Create()
$null=$guardian.AddScript($bootstrapGuardian).AddArgument($rootSelf).AddArgument($control).AddArgument($done).AddArgument($rootStart).AddArgument($false)
$ticket=$guardian.BeginInvoke()
try {
$raw=[IO.File]::ReadAllBytes($entry)
Assert-Control ($raw.Length -eq 76869 -and
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)).ToLowerInvariant() -ceq '620753ecddd1ff1b1d07fa5d3868c8fdfa7170d7770f407165e112ade8551fad') 'source pin'
$text=[Text.UTF8Encoding]::new($false,$true).GetString($raw);$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseInput($text,[ref]$tokens,[ref]$errors)
Assert-Control ($errors.Count -eq 0) 'source syntax'
$all=@($ast.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst]},$true))
$super=@($all | Where-Object Name -CEQ 'Invoke-Supervisor');Assert-Control ($super.Count -eq 1) 'supervisor selector'
# Closed literal mappings are inverted exactly; no predicate/body reimplementation.
$maps='{"Check":[["else{45}","else{2}"]],"Complete-ChildCleanup":[["(Age) -lt 51","(Age) -lt 5"],["''cleanup'' 51","''cleanup'' 5"],["(Age) -lt 52","(Age) -lt 6"],["''cleanup'' 52","''cleanup'' 6"],["(Age) -lt 54","(Age) -lt 7"]],"Check-DiagnosticBindings":[["50L","4L"],["-lt 50","-lt 4"]],"loop":[["''normal'' 45","''normal'' 2"],["(45-(Age))","(2-(Age))"]],"guardian":[["-ge 55","-ge 20"],["else{59}","else{29}"]]}' | ConvertFrom-Json -AsHashtable
$definitions=[ordered]@{};$mapping=[Collections.Generic.List[object]]::new()
$names=@('Stop-Cold','Need','Age','Check','Cause','Public-Register','Public-Open','Public-Close','Public-CloseAll','Read-Bytes','Hash-Stream','Decode','Unique-Json','Keys','Fields','Value','Exact','Array-Exact','Pin-Spec','Validate-Core','Common-Report','Report-Fields','Validate-Cause','Validate-SourceMap','Validate-Report','Write-To','Write-Report','Read-Report','Await-Start','Output-Facts','Parent-Facts','New-Drain','Pump','Set-FirstReason','Note-ExitIndication','Note-EOF','Completion-Gate','Record-Wait','Wait-ForReap','Sample-Child','Complete-ChildCleanup','Check-DiagnosticBindings','Collect-FailedReports')
foreach($name in $names) {
    $node=@($all | Where-Object Name -CEQ $name);Assert-Control ($node.Count -eq 1) ('definition '+$name)
    $original=$node[0].Extent.Text;$mapped=$original
    if($maps.Contains($name)){foreach($pair in $maps[$name]){Assert-Control ($mapped.Contains($pair[0])) ('map '+$name);$mapped=$mapped.Replace($pair[0],$pair[1])}}
    $inverse=$mapped
    if($maps.Contains($name)){foreach($pair in $maps[$name]){$inverse=$inverse.Replace($pair[1],$pair[0])}}
    Assert-Control ($inverse -ceq $original) ('inverse '+$name)
    $definitions[$name]=$mapped
    $mapping.Add([ordered]@{name=$name;source_sha256=(Hash-Text $original);mapped_sha256=(Hash-Text $mapped);inverse_sha256=(Hash-Text $inverse)})
    . ([scriptblock]::Create($mapped))
}
$loop=@($super[0].Body.FindAll({param($n) $n -is [Management.Automation.Language.WhileStatementAst] -and $n.Extent.Text.Contains('Sample-Child')},$true))
$gate=@($super[0].Body.FindAll({param($n) $n -is [Management.Automation.Language.CommandAst] -and $n.GetCommandName() -ceq 'Need' -and $n.Extent.Text.EndsWith("'completion_gate'")},$true))
$decision=@($super[0].Body.FindAll({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$good' -and $n.Right.Extent.Text.Contains('normalCandidate')},$true))
$failure=@($super[0].Body.FindAll({param($n) $n -is [Management.Automation.Language.CatchClauseAst] -and $n.Body.Extent.Text.Contains('Set-FirstReason')},$true))
$guardNode=@($ast.FindAll({param($n) $n -is [Management.Automation.Language.InvokeMemberExpressionAst] -and $n.Member.Extent.Text -ceq 'AddScript'},$true))
Assert-Control ($loop.Count -eq 1 -and $gate.Count -eq 1 -and $decision.Count -eq 1 -and $failure.Count -eq 1 -and $guardNode.Count -eq 1) 'fragment selectors'
$fragments=[ordered]@{loop=$loop[0].Extent.Text;gate=$gate[0].Extent.Text;decision=$decision[0].Extent.Text
    failure=($failure[0].Body.Statements | ForEach-Object {$_.Extent.Text}) -join $lf
    guardian=$guardNode[0].Arguments[0].Extent.Text}
foreach($name in @('metrics','completion')){
    $n=@($super[0].Body.FindAll({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq ('$script:'+$name)},$true))
    Assert-Control ($n.Count -eq 1) ('init '+$name);$fragments[$name]=$n[0].Extent.Text
}
foreach($name in @($fragments.Keys)){
    $original=$fragments[$name];$mapped=$original
    if($maps.Contains($name)){foreach($pair in $maps[$name]){Assert-Control ($mapped.Contains($pair[0])) ('map '+$name);$mapped=$mapped.Replace($pair[0],$pair[1])}}
    $inverse=$mapped
    if($maps.Contains($name)){foreach($pair in $maps[$name]){$inverse=$inverse.Replace($pair[1],$pair[0])}}
    Assert-Control ($inverse -ceq $original) ('inverse '+$name)
    $mapping.Add([ordered]@{name=$name;source_sha256=(Hash-Text $original);mapped_sha256=(Hash-Text $mapped);inverse_sha256=(Hash-Text $inverse)})
    $fragments[$name]=$mapped
}
Assert-Control ($bootstrapGuardian -ceq $fragments.guardian.Substring(1,$fragments.guardian.Length-2)) 'bootstrap guardian provenance'
    $manifest='[{"role":"launcher","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\invoke-migration-s53-cold-preservation.ps1","bytes":76869,"sha256":"620753ecddd1ff1b1d07fa5d3868c8fdfa7170d7770f407165e112ade8551fad"},{"role":"core","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-cold-preservation.ps1","bytes":82999,"sha256":"eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f"},{"role":"output","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-cold-output.ps1","bytes":21721,"sha256":"ced08d88656e7bad142d1e23a826c42e7be4f75da8380c838167610504db7ae0"},{"role":"root_native","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s53-root-copy.ps1","bytes":25596,"sha256":"a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382"},{"role":"deadline","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\scripts\\migration-s5-l1-deadline.ps1","bytes":6038,"sha256":"0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45"},{"role":"shell","path":"C:\\Users\\Paul\\.cache\\codex-runtimes\\codex-primary-runtime\\dependencies\\native\\powershell\\pwsh.exe","bytes":301368,"sha256":"362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139"},{"role":"r42","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md","bytes":20846,"sha256":"20e427de24a510c12fbd328350edb61bdc9d8c2e8752291423c37eaf6d9655c1"},{"role":"r43","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md","bytes":9037,"sha256":"03e4954c7627b9074717231ba616c3725dc7653b8382ad318c0da0c44d595bb3"},{"role":"r44","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-r43-acceptance-and-cold-core-task-r44.md","bytes":7498,"sha256":"eccdf671997812dd699e99fc7e600e178f208b0cd2b5340863707b7fed03ff79"},{"role":"r41","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\S5.3-r41-rootcopies.root-review.json","bytes":7566,"sha256":"193436d3bd2ac5223ef4af74beaf18b66d32b59e50a303a52b137ff52d2c5780"},{"role":"small","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\S5.3-r55-cold-small.root-review.json","bytes":4869,"sha256":"8693c60a808f2c054871fd3f5c05d7ca3e79e08ef3e95e842a4a16bcbf8863a2"},{"role":"c2r","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-core-producer-size-01-attempt-02.accepted.md","bytes":3255,"sha256":"892b4085172b1c4a2b2cc4a4e9a6c162176f57f6417362be904781a6d21d03b2"},{"role":"c2s","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\C-s53-cold-output-wrapper-01-attempt-01.accepted.md","bytes":3704,"sha256":"b8a9d0532ea3f90209b0dd5bcc2d67fa270eda771460d3ac7e1f3f7763eff5ce"},{"role":"delta","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\P2-s53-exhausted-cold-a-01.integration-delta.md","bytes":10614,"sha256":"26bd1c419b61cc35eb11612cc38648c3c2b4d530c69e571468e51b938ac2641c"},{"role":"integration_accepted","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\P2-s53-exhausted-cold-a-01.integration-accepted.md","bytes":4365,"sha256":"b0611fef9a040cfe6dc1189f48c59a82dc910009789835ee7559f8cb8a4e58ab"},{"role":"reserve_amendment","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\P2-s53-exhausted-cold-a-01.control-reserve-amendment.md","bytes":2567,"sha256":"0d4cb8e9f127454692b88d6977b45c82520274d8d1dbf804cea1639a76db842b"},{"role":"source_review","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\P2-s53-exhausted-cold-a-01.integration-source-review.md","bytes":16909,"sha256":"93178bb38eb5e031bb276261968feeb463b2be2dcb4fbf816f129ea762d719e4"},{"role":"source_acceptance","path":"E:\\AI-Projects\\kilogram-p2p-messenger\\ai-docs\\tasks\\long-lived-dialog-a3-migration-s5-handoff-evidence\\P2-s53-exhausted-cold-a-01.integration-source-accepted.md","bytes":4655,"sha256":"85f7f1049712d8f2abeda152e4faa5c94e861813c85fdfca65f79807346329c0"}]' | ConvertFrom-Json
    $sourcePins=[ordered]@{}
    foreach($pin in $manifest){
        Check-Total;$b=[IO.File]::ReadAllBytes($pin.path)
        Assert-Control ($b.Length -eq $pin.bytes -and [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b)).ToLowerInvariant() -ceq $pin.sha256) ('public pin '+$pin.role)
        $sourcePins[$pin.role]=$pin.sha256
    }
    foreach($suffix in @('child.ps1','cases.json')){Assert-Control (-not [IO.File]::Exists("$e\$prefix.$suffix") -and -not [IO.Directory]::Exists("$e\$prefix.$suffix")) 'public collision'}
    foreach($kind in @('normal','grace','forced')){foreach($suffix in @('context','started','claim','p2','parent')){
        Assert-Control (-not [IO.File]::Exists("$e\$prefix.$kind.$suffix.json") -and -not [IO.Directory]::Exists("$e\$prefix.$kind.$suffix.json")) 'public collision'
    }}
    $childSetup=@'
param([ValidateSet('normal','grace','forced')][string]$Kind)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$w='E:\AI-Projects\kilogram-p2p-messenger'
$taskRoot="$w\ai-docs\tasks";$e="$taskRoot\long-lived-dialog-a3-migration-s5-handoff-evidence"
$fixedShell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
$entry="$w\scripts\invoke-migration-s53-cold-preservation.ps1";$corePath="$w\scripts\migration-s53-cold-preservation.ps1";$outputPath="$w\scripts\migration-s53-cold-output.ps1"
$rootNative="$w\scripts\migration-s53-root-copy.ps1";$deadlinePath="$w\scripts\migration-s5-l1-deadline.ps1"
$attempt="C-s53-cold-timeout-controls-01-attempt-01.$Kind"
$raw=[IO.File]::ReadAllBytes("$e\$attempt.context.json");if($raw.Length -gt 16384){throw 'context cap'}
$cfg=[Text.UTF8Encoding]::new($false,$true).GetString($raw) | ConvertFrom-Json -Depth 12 -DateKind String
$StartTimestamp=[long]$cfg.start;$DecisionSha256=$cfg.decision;$sourcePins=[ordered]@{}
foreach($property in $cfg.pins.PSObject.Properties){$sourcePins[$property.Name]=$property.Value}
$cleanupReserve=$false;$cleanupLimit=4;$guard=$null;$managedCloseUnknown=$false;$publicOwned=0;$publicPeak=0
$publicRecords=[Collections.Generic.List[object]]::new()
$self=[Diagnostics.Process]::GetCurrentProcess();$outputOwner=$null;$parentGuards=$null
$preparedHandles=0;$ownerDisposed=$false;$guardsDisposed=$false;$coreFinal=$null
'@
    $childBody=@'
$start=Await-Start
$claim=Common-Report 'claim';$claim.supervisor_pid=$cfg.supervisor_pid;$claim.parent_pid=$self.Id
$claim.parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o');$claim.started_sha256=$start.Sha256
$null=Write-Report 'claim' $claim;Public-CloseAll
if($Kind -ceq 'forced'){[Threading.Thread]::Sleep([Threading.Timeout]::Infinite)}
$target=if($Kind -ceq 'normal'){1}else{3}
$left=[Math]::Floor(($target-(Age))*1000);if($left -gt 0){[Threading.Thread]::Sleep([int]$left)}
if($Kind -ceq 'normal'){$self.Dispose();exit 0}
$cleanupReserve=$true
try{Stop-Cold 'work_expired'}catch{$cause=Cause $_.Exception}
$p=Common-Report 'p2';$p.result='failed_or_unknown';$p.failure='work_expired';$p.cause=[ordered]@{primary=$cause;cleanup=$null}
$p.source_before=$sourcePins;$p.source_after=[ordered]@{};$p.core=$null;$p.output=Output-Facts;$p.parent_guards=Parent-Facts
$p.compiler=[ordered]@{failed=$false;diagnostics=@();fresh_types=[ordered]@{};initializers=[ordered]@{}}
$p.process_checks=0;$p.managed_public_disposed=$true;$p.managed_public_close_native_observed=$false
$p.public_peak=$publicPeak;$p.public_remaining=$publicOwned;$p.core_dispose_attempted=$false;$p.wrapper_cleanup=$null
$p.external_bound=59;$p.interval_ended_before_receipts=$false;$p.p2_qpc=[Diagnostics.Stopwatch]::GetTimestamp()
$h=Write-Report 'p2' $p
$r=Common-Report 'parent';$r.result='failed_or_unknown';$r.failure='work_expired';$r.cause=$p.cause
$r.parent_pid=$self.Id;$r.parent_started_utc=$self.StartTime.ToUniversalTime().ToString('o');$r.p2_sha256=$h
$r.cleanup_known=$false;$r.core_remaining=$null;$r.output_remaining=0;$r.guard_remaining=0
$r.public_remaining=$publicOwned;$r.public_peak=$publicPeak;$r.managed_public_close_native_observed=$false
$r.job_finished=$false;$r.children_after=$null;$r.processes_disposed=$true;$r.observed_seconds=Age
$null=Write-Report 'parent' $r;Public-CloseAll
if($managedCloseUnknown -or $publicOwned -ne 0){exit 2}
$self.Dispose();exit 0 # Intentionally late0; host MUST retain failed timeout.
'@
    $childNames=@('Stop-Cold','Need','Age','Check','Cause','Public-Register','Public-Open','Public-Close','Public-CloseAll','Read-Bytes','Hash-Stream','Decode','Unique-Json','Keys','Fields','Value','Exact','Array-Exact','Pin-Spec','Validate-Core','Common-Report','Report-Fields','Validate-Cause','Validate-SourceMap','Validate-Report','Write-To','Write-Report','Read-Report','Await-Start','Output-Facts','Parent-Facts')
    $childDefinitions=($childNames | ForEach-Object {$definitions[$_]}) -join $lf
    Write-Fresh "$e\$prefix.child.ps1" ($childSetup+$lf+$childDefinitions+$lf+$childBody+$lf)
    $negativeChecks=0
    foreach($kind in @('normal','grace','forced')){
        Check-Total
        $attempt="$prefix.$kind";$DecisionSha256='f'*64;$self=$rootSelf
        $StartTimestamp=[Diagnostics.Stopwatch]::GetTimestamp();$workDeadline=$StartTimestamp+2L*[Diagnostics.Stopwatch]::Frequency
        $cleanupReserve=$false;$cleanupLimit=8;$guard=$null;$managedCloseUnknown=$false;$publicOwned=0;$publicPeak=0
        $publicRecords=[Collections.Generic.List[object]]::new();$out=$null;$err=$null;$reaped=$false;$childExit=$null
        $exitQpc=0L;$reapQpc=0L;$eofQpc=0L;$nextSampleQpc=0L;$normalStage='completion'
        $waitTrace=[Collections.Generic.List[object]]::new()
        $child=$null;$childId=0;$childStartedUtc=$null;$retainedProcessHandle=0L
        $outputOwner=$null;$parentGuards=$null;$preparedHandles=0;$ownerDisposed=$false;$guardsDisposed=$false;$coreFinal=$null
        . ([scriptblock]::Create($fragments.metrics));. ([scriptblock]::Create($fragments.completion))
        $normalCandidate=$false;$failure=$null;$failureCause=$null
        Write-Fresh "$e\$attempt.context.json" (ConvertTo-Json -InputObject ([ordered]@{start=$StartTimestamp;decision=$DecisionSha256;pins=$sourcePins;supervisor_pid=$self.Id}) -Depth 12 -Compress)
        $info=[Diagnostics.ProcessStartInfo]::new($fixedShell);$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.WorkingDirectory=$w
        $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
        foreach($arg in @('-NoLogo','-NoProfile','-NonInteractive','-Mta','-File',"$e\$prefix.child.ps1",'-Kind',$kind)){$info.ArgumentList.Add($arg)}
        $child=[Diagnostics.Process]::new();$child.StartInfo=$info;$control.Child=$child
        Assert-Control ($child.Start()) 'dummy start'
        $childId=$child.Id;$childStartedUtc=$child.StartTime.ToUniversalTime().ToString('o');$retainedProcessHandle=$child.Handle.ToInt64()
        $out=New-Drain $child.StandardOutput;$err=New-Drain $child.StandardError
        $started=Common-Report 'started';$started.supervisor_pid=$self.Id;$started.supervisor_started_utc=$self.StartTime.ToUniversalTime().ToString('o')
        $started.parent_pid=$childId;$started.parent_started_utc=$childStartedUtc;$started.source_pins=$sourcePins
        $started.permission_consumed=$true;$started.work_seconds=45;$started.whole_seconds=60 # Schema fixture fields, NOT test grants.
        $startedHash=Write-Report 'started' $started
        try {
            . ([scriptblock]::Create($fragments.loop))
            $completion.normal_gate=Completion-Gate
            . ([scriptblock]::Create($fragments.gate))
            $completion.normal_gate_passed=$true;$normalCandidate=$true
        }catch{. ([scriptblock]::Create($fragments.failure))}
        $completion.pre_cleanup_gate=Completion-Gate;$completion.pre_cleanup_first_reason=$completion.first_failure_reason
        $cleanupReserve=$true;$normalStage='cleanup';$state=[pscustomobject]@{kill_attempted=$false;cause=$null}
        Complete-ChildCleanup $true $state
        $cleanupKillAttempted=$state.kill_attempted;$cleanupFirstCause=$state.cause
        Public-CloseAll
        $stdioDisposed=$false;$registeredChildReleased=$false
        $out.Reader.Dispose();$err.Reader.Dispose();$stdioDisposed=$true
        if($reaped){$control.Child=$null;$child.Dispose();$registeredChildReleased=$true}
        $diag=$null
        if(-not $normalCandidate){$diag=Collect-FailedReports}
        . ([scriptblock]::Create($fragments.decision))
        Assert-Control ($reaped -and $out.Eof -and $err.Eof -and $publicOwned -eq 0 -and -not $managedCloseUnknown -and -not $control.Unknown) ('cleanup '+$kind)
        if($kind -ceq 'normal'){
            Assert-Control ($good -and $childExit -eq 0 -and $completion.first_reap_stage -ceq 'normal' -and -not $cleanupKillAttempted) 'normal completion gate'
        }else{
            Assert-Control (-not $good -and $failure -ceq 'work_expired' -and $completion.first_failure_reason -ceq 'work_expired' -and
                $completion.pre_cleanup_first_reason -ceq 'work_expired' -and -not $completion.normal_gate_passed -and $completion.first_reap_stage -ceq 'cleanup') 'sticky timeout'
            if($kind -ceq 'grace'){
                Assert-Control ($childExit -eq 0 -and -not $cleanupKillAttempted -and $diag.state -ceq 'bound' -and $null -eq $diag.cause -and
                    -not $diag.parent.Data.cleanup_known -and $null -eq $diag.p2.Data.core) 'late zero binding does not prove Core cleanup'
                foreach($mutation in @('pid','qpc','hash','source','shape')){
                    $c=$diag.claim;$p=$diag.p2
                    $r=ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $diag.parent -Depth 12 -Compress) -Depth 12 -DateKind String
                    $badP=ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $p -Depth 12 -Compress) -Depth 12 -DateKind String
                    switch($mutation){
                        'pid' {$r.Data.parent_pid=1;$expectedCode='schema_value'}
                        'qpc' {$badP.Data.p2_qpc=$StartTimestamp-1;$expectedCode='diagnostic_clock'}
                        'hash' {$r.Data.p2_sha256='0'*64;$expectedCode='schema_value'}
                        'source' {$badP.Data.source_before.launcher='0'*64;$expectedCode='schema_value'}
                        'shape' {$badP.Data | Add-Member -NotePropertyName extra -NotePropertyValue 1;$expectedCode='schema_fields'}
                    }
                    $caught=$null
                    try{Validate-Report 'p2' $badP.Data;Validate-Report 'parent' $r.Data;Check-DiagnosticBindings $c $badP $r}catch{$caught=$_.Exception}
                    Assert-Control ($caught -is [InvalidOperationException] -and $caught.HResult -eq -2146233079 -and
                        $caught.Data['cold_launch_code'] -ceq $expectedCode) ('exact data refusal '+$mutation)
                    $negativeChecks++
                }
            }else{
                Assert-Control ($cleanupKillAttempted -and $childExit -ne 0 -and $reapQpc -ge ($StartTimestamp+5L*[Diagnostics.Stopwatch]::Frequency) -and
                    $diag.state -ceq 'unknown' -and $null -ne $diag.cause -and $null -eq $diag.p2) 'force after grace and missing diagnostics'
            }
        }
        $expectedPeak=if($kind -ceq 'grace'){3}elseif($kind -ceq 'forced'){2}else{1}
        Assert-Control ($publicPeak -eq $expectedPeak) ('actual reopened peak '+$kind)
        foreach($record in $publicRecords){Assert-Control ($record.Closed -and $record.Handle.IsClosed) ('managed reader closure '+$kind)}
        $cases.Add([ordered]@{name=$kind;normal_success=$good;first_failure=$completion.first_failure_reason;exit_code=$childExit
            reap_stage=$completion.first_reap_stage;kill_attempted=$cleanupKillAttempted;reap_qpc=$reapQpc;start_qpc=$StartTimestamp
            public_peak=$publicPeak;public_remaining=$publicOwned;diagnostics_state=if($diag){$diag.state}else{'not_collected'}
            diagnostics_cause=if($diag){$diag.cause}else{$null};completion=$completion;wait_trace=$waitTrace.ToArray()})
        Check-Total
    }
    Assert-Control ($cases.Count -eq 3 -and $negativeChecks -eq 5) 'finite cases'
    $report=[ordered]@{classification='cold-timeout-controls';source_sha256='620753ecddd1ff1b1d07fa5d3868c8fdfa7170d7770f407165e112ade8551fad';mapping=$mapping.ToArray();cases=$cases.ToArray()
        data_refusals=$negativeChecks;work_seconds=20;whole_seconds=30;lane_count=1;retry_count=0
        private_access=$false;core_invoked=$false;launcher_entrypoint_invoked=$false;observed_seconds=([Diagnostics.Stopwatch]::GetTimestamp()-$rootStart)/[double][Diagnostics.Stopwatch]::Frequency}
    Write-Fresh "$e\$prefix.cases.json" (ConvertTo-Json -InputObject $report -Depth 16 -Compress)
    Check-Total
    'Three fixed timeout controls returned; root actual review required.'
} finally {
    if($control.Child){
        try{if(-not $control.Child.HasExited){$control.Child.Kill()};$signal=$control.Child.WaitForExit(500)
            if($signal){$control.Child.Dispose();$control.Child=$null}}catch{$control.Unknown=$true}
    }
    if(-not $control.Child){
        $null=$done.Set()
        if($ticket){
            Assert-Control ($ticket.AsyncWaitHandle.WaitOne(500)) 'guardian stopped'
            $null=$guardian.EndInvoke($ticket);$guardian.Dispose();$done.Dispose()
        }
    }else{
        while(([Diagnostics.Stopwatch]::GetTimestamp()-$rootStart)/[double][Diagnostics.Stopwatch]::Frequency -lt 29){[Threading.Thread]::Sleep(20)}
        $rootSelf.Kill()
    }
    $rootSelf.Dispose()
}
