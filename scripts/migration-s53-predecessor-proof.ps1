# Read-only successor verification. Trusted producer contract is explicit and
# hash-bound in activation; this helper never writes an actual-return receipt.
Set-StrictMode -Version Latest
function Read-KilogramPredecessorProof {
    param([string]$Workspace,[string]$Evidence,$Previous,$Activation,[string]$ActivationFile,[string]$ActivationHash,[string]$BindingPath,[string]$BindingHash)
    $root='01a164ec-589c-7b41-bc34-3ba158d1f22b'
    $contract=Join-Path $Workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-predecessor-proof-r2.md'
    function ProofHash([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
    function Demand($Condition,[string]$Message){if(-not $Condition){throw ('Predecessor proof: '+$Message)}}
    function Relative([string]$Path){$Path.Substring($Workspace.Length+1).Replace('\','/')}
    $rows=[Collections.Generic.List[object]]::new()
    function Anchor($Row,[string]$Expected){
        Demand ($Row.path -ceq (Relative $Expected)) 'exact linked path'
        Demand ($Row.bytes -is [ValueType] -and $Row.bytes -ge 0 -and $Row.bytes -le 8*1024*1024 -and $Row.sha256 -cmatch '^[0-9a-f]{64}$') 'bounded linked artifact'
        Demand ((Get-Item -LiteralPath $Expected).Length -eq $Row.bytes -and (ProofHash $Expected) -ceq $Row.sha256) 'linked bytes changed'
        $rows.Add($Row)
        return $Expected
    }
    function Json([string]$Path){Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json -Depth 50}
    function Finite($Number){$Number -is [ValueType] -and $Number -isnot [bool] -and -not [double]::IsNaN([double]$Number) -and -not [double]::IsInfinity([double]$Number) -and $Number -ge 0}
    $contractHash=ProofHash $contract
    Demand ($Activation.proof_producer_thread_id -ceq $root -and $Activation.proof_contract_sha256 -ceq $contractHash) 'root producer contract is not activated'
    $attempt=[string]$Previous.attempt
    Demand ($attempt -cmatch '^[CF]-s53-[a-z0-9-]+-01-attempt-[0-9]{2}$') 'attempt name'
    $receiptPath=Join-Path $Evidence ($attempt+'.root-actual-return.json')
    Demand ((Get-Item -LiteralPath $receiptPath).Length -le 65536) 'receipt cap'
    $receipt=Json $receiptPath
    $rows.Add(@{path=(Relative $receiptPath);bytes=(Get-Item -LiteralPath $receiptPath).Length;sha256=(ProofHash $receiptPath)})
    $rows.Add(@{path=(Relative $contract);bytes=(Get-Item -LiteralPath $contract).Length;sha256=$contractHash})
    Demand ($receipt.classification -ceq 'root-owned-actual-predecessor-proof-r2' -and $receipt.producer_thread_id -ceq $root -and $receipt.producer_contract_sha256 -ceq $contractHash -and $receipt.observed_after_final_return -eq $true -and -not [string]::IsNullOrWhiteSpace($receipt.inspection_source)) 'trusted later producer and inspection'
    Demand ($receipt.case -ceq $Previous.case -and $receipt.attempt -ceq $attempt -and $receipt.binding_sha256 -ceq $BindingHash -and $receipt.activation_sha256 -ceq $ActivationHash -and $receipt.actual_outer_exit_code -eq 0 -and $receipt.whole_exit_code -eq 0) 'identity/final exits'
    $a=$receipt.artifacts
    $casePath=Anchor $a.case_record (Join-Path $Evidence ($attempt+'.json'))
    $outerPath=Anchor $a.outer_record (Join-Path $Evidence ($attempt+'.outer.json'))
    $inputPath=Anchor $a.input_inventory (Join-Path $Evidence ($attempt+'.inputs.json'))
    $null=Anchor $a.binding $BindingPath;$null=Anchor $a.activation $ActivationFile
    $logPath=Anchor $a.log (Join-Path $Evidence ($attempt+'.log'))
    $historyPath=Anchor $a.raw_history (Join-Path $Evidence ($attempt+'.tool-history.json'))
    $finalPath=Anchor $a.raw_final_return (Join-Path $Evidence ($attempt+'.final-tool-return.json'))
    $case=Json $casePath;$outer=Json $outerPath;$input=Json $inputPath;$history=Json $historyPath;$final=Json $finalPath
    Demand ($case.case -ceq $Previous.case -and $case.attempt -ceq $attempt -and $case.exit_code -eq 0 -and $case.selector -ceq $Previous.selector -and $case.expected_count -eq 1 -and $case.work_seconds -eq $Previous.work_seconds -and $case.whole_seconds -eq $Previous.whole_seconds -and $case.binding_sha256 -ceq $BindingHash -and $case.activation_sha256 -ceq $ActivationHash -and $case.inventory_sha256 -ceq $a.input_inventory.sha256 -and $case.wrapper_returned -eq $true -and $case.wrapper_exit_code -eq 0) 'complete case/input correspondence'
    Demand ($case.counts.named -eq 1 -and $case.counts.passed -eq 1 -and $case.counts.completion -eq 1 -and $case.counts.cache_skip -eq 1 -and $case.baseline_before.status -ceq 'verified_match' -and $case.baseline_after.status -ceq 'verified_match' -and $case.sources_images_verified_after -eq $true -and $case.cleanup_verified -eq $true -and $case.owned_children_after_cleanup -eq 0) 'exact count/baseline/cleanup'
    Demand (-not (Test-Path -LiteralPath (Join-Path $Evidence ($attempt+'.terminal-failure.json')))) 'terminal failure exists'
    Demand ($outer.case -ceq $Previous.case -and $outer.attempt -ceq $attempt -and $outer.binding_sha256 -ceq $BindingHash -and $outer.activation_sha256 -ceq $ActivationHash -and $outer.case_record_sha256 -ceq $a.case_record.sha256 -and $outer.input_inventory_sha256 -ceq $a.input_inventory.sha256 -and $outer.child_runner_exit_code -eq 0 -and $outer.outer_exit_code -eq 0 -and $outer.whole_seconds -eq $Previous.whole_seconds -and $outer.cleanup_verified -eq $true -and $outer.owned_children_after_cleanup -eq 0) 'independent outer result'
    Demand ($input.classification -ceq 'bound_pre_run_inventory' -and $input.case -ceq $Previous.case -and $input.attempt -ceq $attempt -and $input.binding_sha256 -ceq $BindingHash -and $input.activation_sha256 -ceq $ActivationHash -and $input.baseline -ceq $Activation.baseline -and @($input.sources).Count -gt 0 -and @($input.images).Count -gt 0) 'actual pre-run inventory'
    $bound=Json $BindingPath
    Demand (@($bound.runtime_dependencies).Count -eq 1) 'one exact reviewed shell runtime dependency'
    $shell=Get-KilogramS53ShellDependency -Executable $bound.runtime_dependencies[0].path -Expected $bound.runtime_dependencies[0] -RequireCurrentHost
    foreach($record in @($Activation,$input,$case,$outer)){
        Demand (@($record.runtime_dependencies).Count -eq 1) 'runtime identity retained through predecessor/activation/outer/case/input'
        Assert-KilogramS53ShellIdentity $shell $record.runtime_dependencies[0]
    }
    foreach($set in @(@{actual=$input.sources;expected=$bound.inputs},@{actual=$input.images;expected=$bound.images})){
        foreach($row in $set.expected){$found=@($set.actual|Where-Object path -CEQ $row.path);Demand ($found.Count -eq 1 -and $found[0].bytes -eq $row.bytes -and $found[0].sha256 -ceq $row.sha256) 'exact bound source/image row in actual input'}
    }
    $log=Get-Content -LiteralPath $logPath -Raw
    Demand ([regex]::Matches($log,('(?m)^test '+[regex]::Escape($Previous.selector)+' \.\.\.')).Count -eq 1 -and [regex]::Matches($log,'(?m)^test result: ok\. 1 passed; 0 failed; 0 ignored;').Count -eq 1 -and [regex]::Matches($log,'(?m)^status=passed\r?$').Count -eq 1 -and [regex]::Matches($log,'(?m)^cache_status=skipped_explicitly\r?$').Count -eq 1) 'raw exact test log'
    if($Previous.assertion_envelope){
        $run=Join-Path $Workspace ('.tmp/migration-s53-integrated/'+$attempt)
        Demand ($case.run -ceq $run) 'managed assertion run'
        $assertionPath=Anchor $a.assertions (Join-Path $run 'assertions.json');$assertions=Json $assertionPath
        Demand ($a.assertions.sha256 -ceq $case.assertions_sha256 -and $assertions.classification -ceq 'functional_assertions_completed' -and $assertions.whole_attempt_verdict -ceq 'not_established' -and $assertions.case -ceq $Previous.case -and $assertions.run -ceq $run -and $assertions.provenance.kind -ceq 'bound_pre_run_inventory' -and $assertions.provenance.path -ceq $inputPath -and $assertions.provenance.identity -ceq $input.inventory_identity) 'assertion provenance'
    }
    Demand ($final.exit_code -eq 0 -and $final.chunk_id -is [string] -and $final.chunk_id -ceq $receipt.actual_final_chunk -and (-not $final.PSObject.Properties['session_id'] -or $null -eq $final.session_id) -and $final.output -is [string]) 'actual final return/chunk/session'
    Demand (@($history.entries).Count -gt 0 -and @($history.entries).Count -le 128 -and (Finite $history.observed_whole_seconds)) 'complete bounded actual return history'
    $sum=0.0;$ids=[Collections.Generic.HashSet[string]]::new()
    foreach($entry in $history.entries){
        Demand ($entry.tool -cin @('exec_command','write_stdin') -and $ids.Add([string]$entry.item_id)) 'distinct actual tool items'
        $rawPath=[IO.Path]::GetFullPath((Join-Path $Workspace $entry.return.path))
        Demand ($rawPath.StartsWith($Evidence+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -and [IO.Path]::GetFileName($rawPath).StartsWith($attempt+'.',[StringComparison]::Ordinal)) 'owned raw history path'
        $null=Anchor $entry.return $rawPath;$raw=Json $rawPath
        Demand ((Finite $raw.wall_time_seconds) -and $raw.chunk_id -is [string]) 'actual raw return duration/chunk'
        $sum+=[double]$raw.wall_time_seconds
    }
    Demand ($history.entries[0].tool -ceq 'exec_command' -and $history.entries[0].arguments.cmd.Contains($attempt) -and $history.entries[0].arguments.cmd.Contains($ActivationHash)) 'actual invocation arguments'
    Demand ($history.entries[-1].return.sha256 -ceq $a.raw_final_return.sha256) 'actual final raw history anchor'
    Demand ((Finite $receipt.actual_outer_seconds) -and (Finite $outer.elapsed_seconds) -and (Finite $case.elapsed_lower_bound_seconds) -and $receipt.actual_outer_seconds -eq $history.observed_whole_seconds -and $receipt.actual_outer_seconds -ge $sum -and $receipt.actual_outer_seconds -ge $outer.elapsed_seconds -and $receipt.actual_outer_seconds -ge $case.elapsed_lower_bound_seconds -and $receipt.actual_outer_seconds -lt $Previous.whole_seconds -and $case.elapsed_lower_bound_seconds -lt $Previous.work_seconds -and $outer.elapsed_seconds -lt $Previous.whole_seconds) 'complete one-time actual charge and limits'
    return @{seconds=[double]$receipt.actual_outer_seconds;rows=$rows.ToArray()}
}
