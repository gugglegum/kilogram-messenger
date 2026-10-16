[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$registryPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'

$expectedSurfaceBindingCounts = [ordered]@{
    'account-authority' = 9
    'account-recovery' = 13
    'authorized-event' = 3
    'blind-mailbox-capability' = 11
    'blind-mailbox-provider-gossip' = 5
    'blind-mailbox-transport' = 3
    'connection-ticket' = 8
    'desktop-bootstrap' = 1
    'device-session-authorization' = 2
    'endpoint-announcement' = 10
    'history-recovery' = 11
    'history-rewrap' = 12
    'mailbox-provisioning' = 20
    'publication-conflict' = 21
    'ratchet-public-artifacts' = 10
    'runtime-ipc' = 3
    'runtime-mailbox-envelope' = 8
    'sync' = 4
    'ticket-publication-capability' = 4
    'transport-sync' = 1
}

function Assert-ExactStringSet {
    param(
        [AllowEmptyCollection()][string[]]$Actual,
        [AllowEmptyCollection()][string[]]$Expected,
        [string]$Label
    )

    $actualSorted = @($Actual | Sort-Object)
    $expectedSorted = @($Expected | Sort-Object)
    if ($actualSorted.Count -ne $expectedSorted.Count) {
        throw "$Label count mismatch: expected $($expectedSorted.Count), got $($actualSorted.Count)"
    }
    for ($index = 0; $index -lt $expectedSorted.Count; $index++) {
        if ($actualSorted[$index] -cne $expectedSorted[$index]) {
            throw "$Label mismatch at index ${index}: expected '$($expectedSorted[$index])', got '$($actualSorted[$index])'"
        }
    }
}

function Assert-ExactPropertySet {
    param(
        [object]$Value,
        [string[]]$Expected,
        [string]$Label
    )

    if ($null -eq $Value) {
        throw "$Label is null"
    }
    $actual = @($Value.PSObject.Properties | ForEach-Object { $_.Name })
    Assert-ExactStringSet $actual $Expected "$Label properties"
}

function Assert-Rejected {
    param(
        [scriptblock]$Action,
        [string]$Label
    )

    try {
        & $Action
    }
    catch {
        Write-Output "$Label=rejected"
        return
    }
    throw "$Label was unexpectedly accepted"
}

function Get-RustDeclarationPattern {
    param(
        [object]$Binding
    )

    $symbol = [regex]::Escape([string]$Binding.symbol)
    $value = [regex]::Escape([string]$Binding.value)
    switch ([string]$Binding.rust_type) {
        'u8' {
            if ($Binding.value -isnot [byte] -and
                $Binding.value -isnot [int] -and
                $Binding.value -isnot [long]) {
                throw "u8 binding '$($Binding.symbol)' does not contain an integer value"
            }
            $numericValue = [long]$Binding.value
            if ($numericValue -lt 0 -or $numericValue -gt 255) {
                throw "u8 binding '$($Binding.symbol)' is outside the u8 range"
            }
            return "(?m)^\s*(?:pub\s+)?const\s+$symbol\s*:\s*u8\s*=\s*$numericValue\s*;"
        }
        'bytes' {
            if ($Binding.value -isnot [string] -or [string]::IsNullOrEmpty([string]$Binding.value)) {
                throw "bytes binding '$($Binding.symbol)' does not contain a non-empty string literal"
            }
            return "(?ms)^\s*(?:pub\s+)?const\s+$symbol\s*:\s*&\[u8\]\s*=\s*b`"$value`"\s*;"
        }
        'str' {
            if ($Binding.value -isnot [string] -or [string]::IsNullOrEmpty([string]$Binding.value)) {
                throw "str binding '$($Binding.symbol)' does not contain a non-empty string literal"
            }
            return "(?ms)^\s*(?:pub\s+)?const\s+$symbol\s*:\s*&str\s*=\s*`"$value`"\s*;"
        }
        default {
            throw "unsupported Rust binding type '$($Binding.rust_type)'"
        }
    }
}

function Assert-Registry {
    param(
        [object]$Registry,
        [string]$SourceRoot,
        [System.Collections.IDictionary]$ExpectedCounts,
        [bool]$RequireAllClasses = $true
    )

    Assert-ExactPropertySet $Registry @(
        'schema_version',
        'baseline',
        'stability',
        'scope',
        'changes_wire',
        'default_change_policy',
        'surfaces'
    ) 'registry'

    if ([long]$Registry.schema_version -ne 1) {
        throw 'protocol compatibility registry schema_version must be 1'
    }
    if ([string]$Registry.baseline -cne 'M1.1.0') {
        throw 'protocol compatibility registry baseline must be M1.1.0'
    }
    if ([string]$Registry.stability -cne 'experimental') {
        throw 'protocol compatibility registry must remain explicitly experimental'
    }
    if ([string]$Registry.scope -cne 'declared-cross-process-and-interoperability-surfaces') {
        throw 'protocol compatibility registry scope changed without a schema migration'
    }
    if ([bool]$Registry.changes_wire) {
        throw 'M1.1.0 compatibility baseline must not claim a wire change'
    }
    if ([string]$Registry.default_change_policy -cne 'explicit-version-or-identifier-migration') {
        throw 'protocol compatibility default change policy is not fail closed'
    }

    $surfaces = @($Registry.surfaces)
    $expectedIds = @($ExpectedCounts.Keys | ForEach-Object { [string]$_ })
    $surfaceIds = @($surfaces | ForEach-Object { [string]$_.id })
    Assert-ExactStringSet $surfaceIds $expectedIds 'protocol surface inventory'
    for ($index = 0; $index -lt $surfaceIds.Count; $index++) {
        if ($surfaceIds[$index] -cne $expectedIds[$index]) {
            throw 'protocol surfaces must be stored in stable ordinal ID order'
        }
    }

    $allowedClasses = @('local-control', 'network-wire', 'portable-artifact')
    $allowedEncodings = @('iroh-alpn', 'json', 'json-base64url', 'postcard', 'postcard-base64url-uri')
    $allowedCompatibility = @('exact-identifier', 'exact-version', 'explicit-reader-set')
    $allowedMigration = @('new-version-dual-reader', 'new-version-explicit-migration', 'parallel-identifier')
    $bindingKeys = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    $sourceRootFull = [System.IO.Path]::GetFullPath($SourceRoot)
    $sourceRootPrefix = $sourceRootFull.TrimEnd([char[]]@('\', '/')) + [System.IO.Path]::DirectorySeparatorChar
    $bindingCount = 0

    foreach ($surface in $surfaces) {
        Assert-ExactPropertySet $surface @(
            'id', 'class', 'encoding', 'compatibility', 'migration', 'bindings'
        ) "surface '$($surface.id)'"
        $surfaceId = [string]$surface.id
        if ($surfaceId -cnotmatch '^[a-z0-9]+(?:-[a-z0-9]+)*$') {
            throw "invalid protocol surface ID '$surfaceId'"
        }
        if ([string]$surface.class -cnotin $allowedClasses) {
            throw "unsupported protocol surface class '$($surface.class)'"
        }
        if ([string]$surface.encoding -cnotin $allowedEncodings) {
            throw "unsupported protocol encoding '$($surface.encoding)'"
        }
        if ([string]$surface.compatibility -cnotin $allowedCompatibility) {
            throw "unsupported compatibility policy '$($surface.compatibility)'"
        }
        if ([string]$surface.migration -cnotin $allowedMigration) {
            throw "unsupported migration policy '$($surface.migration)'"
        }
        if ([string]$surface.compatibility -ceq 'exact-identifier' -and
            [string]$surface.migration -cne 'parallel-identifier') {
            throw "exact identifier surface '$surfaceId' must migrate through a parallel identifier"
        }
        if ([string]$surface.compatibility -cne 'exact-identifier' -and
            [string]$surface.migration -ceq 'parallel-identifier') {
            throw "versioned surface '$surfaceId' cannot use parallel-identifier migration"
        }

        $bindings = @($surface.bindings)
        $expectedBindingCount = [int]$ExpectedCounts[$surfaceId]
        if ($bindings.Count -ne $expectedBindingCount) {
            throw "surface '$surfaceId' binding count mismatch: expected $expectedBindingCount, got $($bindings.Count)"
        }
        $orderedBindingKeys = @()
        foreach ($binding in $bindings) {
            Assert-ExactPropertySet $binding @('path', 'symbol', 'rust_type', 'value') "binding in '$surfaceId'"
            $relativePath = [string]$binding.path
            $symbolName = [string]$binding.symbol
            if ($relativePath -cnotmatch '^(?:apps|crates)/[A-Za-z0-9_./-]+\.rs$' -or
                $relativePath.Contains('..') -or
                $relativePath.Contains('\')) {
                throw "binding path is not a canonical repository Rust path: '$relativePath'"
            }
            if ($symbolName -cnotmatch '^[A-Z][A-Z0-9_]*$') {
                throw "binding symbol is not a constant identifier: '$symbolName'"
            }
            $bindingKey = "$relativePath#$symbolName"
            if (-not $bindingKeys.Add($bindingKey)) {
                throw "duplicate protocol source binding '$bindingKey'"
            }
            $orderedBindingKeys += $bindingKey

            $sourcePath = [System.IO.Path]::GetFullPath((Join-Path $sourceRootFull ($relativePath -replace '/', [System.IO.Path]::DirectorySeparatorChar)))
            if (-not $sourcePath.StartsWith($sourceRootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "protocol binding escaped the source root: '$relativePath'"
            }
            if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
                throw "protocol binding source is missing: '$relativePath'"
            }
            $source = Get-Content -LiteralPath $sourcePath -Raw
            $pattern = Get-RustDeclarationPattern $binding
            $matches = [regex]::Matches($source, $pattern)
            if ($matches.Count -ne 1) {
                throw "protocol binding '$bindingKey' expected one exact declaration, found $($matches.Count)"
            }
            $bindingCount++
        }
        $sortedBindingKeys = @($orderedBindingKeys | Sort-Object)
        for ($index = 0; $index -lt $orderedBindingKeys.Count; $index++) {
            if ($orderedBindingKeys[$index] -cne $sortedBindingKeys[$index]) {
                throw "surface '$surfaceId' bindings must be stored in stable ordinal path/symbol order"
            }
        }
    }

    if ($RequireAllClasses -and
        (@($surfaces | Where-Object { $_.class -ceq 'network-wire' }).Count -eq 0 -or
        @($surfaces | Where-Object { $_.class -ceq 'portable-artifact' }).Count -eq 0 -or
        @($surfaces | Where-Object { $_.class -ceq 'local-control' }).Count -eq 0)) {
        throw 'protocol registry must retain network, portable-artifact and local-control boundaries'
    }

    return $bindingCount
}

function Invoke-BoundarySelfTest {
    $temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('kilogram-protocol-compatibility-' + [guid]::NewGuid().ToString('N'))
    $sourceDirectory = Join-Path $temporaryRoot 'crates\demo\src'
    New-Item -ItemType Directory -Path $sourceDirectory -Force | Out-Null
    try {
        $sourcePath = Join-Path $sourceDirectory 'lib.rs'
        @(
            'const DEMO_VERSION: u8 = 1;'
            'const DEMO_DOMAIN: &[u8] = b"kilogram:demo:v1\0";'
        ) | Set-Content -LiteralPath $sourcePath -Encoding Ascii
        $registry = [pscustomobject]@{
            schema_version = 1
            baseline = 'M1.1.0'
            stability = 'experimental'
            scope = 'declared-cross-process-and-interoperability-surfaces'
            changes_wire = $false
            default_change_policy = 'explicit-version-or-identifier-migration'
            surfaces = @(
                [pscustomobject]@{
                    id = 'demo'
                    class = 'network-wire'
                    encoding = 'postcard'
                    compatibility = 'exact-version'
                    migration = 'new-version-dual-reader'
                    bindings = @(
                        [pscustomobject]@{
                            path = 'crates/demo/src/lib.rs'
                            symbol = 'DEMO_DOMAIN'
                            rust_type = 'bytes'
                            value = 'kilogram:demo:v1\0'
                        },
                        [pscustomobject]@{
                            path = 'crates/demo/src/lib.rs'
                            symbol = 'DEMO_VERSION'
                            rust_type = 'u8'
                            value = 1
                        }
                    )
                }
            )
        }
        $counts = [ordered]@{ 'demo' = 2 }
        if ((Assert-Registry $registry $temporaryRoot $counts $false) -ne 2) {
            throw 'synthetic protocol registry binding count was not preserved'
        }

        $changedValue = ConvertFrom-Json (ConvertTo-Json $registry -Depth 10)
        $changedValue.surfaces[0].bindings[1].value = 2
        Assert-Rejected { Assert-Registry $changedValue $temporaryRoot $counts $false | Out-Null } 'changed_source_value'

        $missingBinding = ConvertFrom-Json (ConvertTo-Json $registry -Depth 10)
        $missingBinding.surfaces[0].bindings = @($missingBinding.surfaces[0].bindings[0])
        Assert-Rejected { Assert-Registry $missingBinding $temporaryRoot $counts $false | Out-Null } 'removed_binding'

        $escapedPath = ConvertFrom-Json (ConvertTo-Json $registry -Depth 10)
        $escapedPath.surfaces[0].bindings[0].path = '../outside.rs'
        Assert-Rejected { Assert-Registry $escapedPath $temporaryRoot $counts $false | Out-Null } 'escaped_source_path'

        $unknownProperty = ConvertFrom-Json (ConvertTo-Json $registry -Depth 10)
        $unknownProperty.surfaces[0] | Add-Member -NotePropertyName allow_downgrade -NotePropertyValue $true
        Assert-Rejected { Assert-Registry $unknownProperty $temporaryRoot $counts $false | Out-Null } 'unknown_policy_property'

        $wireMutation = ConvertFrom-Json (ConvertTo-Json $registry -Depth 10)
        $wireMutation.changes_wire = $true
        Assert-Rejected { Assert-Registry $wireMutation $temporaryRoot $counts $false | Out-Null } 'undeclared_wire_change'
    }
    finally {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($SelfTest) {
    Invoke-BoundarySelfTest
    Write-Output 'protocol_compatibility_self_test=passed'
    Write-Output 'network_executed=false'
    Write-Output 'test_executables_started=false'
    Write-Output 'release_build=false'
    Write-Output 'archive_created=false'
    exit 0
}
Invoke-BoundarySelfTest | Out-Null

if (-not (Test-Path -LiteralPath $registryPath -PathType Leaf)) {
    throw "protocol compatibility registry is missing: $registryPath"
}
$registry = Get-Content -LiteralPath $registryPath -Raw -Encoding UTF8 | ConvertFrom-Json
$bindingCount = Assert-Registry $registry $workspace $expectedSurfaceBindingCounts
if ($bindingCount -ne 159) {
    throw "protocol source binding count must remain exactly 159, got $bindingCount"
}

$registrySha256 = (Get-FileHash -LiteralPath $registryPath -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Output 'protocol_compatibility_boundary=verified'
Write-Output 'protocol_compatibility_baseline=M1.1.0'
Write-Output 'protocol_stability=experimental'
Write-Output "protocol_surfaces=$(@($registry.surfaces).Count)"
Write-Output "protocol_source_bindings=$bindingCount"
Write-Output "protocol_registry_sha256=$registrySha256"
Write-Output 'wire_changed=false'
Write-Output 'downgrade_accepted=false'
Write-Output 'network_executed=false'
Write-Output 'test_executables_started=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
