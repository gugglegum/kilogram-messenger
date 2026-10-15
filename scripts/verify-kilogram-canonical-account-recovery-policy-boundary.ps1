[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$schemaPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-POLICY-SCHEMA.json'
$vectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json'
$authorityVectorsPath = Join-Path $workspace 'CANONICAL-AUTHORITY-VECTORS.json'
$accountVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-VECTORS.json'
$recoveryVectorsPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-VECTORS.json'
$cddlPath = Join-Path $workspace 'CANONICAL-ACCOUNT-RECOVERY-POLICY.cddl'
$rustPath = Join-Path $workspace 'crates\kilogram-protocol\src\canonical_account_recovery_policy_v1.rs'
$libPath = Join-Path $workspace 'crates\kilogram-protocol\src\lib.rs'
$activePolicyPath = Join-Path $workspace 'crates\kilogram-identity\src\recovery_policy.rs'
$compatibilityPath = Join-Path $workspace 'PROTOCOL-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$mutationsPath = Join-Path $workspace 'CANONICAL-DECODER-MUTATIONS.json'
$referencePath = Join-Path $PSScriptRoot 'verify-kilogram-canonical-account-recovery-policy-reference.mjs'
$rfcPath = Join-Path $workspace 'docs\RFC-0153-m1127-canonical-account-recovery-policy-transition.md'

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
    if ([UInt64]$Schema.schema_version -ne 1 -or [string]$Schema.milestone -cne 'M1.1.27' -or
        [string]$Schema.status -cne 'candidate-not-active' -or [bool]$Schema.changes_active_wire -or
        [string]$Schema.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Schema.cddl.path -cne 'CANONICAL-ACCOUNT-RECOVERY-POLICY.cddl' -or
        [string]$Schema.cddl.root_rule -cne 'kilogram-account-recovery-policy-v1' -or
        [string]$Schema.activation.current_runtime_encoding -cne 'postcard-v1' -or
        [bool]$Schema.activation.current_runtime_changed) {
        throw 'canonical Account recovery policy schema header changed'
    }
    $limits = @(
        [int64]$Schema.limits.maximum_request_bytes,
        [int]$Schema.limits.maximum_approval_bytes,
        [int]$Schema.limits.maximum_approvals,
        [int64]$Schema.limits.maximum_certificate_bytes,
        [int]$Schema.limits.maximum_validity_seconds,
        [int]$Schema.limits.clock_skew_seconds
    ) -join ','
    if ($limits -cne '67182592,16384,64,68296704,1800,120') {
        throw 'canonical Account recovery policy limits changed'
    }
    if (@($Schema.external_objects).Count -ne 2 -or @($Schema.domains).Count -ne 4 -or
        @($Schema.objects).Count -ne 3 -or @($Schema.semantic_rules).Count -ne 14) {
        throw 'canonical Account recovery policy inventory changed'
    }
    foreach ($required in @(
        'MAX_CANONICAL_ACCOUNT_RECOVERY_POLICY_APPROVALS: usize =',
        'kilogram:account-recovery-policy-transition-request-id:cbor-v1\0',
        'kilogram:account-recovery-policy-transition-approval-signature:cbor-v1\0',
        'kilogram:account-recovery-policy-transition-certificate-id:cbor-v1\0',
        'decode_and_verify_account_recovery_policy_transition_request_v1',
        'verify_account_recovery_policy_transition_anchor_v1',
        'decode_and_verify_signed_account_recovery_policy_transition_approval_v1',
        'verify_account_recovery_policy_transition_approval_for_request_v1',
        'decode_and_verify_account_recovery_policy_transition_certificate_v1',
        'verify_account_recovery_policy_transition_certificate_v1',
        'verify_account_recovery_package_successor_v1')) {
        if ($Rust.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
            throw "canonical Account recovery policy Rust contract is missing '$required'"
        }
    }
}

function Assert-Vectors {
    param([object]$Vectors)
    if ([UInt64]$Vectors.schema_version -ne 1 -or
        [string]$Vectors.profile -cne 'kilogram-deterministic-cbor-v1' -or
        [string]$Vectors.activation_status -cne 'not-active') {
        throw 'canonical Account recovery policy vector header changed'
    }
    foreach ($entry in @($Vectors.expected.account_id_hex, $Vectors.expected.old_device_id_hex,
        $Vectors.expected.new_device_id_hex, $Vectors.expected.request.object_id_hex,
        $Vectors.expected.certificate.object_id_hex)) {
        Assert-LowerHex ([string]$entry) 32 'policy identifier'
    }
    Assert-LowerHex ([string]$Vectors.expected.request.cbor_hex) -Label 'transition request'
    Assert-LowerHex ([string]$Vectors.expected.certificate.cbor_hex) -Label 'transition certificate'
    if (@($Vectors.expected.approvals).Count -ne 2) {
        throw 'canonical Account recovery policy approval vector count changed'
    }
    foreach ($approval in @($Vectors.expected.approvals)) {
        Assert-LowerHex ([string]$approval.approver_device_id_hex) 32 'approver Device ID'
        Assert-LowerHex ([string]$approval.content_cbor_hex) -Label 'approval content'
        Assert-LowerHex ([string]$approval.signing_preimage_hex) -Label 'approval preimage'
        Assert-LowerHex ([string]$approval.signature_hex) 64 'approval signature'
        Assert-LowerHex ([string]$approval.signed_cbor_hex) -Label 'signed approval'
        Assert-LowerHex ([string]$approval.object_id_hex) 32 'approval ID'
    }
    if ([UInt64]$Vectors.input.expires_at_unix_seconds -
        [UInt64]$Vectors.input.issued_at_unix_seconds -ne 600 -or
        [UInt64]$Vectors.input.old_epoch -ne 0 -or
        [int]$Vectors.expected.quorum.old_observed -ne 1 -or
        [int]$Vectors.expected.quorum.old_required -ne 1 -or
        [int]$Vectors.expected.quorum.new_observed -ne 2 -or
        [int]$Vectors.expected.quorum.new_required -ne 2 -or
        -not [bool]$Vectors.expected.quorum.joint_majority_satisfied) {
        throw 'canonical Account recovery policy vector semantics changed'
    }
}

