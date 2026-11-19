param([Parameter(Mandatory)][string]$PackageDirectory,[Parameter(Mandatory)][string]$Evidence)
# Static package inspection only. Never evaluate a bootstrap or CMD entry.
. "$PSScriptRoot/test-terminal-t1-3e-lab-install.ps1" -Mode Library
. "$PSScriptRoot/test-terminal-t1-3e-lab-bootstrap.ps1"
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
$packageDirectory=[IO.Path]::GetFullPath($PackageDirectory);$reports=[ordered]@{}
$files=@('test-terminal-t1-3e-lab-agent.ps1','test-terminal-t1-3e-lab-common.ps1','test-terminal-t1-3e-lab-job.cs','test-terminal-t1-3e-lab-install.ps1',
    'test-terminal-t1-3e-lab-native.cs','test-terminal-t1-3e-lab-readiness.ps1','test-terminal-t1-3e-lab-setup.ps1','test-terminal-t1-3e-lab-launch.ps1',
    'test-terminal-t1-3e-lab-diagnostics.ps1','test-terminal-t1-3e-lab-bootstrap.ps1','test-terminal-t1-3e-lab-server.ps1','test-terminal-t1-3e-lab-recovery.ps1','dpapi-context.ps1','lab.json','ssh-probe.json')
foreach($which in @('A','B')){
    $root=Join-Path $packageDirectory "package-$which";$cfg=Get-LabConfig $which;$pin=Lab-Hash (Join-Path $root 'package.json')
    $locks=[Collections.Generic.List[IDisposable]]::new()
    try{
        $p=Assert-LabPackage $root $pin $which $files $cfg $locks
        Require (@(Get-ChildItem -LiteralPath $root -File).Count -eq 18) 'exact_package15_manifest_and_two_CMDs'
        $expected=[ordered]@{'package.json'=$pin};foreach($entry in $p.files.PSObject.Properties){$expected[$entry.Name]=$entry.Value}
        foreach($name in $files|Where-Object {$_ -cnotin @('lab.json','ssh-probe.json','dpapi-context.ps1')}){
            Require ((Lab-Hash (Join-Path $PSScriptRoot $name)) -ceq $p.files.$name) 'payload_matches_review_source'
        }
        Require ((Lab-Hash (Join-Path $PSScriptRoot 'test-terminal-t1-dpapi.ps1')) -ceq $p.files.'dpapi-context.ps1') 'dpapi_source_pin'
        $target=Join-Path $cfg.base 'deploy/autonomous-recovery-setup-review-01';$sizes=[ordered]@{}
        foreach($action in @('RecoverInstall','Teardown')){
            $cmd=[IO.File]::ReadAllText((Join-Path $root "$action-$which.cmd"))
            $maxLine=(@($cmd -split "`n"|ForEach-Object Length)|Measure-Object -Maximum).Maximum
            Require ($maxLine -lt 8191) 'CMD_parser_limit'
            $encoded=[regex]::Match($cmd,'-EncodedCommand ([A-Za-z0-9+/=]+)').Groups[1].Value
            $decoded=[Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($encoded))
            Require ($decoded.Contains('-LaunchAction '+$action) -and $decoded.Contains('-LaunchSide '+$which) -and $decoded.Contains($target) -and $decoded.Contains($pin) -and
                $decoded.Contains($p.files.'test-terminal-t1-3e-lab-launch.ps1')) 'fixed_CMD_target_and_dual_pins'
            $tokens=$null;$errors=$null;[void][Management.Automation.Language.Parser]::ParseInput($decoded,[ref]$tokens,[ref]$errors)
            Require (!$errors.Count) 'CMD_body_parse'
            $bootstrap=New-LabAdminBootstrap $action $which $target $pin (Join-Path $cfg.base ('deploy/setup-'+('1'*32))) $cfg.sid $expected
            [void][Management.Automation.Language.Parser]::ParseInput($bootstrap,[ref]$tokens,[ref]$errors)
            Require (!$errors.Count -and $bootstrap -cnotmatch '@(ENTRY|SOURCE|PIN|EXPECTED|SIDE|SID|SESSION|ACTION)@') 'bootstrap_parse_and_template_completion'
            $encodedLength=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($bootstrap)).Length
            Require ($encodedLength -lt 31000) 'direct_UAC_command_limit_with_prefix_margin'
            Require ($bootstrap.Contains('-LaunchAction ' + "'$action'") -and $bootstrap.Contains('-Elevated -SessionPath')) 'fixed_setup_entry_only'
            [IO.File]::WriteAllText((Join-Path $Evidence "$action-$which-bootstrap.ps1"),$bootstrap,[Text.UTF8Encoding]::new($false))
            $sizes[$action]=@{cmd_max_line=$maxLine;bootstrap_encoded_chars=$encodedLength}
        }
        $reports[$which]=@{manifest_sha256=$pin;files=15;total_files=18;target=$target;sizes=$sizes}
    }finally{foreach($f in $locks){$f.Dispose()}}
}
Lab-Write (Join-Path $Evidence 'checks.json') @{count=2;packages=$reports;host_mutations=0;uac_launches=0;scope='all payload pins, fixed commands and bootstrap parse/limits; no execution or deployment'}
@{passed=2;host_mutations=0;uac_launches=0}|ConvertTo-Json -Compress
