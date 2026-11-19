param([Parameter(Mandatory)][string]$Evidence,[Parameter(Mandatory)][string]$ReviewPackage)
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
. "$PSScriptRoot/test-terminal-t1-3e-lab-bootstrap.ps1"
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
$ReviewPackage=[IO.Path]::GetFullPath($ReviewPackage)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
$cfg=Get-LabConfig B;$pin=Lab-Hash (Join-Path $ReviewPackage 'package.json');$locks=[Collections.Generic.List[IDisposable]]::new()
try{
    $p=Assert-LabPackage $ReviewPackage $pin B (Get-LabBRetryNames) $cfg $locks
    Require (@(Get-ChildItem -LiteralPath $ReviewPackage -Force).Count -eq 17) 'exact15_payloads_manifest_single_retry_CMD'
    foreach($entry in $p.files.PSObject.Properties){
        if($entry.Name -cin @('lab.json','ssh-probe.json')){continue}
        $name=if($entry.Name -ceq 'dpapi-context.ps1'){'test-terminal-t1-dpapi.ps1'}else{$entry.Name}
        Require ((Lab-Hash (Join-Path $PSScriptRoot $name)) -ceq $entry.Value) 'payload_equals_review_source'
    }
    $cmd=[IO.File]::ReadAllText((Join-Path $ReviewPackage 'RetryInstall-B.cmd'))
    $encoded=[regex]::Match($cmd,'-EncodedCommand ([A-Za-z0-9+/=]+)').Groups[1].Value
    $body=[Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($encoded))
    $target=Join-Path $cfg.base 'deploy/autonomous-B-rollback-retry-review-01'
    Require ($body.Contains('-LaunchAction RetryInstallB -LaunchSide B') -and $body.Contains($target) -and $body.Contains($pin) -and $body.Contains($p.files.'test-terminal-t1-3e-lab-launch.ps1')) 'fixed_B_target_action_and_pins'
    $cmdMax=(@($cmd -split "`n"|ForEach-Object Length)|Measure-Object -Maximum).Maximum
    Require ($cmdMax -lt 8191) 'CMD_limit'
    $expected=[ordered]@{'package.json'=$pin};foreach($entry in $p.files.PSObject.Properties){$expected[$entry.Name]=$entry.Value}
    $bootstrap=New-LabAdminBootstrap RetryInstallB B $target $pin (Join-Path $cfg.base ('deploy/setup-'+('1'*32))) $cfg.sid $expected
    $tokens=$null;$errors=$null;[void][Management.Automation.Language.Parser]::ParseInput($bootstrap,[ref]$tokens,[ref]$errors)
    Require (!$errors.Count -and $bootstrap -cnotmatch '@(ENTRY|SOURCE|PIN|EXPECTED|SIDE|SID|SESSION|ACTION)@') 'completed_bootstrap_parse'
    $size=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($bootstrap)).Length
    Require ($size -lt 31000 -and $bootstrap.Contains("-LaunchAction 'RetryInstallB'") -and $bootstrap.Contains("-LaunchSide 'B'")) 'fixed_UAC_entry_and_process_limit'
    [IO.File]::WriteAllText((Join-Path $Evidence 'bootstrap.ps1'),$bootstrap,[Text.UTF8Encoding]::new($false))
    Lab-Write (Join-Path $Evidence 'checks.json') @{count=1;manifest_sha256=$pin;payloads=15;files=17;cmd_max_line=$cmdMax;bootstrap_encoded_chars=$size;
        target=$target;scope='static exact B package pins/command/bootstrap; no deployment or execution';uac_launches=0}
    @{passed=1;manifest=$pin;host_mutations=0;uac_launches=0}|ConvertTo-Json -Compress
}finally{foreach($f in $locks){$f.Dispose()}}
