# Read-only metadata capture. Shared five-second work budget, one-second cleanup,
# no shell fallback/retry, and no synchronous stream tail after the deadline.
Set-StrictMode -Version Latest
function Resolve-KilogramScalarExecutable([string]$Name) {
    $command=Get-Command $Name -CommandType Application -ErrorAction Stop|Select-Object -First 1
    if($null -eq $command -or $command.Source -isnot [string] -or [string]::IsNullOrWhiteSpace($command.Source)){throw 'Executable resolution is not one scalar Application path.'}
    if(-not [IO.Path]::IsPathRooted($command.Source)){throw 'Executable Application path is not absolute.'}
    $item=Get-Item -LiteralPath $command.Source -ErrorAction Stop
    if($item.PSIsContainer -or $item.Extension -ine '.exe'){throw 'Application path is not an existing executable.'}
    return [string]$item.FullName
}
function Invoke-KilogramMetadataCapture {
    param([Parameter(Mandatory)][string]$Executable,[Parameter(Mandatory)][string[]]$Arguments,[Parameter(Mandatory)][string]$Expected,[int]$BudgetMilliseconds=5000,[int]$CleanupMilliseconds=1000)
    if($BudgetMilliseconds -lt 1 -or $BudgetMilliseconds -gt 5000 -or $CleanupMilliseconds -lt 1 -or $CleanupMilliseconds -gt 1000){throw 'Metadata bounds.'}
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $result=[ordered]@{status='verification_unknown';executable_path=$Executable;revision=$null;exit_code=$null;stdout=$null;stderr=$null;stdout_state='not_started';stderr_state='not_started';timed_out=$false;process_started=$false;process_exited=$null;process_tree_kill_requested=$false;cleanup_verified=$false;timeout_milliseconds=$BudgetMilliseconds;cleanup_timeout_milliseconds=$CleanupMilliseconds;error=$null;elapsed_seconds=$null}
    $process=[Diagnostics.Process]::new();$outRead=$null;$errRead=$null
    try {
        if(-not [IO.Path]::IsPathRooted($Executable)){throw 'Metadata executable path is not absolute.'}
        $item=Get-Item -LiteralPath $Executable -ErrorAction Stop
        if($item.PSIsContainer -or $item.Extension -ine '.exe'){throw 'Metadata executable is not an existing EXE.'}
        $process.StartInfo.FileName=$item.FullName
        $process.StartInfo.UseShellExecute=$false;$process.StartInfo.CreateNoWindow=$true
        $process.StartInfo.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
        $process.StartInfo.RedirectStandardOutput=$true;$process.StartInfo.RedirectStandardError=$true
        foreach($argument in $Arguments){[void]$process.StartInfo.ArgumentList.Add($argument)}
        if(-not $process.Start()){throw 'Metadata process did not start.'}
        $result.process_started=$true
        $outRead=$process.StandardOutput.ReadToEndAsync();$errRead=$process.StandardError.ReadToEndAsync()
        $remaining=[Math]::Max(0,$BudgetMilliseconds-[int]$watch.ElapsedMilliseconds)
        if($remaining -eq 0 -or -not $process.WaitForExit($remaining)){$result.timed_out=$true;throw 'Metadata process exceeded its shared work budget.'}
        $result.process_exited=$true;$result.exit_code=$process.ExitCode
        $remaining=[Math]::Max(0,$BudgetMilliseconds-[int]$watch.ElapsedMilliseconds)
        $reads=[Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($outRead,$errRead))
        if($remaining -eq 0 -or -not $reads.Wait($remaining)){$result.timed_out=$true;throw 'Metadata stream capture exceeded its shared work budget.'}
        $result.stdout=$outRead.GetAwaiter().GetResult();$result.stderr=$errRead.GetAwaiter().GetResult()
        $result.stdout_state='captured';$result.stderr_state='captured'
        if($result.stdout.Length -gt 65536 -or $result.stderr.Length -gt 65536){throw 'Metadata output exceeds64KiB cap.'}
        if($result.exit_code -ne 0){throw 'Metadata process returned a nonzero exit.'}
        if([string]::IsNullOrWhiteSpace($result.stdout) -or $result.stdout.Trim() -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$'){throw 'Metadata output is not one complete revision.'}
        $result.revision=$result.stdout.Trim()
        $result.status=if($result.revision -ceq $Expected){'verified_match'}else{'verified_changed'}
    } catch {$result.error=$_.Exception.Message;$result.status='verification_unknown'}
    finally {
        $cleanup=[Diagnostics.Stopwatch]::StartNew()
        try {
            if($result.process_started){
                if(-not $process.HasExited){$result.process_tree_kill_requested=$true;$process.Kill($true)}
                $remaining=[Math]::Max(0,$CleanupMilliseconds-[int]$cleanup.ElapsedMilliseconds)
                $exited=$process.HasExited -or ($remaining -gt 0 -and $process.WaitForExit($remaining))
                $result.process_exited=$exited
                if(-not $exited){throw 'Metadata child cleanup deadline.'}
                # Do not call WaitForExit() or ReadToEnd() here. A descendant
                # can retain a pipe after its parent exits; that stays unknown.
                if($null -ne $outRead -and $null -ne $errRead){
                    $remaining=[Math]::Max(0,$CleanupMilliseconds-[int]$cleanup.ElapsedMilliseconds)
                    $cleanupReads=[Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($outRead,$errRead))
                    if(-not $cleanupReads.IsCompleted -and $remaining -gt 0){$null=$cleanupReads.Wait($remaining)}
                }
                $result.cleanup_verified=$exited -and $null -ne $outRead -and $null -ne $errRead -and $outRead.IsCompletedSuccessfully -and $errRead.IsCompletedSuccessfully
                if(-not $result.cleanup_verified){$result.status='verification_unknown'}
            }
        } catch {$result.status='verification_unknown';$result.error=(@($result.error,$_.Exception.Message)|Where-Object {$_}) -join '; '}
        if($result.stdout -and $result.stdout.Length -gt 65536){$result.stdout=$result.stdout.Substring(0,65536);$result.stdout_state='over_cap'}
        if($result.stderr -and $result.stderr.Length -gt 65536){$result.stderr=$result.stderr.Substring(0,65536);$result.stderr_state='over_cap'}
        $process.Dispose();$cleanup.Stop();$watch.Stop();$result.elapsed_seconds=$watch.Elapsed.TotalSeconds
    }
    return [pscustomobject]$result
}
function Read-KilogramS53Baseline([string]$Workspace,[string]$Expected) {
    $watch=[Diagnostics.Stopwatch]::StartNew()
    try {$executable=Resolve-KilogramScalarExecutable 'git'}
    catch {return [pscustomobject]@{status='verification_unknown';executable_path=$null;revision=$null;error=$_.Exception.Message;cleanup_verified=$false;process_started=$false}}
    $remaining=5000-[int]$watch.ElapsedMilliseconds
    if($remaining -le 0){return [pscustomobject]@{status='verification_unknown';executable_path=$executable;revision=$null;error='Executable resolution exceeded shared metadata budget.';cleanup_verified=$false;process_started=$false}}
    $result=Invoke-KilogramMetadataCapture -Executable $executable -Arguments @('-C',$Workspace,'rev-parse','HEAD') -Expected $Expected -BudgetMilliseconds $remaining
    $watch.Stop();$result.elapsed_seconds=$watch.Elapsed.TotalSeconds
    return $result
}

function Get-KilogramS53BindingName([string]$Case) {
    if($Case -ceq 'F-s53-capacity-recovery-r2-01'){return 'G2-capacity-route-02.binding.candidate.json'}
    if($Case -ceq 'F-s53-exhausted-integrated-01'){return 'C-s53-g1-continuation-C2-01.binding.candidate.json'}
    return 'S5.3-integration-binding-r30.json'
}
function Assert-KilogramS53G2Policy([string]$Case,$Binding) {
    $g2='F-s53-capacity-recovery-r2-01'
    $kind='g2-only-current-capacity-route-candidate'
    if($Case -cne $g2){
        if($Binding.classification -ceq $kind){throw 'G2 binding cannot authorize another case.'}
        return
    }
    if($Binding.classification -cne $kind -or $Binding.baseline -cne '64c2359788ead360314793e47741adacb83b8d4f' -or
        $Binding.source_head -isnot [string] -or $Binding.source_head -cnotmatch '^[0-9a-f]{40}$' -or
        $Binding.cases -isnot [Array] -or $Binding.cases.Count -ne 1){throw 'Exact G2 current source and single-case binding required.'}
    $policy=$Binding.cases[0]
    if($policy.case -cne $g2 -or $policy.selector -cne 'live_l1::handoff::capacity_review2::s53_capacity_recovery_preserves_validated_route_public' -or
        $policy.expected_count -ne 1 -or $policy.build_profile -cne 'verification-scale' -or
        $policy.image -cne 'target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe' -or
        $policy.controller_seconds -ne 240 -or $policy.work_seconds -ne 275 -or $policy.whole_seconds -ne 300 -or
        $Binding.expected_artifacts.sha256 -isnot [string] -or $Binding.expected_artifacts.sha256 -cnotmatch '^[0-9a-f]{64}$'){
        throw 'Exact G2 selector, frozen scale artifacts and finite case policy required.'
    }
}
function Read-KilogramS53CaseBaseline([string]$Workspace,[string]$Case,$Binding) {
    Assert-KilogramS53G2Policy $Case $Binding
    $expected='64c2359788ead360314793e47741adacb83b8d4f'
    if($Case -cin @('F-s53-exhausted-integrated-01','F-s53-capacity-recovery-r2-01')){
        if($Binding.source_head -isnot [string] -or $Binding.source_head -cnotmatch '^[0-9a-f]{40}$'){throw 'Exact current case source HEAD required.'}
        $expected=$Binding.source_head
    }
    Read-KilogramS53Baseline -Workspace $Workspace -Expected $expected
}
function Get-KilogramS53ImageRoles([string]$Case) {
    @(
        'target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe',
        'target/compatibility/long-lived-dialog-migration/kilogram-cli.exe',
        'target/verification-scale/deps/kilogram-cli-bin-kilogram-cli-tests.exe',
        'target/verification-scale/deps/kilogram-cli-lib-kilogram_cli-tests.exe',
        'target/verification-scale/deps/kilogram-cli-test-migration_s5_process-tests.exe',
        'target/verification-scale/deps/kilogram-cli-test-runtime2_process-tests.exe',
        'target/verification-scale/deps/kilogram-transport-iroh-lib-kilogram_transport_iroh-tests.exe',
        'target/verification-scale/deps/kilogram-transport-iroh-test-local_relay_transport-tests.exe',
        'target/x86_64-pc-windows-msvc/verification-scale/kilogram-cli.exe'
    )
    if($Case -ceq 'F-s53-capacity-recovery-r2-01'){'target/compatibility/long-lived-dialog-runtime/kilogram-relay.exe'}
}
function Assert-KilogramS53ImageRoles([string]$Case,$Images) {
    $roles=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($path in @(Get-KilogramS53ImageRoles $Case)){[void]$roles.Add($path)}
    if($Images -isnot [Array] -or $Images.Count -ne $roles.Count){throw 'Exact case image role count required.'}
    foreach($row in $Images){
        if($row.path -isnot [string] -or -not $roles.Remove($row.path)){throw 'Unknown or duplicate case image role.'}
    }
    if($roles.Count -ne 0){throw 'Missing case image role.'}
}

# r29a inventory-envelope helpers BEGIN
function Get-KilogramS53InventoryByteLimit {
    [CmdletBinding()]
    param()
    return [long]2097152
}

function New-KilogramS53InventoryEnvelope {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]$Case,
        [Parameter(Mandatory)]$Attempt,
        [Parameter(Mandatory)]$Workspace,
        [Parameter(Mandatory)]$Binding,
        [Parameter(Mandatory)]$BindingHash,
        [Parameter(Mandatory)]$ActivationSha256,
        [Parameter(Mandatory)]$Shell,
        [Parameter(Mandatory)][AllowEmptyCollection()]$Predecessors,
        [Parameter(Mandatory)][AllowEmptyCollection()]$Prerequisites
    )
    $ErrorActionPreference = 'Stop'
    # Pure construction: no file verification, writes, processes or environment.
    $members = {
        param($Value, [string]$Label, $Fields)
        $map = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
        if ($Value -is [Collections.IDictionary]) {
            foreach ($key in $Value.Keys) {
                if ($key -isnot [string] -or -not $map.TryAdd($key, $Value[$key])) { throw "$Label has invalid or duplicate fields." }
            }
        } elseif ($Value -is [pscustomobject]) {
            foreach ($property in $Value.PSObject.Properties) {
                if ($property -isnot [Management.Automation.PSNoteProperty] -or -not $map.TryAdd($property.Name, $property.Value)) { throw "$Label must contain only data fields." }
            }
        } else { throw "$Label must be an object." }
        if ($null -ne $Fields) {
            if ($map.Count -ne $Fields.Count) { throw "$Label has missing or unknown fields." }
            foreach ($field in $Fields) {
                if (-not $map.ContainsKey($field)) { throw "$Label has missing or unknown fields." }
            }
        }
        return ,$map
    }
    $integralLength = {
        param($Value)
        return (($Value -is [byte] -or $Value -is [sbyte] -or $Value -is [short] -or $Value -is [ushort] -or
                 $Value -is [int] -or $Value -is [uint] -or $Value -is [long] -or $Value -is [ulong]) -and
                $Value -ge 0 -and $Value -le [long]::MaxValue)
    }
    if ($Case -isnot [string] -or $Case -cnotmatch '\A[CF]-s53-[a-z0-9-]+-01\z') { throw 'Invalid inventory case.' }
    if ($Attempt -isnot [string] -or $Attempt -cnotmatch ('\A' + [regex]::Escape($Case) + '-attempt-[0-9]{2}\z')) { throw 'Invalid inventory attempt.' }
    foreach ($digest in @($BindingHash, $ActivationSha256)) {
        if ($digest -isnot [string] -or $digest -cnotmatch '\A[0-9a-f]{64}\z') { throw 'Invalid inventory context digest.' }
    }
    if ($Workspace -isnot [string] -or -not [IO.Path]::IsPathFullyQualified($Workspace) -or $Workspace -cnotmatch '\A[A-Za-z]:[\\/]') { throw 'Inventory workspace must be a fully qualified local path.' }
    $workspaceFull = [IO.Path]::GetFullPath($Workspace).TrimEnd([char[]]@('\', '/'))
    if (-not [string]::Equals($workspaceFull, $Workspace.TrimEnd([char[]]@('\', '/')), [StringComparison]::OrdinalIgnoreCase) -or
        $workspaceFull -ceq [IO.Path]::GetPathRoot($Workspace).TrimEnd([char[]]@('\', '/'))) { throw 'Inventory workspace is not a normalized owned directory.' }
    $workspacePrefix = $workspaceFull + [IO.Path]::DirectorySeparatorChar
    $bound = & $members $Binding 'Binding' $null
    if($Case -ceq 'F-s53-capacity-recovery-r2-01' -or ($bound.ContainsKey('classification') -and $bound['classification'] -ceq 'g2-only-current-capacity-route-candidate')){
        Assert-KilogramS53G2Policy $Case $Binding
    }
    foreach ($field in @('baseline', 'inputs', 'images', 'runtime_dependencies', 'cases', 'code_source_count')) {
        if (-not $bound.ContainsKey($field)) { throw "Binding lacks $field." }
    }
    if ($bound['baseline'] -isnot [string] -or $bound['baseline'] -cne '64c2359788ead360314793e47741adacb83b8d4f') { throw 'Inventory baseline differs from Managed.' }
    foreach ($field in @('inputs', 'images', 'runtime_dependencies', 'cases')) {
        if ($bound[$field] -isnot [Array] -or $bound[$field].Count -eq 0) { throw "Binding $field must be a nonempty array." }
    }
    if (-not (& $integralLength $bound['code_source_count']) -or $bound['code_source_count'] -lt 1 -or $bound['code_source_count'] -gt $bound['inputs'].Count) { throw 'Invalid canonical source count.' }
    if ($Predecessors -isnot [Array] -or $Prerequisites -isnot [Array]) { throw 'Proof and prerequisite rows must be arrays.' }
    $matchingCases = 0
    foreach ($entry in $bound['cases']) {
        $policy = & $members $entry 'Case policy' $null
        if (-not $policy.ContainsKey('case') -or $policy['case'] -isnot [string] -or $policy['case'] -cnotmatch '\A[CF]-s53-[a-z0-9-]+-01\z') { throw 'Malformed bound case policy.' }
        if ($policy['case'] -ceq $Case) { $matchingCases++ }
    }
    if ($matchingCases -ne 1) { throw 'Case is not uniquely present in binding.' }
    $pathKey = {
        param($Value, [bool]$OutsideWorkspace)
        if ($Value -isnot [string] -or [string]::IsNullOrWhiteSpace($Value) -or $Value.IndexOfAny([IO.Path]::GetInvalidPathChars()) -ge 0) { throw 'Invalid inventory row path.' }
        $absolute = [IO.Path]::IsPathFullyQualified($Value)
        if (([IO.Path]::IsPathRooted($Value) -and -not $absolute) -or
            ($absolute -and $Value -cnotmatch '\A[A-Za-z]:[\\/]')) { throw 'Ambiguous or non-local inventory row path.' }
        $componentText = if ($absolute) { $Value.Substring([IO.Path]::GetPathRoot($Value).Length) } else { $Value }
        foreach ($component in $componentText.Replace('\', '/').Split('/')) {
            if ([string]::IsNullOrEmpty($component) -or $component -cin @('.', '..') -or
                $component.EndsWith(' ') -or $component.EndsWith('.') -or
                $component.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0 -or
                $component -imatch '\A(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|\z)') { throw 'Noncanonical Windows inventory path component.' }
        }
        $full = [IO.Path]::GetFullPath($Value, $workspaceFull)
        if (-not $OutsideWorkspace -and -not $full.StartsWith($workspacePrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Inventory row escapes workspace.' }
        return $full
    }
    $null = & $pathKey $workspaceFull $true
    $allPaths = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::OrdinalIgnoreCase)
    $rowValue = {
        param($Value, [string]$Kind)
        $fields = if ($Kind -ceq 'shell') { @('role', 'path', 'bytes', 'sha256') } elseif ($Kind -ceq 'prerequisite') { @('path', 'bytes', 'sha256', 'classification') } else { @('path', 'bytes', 'sha256') }
        $map = & $members $Value "$Kind row" $fields
        if (-not (& $integralLength $map['bytes']) -or $map['sha256'] -isnot [string] -or $map['sha256'] -cnotmatch '\A[0-9a-f]{64}\z') { throw "Invalid $Kind byte length or SHA256." }
        $full = & $pathKey $map['path'] ($Kind -ceq 'shell')
        if ($Kind -ceq 'shell' -and ($map['role'] -isnot [string] -or $map['role'] -cne 'metadata-shell' -or -not [IO.Path]::IsPathFullyQualified($map['path']))) { throw 'One absolute metadata-shell role is required.' }
        if ($Kind -ceq 'prerequisite' -and ($map['classification'] -isnot [string] -or $map['classification'] -cne 'root-allocated public replica manifest; no preparation performed by adapter')) { throw 'Invalid prerequisite role.' }
        if ($allPaths.ContainsKey($full)) {
            $prior = $allPaths[$full]
            if ($prior['path'] -cne $map['path']) { throw 'Ambiguous Windows path aliases in inventory.' }
            if ($prior['bytes'] -ne $map['bytes'] -or $prior['sha256'] -cne $map['sha256']) { throw 'Conflicting inventory versions.' }
        } else { $allPaths.Add($full, $map) }
        $row = [ordered]@{}
        if ($Kind -ceq 'shell') { $row['role'] = $map['role'] }
        $row['path'] = $map['path']; $row['bytes'] = $map['bytes']; $row['sha256'] = $map['sha256']
        if ($Kind -ceq 'prerequisite') { $row['classification'] = $map['classification'] }
        return [pscustomobject]@{ FullPath = $full; Row = $row }
    }
    $sources = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach ($entry in @($bound['inputs']) + @($Predecessors)) {
        $checked = & $rowValue $entry 'source'
        if (-not $sources.ContainsKey($checked.Row['path'])) { $sources.Add($checked.Row['path'], $checked.Row) }
        # rowValue already rejected conflicting versions and literal aliases.
    }
    if ($sources.Count -lt $bound['code_source_count']) { throw 'Incomplete source coverage.' }
    [string[]]$sourcePaths = @($sources.Keys)
    [Array]::Sort($sourcePaths, [StringComparer]::Ordinal)
    $sourceRows = [Collections.Generic.List[object]]::new()
    foreach ($path in $sourcePaths) { $sourceRows.Add($sources[$path]) }
    $requiredImages = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in @(Get-KilogramS53ImageRoles $Case)) { [void]$requiredImages.Add($path) }
    if ($bound['images'].Count -ne $requiredImages.Count) { throw 'Exact case image roles must be present.' }
    $imageRows = [Collections.Generic.List[object]]::new()
    foreach ($entry in $bound['images']) {
        $checked = & $rowValue $entry 'image'
        if (-not $requiredImages.Remove($checked.Row['path'])) { throw 'Unknown or duplicate image role.' }
        $imageRows.Add($checked.Row)
    }
    if ($requiredImages.Count -ne 0 -or $bound['runtime_dependencies'].Count -ne 1) { throw 'Incomplete image or runtime roles.' }
    $expectedShell = & $rowValue $bound['runtime_dependencies'][0] 'shell'
    $actualShell = & $rowValue $Shell 'shell'
    foreach ($field in @('role', 'path', 'bytes', 'sha256')) {
        if ($actualShell.Row[$field] -cne $expectedShell.Row[$field]) { throw 'Reviewed shell tuple mismatch.' }
    }
    $prerequisiteRows = [Collections.Generic.List[object]]::new()
    $prerequisitePaths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in $Prerequisites) {
        $checked = & $rowValue $entry 'prerequisite'
        if (-not $prerequisitePaths.Add($checked.FullPath)) { throw 'Duplicate prerequisite role.' }
        $prerequisiteRows.Add($checked.Row)
    }
    $requiredPrerequisites = if ($Case -ceq 'F-s53-exhausted-integrated-01') { 1 } else { 0 }
    if ($prerequisiteRows.Count -ne $requiredPrerequisites) { throw 'Missing or unexpected prerequisite role.' }
    $run = [IO.Path]::Combine($workspaceFull, '.tmp', 'migration-s53-integrated', $Attempt)
    $inventoryPath = [IO.Path]::Combine($workspaceFull, 'ai-docs', 'tasks', 'long-lived-dialog-a3-migration-s5-handoff-evidence', $Attempt + '.inputs.json')
    $value = [ordered]@{
        case = $Case
        attempt = $Attempt
        classification = 'bound_pre_run_inventory'
        inventory_identity = 'S5.3/integrated/' + $Attempt + '/' + $BindingHash
        baseline = $bound['baseline']
        run = $run
        binding_sha256 = $BindingHash
        activation_sha256 = $ActivationSha256
        sources = $sourceRows.ToArray()
        images = $imageRows.ToArray()
        runtime_dependencies = @($actualShell.Row)
        prerequisites = $prerequisiteRows.ToArray()
        execution_mode_declared = 'NativeToken'
        token_introspection = $false
    }
    $raw = [Text.UTF8Encoding]::new($false, $true).GetBytes((ConvertTo-Json -InputObject $value -Depth 30 -WarningAction Stop -ErrorAction Stop))
    $limit = Get-KilogramS53InventoryByteLimit
    if ($raw.LongLength -gt $limit) { throw "Inventory exceeds fixed $limit-byte bound: $($raw.LongLength)." }
    return [pscustomobject][ordered]@{
        Value = $value
        Bytes = $raw
        ByteCount = $raw.LongLength
        MaximumBytes = $limit
        Workspace = $workspaceFull
        InventoryPath = $inventoryPath
    }
}

function Assert-KilogramS53WrittenInventory {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]$Path,
        [Parameter(Mandatory)]$Envelope
    )
    $ErrorActionPreference = 'Stop'
    $limit = Get-KilogramS53InventoryByteLimit
    if ($Envelope -isnot [pscustomobject] -or @($Envelope.PSObject.Properties).Count -ne 6) { throw 'Prepared inventory envelope required.' }
    foreach ($name in @('Value', 'Bytes', 'ByteCount', 'MaximumBytes', 'Workspace', 'InventoryPath')) {
        if ($null -eq $Envelope.PSObject.Properties[$name] -or $Envelope.PSObject.Properties[$name] -isnot [Management.Automation.PSNoteProperty]) { throw 'Prepared envelope fields differ.' }
    }
    if ($Envelope.Bytes -isnot [byte[]] -or $Envelope.ByteCount -isnot [long] -or $Envelope.MaximumBytes -isnot [long] -or
        $Envelope.ByteCount -ne $Envelope.Bytes.LongLength -or $Envelope.ByteCount -le 0 -or
        $Envelope.ByteCount -gt $limit -or $Envelope.MaximumBytes -ne $limit) { throw 'Invalid prepared byte buffer or fixed bound.' }
    if ($Envelope.Workspace -isnot [string] -or -not [IO.Path]::IsPathFullyQualified($Envelope.Workspace) -or
        $Envelope.Workspace -cnotmatch '\A[A-Za-z]:[\\/]' -or
        $Envelope.Workspace -cne [IO.Path]::GetFullPath($Envelope.Workspace).TrimEnd([char[]]@('\', '/'))) { throw 'Invalid prepared workspace.' }
    if ($Envelope.Value -isnot [Collections.IDictionary] -or $Envelope.Value.Count -ne 14) { throw 'Prepared inventory value must retain fourteen fields.' }
    $value = $Envelope.Value
    $valueFields = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in @('case', 'attempt', 'classification', 'inventory_identity', 'baseline', 'run', 'binding_sha256', 'activation_sha256', 'sources', 'images', 'runtime_dependencies', 'prerequisites', 'execution_mode_declared', 'token_introspection')) { [void]$valueFields.Add($name) }
    foreach ($name in $value.Keys) {
        if ($name -isnot [string] -or -not $valueFields.Remove($name)) { throw 'Prepared inventory value fields differ.' }
    }
    if ($valueFields.Count -ne 0) { throw 'Prepared inventory value fields differ.' }
    foreach ($name in @('classification', 'inventory_identity', 'baseline', 'run', 'execution_mode_declared')) {
        if ($value[$name] -isnot [string]) { throw 'Prepared inventory context must retain string types.' }
    }
    if ($value['sources'] -isnot [Array] -or $value['sources'].Count -eq 0 -or
        $value['images'] -isnot [Array] -or
        $value['runtime_dependencies'] -isnot [Array] -or $value['runtime_dependencies'].Count -ne 1 -or
        $value['prerequisites'] -isnot [Array]) { throw 'Prepared inventory arrays differ.' }
    if ($value['case'] -isnot [string] -or $value['case'] -cnotmatch '\A[CF]-s53-[a-z0-9-]+-01\z' -or
        $value['attempt'] -isnot [string] -or $value['attempt'] -cnotmatch ('\A' + [regex]::Escape($value['case']) + '-attempt-[0-9]{2}\z') -or
        $value['binding_sha256'] -isnot [string] -or $value['binding_sha256'] -cnotmatch '\A[0-9a-f]{64}\z' -or
        $value['activation_sha256'] -isnot [string] -or $value['activation_sha256'] -cnotmatch '\A[0-9a-f]{64}\z') { throw 'Invalid prepared inventory context.' }
    Assert-KilogramS53ImageRoles $value['case'] $value['images']
    if($value['case'] -ceq 'F-s53-capacity-recovery-r2-01' -and $value['prerequisites'].Count -ne 0){throw 'G2 requires zero prerequisites.'}
    $run = [IO.Path]::Combine($Envelope.Workspace, '.tmp', 'migration-s53-integrated', $value['attempt'])
    $ownedPath = [IO.Path]::Combine($Envelope.Workspace, 'ai-docs', 'tasks', 'long-lived-dialog-a3-migration-s5-handoff-evidence', $value['attempt'] + '.inputs.json')
    if ($Envelope.InventoryPath -isnot [string] -or $Envelope.InventoryPath -cne $ownedPath -or
        $value['run'] -cne $run -or $value['classification'] -cne 'bound_pre_run_inventory' -or
        $value['baseline'] -cne '64c2359788ead360314793e47741adacb83b8d4f' -or
        $value['inventory_identity'] -cne ('S5.3/integrated/' + $value['attempt'] + '/' + $value['binding_sha256']) -or
        $value['execution_mode_declared'] -cne 'NativeToken' -or $value['token_introspection'] -isnot [bool] -or $value['token_introspection']) { throw 'Prepared inventory identity or owned path differs.' }
    if ($Path -isnot [string] -or -not [IO.Path]::IsPathFullyQualified($Path)) { throw 'Written inventory must have a fully qualified owned path.' }
    $full = [IO.Path]::GetFullPath($Path)
    if (-not [string]::Equals($Path, $full, [StringComparison]::OrdinalIgnoreCase) -or
        (-not [string]::Equals($full, $ownedPath, [StringComparison]::OrdinalIgnoreCase) -and
         -not [string]::Equals($full, $ownedPath + '.pending', [StringComparison]::OrdinalIgnoreCase))) { throw 'Written inventory is not the exact owned final or pending path.' }
    $encoded = [Text.UTF8Encoding]::new($false, $true).GetBytes((ConvertTo-Json -InputObject $value -Depth 30 -WarningAction Stop -ErrorAction Stop))
    if (-not [Linq.Enumerable]::SequenceEqual[byte]($encoded, $Envelope.Bytes) -or
        ($encoded.Length -ge 3 -and $encoded[0] -eq 239 -and $encoded[1] -eq 187 -and $encoded[2] -eq 191)) { throw 'Prepared bytes no longer equal their exact UTF-8 value.' }
    $current = $full
    while ($true) {
        $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or ($current -ceq $full -and $item.PSIsContainer)) { throw 'Written inventory ownership crosses a reparse point or directory.' }
        if ([string]::Equals($current, $Envelope.Workspace, [StringComparison]::OrdinalIgnoreCase)) { break }
        $current = [IO.Path]::GetDirectoryName($current)
        if ([string]::IsNullOrEmpty($current) -or
            (-not [string]::Equals($current, $Envelope.Workspace, [StringComparison]::OrdinalIgnoreCase) -and
             -not $current.StartsWith($Envelope.Workspace + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) { throw 'Written inventory ownership escaped workspace.' }
    }
    $file = $null; $memory = $null
    try {
        $file = [IO.File]::Open($full, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ($file.Length -gt $limit) { throw 'Written inventory handle exceeds fixed bound.' }
        $memory = [IO.MemoryStream]::new()
        $buffer = [byte[]]::new(8192)
        [long]$total = 0
        while ($total -lt $limit + 1) {
            $count = $file.Read($buffer, 0, [int][Math]::Min([long]$buffer.Length, $limit + 1 - $total))
            if ($count -eq 0) { break }
            $memory.Write($buffer, 0, $count)
            $total += $count
        }
        if ($total -gt $limit) { throw 'Written inventory grew beyond fixed bound.' }
        $actual = $memory.ToArray()
        if ($actual.LongLength -ne $Envelope.ByteCount -or
            -not [Linq.Enumerable]::SequenceEqual[byte]($actual, $Envelope.Bytes)) { throw 'Written inventory bytes differ from prepared bytes.' }
        return [pscustomobject][ordered]@{
            Path = $full
            ByteCount = $actual.LongLength
            Sha256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($actual)).ToLowerInvariant()
            MaximumBytes = $limit
        }
    } finally {
        if ($null -ne $memory) { $memory.Dispose() }
        if ($null -ne $file) { $file.Dispose() }
    }
}
# r29a inventory-envelope helpers END
