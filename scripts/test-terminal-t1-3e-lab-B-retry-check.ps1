param([Parameter(Mandatory)][string]$Evidence,[Parameter(Mandatory)][string]$ReviewPackage)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
$ReviewPackage=[IO.Path]::GetFullPath($ReviewPackage)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
function Clone($Value){ConvertFrom-Json (ConvertTo-Json -InputObject $Value -Depth 18 -Compress)}
function Reject([scriptblock]$Body,[string]$Code){try{&$Body}catch{if($_.Exception.Message.Contains($Code)){return};throw};throw "CHECK:expected_$Code"}
$originalPackage=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/recovery-setup-package-01/package-B'))
$originOwner='setup-6f5c3fe31f11475290ba9c1b58d305c9'
$currentPackage=Lab-Read (Join-Path $ReviewPackage 'package.json');$currentSha=Lab-Hash (Join-Path $ReviewPackage 'package.json')
Require ($currentPackage.side -ceq 'B' -and @($currentPackage.files.PSObject.Properties).Count -eq 15) 'review_package15'
function Get-LabBRetryPackageRoot([string]$Owner){Join-Path $script:fixtureCommon ('Kilogram-Lab/SetupPackages/'+$Owner)}
function CopyPackage([string]$Source,[string]$Owner){
    $dest=Get-LabBRetryPackageRoot $Owner;[void][IO.Directory]::CreateDirectory($dest)
    foreach($name in @(Get-LabBRetryNames)+@('package.json')){[IO.File]::Copy((Join-Path $Source $name),(Join-Path $dest $name),$false)}
}
function Get-LabSnapshot($Cfg,$Want){Clone $script:os}
function Find-LabTask($Name){
    if(!$script:os.task){return $null}
    $t=[pscustomobject]@{Xml=$want.task.xml;descriptor=$want.task.sddl};$t|Add-Member ScriptMethod GetSecurityDescriptor {param($Flags) $this.descriptor};return $t
}
function InitFixture([string]$Directory){
    $script:caseRoot=$Directory;$script:fixtureCommon=Join-Path $Directory 'CommonData'
    $script:cfg=Get-LabConfig B;$script:cfg.base=Join-Path $Directory 'host'
    $script:want=@{config=$cfg;task=(Lab-TaskSpec $cfg);rules=@(Lab-Rules $cfg)}
    $script:stateDir=Join-Path $Directory 'state';[void][IO.Directory]::CreateDirectory($stateDir)
    [void][IO.Directory]::CreateDirectory((Join-Path $cfg.base 'ordinary-agent'))
    CopyPackage $originalPackage $originOwner
    $script:initial=@{task=$null;rules=@();policy=@(@{Name='foreign';Action='Allow';value='preserved'});
        network=@(@{index=19;alias=$cfg.interface_alias;category='Private'});adapter=@{name=$cfg.interface_alias;guid=$cfg.interface_guid;status='Up';addresses=@('192.168.0.152/24')};dynamic_udp=@('49152','16384');
        profiles=@(foreach($store in @('PersistentStore','ActiveStore')){foreach($name in @('Domain','Private','Public')){@{name=$name;store=$store;enabled='False';inbound='Block';outbound='Allow';allow_local='True';allow_inbound='True';disabled=@()}}})}
    Lab-Write (Join-Path (Get-LabBRetryPackageRoot $originOwner) 'fresh-audit.json') @{snapshot=$initial;sha256=(Lab-JsonHash $initial)} -MaxBytes 2097152
    $script:os=Clone $initial
    $j=[pscustomobject]@{phase='installing';setup_session=$originOwner;want=$want;initial=$initial;changes=@('files','task','rule0','rule1','private');rollback_errors=@();file_pins=(Lab-Read (Join-Path $originalPackage 'package.json')).files;
        primary_failure=@{stage='verify-installed';message='LAB_COM_SPEC_DIFFERS'}}
    Save-LabInstallJournal $stateDir $j $null;$j.phase='rolled_back';Save-LabInstallJournal $stateDir $j $null
    $script:journalPath=Join-Path $stateDir 'initial-and-journal.json'
}
$tokens=$null;$errors=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
Require (!$errors.Count) 'installer_parse'
$assignment=$tree.Find({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true)
$table=$assignment.Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
$production=@{}
foreach($pair in $table.KeyValuePairs){if($pair.Item1.Value -cin @('Read','Write','Preflight','Apply','Undo','CanRemove')){
    $production[$pair.Item1.Value]=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()
}}
function Get-LabRelevant($Cfg,$Want){@{profiles=$script:os.profiles;rules=$script:os.rules;network=$script:os.network;adapter=$script:os.adapter}}
function Get-LabOrdinaryState($Cfg,$Want){
    @{backend='com-native-v1';profiles=@($script:os.profiles|Where-Object store -ceq 'ActiveStore'|ForEach-Object {@{name=$_.name;enabled=$_.enabled;inbound=$_.inbound;outbound=$_.outbound;block_all=$false;disabled=@()}});
      rules=@(foreach($rule in $Want.rules){$view=Clone $rule;if($rule.Name -ceq $Want.rules[0].Name){$view.InterfaceAlias='fixture persisted mismatch'};@{name=$rule.Name;view=$view}})}
}
function Get-NetFirewallRule {param($PolicyStore,$Name) @($script:os.rules|Where-Object {$_.store -ceq $PolicyStore -and (!$Name -or $_.view.Name -ceq $Name)}|ForEach-Object view)}
function Get-LabRuleView($Rule,$Store){$Rule}
function Remove-NetFirewallRule {param($PolicyStore,$Name) $script:os.rules=@($script:os.rules|Where-Object {$_.view.Name -cne $Name});$script:os.policy=@($script:os.policy|Where-Object Name -cne $Name)}
function Set-NetFirewallProfile {param($PolicyStore,$Name,$Enabled) $script:profileEnables++;throw 'CHECK:profile_enable_not_expected'}
function Stop-LabIdleAgent($Base,$Handles) {}
$folder=[pscustomobject]@{};$folder|Add-Member ScriptMethod DeleteTask {param($Name,$Flags) $script:os.task=$null}
$service=[pscustomobject]@{};$service|Add-Member ScriptMethod Connect {};$service|Add-Member ScriptMethod GetFolder {param($Path) $folder}
function New-Object {param($ComObject) Require ($ComObject -ceq 'Schedule.Service') 'fixed_COM_boundary';$service}
$writer=${function:Save-LabComRuleComparison}.ToString().Replace("[Environment]::GetFolderPath('CommonApplicationData')",'$script:fixtureCommon')
Set-Item Function:Save-LabComRuleComparison ([scriptblock]::Create($writer))
$cases=[ordered]@{}
foreach($scenario in @('owner','old-package','initial','initial-bytes','history','history-missing','want','pins','errors','incomplete','task','rule','agent','agent-lock','payload','baseline','profile','foreign','no-lock')){
    InitFixture (Join-Path $Evidence $scenario)
    $j=Read-LabJournalStrict $journalPath;$expected=$null;$extraLock=$null
    switch($scenario){
        'owner'{$j.setup_session='setup-'+('f'*32);$expected='LAB_B_RETRY_PROVENANCE'}
        'old-package'{[IO.File]::AppendAllText((Join-Path (Get-LabBRetryPackageRoot $originOwner) 'package.json'),' ');$expected='LAB_B_RETRY_PACKAGE'}
        'initial'{$v=Clone $initial;$v.policy[0].value='changed';Lab-Write (Join-Path $stateDir 'initial.json') $v;$expected='LAB_B_RETRY_INITIAL'}
        'initial-bytes'{[IO.File]::AppendAllText((Join-Path $stateDir 'initial.json'),' ');$expected='LAB_B_RETRY_INITIAL'}
        'history'{$h=@(Get-ChildItem -LiteralPath (Join-Path $stateDir 'journal-history') -File)[0];$v=Lab-Read $h.FullName;$v.initial.policy[0].value='changed';Lab-Write $h.FullName $v;$expected='LAB_B_RETRY_HISTORY'}
        'history-missing'{$h=@(Get-ChildItem -LiteralPath (Join-Path $stateDir 'journal-history') -File)[0];Remove-Item -LiteralPath $h.FullName;$expected='LAB_B_RETRY_HISTORY'}
        'want'{$j.want=Clone $want;$j.want.config.side='A';$expected='LAB_B_RETRY_TERMINAL_REQUIRED'}
        'pins'{$j.file_pins.'lab.json'='0'*64;$expected='LAB_B_RETRY_ORIGIN_PINS'}
        'errors'{$j.rollback_errors=@('unknown');$expected='LAB_B_RETRY_TERMINAL_REQUIRED'}
        'incomplete'{$j.phase='rollback_incomplete';$expected='LAB_B_RETRY_TERMINAL_REQUIRED'}
        'task'{$script:os.task=@{present=$true};$expected='LAB_OLD_RECOVERY_OWNED_REMAINS'}
        'rule'{$script:os.rules=@(@{store='PersistentStore';view=$want.rules[0]});$expected='LAB_OLD_RECOVERY_OWNED_REMAINS'}
        'agent'{$dir=Join-Path $cfg.base 'ordinary-agent/control';[void][IO.Directory]::CreateDirectory($dir);Lab-Write (Join-Path $dir 'state.json') @{active=$true;session=('a'*32)};$expected='LAB_OLD_RECOVERY_AGENT_REMAINS'}
        'agent-lock'{$dir=Join-Path $cfg.base 'ordinary-agent/control';[void][IO.Directory]::CreateDirectory($dir);$extraLock=[IO.File]::Open((Join-Path $dir 'agent.lock'),'CreateNew','Write','None');$expected='LAB_B_RETRY_AGENT_LOCK'}
        'payload'{[IO.File]::WriteAllText((Join-Path $cfg.base 'ordinary-agent/lab.json'),'intervening');$expected='LAB_OLD_RECOVERY_FILES_REMAIN'}
        'baseline'{[IO.File]::WriteAllText((Join-Path $cfg.base 'ordinary-agent/campaign-baseline.json'),'intervening');$expected='LAB_OLD_RECOVERY_FILES_REMAIN'}
        'profile'{$script:os.profiles[1].enabled='True';$expected='LAB_PROFILE_DRIFT'}
        'foreign'{$script:os.policy[0].value='changed'}
        'no-lock'{$expected='LAB_INSTALL_LOCK_REQUIRED'}
    }
    Lab-Write $journalPath $j -MaxBytes 2097152
    $before=Lab-Hash $journalPath;$handles=@{agent=$null;images=$null};$gate=$null
    try{
        if($scenario -cne 'no-lock'){$gate=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None')}
        if($expected){Reject {Assert-LabBRetryAdmission $cfg $want $stateDir $gate $currentPackage $currentSha $handles} $expected}
        else{$null=Assert-LabBRetryAdmission $cfg $want $stateDir $gate $currentPackage $currentSha $handles}
        Require ((Lab-Hash $journalPath) -ceq $before -and !(Test-Path -LiteralPath (Join-Path $stateDir 'B-terminal-retry.json'))) 'admission_before_intent_or_provenance'
    }finally{foreach($h in $handles.Values){if($h){$h.Dispose()}};if($gate){$gate.Dispose()};if($extraLock){$extraLock.Dispose()}}
    $cases[$scenario]=$true
}
# Real production transaction Write/Undo/CanRemove and private Apply arm, real files,
# exact original/new package bytes; only privileged OS observations/mutations are mocked.
InitFixture (Join-Path $Evidence 'known-and-own')
$knownRoot=$caseRoot;$knownCommon=$fixtureCommon;$knownCfg=$cfg;$knownWant=$want;$knownInitial=$initial
$initialHash=Lab-Hash (Join-Path $stateDir 'initial.json');$originalHistory=Get-LabHistoryPins $stateDir
foreach($number in @(1,2)){
    $owner='setup-'+(([string]$number)*32);CopyPackage $ReviewPackage $owner
    $installLock=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$teardownHandles=@{agent=$null;images=$null}
    try{
        $before=Lab-Hash $journalPath
        $retryContext=Assert-LabBRetryAdmission $cfg $want $stateDir $installLock $currentPackage $currentSha $teardownHandles
        Require ($retryContext.kind -ceq $(if($number -eq 1){'known_origin'}else{'own_clean_rollback'}) -and (Lab-Hash $journalPath) -ceq $before) 'known_and_own_admission_before_write'
        $PackageRoot=Get-LabBRetryPackageRoot $owner;$PackageSha=$currentSha;$package=$currentPackage;$packagePins=$package.files
        $files=Get-LabBRetryNames
        $SetupSession=Join-Path $cfg.base ('deploy/'+$owner);[void][IO.Directory]::CreateDirectory($SetupSession)
        $setupMode=$true;$bRetry=$true;$freshB=$false;$script:profileEnables=0;$script:LabPrimary=$null
        $backend=@{Read=$production.Read;Write=$production.Write;Snapshot={Clone $script:os};Preflight={param($Now)};CanRemove=$production.CanRemove;Undo=$production.Undo;
            Verify={param($W) throw 'CHECK:should_fail_before_verify'};
            Apply={param($Change,$W)
                switch($Change){
                    'files'{foreach($name in Get-LabBRetryNames){[IO.File]::Copy((Join-Path $ReviewPackage $name),(Join-Path $cfg.base ('ordinary-agent/'+$name)),$false)}}
                    'task'{$script:os.task=@{present=$true}}
                    {$_ -cin @('rule0','rule1')}{$spec=$W.rules[[int]$Change.Substring(4)];foreach($store in @('PersistentStore','ActiveStore')){$script:os.rules+=@(@{store=$store;view=(Clone $spec)})};$script:os.policy+=@(Clone $spec)}
                    'private'{&$production.Apply 'private' $W}
                }
            }}
        Reject {Invoke-LabTransaction $backend $want 'Provision'} 'LAB_INSTALL_FAILED:LAB_COM_SPEC_DIFFERS;rollback=rolled_back'
        $proof=Complete-LabBRetry $cfg $want $stateDir $package $currentSha $owner
        Require ($proof.verified -and $script:profileEnables -eq 0 -and !$script:os.task -and !@($script:os.rules).Count) 'clean_rollback_without_profile_enable'
        Require ((Lab-Hash (Join-Path $stateDir 'initial.json')) -ceq $initialHash) 'original_initial_bytes_preserved'
        $afterHistory=Get-LabHistoryPins $stateDir
        foreach($name in $originalHistory.Keys){Require ($afterHistory.Contains($name) -and $afterHistory[$name] -ceq $originalHistory[$name]) 'original_history_bytes_preserved'}
        $record=Lab-Read (Join-Path $stateDir 'B-terminal-retry.json') -MaxBytes 2097152
        Require (@($record.attempts).Count -eq $number -and $record.attempts[-1].status -ceq 'clean_rollback') 'protected_own_package_provenance'
        Require (Lab-Equal (Read-LabJournalStrict $journalPath).file_pins $package.files) 'corrected_pins_adopted_at_new_intent'
        $comparison=Lab-Read (Join-Path $PackageRoot 'com-rule-comparison.json') -MaxBytes 2097152
        Require ($comparison.stage -ceq 'pre-profile-comparison' -and $comparison.rules[0].differences[0].field -ceq 'InterfaceAlias') 'actionable_exact_pre_profile_mismatch'
        $cases['transaction_'+$number]=@{owner=$owner;profile_enables=0;package_sha256=$currentSha;proof=$proof}
    }finally{foreach($h in $teardownHandles.Values){if($h){$h.Dispose()}};$installLock.Dispose()}
}
# Clone the now-clean protected provenance for independent refusal fixtures. The
# configured ordinary base is a shared empty disposable fixture, never real B data.
foreach($scenario in @('own-unknown','own-package','own-initial','own-history','own-journal','own-pending','own-missing-package','own-no-ledger','own-bound')){
    $dest=Join-Path $Evidence $scenario;Copy-Item -LiteralPath $knownRoot -Destination $dest -Recurse
    $script:fixtureCommon=Join-Path $dest 'CommonData';$stateDir=Join-Path $dest 'state';$journalPath=Join-Path $stateDir 'initial-and-journal.json'
    $cfg=$knownCfg;$want=$knownWant;$script:os=Clone $knownInitial
    $recordPath=Join-Path $stateDir 'B-terminal-retry.json';$record=Lab-Read $recordPath -MaxBytes 2097152;$j=Read-LabJournalStrict $journalPath
    $expected='LAB_B_RETRY_PROVENANCE'
    switch($scenario){
        'own-unknown'{$j.setup_session='setup-'+('e'*32);Lab-Write $journalPath $j -MaxBytes 2097152}
        'own-package'{$record.package_sha256='0'*64;Lab-Write $recordPath $record -MaxBytes 2097152}
        'own-initial'{[IO.File]::AppendAllText((Join-Path $stateDir 'initial.json'),' ');$expected='LAB_B_RETRY_INITIAL'}
        'own-history'{$h=@(Get-ChildItem -LiteralPath (Join-Path $stateDir 'journal-history') -File)[0];[IO.File]::AppendAllText($h.FullName,' ');$expected='LAB_B_RETRY_HISTORY'}
        'own-journal'{[IO.File]::AppendAllText($journalPath,' ')}
        'own-pending'{$record.attempts[-1].status='intent';Lab-Write $recordPath $record -MaxBytes 2097152}
        'own-missing-package'{$path=Join-Path (Get-LabBRetryPackageRoot $j.setup_session) 'package.json';[IO.File]::AppendAllText($path,' ');$expected='LAB_B_RETRY_PACKAGE'}
        'own-no-ledger'{Remove-Item -LiteralPath $recordPath}
        'own-bound'{
            $record.attempts=@(1..15|ForEach-Object {[pscustomobject]@{owner=('setup-'+[guid]::NewGuid().ToString('N'));status='clean_rollback';package_sha256=$currentSha;journal_sha256=('f'*64)}})+@($record.attempts[-1])
            Lab-Write $recordPath $record -MaxBytes 2097152
        }
    }
    $before=Lab-Hash $journalPath;$gate=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$handles=@{agent=$null;images=$null}
    try{Reject {Assert-LabBRetryAdmission $cfg $want $stateDir $gate $currentPackage $currentSha $handles} $expected;Require ((Lab-Hash $journalPath) -ceq $before) 'own_refusal_preserves_journal'}
    finally{foreach($h in $handles.Values){if($h){$h.Dispose()}};$gate.Dispose()}
    $cases[$scenario]=$true
}
InitFixture (Join-Path $Evidence 'intervening-after-admission')
$installLock=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$teardownHandles=@{agent=$null;images=$null}
try{
    $before=Lab-Hash $journalPath
    $ctx=Assert-LabBRetryAdmission $cfg $want $stateDir $installLock $currentPackage $currentSha $teardownHandles
    [IO.File]::WriteAllText((Join-Path $cfg.base 'ordinary-agent/lab.json'),'intervening after admission')
    & {
        function Assert-LabNetwork($Cfg,$Snapshot,$Actor) {} # Only the already-covered native observation boundary.
        $setupMode=$true;$setupActor=$null;$files=Get-LabBRetryNames
        Reject {&$production.Preflight (Clone $script:os)} 'LAB_FILE_COLLISION'
    }
    Require ((Lab-Hash $journalPath) -ceq $before -and !(Test-Path -LiteralPath (Join-Path $stateDir 'B-terminal-retry.json'))) 'intervening_file_before_intent'
    $cases.production_intervening_file=$true
}finally{foreach($h in $teardownHandles.Values){if($h){$h.Dispose()}};$installLock.Dispose()}
# The distinct original absence-only operation remains strict.
$freshB=$true
Reject {&$production.Read} 'LAB_B_INTENT_ALREADY_EXISTS'
$cases.fresh_absence_guard_preserved=$true
$launchTree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-launch.ps1'),[ref]$tokens,[ref]$errors)
Require (!$errors.Count) 'launcher_parse'
$supervisor=$launchTree.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Invoke-LabSetupSupervised'},$false)
. ([scriptblock]::Create($supervisor.Extent.Text))
$fakeJob=[pscustomobject]@{};$fakeJob|Add-Member ScriptMethod Children {return @()}
function Invoke-LabSetupChild($Operation,$Seconds,$Job,$Prefix){
    $script:routes+=,@{operation=$Operation;seconds=$Seconds}
    [IO.File]::WriteAllText($Prefix+'.stdout.log','');[IO.File]::WriteAllText($Prefix+'.stderr.log','')
    @{exit=$(if($Operation -ceq 'RetryRollbackB'){0}elseif($script:routeCase -ceq 'no-eof'){0}else{1});timeout=$false;capture_complete=($script:routeCase -cne 'no-eof' -or $Operation -ceq 'RetryRollbackB')}
}
foreach($scenario in @('owned','other','pre-intent','no-eof')){
    $script:routeCase=$scenario;$script:routes=@();$LaunchRoot=Join-Path $Evidence ('route-'+$scenario);[void][IO.Directory]::CreateDirectory($LaunchRoot)
    $journalPath=Join-Path $LaunchRoot 'journal.json';$session='setup-'+('8'*32)
    if($scenario -cne 'pre-intent'){Lab-Write $journalPath @{phase='rolled_back';setup_session=$(if($scenario -ceq 'other'){$originOwner}else{$session})}}
    $r=Invoke-LabSetupSupervised $fakeJob @{host='fixture';side='B'} $session $journalPath 'SetupRetryB'
    Require (!$r.success -and $routes[0].operation -ceq 'SetupRetryB' -and $routes[0].seconds -eq 450) 'one_bounded_retry_main'
    if($scenario -cin @('owned','no-eof')){Require ($routes.Count -eq 2 -and $routes[1].operation -ceq 'RetryRollbackB' -and $routes[1].seconds -eq 180) 'only_owned_retry_fallback'}
    else{Require ($routes.Count -eq 1 -and !$r.recovery) 'no_cleanup_of_prior_or_missing_intent'}
    $cases['route_'+$scenario]=$true
}
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;package_sha256=$currentSha;host_mutations=0;
    scope='production admission/provenance/transaction/Undo, real exact old15/new15 bytes and immutable journal/history; OS observations/mutations mocked, no live installation'} -MaxBytes 2097152
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
