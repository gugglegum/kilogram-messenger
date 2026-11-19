# The existing reviewed byte-snapshot/protected-copy bootstrap, shared by two fixed entries.
function New-LabAdminBootstrap([ValidateSet('Install','Teardown','Export')][string]$Action,[ValidateSet('A','B')][string]$Which,
    [string]$Source,[string]$Pin,[string]$Session,[string]$OrdinarySid,$Expected) {
    function Literal([string]$Text){"'"+$Text.Replace("'","''")+"'"}
    $expectedText='@{'+((@($Expected.Keys|ForEach-Object {(Literal $_)+'='+(Literal $Expected[$_])})) -join ';')+'}'
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
 @ENTRY@
}catch{
 if($publicResult -and !(Test-Path -LiteralPath $publicResult)){
  $message=[string]$_.Exception.Message;if($message.Length -gt 2048){$message=$message.Substring(0,2048)+' [truncated]'}
  $errorResult=@{success=$false;action=@ACTION@;stage='bootstrap';error=$message;type=$_.Exception.GetType().FullName;hresult=$_.Exception.HResult;utc=[DateTime]::UtcNow.ToString('o')}
  [IO.File]::WriteAllText($publicResult,($errorResult|ConvertTo-Json -Compress),[Text.UTF8Encoding]::new($false))
  $acl=Get-Acl -LiteralPath $publicResult;$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($ordinarySid),'Read','Allow'));Set-Acl -LiteralPath $publicResult -AclObject $acl
 };throw
}finally{foreach($f in $handles){$f.Dispose()}}
'@
    $entry=if($Action -ceq 'Export'){
        '& (Join-Path $p ''test-terminal-t1-3e-lab-export.ps1'') -ExportAction Run -ExportSide @SIDE@ -ExportRoot $p -ExportPin @PIN@ -Elevated -ExportSession $session'
    }else{
        '& (Join-Path $p ''test-terminal-t1-3e-lab-launch.ps1'') -LaunchAction @ACTION@ -LaunchSide @SIDE@ -LaunchRoot $p -ManifestPin @PIN@ -Elevated -SessionPath $session'
    }
    # Literal template expressions must reach the elevated bootstrap without expanding
    # caller variables; only these fixed entrypoints are permitted.
    $bootstrap=$bootstrap.Replace('@ENTRY@',$entry)
    return $bootstrap.Replace('@SOURCE@',(Literal $Source)).Replace('@EXPECTED@',$expectedText).Replace('@SESSION@',(Literal $Session)).Replace('@ACTION@',(Literal $Action)).Replace('@SIDE@',(Literal $Which)).Replace('@PIN@',(Literal $Pin)).Replace('@SID@',(Literal $OrdinarySid))
}
