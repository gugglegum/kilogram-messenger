param([Parameter(Mandatory)][string]$Evidence)
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-diagnostics.ps1"
. "$PSScriptRoot/test-terminal-t1-3e-lab-bootstrap.ps1"
function Require([bool]$Value,[string]$Why){if(!$Value){throw "CHECK:$Why"}}
if(Test-Path -LiteralPath $Evidence){throw 'CHECK:fresh_evidence_required'}
$Evidence=[IO.Path]::GetFullPath($Evidence);[void][IO.Directory]::CreateDirectory($Evidence)
$cases=[ordered]@{}
foreach($side in @('A','B')){
    $scope=Get-LabExportScope $side
    $dir=Join-Path $Evidence $side;[void][IO.Directory]::CreateDirectory($dir)
    $main=[Text.Encoding]::UTF8.GetBytes('fixture primary stderr');$stdout=[Text.Encoding]::UTF8.GetBytes('fixture stdout')
    [IO.File]::WriteAllBytes((Join-Path $dir 'main.stderr.log'),$main);[IO.File]::WriteAllBytes((Join-Path $dir 'main.stdout.log'),$stdout)
    if($side -ceq 'A'){
        [IO.File]::WriteAllText((Join-Path $dir 'recovery.stderr.log'),'fixture recovery failure')
        [IO.File]::WriteAllText((Join-Path $dir 'recovery.stdout.log'),'')
    }
    Lab-Write (Join-Path $dir 'main-error.json') @{primary=@{stage='policy-audit';message='fixture primary';type='fixture';hresult=1;initial='DO_NOT_EXPORT_FAILURE_EXTRA'};
        terminal=@{stage='rollback-task';message='fixture terminal'}}
    $journal=Join-Path $dir 'initial-and-journal.json'
    if($side -ceq 'A'){
        Lab-Write $journal @{phase='rollback_incomplete';setup_session=$scope.session;primary_failure=@{stage='apply-rule0';message='first failure'};
            rollback_errors=@('task verifier failure');initial=@{credential='DO_NOT_EXPORT_INITIAL'};want=@{secret='DO_NOT_EXPORT_WANT'}}
    }
    $before=@{};foreach($file in Get-ChildItem -LiteralPath $dir -File){$before[$file.FullName]=Lab-Hash $file.FullName}
    $report=Get-LabSetupDiagnostics $dir $journal $scope.session
    Require $report.logs['main.stderr.log'].available 'actual_stderr_available'
    $sample=$report.logs['main.stderr.log'];Require (!$sample.truncated -and $sample.full_hash_complete -and $sample.snapshot_complete) 'complete_small_source'
    $roundtrip=[Convert]::FromBase64String($sample.chunks[0].data_base64)
    Require ([Convert]::ToBase64String($roundtrip) -ceq [Convert]::ToBase64String($main)) 'exact_stderr_bytes'
    Require ($sample.source_sha256 -ceq $before[(Join-Path $dir 'main.stderr.log')]) 'actual_full_log_hash'
    if($side -ceq 'A'){Require ($report.journal.phase -ceq 'rollback_incomplete' -and $report.journal.rollback_error_count -eq 1) 'minimal_recovery_status'}
    else{Require (!$report.journal.available -and !$report.logs['recovery.stderr.log'].available) 'B_missing_recovery_is_explicit'}
    $json=ConvertTo-Json -InputObject $report -Depth 18 -Compress
    Require (!$json.Contains('DO_NOT_EXPORT_')) 'unselected_fields_never_exported'
    foreach($path in $before.Keys){Require ((Lab-Hash $path) -ceq $before[$path]) 'protected_original_unchanged'}
    Lab-Write (Join-Path $Evidence ($side+'-fixture-export.json')) $report -MaxBytes 2097152
    $cases[$side+'_fixed_shape_and_original_preservation']=$true
}
# Very large journal/error fields remain bounded, and a malformed source fails explicitly.
$dir=Join-Path $Evidence 'bounds';[void][IO.Directory]::CreateDirectory($dir)
$journal=Join-Path $dir 'journal.json'
Lab-Write $journal @{phase='rollback_incomplete';setup_session='old';rollback_errors=@(1..30|ForEach-Object {'x'*4000})} -MaxBytes 2097152
$minimal=Get-LabMinimalJournal $journal 'expected'
Require (!$minimal.belongs_to_invocation -and $minimal.rollback_error_count -eq 30 -and $minimal.rollback_errors.Count -eq 16 -and $minimal.rollback_errors_truncated) 'bounded_errors_and_owner_marker'
[IO.File]::WriteAllText($journal,'{broken')
Require (!(Get-LabMinimalJournal $journal 'expected').available) 'malformed_minimal_status_explicit'
$cases.bounded_journal_and_malformed_status=$true
$large=[pscustomobject]@{payload=('x'*180000)}
$readOnlySid=(Get-LabExportScope B).sid;$public=Join-Path $Evidence 'public-large-result.json'
Publish-LabDiagnosticResult $public $large $readOnlySid
Require ((Lab-Read $public -MaxBytes 2097152).payload.Length -eq 180000) 'large_public_result_uses_explicit_bound'
$entries=@((Get-Acl -LiteralPath $public).GetAccessRules($true,$false,[Security.Principal.SecurityIdentifier])|Where-Object {$_.IdentityReference.Value -ceq $readOnlySid})
$forbidden=[Security.AccessControl.FileSystemRights]'Write, Delete, ChangePermissions, TakeOwnership, ExecuteFile'
Require ($entries.Count -eq 1 -and $entries[0].AccessControlType -eq 'Allow' -and !($entries[0].FileSystemRights -band $forbidden)) 'intended_sid_explicit_read_only_no_execute'
$cases.public_size_and_intended_sid_read_only_ACE=$true
foreach($side in @('A','B')){
    $scope=Get-LabExportScope $side
    $expectedSession=if($side -ceq 'A'){'setup-e4125cddbe3f4dba9d3017aec19af6cd'}else{'setup-3b96084a7c5a49889a0d9fcb61fb1e1e'}
    Require ($scope.session -ceq $expectedSession) 'exact_retrospective_scope'
    $exportPackage=Join-Path $Evidence ('package-'+$side)
    $builder=& "$PSScriptRoot/test-terminal-t1-3e-lab-export.ps1" -ExportAction Build -ExportSide $side -ExportRoot $exportPackage | ConvertFrom-Json
    $manifest=Lab-Read (Join-Path $exportPackage 'package.json')
    Require ((Lab-Hash (Join-Path $exportPackage 'package.json')) -ceq $builder.sha256) 'export_manifest_pin'
    Require (@($manifest.files.PSObject.Properties).Count -eq 5) 'small_fixed_export_package'
    $expected=[ordered]@{'package.json'=$builder.sha256}
    foreach($p in $manifest.files.PSObject.Properties){Require ((Lab-Hash (Join-Path $exportPackage $p.Name)) -ceq $p.Value) 'every_export_payload_pin';$expected[$p.Name]=$p.Value}
    $bootstrap=New-LabAdminBootstrap Export $side $exportPackage $builder.sha256 (Join-Path $scope.base ('deploy/setup-'+('1'*32))) $scope.sid $expected
    Require ($bootstrap.Contains("test-terminal-t1-3e-lab-export.ps1") -and !$bootstrap.Contains(" -LaunchAction ")) 'only_fixed_export_entry'
    $t=$null;$e=$null;[void][Management.Automation.Language.Parser]::ParseInput($bootstrap,[ref]$t,[ref]$e);Require (!$e.Count) 'shared_export_bootstrap_parse'
    Require ([Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($bootstrap)).Length -lt 32000) 'UAC_direct_process_limit'
    [IO.File]::WriteAllText((Join-Path $Evidence ('bootstrap-'+$side+'.ps1')),$bootstrap,[Text.UTF8Encoding]::new($false))
    $cmd=[IO.File]::ReadAllText((Join-Path $exportPackage ('Export-'+$side+'.cmd')))
    Require ((@($cmd -split "`n"|ForEach-Object Length|Measure-Object -Maximum).Maximum) -lt 8191) 'CMD_limit'
    $cases[$side+'_package_pins_scope_bootstrap_and_CMD']=$true
}
Lab-Write (Join-Path $Evidence 'checks.json') @{cases=$cases;count=$cases.Count;host_mutations=0;uac_launches=0;
    scope='fixed export source/getters on disposable exact-shape fixtures; protected admin collection and effective ACL not executed'}
@{passed=$cases.Count;host_mutations=0;uac_launches=0}|ConvertTo-Json -Compress