foreach ($path in @($schemaPath, $vectorsPath, $authorityVectorsPath, $accountVectorsPath,
    $recoveryVectorsPath, $cddlPath, $rustPath, $libPath, $activePolicyPath,
    $compatibilityPath, $coveragePath, $mutationsPath, $referencePath, $rfcPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "canonical Account recovery policy file is missing: $path"
    }
}
$schema = Get-Content -LiteralPath $schemaPath -Raw | ConvertFrom-Json
$vectors = Get-Content -LiteralPath $vectorsPath -Raw | ConvertFrom-Json
$cddl = Get-Content -LiteralPath $cddlPath -Raw
$rust = Get-Content -LiteralPath $rustPath -Raw
$lib = Get-Content -LiteralPath $libPath -Raw
$activePolicy = Get-Content -LiteralPath $activePolicyPath -Raw
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$mutations = Get-Content -LiteralPath $mutationsPath -Raw | ConvertFrom-Json
$reference = Get-Content -LiteralPath $referencePath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw
Assert-Schema $schema $rust
Assert-Vectors $vectors

foreach ($required in @('account-recovery-policy-transition-request-v1',
    'signed-account-recovery-policy-transition-approval-v1',
    'account-recovery-policy-transition-certificate-v1')) {
    if ($cddl.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "Account recovery policy CDDL is missing '$required'"
    }
}
if ($lib.IndexOf('pub mod canonical_account_recovery_policy_v1;', [StringComparison]::Ordinal) -lt 0) {
    throw 'canonical Account recovery policy module is not exported'
}
if ($activePolicy.IndexOf('kilogram:account-recovery-policy-transition-approval:v1\0',
        [StringComparison]::Ordinal) -lt 0 -or
    $activePolicy.IndexOf('canonical_account_recovery_policy_v1',
        [StringComparison]::Ordinal) -ge 0) {
    throw 'inactive canonical Account recovery policy leaked into active implementation'
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
    throw 'canonical Account recovery policy aggregate coverage boundary changed'
}
if ([int]$mutations.expected_mutations -ne 27711 -or
    [int]$mutations.maximum_mutations -ne 30000) {
    throw 'canonical Account recovery policy decoder mutation bound changed'
}
$expectedMutationIds = @('account-recovery-policy-transition-request-v1',
    'signed-account-recovery-policy-transition-approval-v1',
    'account-recovery-policy-transition-certificate-v1')
$actualMutationIds = @($mutations.fixtures |
    Where-Object { $_.vector_file -ceq 'CANONICAL-ACCOUNT-RECOVERY-POLICY-VECTORS.json' } |
    ForEach-Object { $_.id })
if (($actualMutationIds -join ',') -cne ($expectedMutationIds -join ',')) {
    throw 'canonical Account recovery policy mutation fixtures changed'
}
foreach ($required in @('strict majority of the old roster',
    'majority of the new roster', 'locally authenticated monotonic anchor',
    'does not activate')) {
    if ($rfc.IndexOf($required, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
        throw "canonical Account recovery policy RFC is missing '$required'"
    }
}
foreach ($required in @('blake3Hash(', 'node:crypto',
    'canonical_account_recovery_policy_reference_self_test=passed')) {
    if ($reference.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "canonical Account recovery policy reference is missing '$required'"
    }
}

$referenceOutput = @(& node $referencePath --schema $schemaPath --vectors $vectorsPath `
    --authority-vectors $authorityVectorsPath --account-vectors $accountVectorsPath `
    --recovery-vectors $recoveryVectorsPath --self-test 2>&1)
if ($LASTEXITCODE -ne 0 -or
    'canonical_account_recovery_policy_reference_self_test=passed' -notin
    @($referenceOutput | ForEach-Object { [string]$_ })) {
    throw "canonical Account recovery policy independent reference failed`n$($referenceOutput -join [Environment]::NewLine)"
}

if ($SelfTest) {
    $activeSchema = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $activeSchema.status = 'active'
    Assert-Rejected { Assert-Schema $activeSchema $rust } 'active_without_migration'
    $changedLimit = $schema | ConvertTo-Json -Depth 20 | ConvertFrom-Json
    $changedLimit.limits.maximum_approvals = 65
    Assert-Rejected { Assert-Schema $changedLimit $rust } 'changed_approval_limit'
    $changedVector = $vectors | ConvertTo-Json -Depth 30 | ConvertFrom-Json
    $changedVector.expected.quorum.new_required = 1
    Assert-Rejected { Assert-Vectors $changedVector } 'weakened_new_roster_quorum'
    Write-Output 'canonical_account_recovery_policy_self_test=passed'
}

Write-Output 'canonical_account_recovery_policy_boundary=verified'
Write-Output 'account_recovery_coverage=complete'
Write-Output 'joint_old_new_roster_majority=verified'
Write-Output 'monotonic_successor=verified'
Write-Output 'portable_anchor_check=required'
Write-Output 'bounded_decoder_mutations=27711'
Write-Output 'active_postcard_v1_changed=false'
Write-Output 'network_executed=false'
