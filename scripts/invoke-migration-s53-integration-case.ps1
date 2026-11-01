[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[CF]-s53-[a-z0-9-]+-01$')][string]$Case,
    [Parameter(Mandatory)][ValidatePattern('^[CF]-s53-[a-z0-9-]+-01-attempt-[0-9]{2}$')][string]$Attempt,
    [Parameter(Mandatory)][string]$ActivationFile,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{64}$')][string]$ActivationSha256,
    [Parameter(Mandatory)][string]$ShellExecutable
)
# This source submission creates no activation. Root must bind an allocation,
# exact images and source, case order, prerequisites, and independent outer return.
$timer=[Diagnostics.Stopwatch]::StartNew()
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence'
$baseline='64c2359788ead360314793e47741adacb83b8d4f'
function Hash([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
function Write-NewJson([string]$Path,$Value){
    $raw=[Text.UTF8Encoding]::new($false).GetBytes((ConvertTo-Json -InputObject $Value -Depth 30))
    $file=[IO.File]::Open($Path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try{$file.Write($raw,0,$raw.Length);$file.Flush($true)}finally{$file.Dispose()}
}
function Verify-Rows($Rows){foreach($row in $Rows){
    $path=[IO.Path]::GetFullPath((Join-Path $workspace $row.path))
    if(-not $path.StartsWith($workspace+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Bound path escapes workspace.'}
    if((Get-Item -LiteralPath $path).Length -ne $row.bytes -or (Hash $path) -cne $row.sha256){throw "Bound input changed: $($row.path)"}
}}
# G1a case-local cardinality and immutable tuple; no filesystem/preparation.
function Assert-KilogramS53CasePrerequisites {
    param([string]$Case,$Predecessors,$Prerequisites,[string]$Workspace)
    if(@($Predecessors).Count -ne 0){throw 'G1a requires zero predecessors.'}
    $rows=@($Prerequisites)
    if($Case -cne 'F-s53-exhausted-integrated-01'){
        if($rows.Count -ne 0){throw 'Non-G1 cases require zero prerequisites.'}
        return
    }
    if($rows.Count -ne 1){throw 'G1a requires exactly one prerequisite.'}
    $row=$rows[0]
    if($row -isnot [Collections.IDictionary] -or $row.Count -ne 4){throw 'G1a prerequisite tuple fields.'}
    foreach($field in @('path','bytes','sha256','classification')){if(-not $row.Contains($field)){throw 'G1a prerequisite tuple fields.'}}
    $path=Join-Path $Workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/P2-s53-exhausted-M1.candidate.json'
    if($row.path -isnot [string] -or $row.path -cne $path -or
        ($row.bytes -isnot [long] -and $row.bytes -isnot [int]) -or $row.bytes -ne 23116 -or
        $row.sha256 -cne '788076595dbebdfc7157731609d4a1c1ce613e78821c4d866bded41f16e50c77' -or
        $row.classification -cne 'root-allocated public replica manifest; no preparation performed by adapter'){throw 'G1a prerequisite tuple binding.'}
}
function Baseline {
    $expectedHead=$baseline
    if($Case -ceq 'F-s53-exhausted-integrated-01'){
        if($binding.source_head -isnot [string] -or $binding.source_head -cnotmatch '^[0-9a-f]{40}$'){throw 'Exact current G1 source HEAD required'}
        $expectedHead=$binding.source_head
    }
    Read-KilogramS53Baseline -Workspace $workspace -Expected $expectedHead
}
. (Join-Path $PSScriptRoot 'migration-s53-metadata.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-predecessor-proof.ps1')
. (Join-Path $PSScriptRoot 'migration-s53-runtime-dependencies.ps1')
function Restore-Env([string]$Name,$Value){
    if($null -eq $Value){if(Test-Path -LiteralPath ('Env:'+$Name)){Remove-Item -LiteralPath ('Env:'+$Name)}}
    else{[Environment]::SetEnvironmentVariable($Name,[string]$Value,'Process')}
    if([Environment]::GetEnvironmentVariable($Name,'Process') -cne $Value){throw "Environment restore failed: $Name"}
}
# r29b bounded admission receiver BEGIN
function Read-KilogramS53InventoryAdmission {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]$Pointer,
        [Parameter(Mandatory)]$Candidate,
        [Parameter(Mandatory)]$Workspace,
        [Parameter(Mandatory)]$Case,
        [Parameter(Mandatory)]$Attempt,
        [Parameter(Mandatory)]$BindingRow,
        [Parameter(Mandatory)]$ExpectedArtifacts,
        [Parameter(Mandatory)][Collections.IDictionary]$Observation
    )
    $ErrorActionPreference = 'Stop'
    $tuple = {
        param($Value, [string]$Label, [long]$Maximum)
        $map = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
        if ($Value -is [Collections.IDictionary]) {
            foreach ($key in $Value.Keys) {
                if ($key -isnot [string] -or -not $map.TryAdd($key, $Value[$key])) { throw "$Label tuple fields are ambiguous." }
            }
        } elseif ($Value -is [pscustomobject]) {
            foreach ($property in $Value.PSObject.Properties) {
                if ($property -isnot [Management.Automation.PSNoteProperty] -or -not $map.TryAdd($property.Name, $property.Value)) { throw "$Label tuple must contain data fields." }
            }
        } else { throw "$Label tuple must be an object." }
        if ($map.Count -ne 3) { throw "$Label tuple has missing or unknown fields." }
        foreach ($field in @('path', 'bytes', 'sha256')) {
            if (-not $map.ContainsKey($field)) { throw "$Label tuple has missing or unknown fields." }
        }
        $length = $map['bytes']
        $integral = $length -is [byte] -or $length -is [sbyte] -or $length -is [short] -or $length -is [ushort] -or
                    $length -is [int] -or $length -is [uint] -or $length -is [long] -or $length -is [ulong]
        if (-not $integral -or $length -le 0 -or $length -gt $Maximum -or $map['path'] -isnot [string] -or
            [string]::IsNullOrWhiteSpace($map['path']) -or $map['sha256'] -isnot [string] -or
            $map['sha256'] -cnotmatch '\A[0-9a-f]{64}\z') { throw "$Label tuple types, size or digest are invalid." }
        return ,$map
    }
    $cap = Get-KilogramS53InventoryByteLimit
    if ($cap -ne 2097152 -or $Workspace -isnot [string] -or -not [IO.Path]::IsPathFullyQualified($Workspace) -or
        $Workspace -cnotmatch '\A[A-Za-z]:[\\/]' -or $Case -isnot [string] -or $Case -cnotmatch '\A[CF]-s53-[a-z0-9-]+-01\z' -or
        $Attempt -isnot [string] -or $Attempt -cnotmatch ('\A' + [regex]::Escape($Case) + '-attempt-[0-9]{2}\z')) { throw 'Admission context or fixed inventory bound is invalid.' }
    $rootPath = [IO.Path]::GetFullPath($Workspace).TrimEnd([char[]]@('\', '/'))
    if (-not [string]::Equals($rootPath, $Workspace.TrimEnd([char[]]@('\', '/')), [StringComparison]::OrdinalIgnoreCase)) { throw 'Admission workspace is not normalized.' }
    $relative = 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-evidence/' + $Attempt + '.inventory-admission.json'
    $fullPath = [IO.Path]::GetFullPath($relative, $rootPath)
    $activated = & $tuple $Pointer 'Admission' 65536
    if ($activated['path'] -cne $relative) { throw 'Admission pointer is not the exact workspace-relative owned path.' }
    $boundTuple = & $tuple $BindingRow 'Binding' ([long]::MaxValue)
    $manifestTuple = & $tuple $ExpectedArtifacts 'Manifest' ([long]::MaxValue)
    $Observation['receipt'] = [ordered]@{path=$activated['path'];bytes=$activated['bytes'];sha256=$activated['sha256']}
    $Observation['pointer_shape_validated'] = $true
    if ($Candidate -isnot [pscustomobject] -or $Candidate.Bytes -isnot [byte[]] -or
        $Candidate.ByteCount -isnot [long] -or $Candidate.ByteCount -le 0 -or $Candidate.ByteCount -gt $cap -or
        $Candidate.ByteCount -ne $Candidate.Bytes.LongLength -or $Candidate.MaximumBytes -ne $cap -or
        $Candidate.Workspace -cne $rootPath -or $Candidate.Value -isnot [Collections.IDictionary]) { throw 'Complete prepared placeholder candidate required.' }
    $value = $Candidate.Value
    $placeholder = '0' * 64
    if ($value['case'] -cne $Case -or $value['attempt'] -cne $Attempt -or
        $value['activation_sha256'] -cne $placeholder -or $value['binding_sha256'] -cne $boundTuple['sha256']) { throw 'Placeholder candidate context differs from activated binding.' }
    $utf8 = [Text.UTF8Encoding]::new($false, $true)
    $encoded = $utf8.GetBytes((ConvertTo-Json -InputObject $value -Depth 30 -WarningAction Stop -ErrorAction Stop))
    if (-not [Linq.Enumerable]::SequenceEqual[byte]($encoded, $Candidate.Bytes)) { throw 'Candidate bytes differ from its full value.' }
    $candidateSha = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Candidate.Bytes)).ToLowerInvariant()
    $Observation['placeholder_candidate_bytes'] = $Candidate.ByteCount
    $Observation['placeholder_candidate_sha256'] = $candidateSha
    $arrayNames = @('sources', 'images', 'runtime_dependencies', 'prerequisites')
    $rowCounts = [ordered]@{}; $rowDigests = [ordered]@{}
    foreach ($name in $arrayNames) {
        $rows = $value[$name]
        if ($rows -isnot [Array]) { throw "Candidate $name must retain the complete array." }
        $rowCounts[$name] = $rows.LongLength
        $arrayBytes = $utf8.GetBytes((ConvertTo-Json -InputObject $rows -Depth 30 -Compress -WarningAction Stop -ErrorAction Stop))
        $rowDigests[$name] = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($arrayBytes)).ToLowerInvariant()
    }
    foreach ($row in $value['sources']) {
        if ([string]::Equals([IO.Path]::GetFullPath($row['path'], $rootPath), $fullPath, [StringComparison]::OrdinalIgnoreCase)) { throw 'Admission receipt cannot be part of its own candidate source list.' }
    }
    $current = $fullPath
    while ($true) {
        $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or ($current -ceq $fullPath -and $item.PSIsContainer)) { throw 'Admission path crosses a reparse point or is a directory.' }
        if ([string]::Equals($current, $rootPath, [StringComparison]::OrdinalIgnoreCase)) { break }
        $current = [IO.Path]::GetDirectoryName($current)
        if ([string]::IsNullOrEmpty($current) -or
            (-not [string]::Equals($current, $rootPath, [StringComparison]::OrdinalIgnoreCase) -and
             -not $current.StartsWith($rootPath + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) { throw 'Admission ownership escaped workspace.' }
    }
    $file = $null; $memory = $null; $document = $null
    try {
        $file = [IO.File]::Open($fullPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ($file.Length -gt 65536 -or $file.Length -ne $activated['bytes']) { throw 'Admission opened-handle size differs from activated tuple.' }
        $memory = [IO.MemoryStream]::new()
        $buffer = [byte[]]::new(4096)
        [long]$total = 0
        while ($total -lt 65537) {
            $count = $file.Read($buffer, 0, [int][Math]::Min([long]$buffer.Length, 65537 - $total))
            if ($count -eq 0) { break }
            $memory.Write($buffer, 0, $count); $total += $count
        }
        if ($total -gt 65536 -or $total -ne $activated['bytes']) { throw 'Admission actual size differs from activated tuple.' }
        $raw = $memory.ToArray()
        $actualSha = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)).ToLowerInvariant()
        if ($actualSha -cne $activated['sha256']) { throw 'Admission actual-buffer SHA256 differs from activated tuple.' }
        # Authentication precedes parsing or trusting any receipt field.
        $document = [Text.Json.JsonDocument]::Parse([string]$utf8.GetString($raw))
        $objectFields = {
            param($Element, $Fields, [string]$Label)
            if ($Element.ValueKind -ne [Text.Json.JsonValueKind]::Object) { throw "$Label must be a JSON object." }
            $remaining = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach ($field in $Fields) { [void]$remaining.Add($field) }
            foreach ($property in $Element.EnumerateObject()) {
                if (-not $remaining.Remove($property.Name)) { throw "$Label has unknown or duplicate JSON fields." }
            }
            if ($remaining.Count) { throw "$Label has missing JSON fields." }
            return $Element
        }
        $stringField = {
            param($Element, [string]$Field)
            $member = $Element.GetProperty($Field)
            if ($member.ValueKind -ne [Text.Json.JsonValueKind]::String) { throw "Admission $Field must be a string." }
            return $member.GetString()
        }
        $integerField = {
            param($Element, [string]$Field, [long]$Minimum, [long]$Maximum)
            $member = $Element.GetProperty($Field); [long]$number = 0
            if ($member.ValueKind -ne [Text.Json.JsonValueKind]::Number -or -not $member.TryGetInt64([ref]$number) -or
                $number -lt $Minimum -or $number -gt $Maximum) { throw "Admission $Field must be a bounded integral number." }
            return $number
        }
        $receipt = & $objectFields $document.RootElement @('classification','schema_version','case','attempt','run','inventory_identity','baseline','binding','expected_artifacts','serializer','maximum_bytes','activation_sha256_placeholder','candidate_bytes','candidate_sha256','row_counts','row_array_sha256') 'Admission'
        if ((& $stringField $receipt 'classification') -cne 's53-full-inventory-admission-r29' -or
            (& $integerField $receipt 'schema_version' 1 1) -ne 1 -or
            (& $stringField $receipt 'serializer') -cne 'powershell-json-depth30-pretty-utf8-no-bom-no-final-newline-r29' -or
            (& $integerField $receipt 'maximum_bytes' $cap $cap) -ne $cap -or
            (& $stringField $receipt 'activation_sha256_placeholder') -cne $placeholder) { throw 'Admission schema, serializer, placeholder or fixed cap differs.' }
        foreach ($field in @('case', 'attempt', 'run', 'inventory_identity', 'baseline')) {
            if ((& $stringField $receipt $field) -cne $value[$field]) { throw "Admission $field differs from complete candidate." }
        }
        foreach ($pair in @(@{name='binding';expected=$boundTuple}, @{name='expected_artifacts';expected=$manifestTuple})) {
            $linked = & $objectFields ($receipt.GetProperty($pair.name)) @('path','bytes','sha256') $pair.name
            if ((& $stringField $linked 'path') -cne $pair.expected['path'] -or
                (& $integerField $linked 'bytes' 1 ([long]::MaxValue)) -ne $pair.expected['bytes'] -or
                (& $stringField $linked 'sha256') -cne $pair.expected['sha256']) { throw "Admission $($pair.name) tuple differs." }
        }
        if ((& $integerField $receipt 'candidate_bytes' 1 $cap) -ne $Candidate.ByteCount -or
            (& $stringField $receipt 'candidate_sha256') -cne $candidateSha) { throw 'Admission complete candidate bytes or SHA256 differs.' }
        $counts = & $objectFields ($receipt.GetProperty('row_counts')) $arrayNames 'Admission row_counts'
        $digests = & $objectFields ($receipt.GetProperty('row_array_sha256')) $arrayNames 'Admission row_array_sha256'
        foreach ($name in $arrayNames) {
            if ((& $integerField $counts $name 0 ([long]::MaxValue)) -ne $rowCounts[$name] -or
                (& $stringField $digests $name) -cne $rowDigests[$name]) { throw "Admission full ordered $name array differs." }
        }
        return [pscustomobject][ordered]@{
            Receipt = [ordered]@{path=$activated['path'];bytes=$raw.LongLength;sha256=$actualSha}
            CandidateBytes = $Candidate.ByteCount
            CandidateSha256 = $candidateSha
            MaximumBytes = $cap
            RowCounts = $rowCounts
            RowArraySha256 = $rowDigests
        }
    } finally {
        if ($null -ne $document) { $document.Dispose() }
        if ($null -ne $memory) { $memory.Dispose() }
        if ($null -ne $file) { $file.Dispose() }
    }
}
# r29b bounded admission receiver END

if(-not $Attempt.StartsWith($Case+'-attempt-',[StringComparison]::Ordinal)){throw 'Case/attempt mismatch.'}
if(-not [IO.Path]::IsPathRooted($ActivationFile) -or (Get-Item -LiteralPath $ActivationFile).Length -gt 64*1024 -or (Hash $ActivationFile) -cne $ActivationSha256){throw 'Exact bounded root activation required.'}
$activation=Get-Content -LiteralPath $ActivationFile -Raw|ConvertFrom-Json
$bindingName=if($Case -ceq 'F-s53-exhausted-integrated-01'){'C-s53-g1-continuation-C2-01.binding.candidate.json'}else{'S5.3-integration-binding-r30.json'}
$bindingPath=Join-Path $evidence $bindingName
$bindingHash=Hash $bindingPath
$binding=Get-Content -LiteralPath $bindingPath -Raw|ConvertFrom-Json
if($activation.classification -cne 'root-approved-integration-allocation' -or $activation.activated -ne $true -or $activation.binding_sha256 -cne $bindingHash -or $activation.baseline -cne $baseline -or $activation.execution_mode -cne 'NativeToken' -or $activation.stop_on_first_failure -ne $true -or $activation.require_actual_outer_return_before_successor -ne $true){throw 'Inactive/unbound root allocation.'}
if($activation.allocation_id -cnotmatch '^s53-integrated-[a-z0-9-]+$'){throw 'Allocation identity.'}
if(@($binding.runtime_dependencies).Count -ne 1 -or @($activation.runtime_dependencies).Count -ne 1){throw 'Exactly one reviewed shell dependency required.'}
$shell=Get-KilogramS53ShellDependency -Executable $ShellExecutable -Expected $binding.runtime_dependencies[0] -RequireCurrentHost
Assert-KilogramS53ShellIdentity $shell $activation.runtime_dependencies[0]
$proofContract=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-handoff-predecessor-proof-r2.md'
if($activation.proof_producer_thread_id -cne '01a164ec-589c-7b41-bc34-3ba158d1f22b' -or $activation.proof_contract_sha256 -cne (Hash $proofContract)){throw 'Root-owned actual-return producer contract required.'}
foreach($group in @('active_limits_seconds','carried_actual_seconds','allocation_ceiling_seconds')){foreach($set in @('C','F','P')){
    $amount=$activation.$group.$set
    if($amount -isnot [ValueType] -or $amount -is [bool] -or [double]::IsNaN([double]$amount) -or [double]::IsInfinity([double]$amount) -or $amount -lt 0){throw 'Invalid root budget amount.'}
}}
$policy=@($binding.cases|Where-Object case -CEQ $Case)
$allocated=@($activation.cases|Where-Object case -CEQ $Case|Where-Object attempt -CEQ $Attempt)
if($policy.Count -ne 1 -or $allocated.Count -ne 1){throw 'Case not uniquely activated.'}
$policy=$policy[0];$allocated=$allocated[0]
$buildProfile='verification';$frozenArguments=@{}
if($Case -cin @('F-s53-exhausted-integrated-01','C-s53-admission-r6-01','C-s53-regressions-r2-01','C-s53-seen-batch-r1-01','C-s53-mixed-integrated-01','C-s53-policy-integrated-01','F-s53-discovery-integrated-01','F-s53-preq-integrated-01','F-s53-p-after-commit-01','F-s53-p-before-commit-01','F-s53-p-before-publication-01','F-s53-a-after-commit-01','F-s53-k-after-commit-01','F-s53-committed-after-commit-01','F-s53-a-before-commit-01','F-s53-a-before-publication-01','F-s53-k-before-commit-01','F-s53-k-before-publication-01','F-s53-committed-before-commit-01','F-s53-committed-before-publication-01')){
    if($policy.build_profile -cne 'verification-scale' -or $allocated.build_profile -cne 'verification-scale' -or $activation.expected_artifacts_sha256 -cne $binding.expected_artifacts.sha256){throw 'Frozen scale activation/profile identity required.'}
    $buildProfile='verification-scale'
    $frozenArguments=@{ExpectedArtifactsManifest=(Join-Path $workspace $binding.expected_artifacts.path);ExpectedArtifactsSha256=$binding.expected_artifacts.sha256;CargoTarget='x86_64-pc-windows-msvc';StableArtifactsDirectory=(Join-Path $workspace 'target/verification-scale/deps')}
}
if($policy.expected_count -ne 1 -or $allocated.work_seconds -ne $policy.work_seconds -or $allocated.whole_seconds -ne $policy.whole_seconds -or $allocated.selector -cne $policy.selector){throw 'Root case limits/selector changed.'}
$start=Join-Path $evidence ($Attempt+'.started.json')
$recordPath=Join-Path $evidence ($Attempt+'.json')
$inputPath=Join-Path $evidence ($Attempt+'.inputs.json')
$pendingInputPath=$inputPath+'.pending'
$log=Join-Path $evidence ($Attempt+'.log')
foreach($path in @($start,$recordPath,$inputPath,$pendingInputPath,$log)){if(Test-Path -LiteralPath $path){throw 'One attempt only; preserve existing evidence.'}}
$lockPath=Join-Path $workspace ('.tmp/'+$activation.allocation_id+'.lock')
$lock=[IO.File]::Open($lockPath,[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
$deadline=$null;$code=1;$failure=$null;$cleanup=$false;$inputHash=$null;$assertionHash=$null
$verified=$false;$returned=$false;$wrapperExit=$null;$counts=$null;$baselineBefore=$null;$baselineAfter=$null
$charged=@{C=0.0;F=0.0;P=0.0};$predecessors=@()
$run=Join-Path $workspace ('.tmp/migration-s53-integrated/'+$Attempt)
$inventoryEnvelope=$null;$placeholderCandidate=$null;$inventoryArguments=$null;$inventoryAdmissionPointer=$null;$inventoryAdmissionRead=$null;$inventoryBindingRow=$null
$inventoryFinalErrors=[Collections.Generic.List[string]]::new()
$inventoryAdmissionEvidence=[ordered]@{
    receipt=$null
    maximum_bytes=[long]2097152
    placeholder_candidate_bytes=$null
    placeholder_candidate_sha256=$null
    actual_inventory_bytes=$null
    actual_inventory_sha256=$null
    pointer_shape_validated=$false
    receipt_validated_before_publish=$false
    receipt_validated_before_run=$false
    receipt_validated_after_cleanup=$false
    pending_created=$false
    pending_validated=$false
    published=$false
    inventory_validated_before_run=$false
    inventory_validated_after_cleanup=$false
    run_reached=$false
    phase='not_started'
    failure_phase=$null
    final_validation_errors=@()
}
$environment=@{}
foreach($name in @('CARGO_NET_OFFLINE','KILOGRAM_S53_CASE','KILOGRAM_S53_INVENTORY','KILOGRAM_S53_INVENTORY_ID','KILOGRAM_S53_SHELL','KILOGRAM_S53_LOSS','KILOGRAM_S53_EXHAUSTED_MANIFEST','KILOGRAM_TEST_OD1_SCHEDULE','KILOGRAM_TEST_OD1_CLOCK_FILE','KILOGRAM_TEST_OD1_PRIMARY_PAUSE_FILE','KILOGRAM_TEST_MIGRATION_PAUSE_FILE','KILOGRAM_TEST_OD2_SCHEDULE','KILOGRAM_TEST_OD2_TIMINGS')){$environment[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
try{
    . (Join-Path $PSScriptRoot 'migration-s5-l1-deadline.ps1')
    $deadline=[KilogramS5Deadline]::new($policy.work_seconds-$timer.Elapsed.TotalSeconds,$policy.whole_seconds-$timer.Elapsed.TotalSeconds)
    $inventoryAdmissionEvidence.phase='activation_admission'
    if($activation.cases -isnot [Array] -or $activation.cases.Count -ne 1 -or
        $activation.cases[0].case -isnot [string] -or $activation.cases[0].case -cne $Case -or
        $activation.cases[0].attempt -isnot [string] -or $activation.cases[0].attempt -cne $Attempt){throw 'r29 admission requires exactly one allocated case/attempt.'}
    $capProperty=$activation.PSObject.Properties['inventory_max_bytes']
    if($null -eq $capProperty){throw 'Activated fixed inventory cap is missing.'}
    $activatedCap=$capProperty.Value
    $capIntegral=$activatedCap -is [byte] -or $activatedCap -is [sbyte] -or $activatedCap -is [short] -or $activatedCap -is [ushort] -or
                 $activatedCap -is [int] -or $activatedCap -is [uint] -or $activatedCap -is [long] -or $activatedCap -is [ulong]
    if(-not $capIntegral -or $activatedCap -ne 2097152 -or (Get-KilogramS53InventoryByteLimit) -ne $activatedCap){throw 'Activated inventory cap differs from fixed helper cap.'}
    $pointerProperty=$allocated.PSObject.Properties['inventory_admission']
    if($null -eq $pointerProperty){throw 'Activated admission tuple is missing.'}
    $inventoryAdmissionPointer=$pointerProperty.Value
    . (Join-Path $PSScriptRoot 'cargo-resource-policy.ps1')
    $null=Set-KilogramCargoResourcePolicy -RequestedJobs 2
    $env:CARGO_NET_OFFLINE='true'
    $baselineBefore=Baseline
    if($baselineBefore.status -cne 'verified_match'){throw ('Baseline before is '+$baselineBefore.status+': '+$baselineBefore.error)}
    Verify-Rows $binding.inputs;Verify-Rows $binding.images
    # Root reconciles actual outer durations and makes each immutable predecessor
    # receipt. Case-local elapsed or an assertion snapshot alone cannot unlock it.
    $ordinal=[Array]::IndexOf(@($activation.cases|ForEach-Object attempt),$Attempt)
    if($ordinal -lt 0){throw 'Attempt sequence identity.'}
    if($ordinal -ne 0){throw 'r29 admission cannot authenticate future outputs of a multi-case sequence.'}
    $charged=@{C=0.0;F=0.0;P=0.0};$predecessors=@()
    for($n=0;$n -lt $ordinal;$n++){
        $previous=$activation.cases[$n]
        $previousPolicy=@($binding.cases|Where-Object case -CEQ $previous.case)
        if($previousPolicy.Count -ne 1 -or $previousPolicy[0].selector -cne $previous.selector -or $previousPolicy[0].work_seconds -ne $previous.work_seconds -or $previousPolicy[0].whole_seconds -ne $previous.whole_seconds){throw 'Predecessor exact case policy.'}
        $proofPrevious=[pscustomobject]@{case=$previous.case;attempt=$previous.attempt;selector=$previous.selector;work_seconds=$previous.work_seconds;whole_seconds=$previous.whole_seconds;assertion_envelope=$previousPolicy[0].assertion_envelope}
        $proof=Read-KilogramPredecessorProof -Workspace $workspace -Evidence $evidence -Previous $proofPrevious -Activation $activation -ActivationFile $ActivationFile -ActivationHash $ActivationSha256 -BindingPath $bindingPath -BindingHash $bindingHash
        $set=$previous.case.Substring(0,1);$charged[$set]+=$proof.seconds
        $predecessors+=@($proof.rows)
    }
    $predecessors=@($predecessors|Group-Object path|ForEach-Object {
        $unique=@($_.Group|ForEach-Object {$_.bytes.ToString()+'|'+$_.sha256}|Sort-Object -Unique)
        if($unique.Count -ne 1){throw 'Conflicting immutable predecessor anchor.'}
        $_.Group[0]
    })
    foreach($set in @('C','F','P')){
        $need=if($set -ceq $Case.Substring(0,1)){$policy.whole_seconds}else{0}
        if($charged[$set]+$need -gt $activation.active_limits_seconds.$set -or $activation.carried_actual_seconds.$set+$charged[$set]+$need -gt $activation.allocation_ceiling_seconds.$set){throw "Insufficient explicitly allocated $set balance."}
    }
    if(Test-Path -LiteralPath $run){throw 'Fresh managed run already exists.'}
    [void][IO.Directory]::CreateDirectory($run)
    Write-NewJson $start @{case=$Case;attempt=$Attempt;classification='attempt-start-not-final-result';binding_sha256=$bindingHash;activation_sha256=$ActivationSha256;started_utc=[DateTime]::UtcNow.ToString('O');work_seconds=$policy.work_seconds;whole_seconds=$policy.whole_seconds}
    $identity='S5.3/integrated/'+$Attempt+'/'+$bindingHash
    $prerequisites=@()
    if($Case -ceq 'F-s53-exhausted-integrated-01'){
        $manifest=Join-Path $evidence 'P2-s53-exhausted-M1.candidate.json'
        $d2=Join-Path $evidence 'P2-s53-exhausted-M1.D2-root-accepted.md'
        if($allocated.exhausted_manifest -cne $manifest -or $allocated.exhausted_manifest_sha256 -cne '788076595dbebdfc7157731609d4a1c1ce613e78821c4d866bded41f16e50c77' -or
            $allocated.exhausted_prerequisite_d2_sha256 -cne '9ea318a5531dc9a5b5ff605eae2bcf4328772db9cea7e84fe383baf4bbcc33a9' -or
            (Get-Item -LiteralPath $d2).Length -ne 5661 -or (Hash $d2) -cne $allocated.exhausted_prerequisite_d2_sha256 -or
            (Get-Item -LiteralPath $manifest).Length -ne 23116 -or (Hash $manifest) -cne $allocated.exhausted_manifest_sha256 -or
            $allocated.charged_preparation_accepted -ne $true){throw 'Exact M1 and existing D2 must be bound by the root allocation.'}
        $prerequisites+=@{path=$manifest;sha256=(Hash $manifest);bytes=(Get-Item -LiteralPath $manifest).Length;classification='root-allocated public replica manifest; no preparation performed by adapter'}
    }
    if(@(Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.ProcessName -like 'kilogram-*'}).Count){throw 'Live Kilogram process prevents stable recopy.'}
    # Use the exact same package/feature graph as the frozen compile. Recheck
    # staged bytes before creating the actual pre-run inventory or discovery.
    & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -CargoJobs 2 -Profile $buildProfile @frozenArguments -BuildEvidencePath $(if($frozenArguments.Count){Join-Path $evidence ($Attempt+'.compile-artifacts.json')}else{$null}) -SkipCacheStatus *> (Join-Path $evidence ($Attempt+'.compile.log'))
    if($LASTEXITCODE -ne 0){throw 'Compile-only pre-run staging failed.'}
    Verify-Rows $binding.inputs;Verify-Rows $binding.images
    $null=Get-KilogramS53ShellDependency -Executable $shell.path -Expected $shell -RequireCurrentHost
    $inventoryAdmissionEvidence.phase='placeholder_candidate'
    Assert-KilogramS53CasePrerequisites -Case $Case -Predecessors $predecessors -Prerequisites $prerequisites -Workspace $workspace
    if((Hash $bindingPath) -cne $bindingHash){throw 'Binding changed before complete candidate construction.'}
    $inventoryBindingRow=[ordered]@{path=[IO.Path]::GetRelativePath($workspace,$bindingPath).Replace('\','/');bytes=(Get-Item -LiteralPath $bindingPath).Length;sha256=$bindingHash}
    $inventoryArguments=@{Case=$Case;Attempt=$Attempt;Workspace=$workspace;Binding=$binding;BindingHash=$bindingHash;Shell=$shell;Predecessors=$predecessors;Prerequisites=$prerequisites}
    $placeholderCandidate=New-KilogramS53InventoryEnvelope @inventoryArguments -ActivationSha256 ('0'*64)
    if($placeholderCandidate.Value['run'] -cne $run -or $placeholderCandidate.Value['inventory_identity'] -cne $identity -or $placeholderCandidate.InventoryPath -cne $inputPath){throw 'Derived complete candidate context differs from adapter.'}
    $inventoryAdmissionEvidence.phase='receipt_before_publication'
    $inventoryAdmissionRead=Read-KilogramS53InventoryAdmission -Pointer $inventoryAdmissionPointer -Candidate $placeholderCandidate -Workspace $workspace -Case $Case -Attempt $Attempt -BindingRow $inventoryBindingRow -ExpectedArtifacts $binding.expected_artifacts -Observation $inventoryAdmissionEvidence
    $inventoryAdmissionEvidence.receipt_validated_before_publish=$true
    $inventoryAdmissionEvidence.phase='real_inventory_envelope'
    $inventoryEnvelope=New-KilogramS53InventoryEnvelope @inventoryArguments -ActivationSha256 $ActivationSha256
    $realWithPlaceholder=[ordered]@{}
    foreach($field in $inventoryEnvelope.Value.Keys){$realWithPlaceholder[$field]=$inventoryEnvelope.Value[$field]}
    $realWithPlaceholder['activation_sha256']='0'*64
    $normalizedReal=[Text.UTF8Encoding]::new($false,$true).GetBytes((ConvertTo-Json -InputObject $realWithPlaceholder -Depth 30 -WarningAction Stop -ErrorAction Stop))
    if($inventoryEnvelope.ByteCount -ne $inventoryAdmissionRead.CandidateBytes -or
        -not [Linq.Enumerable]::SequenceEqual[byte]($normalizedReal,$placeholderCandidate.Bytes)){throw 'Actual activation substitution changed more than the fixed-width digest.'}
    $inventoryAdmissionEvidence.phase='pending_write'
    $pendingFile=$null
    try{
        $pendingFile=[IO.File]::Open($pendingInputPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
        $inventoryAdmissionEvidence.pending_created=$true
        $pendingFile.Write($inventoryEnvelope.Bytes,0,$inventoryEnvelope.Bytes.Length)
        $pendingFile.Flush($true)
    }finally{if($null -ne $pendingFile){$pendingFile.Dispose()}}
    $inventoryAdmissionEvidence.phase='pending_readback'
    $null=Assert-KilogramS53WrittenInventory -Path $pendingInputPath -Envelope $inventoryEnvelope
    $inventoryAdmissionEvidence.pending_validated=$true
    if([IO.Path]::GetDirectoryName($pendingInputPath) -cne [IO.Path]::GetDirectoryName($inputPath)){throw 'Inventory publication must stay in the same directory.'}
    $inventoryAdmissionEvidence.phase='publish_no_overwrite'
    [IO.File]::Move($pendingInputPath,$inputPath,$false)
    $inventoryAdmissionEvidence.published=$true
    $inventoryAdmissionEvidence.phase='published_readback'
    $written=Assert-KilogramS53WrittenInventory -Path $inputPath -Envelope $inventoryEnvelope
    $inputHash=$written.Sha256
    $inventoryAdmissionEvidence.actual_inventory_bytes=$written.ByteCount
    $inventoryAdmissionEvidence.actual_inventory_sha256=$inputHash
    foreach($name in $environment.Keys){Restore-Env $name $null}
    $env:CARGO_NET_OFFLINE='true';$env:KILOGRAM_S53_CASE=$Case;$env:KILOGRAM_S53_INVENTORY=$inputPath;$env:KILOGRAM_S53_INVENTORY_ID=$identity;$env:KILOGRAM_S53_SHELL=$shell.path
    if($policy.loss){$env:KILOGRAM_S53_LOSS=$policy.loss}
    if($Case -ceq 'C-s53-policy-integrated-01'){$clock=Join-Path $run 'policy.clock';[IO.File]::WriteAllText($clock,'0');$env:KILOGRAM_TEST_OD1_CLOCK_FILE=$clock}
    if($Case -cin @('C-s53-regressions-r2-01','C-s53-admission-r6-01')){
        $clock=Join-Path $run 'regressions.clock';[IO.File]::WriteAllText($clock,'0');$env:KILOGRAM_TEST_OD1_CLOCK_FILE=$clock
        $schedule=Join-Path $run 'regressions.schedule.json';[IO.File]::WriteAllText($schedule,'{}');$env:KILOGRAM_TEST_OD1_SCHEDULE=$schedule
    }
    if($Case -ceq 'C-s53-seen-batch-r1-01'){
        $clock=Join-Path $run 'seen-batch.clock';[IO.File]::WriteAllText($clock,'0');$env:KILOGRAM_TEST_OD1_CLOCK_FILE=$clock
        $schedule=Join-Path $run 'seen-batch.schedule.json';[IO.File]::WriteAllText($schedule,'{}');$env:KILOGRAM_TEST_OD1_SCHEDULE=$schedule
    }
    if($Case -ceq 'F-s53-exhausted-integrated-01'){$env:KILOGRAM_S53_EXHAUSTED_MANIFEST=$manifest}
    if(@(Get-Process -ErrorAction SilentlyContinue|Where-Object {$_.ProcessName -like 'kilogram-*'}).Count){throw 'Live Kilogram process prevents stable recopy.'}
    $inventoryAdmissionEvidence.phase='before_run'
    if((Hash $bindingPath) -cne $bindingHash -or (Get-Item -LiteralPath $bindingPath).Length -ne $inventoryBindingRow.bytes){throw 'Binding changed before Run admission recheck.'}
    $null=Read-KilogramS53InventoryAdmission -Pointer $inventoryAdmissionPointer -Candidate $placeholderCandidate -Workspace $workspace -Case $Case -Attempt $Attempt -BindingRow $inventoryBindingRow -ExpectedArtifacts $binding.expected_artifacts -Observation $inventoryAdmissionEvidence
    $inventoryAdmissionEvidence.receipt_validated_before_run=$true
    $beforeRunInventory=Assert-KilogramS53WrittenInventory -Path $inputPath -Envelope $inventoryEnvelope
    if($beforeRunInventory.Sha256 -cne $inputHash -or $beforeRunInventory.ByteCount -ne $inventoryAdmissionRead.CandidateBytes){throw 'Actual inventory changed before Run.'}
    $inventoryAdmissionEvidence.inventory_validated_before_run=$true
    $deadline.CheckWork()
    $inventoryAdmissionEvidence.run_reached=$true
    $inventoryAdmissionEvidence.phase='run'
    & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -CargoJobs 2 -Profile $buildProfile @frozenArguments -BuildEvidencePath $(if($frozenArguments.Count){Join-Path $evidence ($Attempt+'.run-artifacts.json')}else{$null}) -Filter $policy.selector -Run -NoCapture -SkipCacheStatus *> $log
    $wrapperExit=$LASTEXITCODE;$returned=$true
    $raw=Get-Content -LiteralPath $log -Raw
    $counts=@{named=[regex]::Matches($raw,('(?m)^test '+[regex]::Escape($policy.selector)+' \.\.\.')).Count;passed=[regex]::Matches($raw,'(?m)^test result: ok\. 1 passed; 0 failed; 0 ignored;').Count;completion=[regex]::Matches($raw,'(?m)^status=passed\r?$').Count;cache_skip=[regex]::Matches($raw,'(?m)^cache_status=skipped_explicitly\r?$').Count}
    if($wrapperExit -ne 0 -or $counts.named -ne 1 -or $counts.passed -ne 1 -or $counts.completion -ne 1 -or $counts.cache_skip -ne 1){throw 'Exact named1/0, wrapper completion and explicit cache-skip required.'}
    $assertionPath=Join-Path $run 'assertions.json'
    # Fresh S4 has its original independently bounded assertion/log controller.
    if($policy.assertion_envelope){
        $assertions=Get-Content -LiteralPath $assertionPath -Raw|ConvertFrom-Json
        if($assertions.classification -cne 'functional_assertions_completed' -or $assertions.whole_attempt_verdict -cne 'not_established' -or $assertions.case -cne $Case -or $assertions.run -cne $run -or $assertions.provenance.kind -cne 'bound_pre_run_inventory' -or $assertions.provenance.path -cne $inputPath -or $assertions.provenance.identity -cne $identity){throw 'Bound assertion snapshot missing or mismatched.'}
        $assertionHash=Hash $assertionPath
    }
    $inventoryAdmissionEvidence.phase='functional_assertions_completed'
    $deadline.CheckWork();$code=0
}catch{$failure=$_.Exception.Message;$inventoryAdmissionEvidence.failure_phase=$inventoryAdmissionEvidence.phase;$code=1}
finally{
    try{foreach($name in $environment.Keys){Restore-Env $name $environment[$name]}}catch{$code=1;$failure+="; environment: $($_.Exception.Message)"}
    try{
        if($deadline){$deadline.CleanupChildren(5000);if(@($deadline.Children()).Count -ne 0){throw 'Owned children remain.'};$cleanup=$true}
        if($inventoryAdmissionEvidence.published){
            try{
                $afterInventory=Assert-KilogramS53WrittenInventory -Path $inputPath -Envelope $inventoryEnvelope
                if(-not $inputHash -or $afterInventory.Sha256 -cne $inputHash -or $afterInventory.ByteCount -ne $inventoryAdmissionRead.CandidateBytes){throw 'Actual pre-run inventory changed or publication was not fully verified.'}
                $inventoryAdmissionEvidence.inventory_validated_after_cleanup=$true
            }catch{$inventoryFinalErrors.Add('inventory: '+$_.Exception.Message)}
        }
        if($null -ne $inventoryAdmissionRead){
            try{
                if((Hash $bindingPath) -cne $bindingHash -or (Get-Item -LiteralPath $bindingPath).Length -ne $inventoryBindingRow.bytes){throw 'Binding changed during attempt.'}
                $null=Read-KilogramS53InventoryAdmission -Pointer $inventoryAdmissionPointer -Candidate $placeholderCandidate -Workspace $workspace -Case $Case -Attempt $Attempt -BindingRow $inventoryBindingRow -ExpectedArtifacts $binding.expected_artifacts -Observation $inventoryAdmissionEvidence
                $inventoryAdmissionEvidence.receipt_validated_after_cleanup=$true
            }catch{$inventoryFinalErrors.Add('admission: '+$_.Exception.Message)}
        }
        $inventoryAdmissionEvidence.final_validation_errors=$inventoryFinalErrors.ToArray()
        if($inventoryFinalErrors.Count){
            $code=1
            if($null -eq $inventoryAdmissionEvidence.failure_phase){$inventoryAdmissionEvidence.failure_phase='final_inventory_admission_validation'}
            $failure+='; inventory/admission final validation: '+($inventoryFinalErrors -join '; ')
        }
        if($Case -ceq 'F-s53-exhausted-integrated-01' -and (Hash $manifest) -cne $allocated.exhausted_manifest_sha256){throw 'Public replica prerequisite changed.'}
        Verify-Rows $binding.inputs;Verify-Rows $binding.images;Verify-Rows $predecessors
        $null=Get-KilogramS53ShellDependency -Executable $shell.path -Expected $shell -RequireCurrentHost
        $baselineAfter=Baseline
        if($baselineAfter.status -cne 'verified_match'){throw ('Baseline after is '+$baselineAfter.status+': '+$baselineAfter.error)}
        $verified=$inventoryFinalErrors.Count -eq 0
    }catch{$code=1;$failure+="; final validation: $($_.Exception.Message)"}
}
try{
    if(-not $cleanup -or -not $verified -or $deadline.WorkExpired -or $timer.Elapsed.TotalSeconds -ge $policy.work_seconds){$code=1;$failure+='; expired work or incomplete cleanup/verification'}
    $record=@{case=$Case;attempt=$Attempt;classification='whole-contributing-runner; independent actual outer return still required';exit_code=$code;error=$failure;elapsed_lower_bound_seconds=$timer.Elapsed.TotalSeconds;selector=$policy.selector;expected_count=1;counts=$counts;binding_sha256=$bindingHash;activation_sha256=$ActivationSha256;runtime_dependencies=@($shell);inventory_sha256=$inputHash;assertions_sha256=$assertionHash;wrapper_returned=$returned;wrapper_exit_code=$wrapperExit;baseline_before=$baselineBefore;baseline_after=$baselineAfter;sources_images_verified_after=$verified;owned_children_after_cleanup=if($cleanup){0}else{$null};cleanup_verified=$cleanup;work_seconds=$policy.work_seconds;whole_seconds=$policy.whole_seconds;run=$run;predecessor_actual_charges=$charged;inventory_admission=$inventoryAdmissionEvidence;rule='Root records actual outer return in a later distinct tool call, reconciles all time once, and preserves failed attempts.'}
    Write-NewJson $recordPath $record
    if($deadline.WorkExpired -or $timer.Elapsed.TotalSeconds -ge $policy.work_seconds){throw 'Deadline during terminal writes; raw record is not a whole PASS.'}
    $deadline.Finish()
}catch{
    $code=1
    Write-NewJson (Join-Path $evidence ($Attempt+'.terminal-failure.json')) @{case=$Case;attempt=$Attempt;error=$_.Exception.Message;whole_exit_code=1;elapsed_lower_bound_seconds=$timer.Elapsed.TotalSeconds;raw_case_record_is_not_whole_pass=$true}
}finally{$lock.Dispose();$timer.Stop()}
@{case=$Case;attempt=$Attempt;exit_code=$code;elapsed_lower_bound_seconds=$timer.Elapsed.TotalSeconds;counts=$counts;independent_outer_return_required=$true}|ConvertTo-Json -Depth 5
exit $code
