[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$accountVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_account_recovery_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activeAccountPath = Join-Path $workspace 'crates\kilogram-identity\src\account.rs'
$activeQuorumPath = Join-Path $workspace 'crates\kilogram-identity\src\recovery_quorum.rs'
$activePolicyPath = Join-Path $workspace 'crates\kilogram-identity\src\recovery_policy.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-account-recovery-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0152-m1126-canonical-account-recovery-quorum.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Assert-LowerHex {
    param([string]$Value, [int]$Bytes = -1, [string]$Label)
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -cnotmatch '^[0-9a-f]+$' -or
        $Value.Length % 2 -ne 0 -or ($Bytes -ge 0 -and $Value.Length -ne $Bytes * 2)) {
        throw "$Label is not canonical lowercase hexadecimal"
    }
}

function Assert-Schema {
    param([object]$Schema, [string]$Rust)
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.26' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-ACCOUNT-RECOVERY.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-account-recovery-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v1' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical Account recovery schema header changed'
    }
    $limits = @(
        [int]$Schema.limits.maximum_memberships,
        [int]$Schema.limits.maximum_nested_cbor_bytes,
        [int]$Schema.limits.maximum_package_bytes,
        [int]$Schema.limits.maximum_witness_bytes,
        [int]$Schema.limits.maximum_request_bytes,
        [int]$Schema.limits.maximum_approval_bytes,
        [int]$Schema.limits.maximum_approvals,
        [int]$Schema.limits.maximum_validity_seconds,
        [int]$Schema.limits.clock_skew_seconds
    ) -join ','
    if ($limits -cne '16384,524288,33554432,4096,33624064,16384,32,1800,120') {
        throw 'canonical Account recovery limits changed'
    }
    if (@($Schema.objects).Count -ne 4 -or @($Schema.domains).Count -ne 8) {
        throw 'canonical Account recovery object or domain inventory changed'
    }
    foreach ($required in @(
        'MAX_CANONICAL_ACCOUNT_RECOVERY_MEMBERSHIPS: usize = 16 * 1024',
        'MAX_CANONICAL_ACCOUNT_RECOVERY_VALIDITY_SECONDS: u64 = 1_800',
        'kilogram:account-root-recovery-package-signature:cbor-v1\0',
        'kilogram:account-root-recovery-approval-signature:cbor-v1\0',
        'decode_and_verify_signed_account_recovery_package_v1',
        'decode_and_verify_signed_account_recovery_witness_v1',
        'decode_and_verify_account_recovery_approval_request_v1',
        'decode_and_verify_signed_account_recovery_approval_v1',
        'verify_account_recovery_approval_for_request_v1',
        'verify_account_recovery_quorum_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical Account recovery Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical Account recovery vector header changed'
    }
    Assert-LowerHex ([string]$Vectors.expected.account_id_hex) 32 'Account ID'
    Assert-LowerHex ([string]$Vectors.expected.approver_device_id_hex) 32 'approver Device ID'
    foreach ($entry in @($Vectors.expected.package, $Vectors.expected.witness,
        $Vectors.expected.approval)) {
        Assert-LowerHex ([string]$entry.content_cbor_hex) -Label 'signed content'
        Assert-LowerHex ([string]$entry.signing_preimage_hex) -Label 'signing preimage'
        Assert-LowerHex ([string]$entry.signature_hex) 64 'signature'
        Assert-LowerHex ([string]$entry.signed_cbor_hex) -Label 'signed CBOR'
    }
    foreach ($entry in @($Vectors.expected.package, $Vectors.expected.request,
        $Vectors.expected.approval)) {
        Assert-LowerHex ([string]$entry.id_preimage_hex) -Label 'ID preimage'
        Assert-LowerHex ([string]$entry.object_id_hex) 32 'object ID'
    }
    foreach ($entry in @($Vectors.expected.state_vector, $Vectors.expected.roster)) {
        Assert-LowerHex ([string]$entry.input_cbor_hex) -Label 'digest input'
        Assert-LowerHex ([string]$entry.preimage_hex) -Label 'digest preimage'
        Assert-LowerHex ([string]$entry.digest_hex) 32 'digest'
    }
    if ([UInt64]$Vectors.input.expires_at_unix_seconds -
        [UInt64]$Vectors.input.issued_at_unix_seconds -ne 600 -or
        [UInt64]$Vectors.input.approved_at_unix_seconds -ne 1800000005 -or
        [string]$Vectors.expected.quorum.claim -cne 'current-device-majority-observed' -or
        [int]$Vectors.expected.quorum.required_approvals -ne 1) {
        throw 'canonical Account recovery vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $authorityVectorsPath, $accountVectorsPath,
    $cddlPath, $rustPath, $libPath, $activeAccountPath, $activeQuorumPath, $activePolicyPath,
    $compatibilityPath, $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical Account recovery file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$activeAccount = Get-Content -LiteralPath $activeAccountPath -Raw
$activeQuorum = Get-Content -LiteralPath $activeQuorumPath -Raw
$activePolicy = Get-Content -LiteralPath $activePolicyPath -Raw
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('signed-account-recovery-package-v1',
    'signed-account-recovery-witness-v1', 'account-recovery-approval-request-v1',
    'signed-account-recovery-approval-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "Account recovery CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_account_recovery_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical Account recovery module is not exported'
}
foreach ($pair in @(
    @($activeAccount, 'kilogram:account-root-recovery-package-signature:v1\0'),
    @($activeQuorum, 'kilogram:account-root-recovery-approval:v1\0'),
    @($activePolicy, 'kilogram:account-recovery-policy-transition-approval:v1\0'))) {
    if ($pair[0].IndexOf($pair[1], [StringComparison]::Ordinal) -lt 0) {
        throw "active Account recovery contract changed: missing '$($pair[1])'"
    }
    if ($pair[0].IndexOf('canonical_account_recovery_v1', [StringComparison]::Ordinal) -ge 0) {
        throw 'inactive canonical Account recovery leaked into active implementation'
    }
}
$source = @($compatibility.surfaces | Where-Object { $_.id -ceq 'account-recovery' })
if ($source.Count -ne 1 -or [string]$source[0].encoding -cne 'postcard') {
    throw 'active compatibility registry no longer records Postcard Account recovery'
}
$surface = @($coverage.surfaces | Where-Object { $_.id -ceq 'account-recovery' })
$expectedCovered = @('root-signed-authority-package', 'independent-root-signed-witness',
    'fresh-challenge-request', 'current-device-approval', 'strict-current-roster-majority',
    'monotonic-policy-transition-request', 'joint-old-new-roster-approvals',
    'policy-transition-certificate', 'local-monotonic-anchor-binding')
if ($surface.Count -ne 1 -or [string]$surface[0].coverage -cne 'complete' -or
    (@($surface[0].covered) -join ',') -cne ($expectedCovered -join ',') -or
    @($surface[0].remaining).Count -ne 0) {
    throw 'canonical Account recovery aggregate coverage boundary changed'
}
if ([int]$mutations.expected_mutations -ne 16311 -or
    [int]$mutations.maximum_mutations -ne 20000) {
    throw 'canonical Account recovery decoder mutation bound changed'
}
$expectedMutationIds = @('signed-account-recovery-package-v1',
    'signed-account-recovery-witness-v1', 'account-recovery-approval-request-v1',
    'signed-account-recovery-approval-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-ACCOUNT-RECOVERY-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical Account recovery mutation fixtures changed'
}
foreach ($required in @('matching stale pair', 'strict majority',
    'joint consent from the old and new rosters', 'does not activate')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical Account recovery RFC is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --authority-vectors $authorityVectorsPath --account-vectors $accountVectorsPath `
    --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_account_recovery_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical Account recovery independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_validity_seconds = 1801
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_validity_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.approval.signature_hex = '00'
    Assert-Rejected { Assert-Vectors $changedVector } 'invalid_approval_signature_shape'
    Write-Output 'canonical_account_recovery_self_test=passed'
}

Write-Output 'canonical_account_recovery_boundary=verified'
Write-Output 'account_recovery_coverage=complete'
Write-Output 'independent_ed25519_and_blake3=verified'
Write-Output 'strict_current_roster_majority=verified'
Write-Output 'cross_roster_fork_safety=joint-transition-required'
Write-Output 'bounded_decoder_mutations=16311'
Write-Output 'active_postcard_v1_changed=false'
Write-Output 'network_executed=false'
