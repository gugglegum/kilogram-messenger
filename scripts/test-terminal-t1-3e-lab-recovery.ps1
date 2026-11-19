# Fixed campaign recovery. The old initial snapshot and eleven-file pin scope are immutable.
function Read-LabJournalStrict([string]$Path){
    try{$null=Get-Item -LiteralPath $Path -Force -ErrorAction Stop}
    catch{if($_.CategoryInfo.Category -eq 'ObjectNotFound'){return $null};throw}
    $j=Lab-Read $Path -MaxBytes 2097152
    if(!$j){throw 'LAB_JOURNAL_DISAPPEARED'};return $j
}
function Get-LabLegacyRoot {
    Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab/SetupPackages/setup-e4125cddbe3f4dba9d3017aec19af6cd'
}
function Get-LabHistoryPins([string]$StateDirectory){
    $dir=Join-Path $StateDirectory 'journal-history';$pins=[ordered]@{}
    if(Test-Path -LiteralPath $dir){
        Lab-Plain $dir;$files=@(Get-ChildItem -LiteralPath $dir -File -Force)
        if($files.Count -gt 128){throw 'LAB_HISTORY_BOUND'}
        foreach($file in $files){if($file.Name -cnotmatch '^[0-9a-f]{32}\.json$'){throw 'LAB_HISTORY_SCOPE'};$pins[$file.Name]=Lab-Hash $file.FullName}
    }
    return $pins
}
function Assert-LabOldRecovery($Cfg,$Want,[string]$StateDirectory){
    if($Cfg.side -cne 'A'){throw 'LAB_OLD_RECOVERY_SIDE'}
    $owner='setup-e4125cddbe3f4dba9d3017aec19af6cd'
    $j=Read-LabJournalStrict (Join-Path $StateDirectory 'initial-and-journal.json')
    if(!$j -or $j.setup_session -cne $owner -or $j.phase -cnotin @('recovering','rollback_incomplete') -or !(Lab-Equal $j.want $Want)){throw 'LAB_OLD_RECOVERY_SCOPE'}
    $legacy=Get-LabLegacyRoot
    if((Lab-Hash (Join-Path $legacy 'package.json')) -cne '5e7b721ccd38192e2dfbb7e674446ab84cb3e8c74de4ffbcb3fc7ab6ae8a39f4'){throw 'LAB_OLD_PACKAGE_PIN'}
    $package=Lab-Read (Join-Path $legacy 'package.json')
    $names=@('test-terminal-t1-3e-lab-agent.ps1','test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-job.cs','test-terminal-t1-3e-lab-install.ps1',
        'test-terminal-t1-3e-lab-native.cs','test-terminal-t1-3e-lab-readiness.ps1','test-terminal-t1-3e-lab-setup.ps1','test-terminal-t1-3e-lab-launch.ps1','dpapi-context.ps1','lab.json','ssh-probe.json')
    Lab-Keys $package.files $names;Lab-Keys $j.file_pins $names
    if($package.v -ne 2 -or $package.side -cne 'A' -or !(Lab-Equal $j.file_pins $package.files)){throw 'LAB_OLD_FILE_PIN_SCOPE'}
    foreach($name in $names){if((Lab-Hash (Join-Path $legacy $name)) -cne $package.files.$name){throw 'LAB_OLD_PACKAGE_FILE_DRIFT'}}
    $initialPath=Join-Path $StateDirectory 'initial.json'
    if(!(Lab-Equal (Lab-Read $initialPath -MaxBytes 2097152) $j.initial)){throw 'LAB_INITIAL_BASELINE_CHANGED'}
    # Only a prefix of the fixed transaction is a possible failed install.
    $order=@('files','task','rule0','rule1','private');$changes=@($j.changes)
    if(!$changes.Count -or $changes.Count -gt $order.Count){throw 'LAB_OLD_CHANGE_SCOPE'}
    for($n=0;$n -lt $changes.Count;$n++){if($changes[$n] -cne $order[$n]){throw 'LAB_OLD_CHANGE_SCOPE'}}
    $now=Get-LabSnapshot $Cfg $Want
    Assert-LabRestoreScope $now $j.initial $Want
    $task=Find-LabTask $Want.task.name
    if($task){Assert-LabTask $task $Want.task}
    $seen=@{}
    foreach($row in $now.rules){
        $spec=@($Want.rules|Where-Object {$_.Name -ceq $row.view.Name})
        $key=$row.store+'/'+$row.view.Name
        if($row.store -cnotin @('PersistentStore','ActiveStore') -or $spec.Count -ne 1 -or $seen.ContainsKey($key) -or !(Lab-Equal $row.view $spec[0])){throw 'LAB_RULE_DRIFT'}
        $seen[$key]=$true
    }
    foreach($name in $names){
        $path=Join-Path $Cfg.base "ordinary-agent/$name"
        if((Test-Path -LiteralPath $path) -and (Lab-Hash $path) -cne $package.files.$name){throw 'LAB_FILE_DRIFT'}
    }
    $baseline=Join-Path $Cfg.base 'ordinary-agent/campaign-baseline.json'
    if(Test-Path -LiteralPath $baseline){
        $intent=Lab-Read (Join-Path $StateDirectory 'public-baseline-intent.json')
        if(!$intent -or (Lab-Hash $baseline) -cne $intent.sha256){throw 'LAB_BASELINE_DRIFT'}
    }
    return @{owner=$owner;initial_sha256=(Lab-Hash $initialPath);file_pins=$package.files;names=$names;history=(Get-LabHistoryPins $StateDirectory)}
}
function Assert-LabOldRecoveryCompleted($Cfg,$Want,[string]$StateDirectory,$Before){
    $j=Read-LabJournalStrict (Join-Path $StateDirectory 'initial-and-journal.json')
    if(!$j -or $j.phase -cne 'rolled_back' -or $j.setup_session -cne $Before.owner -or !(Lab-Equal $j.want $Want) -or
       !(Lab-Equal $j.file_pins $Before.file_pins) -or (Lab-Hash (Join-Path $StateDirectory 'initial.json')) -cne $Before.initial_sha256 -or
       !(Lab-Equal (Lab-Read (Join-Path $StateDirectory 'initial.json') -MaxBytes 2097152) $j.initial)){throw 'LAB_OLD_RECOVERY_NOT_CONFIRMED'}
    $nowHistory=Get-LabHistoryPins $StateDirectory
    $prior=$Before.history|ConvertTo-Json -Depth 10 -Compress|ConvertFrom-Json
    foreach($entry in $prior.PSObject.Properties){if(!$nowHistory.Contains($entry.Name) -or $nowHistory[$entry.Name] -cne $entry.Value){throw 'LAB_HISTORY_CHANGED'}}
    $now=Get-LabSnapshot $Cfg $Want
    Assert-LabRestoreScope $now $j.initial $Want
    if($now.task -or @($now.rules).Count -or !(Lab-Equal $now.profiles $j.initial.profiles)){throw 'LAB_OLD_RECOVERY_OWNED_REMAINS'}
    foreach($name in @($Before.names)+@('campaign-baseline.json')){
        if(Test-Path -LiteralPath (Join-Path $Cfg.base "ordinary-agent/$name")){throw 'LAB_OLD_RECOVERY_FILES_REMAIN'}
    }
    $control=Join-Path $Cfg.base 'ordinary-agent/control'
    $state=Read-LabJournalStrict (Join-Path $control 'state.json')
    if($state){
        Lab-Hex $state.session
        if($state.active -or @([KilogramLabJob]::Inspect("Local\Kilogram-Lab-$($state.session)")).Count){throw 'LAB_OLD_RECOVERY_AGENT_REMAINS'}
    }
    return @{verified=$true;owner=$Before.owner;phase='rolled_back';initial_sha256=$Before.initial_sha256;history_preserved=$true;old_file_pins_preserved=$true;owned_absent=$true}
}
function Assert-LabRecoveredSetup($Cfg,$Want,[string]$StateDirectory,[string]$ProtectedPackage){
    $proof=Lab-Read (Join-Path $ProtectedPackage 'old-recovery-proof.json') -MaxBytes 2097152
    if(!$proof -or $proof.before.owner -cne 'setup-e4125cddbe3f4dba9d3017aec19af6cd' -or !$proof.result.verified){throw 'LAB_OLD_RECOVERY_PROOF_REQUIRED'}
    # Repeat the completion check under the fresh install lock before replacing any pins.
    $null=Assert-LabOldRecoveryCompleted $Cfg $Want $StateDirectory $proof.before
    return $proof.result
}
