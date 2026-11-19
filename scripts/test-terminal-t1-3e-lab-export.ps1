param(
    [Parameter(Mandatory)][ValidateSet('Build','Run','Collect')][string]$ExportAction,
    [Parameter(Mandatory)][ValidateSet('A','B')][string]$ExportSide,
    [Parameter(Mandatory)][string]$ExportRoot,
    [string]$ExportPin,
    [switch]$Elevated,
    [string]$ExportSession
)
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-diagnostics.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-bootstrap.ps1"
$exportScope=Get-LabExportScope $ExportSide
$exportNames=@('test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-diagnostics.ps1','test-terminal-t1-3e-lab-bootstrap.ps1',
    'test-terminal-t1-3e-lab-export.ps1','test-terminal-t1-3e-lab-job.cs')
function ExportLiteral([string]$Text){"'"+$Text.Replace("'","''")+"'"}
function ExportEncoded([string]$Text){[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($Text))}
if($ExportAction -ceq 'Build'){
    if(Test-Path -LiteralPath $ExportRoot){throw 'LAB_EXPORT_PACKAGE_EXISTS'}
    [void][IO.Directory]::CreateDirectory($ExportRoot);Lab-Plain $ExportRoot
    $pins=[ordered]@{}
    foreach($name in $exportNames){[IO.File]::Copy((Join-Path $PSScriptRoot $name),(Join-Path $ExportRoot $name));$pins[$name]=Lab-Hash (Join-Path $ExportRoot $name)}
    Lab-Write (Join-Path $ExportRoot 'package.json') @{v=1;purpose='exact-setup-log-export';side=$ExportSide;files=$pins}
    $ExportPin=Lab-Hash (Join-Path $ExportRoot 'package.json')
    $target=Join-Path $exportScope.base 'deploy/autonomous-setup-export-review-01'
    $entry=Join-Path $target 'test-terminal-t1-3e-lab-export.ps1'
    $body='$ErrorActionPreference="Stop";$p='+(ExportLiteral $entry)+';if((Get-FileHash -LiteralPath $p).Hash -ine '+(ExportLiteral $pins['test-terminal-t1-3e-lab-export.ps1'])+'){throw "LAB_EXPORT_ENTRY_PIN"}; & $p -ExportAction Run -ExportSide '+$ExportSide+' -ExportRoot '+(ExportLiteral $target)+' -ExportPin '+$ExportPin
    $cmd="@echo off`r`n"+'"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand '+(ExportEncoded $body)+"`r`nset LAB_EXPORT_EXIT=%ERRORLEVEL%`r`necho Kilogram log export exited with code %LAB_EXPORT_EXIT%`r`npause`r`nexit /b %LAB_EXPORT_EXIT%`r`n"
    [IO.File]::WriteAllText((Join-Path $ExportRoot ('Export-'+$ExportSide+'.cmd')),$cmd,[Text.Encoding]::ASCII)
    @{package=$ExportRoot;sha256=$ExportPin;origin_session=$exportScope.session}|ConvertTo-Json -Compress
    return
}
if([Environment]::MachineName -cne $exportScope.host){throw 'LAB_EXPORT_HOST'}
Lab-Hex $ExportPin 64
$held=[Collections.Generic.List[IDisposable]]::new()
$resultPath=$null
try {
    $manifest=Join-Path $ExportRoot 'package.json';Lab-Plain $manifest
    $stream=[IO.File]::OpenRead($manifest);$held.Add($stream)
    if((Get-FileHash -InputStream $stream).Hash -ine $ExportPin){throw 'LAB_EXPORT_PACKAGE_PIN'}
    $package=Lab-Read $manifest;Lab-Keys $package @('v','purpose','side','files');Lab-Keys $package.files $exportNames
    if($package.v -ne 1 -or $package.purpose -cne 'exact-setup-log-export' -or $package.side -cne $ExportSide){throw 'LAB_EXPORT_SCOPE'}
    foreach($name in $exportNames){
        $p=Join-Path $ExportRoot $name;Lab-Plain $p;$stream=[IO.File]::OpenRead($p);$held.Add($stream)
        if((Get-FileHash -InputStream $stream).Hash -ine $package.files.$name){throw 'LAB_EXPORT_PACKAGE_PIN'}
    }
    $who=[Security.Principal.WindowsIdentity]::GetCurrent()
    try{$admin=([Security.Principal.WindowsPrincipal]::new($who)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);$sid=$who.User.Value}finally{$who.Dispose()}
    if($Elevated -or $ExportAction -ceq 'Collect'){
        if(!$admin){throw 'LAB_EXPORT_ADMIN_REQUIRED'}
        $leaf=Split-Path -Leaf $ExportSession
        if($leaf -cnotmatch '^setup-[0-9a-f]{32}$' -or [IO.Path]::GetFullPath($ExportSession) -ine (Join-Path $exportScope.base ('deploy/'+$leaf))){throw 'LAB_EXPORT_SESSION'}
        $root=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab'
        if([IO.Path]::GetFullPath($ExportRoot) -ine (Join-Path $root ('SetupPackages/'+$leaf))){throw 'LAB_EXPORT_PROTECTED_STAGE'}
        $resultPath=Join-Path $root ('SetupResults/'+$leaf+'/result.json')
    }
    if($ExportAction -ceq 'Collect'){
        # Original session/path/manifest are code constants for this campaign, not CLI inputs.
        $original=Join-Path $root ('SetupPackages/'+$exportScope.session);Lab-Plain $original
        $acl=Get-Acl -LiteralPath $original
        if(!$acl.AreAccessRulesProtected -or $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -cnotin @('S-1-5-18','S-1-5-32-544')){throw 'LAB_EXPORT_ORIGINAL_ACL'}
        foreach($ace in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])){
            if($ace.AccessControlType -ne 'Allow' -or $ace.IdentityReference.Value -cnotin @('S-1-5-18','S-1-5-32-544')){throw 'LAB_EXPORT_ORIGINAL_ACL'}
        }
        if((Lab-Hash (Join-Path $original 'package.json')) -cne $exportScope.manifest){throw 'LAB_EXPORT_ORIGINAL_PIN'}
        $journal=Join-Path $root ('t13e-lab-'+$ExportSide+'/initial-and-journal.json')
        $diagnostics=Get-LabSetupDiagnostics $original $journal $exportScope.session
        $result=@{purpose='exact-setup-log-export';host=$exportScope.host;side=$ExportSide;origin_session=$exportScope.session;
            success=($diagnostics.logs['main.stderr.log'].available -and $diagnostics.logs['main.stdout.log'].available);diagnostics=$diagnostics;
            original_manifest=$exportScope.manifest;task_policy_mutations=0;utc=[DateTime]::UtcNow.ToString('o')}
        Publish-LabDiagnosticResult $resultPath $result $exportScope.sid
        return
    }
    if($Elevated){
        $job=[KilogramLabJob]::new('Local\Kilogram-Lab-Export-'+[guid]::NewGuid().ToString('N'));$p=$null;$guard=$null
        try {
            $info=[Diagnostics.ProcessStartInfo]::new("$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe")
            $info.Arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+(Join-Path $ExportRoot 'test-terminal-t1-3e-lab-export.ps1')+'" -ExportAction Collect -ExportSide '+$ExportSide+' -ExportRoot "'+$ExportRoot+'" -ExportPin '+$ExportPin+' -ExportSession "'+$ExportSession+'"'
            $info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
            $p=[Diagnostics.Process]::Start($info);$job.Verify($p);$p.PriorityClass='BelowNormal';$guard=$job.Bound($p,45000)
            $o=$p.StandardOutput.BaseStream.CopyToAsync([IO.Stream]::Null);$e=$p.StandardError.BaseStream.CopyToAsync([IO.Stream]::Null)
            if(!$p.WaitForExit(48000) -or $guard.Expired -or $p.ExitCode -ne 0 -or !$o.Wait(2000) -or !$e.Wait(2000)){throw 'LAB_EXPORT_CHILD_FAILED_OR_TIMEOUT'}
            $result=Lab-Read $resultPath -MaxBytes 2097152
            if(!$result -or !$result.success){throw 'LAB_EXPORT_INCOMPLETE_SEE_RESULT'}
        }catch{
            if(![IO.File]::Exists($resultPath)){Publish-LabDiagnosticResult $resultPath @{success=$false;purpose='exact-setup-log-export';origin_session=$exportScope.session;
                error=(Get-LabFailureSummary $_ 'export-collector');task_policy_mutations=0} $exportScope.sid}
            throw
        }finally{try{$job.CleanupChildren(2000)}finally{if($guard){$guard.Dispose()};if($p){$p.Dispose()};$job.Dispose()}}
        return
    }
    if($admin -or $sid -cne $exportScope.sid){throw 'LAB_EXPORT_ORDINARY_TOKEN'}
    if([IO.Path]::GetFullPath($ExportRoot) -ine (Join-Path $exportScope.base 'deploy/autonomous-setup-export-review-01')){throw 'LAB_EXPORT_LOCATION'}
    $ExportSession=Join-Path $exportScope.base ('deploy/setup-'+[guid]::NewGuid().ToString('N'))
    [void][IO.Directory]::CreateDirectory($ExportSession);Lab-Plain $ExportSession
    $expected=[ordered]@{'package.json'=$ExportPin};foreach($entry in $package.files.PSObject.Properties){$expected[$entry.Name]=$entry.Value}
    $bootstrap=New-LabAdminBootstrap 'Export' $ExportSide $ExportRoot $ExportPin $ExportSession $exportScope.sid $expected
    $elevatedProcess=$null
    try{
        $elevatedProcess=Start-Process -FilePath "$env:windir/System32/WindowsPowerShell/v1.0/powershell.exe" -ArgumentList @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',(ExportEncoded $bootstrap)) -Verb RunAs -WindowStyle Hidden -PassThru
        # No work deadline precedes UAC acceptance. One fixed45s read-only child afterwards.
        if(!$elevatedProcess.WaitForExit(75000)){throw 'LAB_EXPORT_SUPERVISOR_UNCONFIRMED'}
        $resultPath=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) ('Kilogram-Lab/SetupResults/'+(Split-Path -Leaf $ExportSession)+'/result.json')
        $result=Lab-Read $resultPath -MaxBytes 2097152
        if($elevatedProcess.ExitCode -ne 0 -or !$result -or !$result.success){throw 'LAB_EXPORT_FAILED_SEE_RESULT'}
        @{state='exported';result=$resultPath;origin_session=$exportScope.session;task_policy_mutations=0}|ConvertTo-Json -Compress
    }catch{Lab-Write (Join-Path $ExportSession 'launcher-error.json') (Get-LabFailureSummary $_ 'export-launcher');throw}
    finally{if($elevatedProcess){$elevatedProcess.Dispose()}}
}catch{
    # Preserve the collector's real primary when it fails before completing its report.
    # Only a previously validated protected result location is eligible for publication.
    $failure=$_
    if(($Elevated -or $ExportAction -ceq 'Collect') -and $resultPath){
        try{if(![IO.File]::Exists($resultPath)){Publish-LabDiagnosticResult $resultPath @{success=$false;purpose='exact-setup-log-export';origin_session=$exportScope.session;
            error=(Get-LabFailureSummary $failure 'export-collector');task_policy_mutations=0} $exportScope.sid}}catch{}
    }
    throw $failure
}finally{foreach($f in $held){$f.Dispose()}}
