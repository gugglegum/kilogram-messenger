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
    $now=Get-LabSnapshot $Cfg $Want -Cleanup
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
    $now=Get-LabSnapshot $Cfg $Want -Cleanup
    Assert-LabRestoreScope $now $j.initial $Want
    if($now.task -or @($now.rules).Count){throw 'LAB_OLD_RECOVERY_OWNED_REMAINS'}
    # Other profiles/settings are not owned. The specific Private enabled write
    # must have reached its rollback target; no full historical snapshot equality.
    $rollbackInitial=Get-LabRollbackInitial $j
    foreach($p in @($now.profiles|Where-Object name -ceq 'Private')){
        $target=@($rollbackInitial.profiles|Where-Object {$_.name -ceq 'Private' -and $_.store -ceq $p.store})
        if($target.Count -ne 1 -or $p.enabled -cne $target[0].enabled){throw 'LAB_PROFILE_DRIFT'}
    }
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

# One fixed B terminal-rollback retry. Protected campaign provenance, not an
# ordinary caller's approval file, admits later clean rollbacks of this package.
function Get-LabBRetryPackageRoot([string]$Owner){
    if($Owner -cnotmatch '^setup-[0-9a-f]{32}$'){throw 'LAB_B_RETRY_OWNER'}
    Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) ('Kilogram-Lab/SetupPackages/'+$Owner)
}
function Get-LabBRetryNames {
    @('test-terminal-t1-3e-lab-agent.ps1','test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-job.cs','test-terminal-t1-3e-lab-install.ps1',
      'test-terminal-t1-3e-lab-native.cs','test-terminal-t1-3e-lab-readiness.ps1','test-terminal-t1-3e-lab-setup.ps1','test-terminal-t1-3e-lab-launch.ps1',
      'test-terminal-t1-3e-lab-diagnostics.ps1','test-terminal-t1-3e-lab-bootstrap.ps1','test-terminal-t1-3e-lab-server.ps1','test-terminal-t1-3e-lab-recovery.ps1','dpapi-context.ps1','lab.json','ssh-probe.json')
}
function Assert-LabBRetryPackage([string]$Owner,[string]$Pin,$Files=$null){
    $root=Get-LabBRetryPackageRoot $Owner
    if((Lab-Hash (Join-Path $root 'package.json')) -cne $Pin){throw 'LAB_B_RETRY_PACKAGE'}
    $p=Lab-Read (Join-Path $root 'package.json');Lab-Keys $p @('v','side','files');Lab-Keys $p.files (Get-LabBRetryNames)
    if($p.v -ne 2 -or $p.side -cne 'B' -or ($Files -and !(Lab-Equal $p.files $Files))){throw 'LAB_B_RETRY_PACKAGE'}
    foreach($name in Get-LabBRetryNames){if((Lab-Hash (Join-Path $root $name)) -cne $p.files.$name){throw 'LAB_B_RETRY_PACKAGE_FILE'}}
    return $p
}
function Assert-LabBRetryAnchor($Cfg,$Want,[string]$StateDirectory){
    if($Cfg.side -cne 'B'){throw 'LAB_B_RETRY_SIDE'}
    $owner='setup-6f5c3fe31f11475290ba9c1b58d305c9';$pin='8a15960ecc53d631b45cf38e22e792d465aab5c9394714004418957e6aa9fa12'
    $origin=Assert-LabBRetryPackage $owner $pin
    $auditPath=Join-Path (Get-LabBRetryPackageRoot $owner) 'fresh-audit.json'
    $audit=Read-LabJournalStrict $auditPath;$initialPath=Join-Path $StateDirectory 'initial.json'
    $initial=Read-LabJournalStrict $initialPath
    if(!$audit -or !$initial -or !(Lab-Equal $audit.snapshot $initial) -or $audit.sha256 -cne (Lab-JsonHash $audit.snapshot) -or $initial.task -or @($initial.rules).Count){throw 'LAB_B_RETRY_INITIAL'}
    # The original PS5 Lab-Write serialized this very snapshot with one LF. Bind
    # its exact bytes before the first ledger exists, rather than rebasing a hash.
    $bytes=[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $audit.snapshot -Depth 18 -Compress)+[char]10)
    $hash=[Security.Cryptography.SHA256]::Create()
    try{$expectedInitial=[BitConverter]::ToString($hash.ComputeHash($bytes)).Replace('-','').ToLowerInvariant()}finally{$hash.Dispose()}
    if((Lab-Hash $initialPath) -cne $expectedInitial){throw 'LAB_B_RETRY_INITIAL'}
    @{owner=$owner;manifest=$pin;file_pins=$origin.files;initial=$initial;initial_sha256=(Lab-Hash $initialPath);
      audit_sha256=(Lab-Hash $auditPath);want=$Want}
}
function Assert-LabBRetryHistory([string]$StateDirectory,$Expected,[string]$AllowedOwner,$Want,$Initial,$Files,[switch]$Exact){
    $now=Get-LabHistoryPins $StateDirectory
    $before=$Expected|ConvertTo-Json -Depth 10 -Compress|ConvertFrom-Json
    foreach($entry in $before.PSObject.Properties){if(!$now.Contains($entry.Name) -or $now[$entry.Name] -cne $entry.Value){throw 'LAB_B_RETRY_HISTORY'}}
    foreach($name in $now.Keys){
        if($before.PSObject.Properties[$name]){continue}
        if($Exact){throw 'LAB_B_RETRY_HISTORY'}
        $j=Read-LabJournalStrict (Join-Path $StateDirectory ('journal-history/'+$name))
        if($j.setup_session -cne $AllowedOwner -or $j.phase -cnotin @('installing','rollback_incomplete','recovering','rolled_back') -or
           !(Lab-Equal $j.want $Want) -or !(Lab-Equal $j.initial $Initial) -or !(Lab-Equal $j.file_pins $Files)){throw 'LAB_B_RETRY_HISTORY'}
        $order=@('files','task','rule0','rule1','private');$changes=@($j.changes)
        if($changes.Count -gt 5){throw 'LAB_B_RETRY_HISTORY'}
        for($n=0;$n -lt $changes.Count;$n++){if($changes[$n] -cne $order[$n]){throw 'LAB_B_RETRY_HISTORY'}}
    }
    return $now
}
function Get-LabBRetryUpgrade {
    @{owner='setup-33ab20c7666e46e4bdacd13f1ca6a735';package_sha256='a135cfcb8e6c01d4c20d822e4dec4da9c1df3853d7b06a55a95039d591a0e9e0';
      initial_sha256='ef680f33b876e1ca5bad20999c1ebbac16bd42494fd2d27f9683bed98c669e0f';archive='B-terminal-retry-a135-predecessor.json'}
}
function Assert-LabBRetryPredecessor($Store,$Anchor,[string]$PackageSha,[string]$StateDirectory){
    $scope=Get-LabBRetryUpgrade;$m=$Store.migration
    if($m.owner -cne $scope.owner -or $m.from_package -cne $scope.package_sha256 -or $m.to_package -cne $PackageSha -or
       $m.initial_sha256 -cne $scope.initial_sha256 -or $Anchor.initial_sha256 -cne $scope.initial_sha256){throw 'LAB_B_RETRY_TRANSITION'}
    $path=Join-Path $StateDirectory $scope.archive
    if(!(Test-Path -LiteralPath $path) -or (Lab-Hash $path) -cne $m.ledger_sha256){throw 'LAB_B_RETRY_PREDECESSOR'}
    $old=Read-LabJournalStrict $path
    if($old.PSObject.Properties['migration'] -or @($old.attempts).Count -ne 1 -or $old.attempts[0].owner -cne $scope.owner -or
       $old.attempts[0].status -cne 'clean_rollback' -or !(Lab-Equal $old.attempts[0] $Store.attempts[0])){throw 'LAB_B_RETRY_PREDECESSOR'}
    Assert-LabBRetryLedger $old $Anchor $scope.package_sha256 $StateDirectory
    $now=Get-LabHistoryPins $StateDirectory
    foreach($entry in $old.history_pins.PSObject.Properties){if(!$now.Contains($entry.Name) -or $now[$entry.Name] -cne $entry.Value){throw 'LAB_B_RETRY_HISTORY'}}
}
function Assert-LabBRetryLedger($Store,$Anchor,[string]$PackageSha,[string]$StateDirectory){
    if(!$Store -or $Store.v -ne 1 -or $Store.runner -cne 'B-terminal-rollback-retry-01' -or $Store.side -cne 'B' -or
       $Store.origin -cne $Anchor.owner -or $Store.origin_manifest -cne $Anchor.manifest -or $Store.package_sha256 -cne $PackageSha -or
       $Store.initial_sha256 -cne $Anchor.initial_sha256 -or $Store.origin_audit_sha256 -cne $Anchor.audit_sha256 -or
       !(Lab-Equal $Store.want $Anchor.want) -or @($Store.attempts).Count -lt 1 -or @($Store.attempts).Count -gt 16){throw 'LAB_B_RETRY_PROVENANCE'}
    $hasMigration=[bool]$Store.PSObject.Properties['migration']
    if($hasMigration){Assert-LabBRetryPredecessor $Store $Anchor $PackageSha $StateDirectory}
    $seen=@{};$scope=Get-LabBRetryUpgrade
    $index=0
    foreach($a in $Store.attempts){
        $legacy=$hasMigration -and $index -eq 0 -and $a.owner -ceq $scope.owner -and $a.package_sha256 -ceq $scope.package_sha256 -and $a.status -ceq 'clean_rollback'
        if($a.owner -cnotmatch '^setup-[0-9a-f]{32}$' -or $a.owner -ceq $Anchor.owner -or $seen.ContainsKey($a.owner) -or
           $a.status -cnotin @('intent','clean_rollback') -or ($a.package_sha256 -cne $PackageSha -and !$legacy)){throw 'LAB_B_RETRY_PROVENANCE'}
        $seen[$a.owner]=$true
        if($index -lt @($Store.attempts).Count-1 -and $a.status -cne 'clean_rollback'){throw 'LAB_B_RETRY_PROVENANCE'}
        $index++
    }
}
function Assert-LabBRetryLock([IO.FileStream]$Lock,[string]$StateDirectory){
    if(!$Lock -or !$Lock.CanWrite -or $Lock.SafeFileHandle.IsClosed -or $Lock.Name -ine (Join-Path $StateDirectory 'install.lock')){throw 'LAB_INSTALL_LOCK_REQUIRED'}
}
function Assert-LabBRetryAdmission($Cfg,$Want,[string]$StateDirectory,[IO.FileStream]$Lock,$Package,[string]$PackageSha,$Handles){
    Assert-LabBRetryLock $Lock $StateDirectory;Lab-Hex $PackageSha 64
    $anchor=Assert-LabBRetryAnchor $Cfg $Want $StateDirectory
    $j=Read-LabJournalStrict (Join-Path $StateDirectory 'initial-and-journal.json')
    if(!$j -or $j.phase -cne 'rolled_back' -or @($j.rollback_errors).Count -or !(Lab-Equal $j.want $Want) -or !(Lab-Equal $j.initial $anchor.initial)){throw 'LAB_B_RETRY_TERMINAL_REQUIRED'}
    $store=Read-LabJournalStrict (Join-Path $StateDirectory 'B-terminal-retry.json')
    $upgrade=Get-LabBRetryUpgrade;$migration=$null
    if($j.setup_session -ceq $anchor.owner -and !$store){
        if(!(Lab-Equal $j.file_pins $anchor.file_pins) -or ($j.changes -join ',') -cne 'files,task,rule0,rule1,private'){throw 'LAB_B_RETRY_ORIGIN_PINS'}
        $history=Assert-LabBRetryHistory $StateDirectory ([ordered]@{}) $anchor.owner $Want $anchor.initial $anchor.file_pins
        if(!$history.Count){throw 'LAB_B_RETRY_HISTORY'}
        $kind='known_origin'
    }elseif($j.setup_session -ceq $upgrade.owner -and $store -and $store.package_sha256 -ceq $upgrade.package_sha256 -and $PackageSha -cne $upgrade.package_sha256){
        Assert-LabBRetryLedger $store $anchor $upgrade.package_sha256 $StateDirectory
        $last=@($store.attempts)[-1]
        if($anchor.initial_sha256 -cne $upgrade.initial_sha256 -or $store.PSObject.Properties['migration'] -or @($store.attempts).Count -ne 1 -or
           $last.owner -cne $upgrade.owner -or $last.status -cne 'clean_rollback' -or $last.journal_sha256 -cne (Lab-Hash (Join-Path $StateDirectory 'initial-and-journal.json'))){throw 'LAB_B_RETRY_TRANSITION'}
        $prior=Assert-LabBRetryPackage $upgrade.owner $upgrade.package_sha256
        if(!(Lab-Equal $j.file_pins $prior.files)){throw 'LAB_B_RETRY_TRANSITION'}
        $history=Assert-LabBRetryHistory $StateDirectory $store.history_pins '' $Want $anchor.initial $prior.files -Exact
        $migration=@{owner=$upgrade.owner;from_package=$upgrade.package_sha256;to_package=$PackageSha;initial_sha256=$upgrade.initial_sha256;
            ledger_sha256=(Lab-Hash (Join-Path $StateDirectory 'B-terminal-retry.json'))}
        $archive=Join-Path $StateDirectory $upgrade.archive
        if((Test-Path -LiteralPath $archive) -and (Lab-Hash $archive) -cne $migration.ledger_sha256){throw 'LAB_B_RETRY_PREDECESSOR'}
        $kind='known_package_transition'
    }else{
        Assert-LabBRetryLedger $store $anchor $PackageSha $StateDirectory
        $last=@($store.attempts)[-1]
        if($last.owner -cne $j.setup_session -or $last.status -cne 'clean_rollback' -or $last.journal_sha256 -cne (Lab-Hash (Join-Path $StateDirectory 'initial-and-journal.json')) -or
           !(Lab-Equal $j.file_pins $Package.files) -or @($store.attempts).Count -ge 16){throw 'LAB_B_RETRY_PROVENANCE'}
        $null=Assert-LabBRetryPackage $j.setup_session $PackageSha $Package.files
        $history=Assert-LabBRetryHistory $StateDirectory $store.history_pins '' $Want $anchor.initial $Package.files -Exact
        $kind='own_clean_rollback'
    }
    # Hold idle-agent/update locks only through the new intent, then release before
    # the ordinary task probe. A startup race without state must also be refused.
    $control=Join-Path $Cfg.base 'ordinary-agent/control';[void][IO.Directory]::CreateDirectory($control);Lab-Plain $control
    try{
        $Handles.agent=[IO.File]::Open((Join-Path $control 'agent.lock'),'OpenOrCreate','Write','None')
        $Handles.images=[IO.File]::Open((Join-Path $control 'images.lock'),'OpenOrCreate','Write','None')
    }catch{throw ('LAB_B_RETRY_AGENT_LOCK:'+ $_.Exception.Message)}
    $before=@{owner=$j.setup_session;file_pins=$j.file_pins;names=(Get-LabBRetryNames);initial_sha256=$anchor.initial_sha256;history=$history}
    $null=Assert-LabOldRecoveryCompleted $Cfg $Want $StateDirectory $before
    @{kind=$kind;anchor=$anchor;history=$history;prior_owner=$j.setup_session;prior_pins=$j.file_pins;store=$store;migration=$migration}
}
function Register-LabBRetryIntent($Context,$Want,[string]$StateDirectory,[string]$Owner,$Package,[string]$PackageSha){
    if($Owner -cnotmatch '^setup-[0-9a-f]{32}$' -or $Owner -ceq $Context.prior_owner){throw 'LAB_B_RETRY_OWNER'}
    $j=Read-LabJournalStrict (Join-Path $StateDirectory 'initial-and-journal.json')
    if($j.setup_session -cne $Owner -or $j.phase -cne 'installing' -or @($j.changes).Count -or !(Lab-Equal $j.file_pins $Package.files) -or
       !(Lab-Equal $j.want $Want) -or (Lab-Hash (Join-Path $StateDirectory 'initial.json')) -cne $Context.anchor.initial_sha256){throw 'LAB_B_RETRY_INTENT'}
    $history=Assert-LabBRetryHistory $StateDirectory $Context.history $Context.prior_owner $Want $Context.anchor.initial $Context.prior_pins
    $store=$Context.store
    if($Context.migration){
        $path=Join-Path $StateDirectory 'B-terminal-retry.json';$scope=Get-LabBRetryUpgrade
        if((Lab-Hash $path) -cne $Context.migration.ledger_sha256){throw 'LAB_B_RETRY_TRANSITION'}
        $archive=Join-Path $StateDirectory $scope.archive
        if(Test-Path -LiteralPath $archive){if((Lab-Hash $archive) -cne $Context.migration.ledger_sha256){throw 'LAB_B_RETRY_PREDECESSOR'}}
        else{
            $bytes=[IO.File]::ReadAllBytes($path)
            if($bytes.Length -gt 2097152){throw 'LAB_JSON_SIZE'}
            $out=[IO.File]::Open($archive,'CreateNew','Write','None')
            try{$out.Write($bytes,0,$bytes.Length);$out.Flush($true)}finally{$out.Dispose()}
            if((Lab-Hash $archive) -cne $Context.migration.ledger_sha256){throw 'LAB_B_RETRY_PREDECESSOR'}
        }
        $store.package_sha256=$PackageSha
        $store|Add-Member -NotePropertyName migration -NotePropertyValue $Context.migration
    }
    if(!$store){$store=[pscustomobject]@{v=1;runner='B-terminal-rollback-retry-01';side='B';origin=$Context.anchor.owner;origin_manifest=$Context.anchor.manifest;
        package_sha256=$PackageSha;initial_sha256=$Context.anchor.initial_sha256;origin_audit_sha256=$Context.anchor.audit_sha256;want=$Context.anchor.want;
        history_pins=$history;attempts=@()}}
    if(@($store.attempts|Where-Object owner -ceq $Owner).Count){throw 'LAB_B_RETRY_OWNER'}
    $store.attempts=@($store.attempts)+@([pscustomobject]@{owner=$Owner;status='intent';package_sha256=$PackageSha;journal_sha256=(Lab-Hash (Join-Path $StateDirectory 'initial-and-journal.json'))})
    $store.history_pins=$history
    Lab-Write (Join-Path $StateDirectory 'B-terminal-retry.json') $store -MaxBytes 2097152
}
function Assert-LabBRetryRollbackOwner($Cfg,$Want,[string]$StateDirectory,$Package,[string]$PackageSha,[string]$Owner){
    $anchor=Assert-LabBRetryAnchor $Cfg $Want $StateDirectory
    $j=Read-LabJournalStrict (Join-Path $StateDirectory 'initial-and-journal.json')
    if(!$j -or $j.setup_session -cne $Owner -or $j.phase -cnotin @('installing','rollback_incomplete','recovering','rolled_back') -or
       !(Lab-Equal $j.want $Want) -or !(Lab-Equal $j.initial $anchor.initial) -or !(Lab-Equal $j.file_pins $Package.files)){throw 'LAB_B_RETRY_ROLLBACK_OWNER'}
    # A failed provenance write must not prevent this invocation's own bounded
    # cleanup; it can never grant a future retry without a clean protected record.
    return $anchor
}
function Complete-LabBRetry($Cfg,$Want,[string]$StateDirectory,$Package,[string]$PackageSha,[string]$Owner){
    $anchor=Assert-LabBRetryRollbackOwner $Cfg $Want $StateDirectory $Package $PackageSha $Owner
    $j=Read-LabJournalStrict (Join-Path $StateDirectory 'initial-and-journal.json')
    if($j.phase -cne 'rolled_back' -or @($j.rollback_errors).Count){throw 'LAB_B_RETRY_TERMINAL_REQUIRED'}
    $store=Read-LabJournalStrict (Join-Path $StateDirectory 'B-terminal-retry.json')
    Assert-LabBRetryLedger $store $anchor $PackageSha $StateDirectory
    $last=@($store.attempts)[-1]
    if($last.owner -cne $Owner){throw 'LAB_B_RETRY_PROVENANCE'}
    $history=Assert-LabBRetryHistory $StateDirectory $store.history_pins $Owner $Want $anchor.initial $Package.files
    $before=@{owner=$Owner;file_pins=$Package.files;names=(Get-LabBRetryNames);initial_sha256=$anchor.initial_sha256;history=$history}
    $proof=Assert-LabOldRecoveryCompleted $Cfg $Want $StateDirectory $before
    $last.status='clean_rollback';$last.journal_sha256=Lab-Hash (Join-Path $StateDirectory 'initial-and-journal.json');$store.history_pins=$history
    Lab-Write (Join-Path $StateDirectory 'B-terminal-retry.json') $store -MaxBytes 2097152
    return $proof
}
