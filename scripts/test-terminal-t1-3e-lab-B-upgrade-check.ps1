param([Parameter(Mandatory)][string]$Evidence,[Parameter(Mandatory)][string]$ReviewPackage)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
$ReviewPackage=[IO.Path]::GetFullPath($ReviewPackage)
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-job.cs"
# Reuse the existing exact-package/filesystem fixture boundary, not the admission
# implementation. Privileged observations and protected roots remain disposable.
$tokens=$null;$errors=$null
$tree=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-B-retry-check.ps1'),[ref]$tokens,[ref]$errors)
$names=@('Require','Clone','Reject','Get-LabBRetryPackageRoot','CopyPackage','Get-LabSnapshot','Find-LabTask','InitFixture')
$nodes=@($tree.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -cin $names},$false))
if($nodes.Count -ne $names.Count){throw 'CHECK:fixture_functions'}
. ([scriptblock]::Create(($nodes|ForEach-Object {$_.Extent.Text}) -join "`n"))
$originalPackage=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/recovery-setup-package-01/package-B'))
$priorPackage=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../.tmp/terminal-t1-3e/B-rollback-retry-package-03/package-B'))
$prior=Lab-Read (Join-Path $priorPackage 'package.json');$priorSha=Lab-Hash (Join-Path $priorPackage 'package.json')
Require ($priorSha -ceq 'a135cfcb8e6c01d4c20d822e4dec4da9c1df3853d7b06a55a95039d591a0e9e0') 'exact_published_predecessor'
$current=Lab-Read (Join-Path $ReviewPackage 'package.json');$currentSha=Lab-Hash (Join-Path $ReviewPackage 'package.json')
$originOwner='setup-6f5c3fe31f11475290ba9c1b58d305c9';$priorOwner='setup-33ab20c7666e46e4bdacd13f1ca6a735'
$scopeSource=${function:Get-LabBRetryUpgrade}
function Get-LabBRetryUpgrade {
    $v=&$scopeSource;$v.initial_sha256=$script:fixtureInitialHash;return $v
}
function InitPredecessor([string]$Directory){
    InitFixture $Directory
    $script:fixtureInitialHash=Lab-Hash (Join-Path $stateDir 'initial.json')
    # Execute the accepted a135 recovery helpers in a local function scope to
    # create real old-format intent/clean provenance, then automatically restore
    # current functions on scope exit. No fabricated legacy authorization record.
    & {
        . (Join-Path $priorPackage 'test-terminal-t1-3e-lab-recovery.ps1')
        function Get-LabBRetryPackageRoot([string]$Owner){Join-Path $script:fixtureCommon ('Kilogram-Lab/SetupPackages/'+$Owner)}
        CopyPackage $priorPackage $priorOwner
        $gate=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$handles=@{agent=$null;images=$null}
        try{
            $ctx=Assert-LabBRetryAdmission $cfg $want $stateDir $gate $prior $priorSha $handles
            $j=Read-LabJournalStrict $journalPath;$j.phase='installing';$j.setup_session=$priorOwner;$j.changes=@()
            Save-LabInstallJournal $stateDir $j $prior.files
            Register-LabBRetryIntent $ctx $want $stateDir $priorOwner $prior $priorSha
            $j.changes=@('files','task','rule0','rule1','private');Save-LabInstallJournal $stateDir $j $prior.files
            $j.phase='rolled_back';$j.rollback_errors=@();Save-LabInstallJournal $stateDir $j $prior.files
            $null=Complete-LabBRetry $cfg $want $stateDir $prior $priorSha $priorOwner
        }finally{foreach($h in $handles.Values){if($h){$h.Dispose()}};$gate.Dispose()}
    }
}
$installer=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-terminal-t1-3e-lab-install.ps1'),[ref]$tokens,[ref]$errors)
$assignment=$installer.Find({param($n) $n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Left.Extent.Text -ceq '$backend'},$true)
$table=$assignment.Right.Find({param($n) $n -is [Management.Automation.Language.HashtableAst]},$true)
$pair=@($table.KeyValuePairs|Where-Object {$_.Item1.Value -ceq 'Write'})[0]
$write=$pair.Item2.Find({param($n) $n -is [Management.Automation.Language.ScriptBlockExpressionAst]},$true).ScriptBlock.GetScriptBlock()
$cases=[ordered]@{}
foreach($scenario in @('wrong-owner','wrong-package','wrong-initial-authority','incomplete','errors','pending','history','old-pins','archive-conflict')){
    InitPredecessor (Join-Path $Evidence $scenario)
    $recordPath=Join-Path $stateDir 'B-terminal-retry.json';$j=Read-LabJournalStrict $journalPath;$record=Lab-Read $recordPath -MaxBytes 2097152
    $expected='LAB_B_RETRY_PROVENANCE'
    switch($scenario){
        'wrong-owner'{$j.setup_session='setup-'+('f'*32);Lab-Write $journalPath $j -MaxBytes 2097152}
        'wrong-package'{$record.package_sha256='0'*64;Lab-Write $recordPath $record -MaxBytes 2097152}
        'wrong-initial-authority'{$script:fixtureInitialHash='0'*64;$expected='LAB_B_RETRY_TRANSITION'}
        'incomplete'{$j.phase='rollback_incomplete';Lab-Write $journalPath $j -MaxBytes 2097152;$expected='LAB_B_RETRY_TERMINAL_REQUIRED'}
        'errors'{$j.rollback_errors=@('unconfirmed');Lab-Write $journalPath $j -MaxBytes 2097152;$expected='LAB_B_RETRY_TERMINAL_REQUIRED'}
        'pending'{$record.attempts[0].status='intent';Lab-Write $recordPath $record -MaxBytes 2097152;$expected='LAB_B_RETRY_TRANSITION'}
        'history'{$h=@(Get-ChildItem -LiteralPath (Join-Path $stateDir 'journal-history') -File)[0];[IO.File]::AppendAllText($h.FullName,' ');$expected='LAB_B_RETRY_HISTORY'}
        'old-pins'{$j.file_pins.'lab.json'='0'*64;Lab-Write $journalPath $j -MaxBytes 2097152;$expected='LAB_B_RETRY_TRANSITION'}
        'archive-conflict'{Lab-Write (Join-Path $stateDir (Get-LabBRetryUpgrade).archive) @{unexpected=$true};$expected='LAB_B_RETRY_PREDECESSOR'}
    }
    $before=Lab-Hash $journalPath;$recordBefore=Lab-Hash $recordPath
    $gate=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$handles=@{agent=$null;images=$null}
    try{
        Reject {Assert-LabBRetryAdmission $cfg $want $stateDir $gate $current $currentSha $handles} $expected
        Require ((Lab-Hash $journalPath) -ceq $before -and (Lab-Hash $recordPath) -ceq $recordBefore) 'refusal_before_intent_and_provenance_write'
    }finally{foreach($h in $handles.Values){if($h){$h.Dispose()}};$gate.Dispose()}
    $cases[$scenario]=$true
}
InitPredecessor (Join-Path $Evidence 'transition-and-continuation')
$initialHash=Lab-Hash (Join-Path $stateDir 'initial.json');$oldLedgerHash=Lab-Hash (Join-Path $stateDir 'B-terminal-retry.json');$oldHistory=Get-LabHistoryPins $stateDir
foreach($n in @(1,2)){
    $owner='setup-'+(([string]$n)*32);CopyPackage $ReviewPackage $owner
    $gate=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$teardownHandles=@{agent=$null;images=$null}
    try{
        $before=Lab-Hash $journalPath
        $retryContext=Assert-LabBRetryAdmission $cfg $want $stateDir $gate $current $currentSha $teardownHandles
        Require ($retryContext.kind -ceq $(if($n -eq 1){'known_package_transition'}else{'own_clean_rollback'}) -and (Lab-Hash $journalPath) -ceq $before) 'exact_transition_then_own_continuation'
        $j=Read-LabJournalStrict $journalPath;$j.phase='installing';$j.changes=@();$SetupSession=Join-Path $cfg.base ('deploy/'+$owner)
        $setupMode=$true;$bRetry=$true;$package=$current;$packagePins=$current.files;$PackageSha=$currentSha
        $null=&$write $j
        Require (!$teardownHandles.agent -and !$teardownHandles.images) 'admission_locks_released_at_intent'
        $archive=Join-Path $stateDir (Get-LabBRetryUpgrade).archive
        Require ((Lab-Hash $archive) -ceq $oldLedgerHash) 'exact_old_ledger_archived_without_reset'
        $j.phase='rolled_back';$j.rollback_errors=@();$null=&$write $j
        $proof=Complete-LabBRetry $cfg $want $stateDir $current $currentSha $owner
        $record=Lab-Read (Join-Path $stateDir 'B-terminal-retry.json') -MaxBytes 2097152
        Require ($proof.verified -and @($record.attempts).Count -eq ($n+1) -and $record.attempts[0].owner -ceq $priorOwner -and $record.attempts[0].package_sha256 -ceq $priorSha) 'old_owner_record_preserved_new_clean_attempt_appended'
        Require ($record.attempts[-1].package_sha256 -ceq $currentSha -and $record.attempts[-1].status -ceq 'clean_rollback') 'own_new_package_provenance'
        Require ((Lab-Hash (Join-Path $stateDir 'initial.json')) -ceq $initialHash) 'original_initial_bytes_preserved'
        $now=Get-LabHistoryPins $stateDir;foreach($name in $oldHistory.Keys){Require ($now.Contains($name) -and $now[$name] -ceq $oldHistory[$name]) 'history_preserved'}
        $cases['positive_'+$n]=@{owner=$owner;kind=$retryContext.kind;initial_sha256=$initialHash;predecessor_sha256=$oldLedgerHash}
    }finally{foreach($h in $teardownHandles.Values){if($h){$h.Dispose()}};$gate.Dispose()}
}
# Archive corruption is a refusal on later same-package attempts, never a reset.
$archive=Join-Path $stateDir (Get-LabBRetryUpgrade).archive;[IO.File]::AppendAllText($archive,' ')
$gate=[IO.File]::Open((Join-Path $stateDir 'install.lock'),'OpenOrCreate','Write','None');$handles=@{agent=$null;images=$null}
try{Reject {Assert-LabBRetryAdmission $cfg $want $stateDir $gate $current $currentSha $handles} 'LAB_B_RETRY_PREDECESSOR';$cases.archive_corruption_refused=$true}
finally{foreach($h in $handles.Values){if($h){$h.Dispose()}};$gate.Dispose()}
Lab-Write (Join-Path $Evidence 'checks.json') @{count=$cases.Count;cases=$cases;package_sha256=$currentSha;host_mutations=0;
    scope='accepted a135 helpers generate old clean provenance; current admission/production Write/complete preserve exact predecessor/initial/history, with mapped protected roots and exported initial-hash authority fixture; no host installation'} -MaxBytes 2097152
@{passed=$cases.Count;host_mutations=0}|ConvertTo-Json -Compress
