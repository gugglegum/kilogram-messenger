param(
    [Parameter(Mandatory)][ValidateSet('Build','Install','Teardown')][string]$LaunchAction,
    [Parameter(Mandatory)][ValidateSet('A','B')][string]$LaunchSide,
    [Parameter(Mandatory)][string]$LaunchRoot,
    [Parameter(Mandatory)][string]$ManifestPin,
    [switch]$Elevated,
    [string]$SessionPath
)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
$launchCfg=Get-LabConfig $LaunchSide
$launchFiles=@('test-terminal-t1-3e-lab-agent.ps1','test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-job.cs','test-terminal-t1-3e-lab-install.ps1',
    'test-terminal-t1-3e-lab-native.cs','test-terminal-t1-3e-lab-readiness.ps1','test-terminal-t1-3e-lab-setup.ps1','test-terminal-t1-3e-lab-launch.ps1','dpapi-context.ps1','lab.json','ssh-probe.json')
$launchLocks=[Collections.Generic.List[IDisposable]]::new()
function Quote-Lab([string]$Text){"'"+$Text.Replace("'","''")+"'"}
function Encode-Lab([string]$Text){[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($Text))}
function Publish-LabSetupResult([string]$Path,$Value) {
    # The containing directory is protected; grant read on this public receipt only.
    Lab-Write $Path $Value
    $acl=Get-Acl -LiteralPath $Path
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($launchCfg.sid),'Read','Allow'))
    Set-Acl -LiteralPath $Path -AclObject $acl
}
function Start-LabPinnedSsh($Config,$PackageDirectory) {
    $s=Lab-Read (Join-Path $PackageDirectory 'ssh-probe.json')
    Lab-Keys $s @('host','source','user','identity','known_hosts','fingerprint','known_hosts_sha256')
    if($s.host -cne $Config.peer -or $s.source -cne $Config.ip -or $s.user -cne 'test' -or
       $s.identity -cne 'C:\Users\Paul\.ssh\kilogram-test-laptop-final-ed25519' -or
       $s.known_hosts -cne 'E:\AI-Projects\kilogram-p2p-messenger\.tmp\remote-access\known_hosts' -or
       $s.fingerprint -cne 'SHA256:vvR2wQIJmbHD5033l5xVMbrSGyUcucNCy711PYU4oyc' -or
       (Lab-Hash $s.known_hosts) -cne $s.known_hosts_sha256){throw 'LAB_SSH_PROBE_PIN'}
    $fingerprints=@(& "$env:windir/System32/OpenSSH/ssh-keygen.exe" -lf $s.known_hosts)
    if($LASTEXITCODE -ne 0 -or !@($fingerprints|Where-Object {$_ -cmatch [regex]::Escape($s.fingerprint)}).Count){throw 'LAB_SSH_HOSTKEY'}
    Lab-Plain $s.identity
    $remote='powershell.exe -NoProfile -NonInteractive -EncodedCommand '+(Encode-Lab 'Start-Sleep -Seconds 750')
    $info=[Diagnostics.ProcessStartInfo]::new("$env:windir/System32/OpenSSH/ssh.exe")
    $info.Arguments='-F NUL -T -oBatchMode=yes -oIdentitiesOnly=yes -oStrictHostKeyChecking=yes -oHostKeyAlgorithms=ssh-ed25519 -oUpdateHostKeys=no -oGlobalKnownHostsFile=NUL '+
        '-oUserKnownHostsFile="'+$s.known_hosts+'" -oBindAddress='+$s.source+' -oClearAllForwardings=yes -oConnectTimeout=5 -oConnectionAttempts=1 -oServerAliveInterval=10 -oServerAliveCountMax=2 '+
        '-i "'+$s.identity+'" '+$s.user+'@'+$s.host+' '+$remote
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $p=[Diagnostics.Process]::Start($info);$p.StandardInput.Close()
    $null=$p.StandardOutput.BaseStream.CopyToAsync([IO.Stream]::Null);$null=$p.StandardError.BaseStream.CopyToAsync([IO.Stream]::Null)
    return $p
}
function Invoke-LabSetupChild([string]$Operation,[int]$Seconds,$SessionJob,[string]$LogPrefix) {
    $info=[Diagnostics.ProcessStartInfo]::new("$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe")
    $info.Arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+(Join-Path $LaunchRoot 'test-terminal-t1-3e-lab-install.ps1')+'" -Mode '+$Operation+
        ' -Side '+$LaunchSide+' -PackageRoot "'+$LaunchRoot+'" -PackageSha '+$ManifestPin+' -SetupSession "'+$SessionPath+'"'
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $out=[IO.File]::Open($LogPrefix+'.stdout.log','CreateNew','Write','Read');$err=[IO.File]::Open($LogPrefix+'.stderr.log','CreateNew','Write','Read')
    $p=$null;$guard=$null
    try {
        $p=[Diagnostics.Process]::Start($info);$SessionJob.Verify($p);$p.PriorityClass='BelowNormal';$guard=$SessionJob.Bound($p,$Seconds*1000)
        $o=$p.StandardOutput.BaseStream.CopyToAsync($out);$e=$p.StandardError.BaseStream.CopyToAsync($err)
        if(!$p.WaitForExit(($Seconds+3)*1000)){throw 'LAB_SETUP_CHILD_UNRESPONSIVE'}
        if(!$o.Wait(3000) -or !$e.Wait(3000)){throw 'LAB_SETUP_CAPTURE_INCOMPLETE'}
        return @{exit=$p.ExitCode;timeout=$guard.Expired;stdout=$LogPrefix+'.stdout.log';stderr=$LogPrefix+'.stderr.log'}
    }finally{
        try{$SessionJob.CleanupChildren(2000)}finally{if($guard){$guard.Dispose()};if($p){$p.Dispose()};$out.Dispose();$err.Dispose()}
    }
}
function Invoke-LabSetupSupervised($SessionJob,$Config,[string]$InstallSession,[string]$JournalPath,[ValidateSet('Setup','Teardown')][string]$Operation,
    [ValidateRange(1,450)][int]$MainSeconds=450,[ValidateRange(1,180)][int]$RecoverySeconds=180) {
    try{$main=Invoke-LabSetupChild $Operation $MainSeconds $SessionJob (Join-Path $LaunchRoot 'main')}
    catch{$main=@{exit=$null;timeout=$true;error=$_.Exception.Message}}
    $rollback=$null
    if($Operation -ceq 'Setup' -and ($main.timeout -or $main.exit -ne 0)){
        $saved=Lab-Read $JournalPath -MaxBytes 2097152
        # Only the invocation that wrote the intent owns timeout recovery.
        if($saved -and $saved.PSObject.Properties['setup_session'] -and $saved.setup_session -ceq (Split-Path -Leaf $InstallSession)){
            if(@($SessionJob.Children()).Count){$rollback=@{exit=$null;error='LAB_SETUP_CLEANUP_UNCONFIRMED'}}
            else{try{$rollback=Invoke-LabSetupChild 'Teardown' $RecoverySeconds $SessionJob (Join-Path $LaunchRoot 'recovery')}catch{$rollback=@{exit=$null;error=$_.Exception.Message}}}
        }
    }
    return @{host=$Config.host;action=$(if($Operation -ceq 'Setup'){'Install'}else{'Teardown'});main=$main;recovery=$rollback;
        success=(!$main.timeout -and $main.exit -eq 0);source=$LaunchRoot}
}
try {
    $launchPackage=Assert-LabPackage $LaunchRoot $ManifestPin $LaunchSide $launchFiles $launchCfg $launchLocks
    if($LaunchAction -ceq 'Build'){
        $target=Join-Path $launchCfg.base 'deploy/autonomous-combined-install-review-01'
        foreach($operation in @('Install','Teardown')){
            $script=Join-Path $target 'test-terminal-t1-3e-lab-launch.ps1'
            $body='$ErrorActionPreference="Stop";$p='+(Quote-Lab $script)+';if((Get-FileHash -LiteralPath $p).Hash -ine '+(Quote-Lab $launchPackage.files.'test-terminal-t1-3e-lab-launch.ps1')+'){throw "LAB_LAUNCH_PIN"}; & $p -LaunchAction '+$operation+' -LaunchSide '+$LaunchSide+' -LaunchRoot '+(Quote-Lab $target)+' -ManifestPin '+$ManifestPin
            $cmd="@echo off`r`n"+'"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand '+(Encode-Lab $body)+"`r`nset LAB_EXIT=%ERRORLEVEL%`r`necho Kilogram lab launcher exited with code %LAB_EXIT%`r`npause`r`nexit /b %LAB_EXIT%`r`n"
            [IO.File]::WriteAllText((Join-Path $LaunchRoot ($operation+'-'+$LaunchSide+'.cmd')),$cmd,[Text.Encoding]::ASCII)
        }
        return
    }
    if([Environment]::MachineName -cne $launchCfg.host){throw 'LAB_HOST'}
    if($Elevated){
        $null=Get-LabSetupSession $launchCfg $SessionPath
        $protectedRoot=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab/SetupPackages'
        if([IO.Path]::GetFullPath($LaunchRoot) -ine (Join-Path $protectedRoot (Split-Path -Leaf $SessionPath))){throw 'LAB_SETUP_PROTECTED_SOURCE'}
        $publicResult=Join-Path (Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab/SetupResults') ((Split-Path -Leaf $SessionPath)+'/result.json')
        $job=[KilogramLabJob]::new('Local\Kilogram-Lab-Setup-'+[guid]::NewGuid().ToString('N'))
        try {
            $operation=if($LaunchAction -ceq 'Install'){'Setup'}else{'Teardown'}
            $journal=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) "Kilogram-Lab/$($launchCfg.campaign)-$LaunchSide/initial-and-journal.json"
            $result=Invoke-LabSetupSupervised $job $launchCfg $SessionPath $journal $operation
            Lab-Write (Join-Path $LaunchRoot 'supervisor-result.json') $result
            Publish-LabSetupResult $publicResult $result
            if(!$result.success){throw 'LAB_FIXED_SETUP_FAILED'}
        }finally{$job.Dispose()}
        return
    }
    $null=Lab-Config (Join-Path $LaunchRoot 'lab.json')
    $target=Join-Path $launchCfg.base 'deploy/autonomous-combined-install-review-01'
    if([IO.Path]::GetFullPath($LaunchRoot) -ine $target){throw 'LAB_LAUNCH_LOCATION'}
    $SessionPath=Join-Path $launchCfg.base ('deploy/setup-'+[guid]::NewGuid().ToString('N'))
    [void][IO.Directory]::CreateDirectory($SessionPath);Lab-Plain $SessionPath
    $expected=[ordered]@{'package.json'=$ManifestPin};foreach($entry in $launchPackage.files.PSObject.Properties){$expected[$entry.Name]=$entry.Value}
    $expectedText='@{'+((@($expected.Keys|ForEach-Object {(Quote-Lab $_)+'='+(Quote-Lab $expected[$_])})) -join ';')+'}'
    # This complete bootstrap is passed to RunAs, rather than loading writable helpers as admin.
    # It snapshots bytes through held handles, verifies every fixed pin, then executes only a
    # private admin/System copy. No package code runs elevated from the ordinary deploy tree.
    $bootstrap=@'
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1"
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
$source=@SOURCE@;$expected=@EXPECTED@;$session=@SESSION@;$ordinarySid=@SID@
function Plain($p){$i=Get-Item -LiteralPath $p -Force;while($i){if($i.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'LAB_REPARSE'};$i=if($i -is [IO.FileInfo]){$i.Directory}else{$i.Parent}}}
$handles=@();$bytes=@{}
try{
 $root=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab'
 foreach($p in @($root,(Join-Path $root 'SetupResults'),(Join-Path $root ('SetupResults/'+(Split-Path -Leaf $session))),(Join-Path $root 'SetupPackages'),(Join-Path $root ('SetupPackages/'+(Split-Path -Leaf $session))))){
  $acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false);$acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
  foreach($sid in @('S-1-5-18','S-1-5-32-544')){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid),'FullControl','ContainerInherit, ObjectInherit','None','Allow'))}
  if(!(Test-Path -LiteralPath $p)){[void][IO.Directory]::CreateDirectory($p,$acl)};Plain $p;$a=Get-Acl -LiteralPath $p
  if(!$a.AreAccessRulesProtected -or $a.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin @('S-1-5-18','S-1-5-32-544')){throw 'LAB_STATE_ACL'}
  foreach($ace in $a.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])){if($ace.AccessControlType -ne 'Allow' -or $ace.IdentityReference.Value -notin @('S-1-5-18','S-1-5-32-544')){throw 'LAB_STATE_ACL'}}
 }
 $stage=$p;$publicResult=Join-Path $root ('SetupResults/'+(Split-Path -Leaf $session)+'/result.json')
 foreach($name in $expected.Keys){$p=Join-Path $source $name;Plain $p;$f=[IO.File]::OpenRead($p);$handles+=,$f;if($f.Length -gt 131072 -or (Get-FileHash -InputStream $f).Hash -ine $expected[$name]){throw 'LAB_PACKAGE_PIN'};$f.Position=0;$m=[IO.MemoryStream]::new();try{$f.CopyTo($m);$bytes[$name]=$m.ToArray()}finally{$m.Dispose()}}
 $p=$stage
 foreach($name in $expected.Keys){$f=[IO.File]::Open((Join-Path $p $name),'CreateNew','Write','None');try{$f.Write($bytes[$name],0,$bytes[$name].Length);$f.Flush($true)}finally{$f.Dispose()}}
 Add-Type -Path (Join-Path $p 'test-terminal-t1-3e-lab-job.cs')
 & (Join-Path $p 'test-terminal-t1-3e-lab-launch.ps1') -LaunchAction @ACTION@ -LaunchSide @SIDE@ -LaunchRoot $p -ManifestPin @PIN@ -Elevated -SessionPath $session
}catch{
 if($publicResult -and !(Test-Path -LiteralPath $publicResult)){
  $errorResult=@{success=$false;action=@ACTION@;error=$_.Exception.Message;utc=[DateTime]::UtcNow.ToString('o')}
  [IO.File]::WriteAllText($publicResult,($errorResult|ConvertTo-Json -Compress),[Text.UTF8Encoding]::new($false))
  $acl=Get-Acl -LiteralPath $publicResult;$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($ordinarySid),'Read','Allow'));Set-Acl -LiteralPath $publicResult -AclObject $acl
 };throw
}finally{foreach($f in $handles){$f.Dispose()}}
'@
    $bootstrap=$bootstrap.Replace('@SOURCE@',(Quote-Lab $LaunchRoot)).Replace('@EXPECTED@',$expectedText).Replace('@SESSION@',(Quote-Lab $SessionPath)).Replace('@ACTION@',(Quote-Lab $LaunchAction)).Replace('@SIDE@',(Quote-Lab $LaunchSide)).Replace('@PIN@',(Quote-Lab $ManifestPin)).Replace('@SID@',(Quote-Lab $launchCfg.sid))
    $actorProcess=$null;$adminProcess=$null
    try {
        # No actor/deadline is started before the user accepts the single UAC prompt.
        $adminProcess=Start-Process -FilePath "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe" -ArgumentList @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',(Encode-Lab $bootstrap)) -Verb RunAs -WindowStyle Hidden -PassThru
        if($LaunchSide -ceq 'A' -and $LaunchAction -ceq 'Install'){
            $actorProcess=Start-LabPinnedSsh $launchCfg $LaunchRoot
            Lab-Write (Join-Path $SessionPath 'actor.json') @{pid=$actorProcess.Id;started=$actorProcess.StartTime.ToFileTimeUtc();session=(Split-Path -Leaf $SessionPath)}
        }
        if(!$adminProcess.WaitForExit(690000)){throw 'LAB_SETUP_SUPERVISOR_UNCONFIRMED'}
        $resultPath=Join-Path (Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab/SetupResults') ((Split-Path -Leaf $SessionPath)+'/result.json')
        $result=Lab-Read $resultPath
        if($adminProcess.ExitCode -ne 0 -or !$result -or !$result.success){throw 'LAB_SETUP_FAILED_SEE_RECEIPTS'}
        $result|ConvertTo-Json -Depth 8
    }catch{
        Lab-Write (Join-Path $SessionPath 'launcher-error.json') @{message=$_.Exception.Message;type=$_.Exception.GetType().FullName;utc=[DateTime]::UtcNow.ToString('o')}
        throw
    }finally{
        if($actorProcess){try{if(!$actorProcess.HasExited){$actorProcess.Kill()};$null=$actorProcess.WaitForExit(3000)}finally{$actorProcess.Dispose()}}
        if($adminProcess){$adminProcess.Dispose()}
    }
}finally{foreach($f in $launchLocks){$f.Dispose()}}
