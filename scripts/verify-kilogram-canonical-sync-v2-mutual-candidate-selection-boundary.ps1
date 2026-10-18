[CmdletBinding()]
param([switch]$SelfTest)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-MUTUAL-CANDIDATE-SELECTION.json'
$migrationPath = Join-Path $workspace 'CANONICAL-MIGRATION.json'
$transportManifestPath = Join-Path $workspace 'CANONICAL-TRANSPORT-SYNC.json'
$compatibilityPath = Join-Path $workspace 'CANONICAL-SYNC-HISTORY-COMPATIBILITY.json'
$coveragePath = Join-Path $workspace 'CANONICAL-COVERAGE.json'
$capabilityPath = Join-Path $workspace 'crates\kilogram-sync-capability\src\lib.rs'
$adapterPath = Join-Path $workspace 'crates\kilogram-session\src\canonical_sync_v2_adapter.rs'
$sessionCargoPath = Join-Path $workspace 'crates\kilogram-session\Cargo.toml'
$wirePath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$transportPath = Join-Path $workspace 'crates\kilogram-transport-iroh\src\lib.rs'
$cliPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$rfcPath = Join-Path $workspace 'docs\RFC-0195-m1168-mutual-authenticated-candidate-selection.md'

function Assert-Rejected {
    param([scriptblock]$Action, [string]$Label)
    try { & $Action } catch { Write-Output "$Label=rejected"; return }
    throw "$Label was unexpectedly accepted"
}

function Copy-JsonObject {
    param([object]$Value)
    return $Value | ConvertTo-Json -Depth 100 | ConvertFrom-Json
}

function Assert-ExactStringSet {
    param([string[]]$Actual, [string[]]$Expected, [string]$Label)
    $actualSorted = @($Actual | Sort-Object)
    $expectedSorted = @($Expected | Sort-Object)
    if ($actualSorted.Count -ne $expectedSorted.Count) {
        throw "$Label count mismatch"
    }
    for ($index = 0; $index -lt $expectedSorted.Count; $index++) {
        if ($actualSorted[$index] -cne $expectedSorted[$index]) {
            throw "$Label mismatch at index $index"
        }
    }
}

function Assert-Contract {
    param(
        [object]$Manifest,
        [object]$Migration,
        [object]$TransportManifest,
        [object]$Compatibility,
        [object]$Coverage,
        [string]$Capability,
        [string]$Adapter,
        [string]$SessionCargo,
        [string]$Wire,
        [string]$Transport,
        [string]$Cli,
        [string]$Rfc
    )

    $status = 'dormant-mutual-authenticated-candidate-selection-verified-not-active'
    $aggregateStatus = 'canonical-sync-v2-ready-for-explicit-activation-decision'
    if ([UInt64]$Manifest.schema_version -ne 1 -or
        [string]$Manifest.milestone -cne 'M1.1.68' -or
        [string]$Manifest.status -cne $status -or
        [bool]$Manifest.changes_active_wire -or
        [string]$Manifest.protocol_identifier -cne 'kilogram/m1/sync/cbor/2') {
        throw 'mutual candidate selection header changed'
    }
    if ([string]$Manifest.implementation.listener_capability -cne
            'SignedSyncTransportCapability' -or
        [string]$Manifest.implementation.requester_offer -cne
            'SignedSyncTransportCandidateOffer' -or
        [string]$Manifest.implementation.listener_acceptance -cne
            'SignedSyncTransportCandidateAcceptance' -or
        [string]$Manifest.implementation.durable_verifier -cne
            'verify_durable_mutual_canonical_candidate_selection' -or
        [string]$Manifest.implementation.result -cne
            'CanonicalSyncV2MutualCandidateSelection') {
        throw 'mutual candidate selection implementation changed'
    }

    $signatureDomains = @(
        [string]$Manifest.signature_domains.listener_capability,
        [string]$Manifest.signature_domains.requester_offer,
        [string]$Manifest.signature_domains.listener_acceptance
    )
    Assert-ExactStringSet $signatureDomains @(
        'kilogram:sync-transport-capability-signature:v2\0',
        'kilogram:sync-transport-candidate-offer-signature:v1\0',
        'kilogram:sync-transport-candidate-acceptance-signature:v1\0'
    ) 'candidate signature domains'
    if (($signatureDomains | Select-Object -Unique).Count -ne 3 -or
        [string]$Manifest.content_address_domains.listener_capability -cne
            'kilogram:sync-transport-capability-artifact-id:v2\0' -or
        [string]$Manifest.content_address_domains.requester_offer -cne
            'kilogram:sync-transport-candidate-offer-id:v1\0' -or
        [string]$Manifest.content_address_domains.listener_capability -ceq
            [string]$Manifest.content_address_domains.requester_offer) {
        throw 'candidate domain separation changed'
    }

    Assert-ExactStringSet @($Manifest.offer_bindings) @(
        'listener-capability-id', 'conversation-id', 'listener-account-id',
        'listener-device-id', 'requester-account-id', 'requester-device-id',
        'session-binding', 'listener-authority-revision',
        'requester-authority-revision', 'migration-epoch',
        'canonical-cbor-v2-protocol', 'no-fallback-after-selected-failure'
    ) 'requester offer bindings'
    Assert-ExactStringSet @($Manifest.acceptance_bindings) @(
        'requester-offer-id', 'conversation-id', 'listener-device-id',
        'requester-device-id', 'session-binding', 'migration-epoch',
        'canonical-cbor-v2-protocol', 'no-fallback-after-selected-failure'
    ) 'listener acceptance bindings'

    foreach ($field in @(
        'listener_device_session_authorization_required',
        'requester_device_session_authorization_required',
        'sync_history_capability_required',
        'account_device_revision_and_session_exact_bound',
        'listener_capability_time_and_ticket_binding_reverified'
    )) {
        if (-not [bool]$Manifest.authority_contract.$field) {
            throw "authority contract '$field' is not verified"
        }
    }
    foreach ($field in @(
        'client_runtime_identity_exact_bound',
        'responder_runtime_identity_exact_bound',
        'conversation_device_session_and_protocol_exact_bound',
        'completed_legacy_import_prerequisite_inherited'
    )) {
        if (-not [bool]$Manifest.durable_contract.$field) {
            throw "durable contract '$field' is not verified"
        }
    }
    foreach ($field in @(
        'unknown_selection_version', 'zero_or_unsafe_binding',
        'wrong_device_signature', 'capability_or_offer_substitution',
        'cross_offer_acceptance', 'authority_or_revision_mismatch',
        'durable_runtime_identity_mismatch', 'expired_listener_capability',
        'fallback_after_selected_failure'
    )) {
        if (-not [bool]$Manifest.fail_closed.$field) {
            throw "fail-closed contract '$field' is not verified"
        }
    }
    if (-not [bool]$Manifest.activation.mutual_candidate_selection_integrated -or
        [bool]$Manifest.activation.selection_authorizes_transport_activation -or
        [bool]$Manifest.activation.endpoint_registered -or
        [bool]$Manifest.activation.canonical_dial_enabled -or
        [bool]$Manifest.activation.active_postcard_path_changed -or
        [bool]$Manifest.scope.network_executed -or
        [bool]$Manifest.scope.release_build -or
        [bool]$Manifest.scope.archive_created -or
        [bool]$Manifest.scope.stable_executable_replaced) {
        throw 'mutual candidate activation boundary changed'
    }
    Assert-ExactStringSet @($Manifest.remaining) @(
        'dual-reader-runtime-implementation',
        'mixed-version-and-downgrade-tests',
        'rollback-drill',
        'sustained-fuzzing',
        'fresh-hosted-and-field-evidence'
    ) 'mutual candidate remaining work'

    if ([string]$Migration.milestone -cne 'M1.1.80' -or
        [string]$Migration.status -cne $aggregateStatus -or
        [string]$Migration.current_state -cne 'legacy-only' -or
        [string]$Migration.history_compatible_successor.dormant_mutual_authenticated_candidate_selection -cne
            'verified' -or
        [string]$Migration.selection.mutual_authenticated_candidate_selection -cne
            'verified' -or
        [bool]$Migration.selection.selection_authorizes_registration_or_dial -or
        [bool]$Migration.history_compatible_successor.registered -or
        [bool]$Migration.history_compatible_successor.dialed) {
        throw 'canonical migration mutual selection state changed'
    }
    $requirement = @(
        @($Migration.activation_requirements) |
            Where-Object { [string]$_.id -ceq 'mutual-authenticated-capability-selection' }
    )
    if ($requirement.Count -ne 1 -or [string]$requirement[0].status -cne 'verified') {
        throw 'mutual capability selection requirement is not verified'
    }
    if ([string]$TransportManifest.milestone -cne 'M1.1.80' -or
        [string]$TransportManifest.status -cne $aggregateStatus -or
        -not [bool]$TransportManifest.history_compatible_successor.dormant_mutual_authenticated_candidate_selection -or
        -not [bool]$TransportManifest.selection.mutual_authenticated_candidate_selection -or
        -not [bool]$TransportManifest.selection.both_durable_adapter_identities_required -or
        [bool]$TransportManifest.selection.selection_authorizes_registration_or_dial -or
        'mutual-authenticated-capability-selection' -cin @($TransportManifest.remaining) -or
        'dual-reader-runtime-implementation' -cin @($TransportManifest.remaining) -or
        [bool]$TransportManifest.history_compatible_successor.registered -or
        [bool]$TransportManifest.history_compatible_successor.dialed) {
        throw 'transport manifest mutual selection state changed'
    }
    if ([string]$Compatibility.milestone -cne 'M1.1.80' -or
        [string]$Compatibility.status -cne $aggregateStatus -or
        -not [bool]$Compatibility.required_successor.dormant_mutual_authenticated_candidate_selection_integrated) {
        throw 'history compatibility lost mutual selection state'
    }
    $surface = @($Coverage.surfaces | Where-Object { [string]$_.id -ceq 'transport-sync' })
    if ([string]$Coverage.milestone -cne 'M1.1.80' -or
        $surface.Count -ne 1 -or
        'CANONICAL-SYNC-V2-MUTUAL-CANDIDATE-SELECTION.json' -cnotin @($surface[0].artifacts) -or
        'scripts/verify-kilogram-canonical-sync-v2-mutual-candidate-selection-boundary.ps1' -cnotin
            @($surface[0].artifacts) -or
        'dormant-mutual-authenticated-candidate-selection' -cnotin @($surface[0].covered) -or
        'mutual-authenticated-capability-selection' -cin @($surface[0].remaining) -or
        'dual-reader-runtime-implementation' -cin @($surface[0].remaining)) {
        throw 'coverage manifest mutual selection state changed'
    }

    foreach ($needle in @(
        'SYNC_TRANSPORT_CAPABILITY_ARTIFACT_ID_DOMAIN',
        'SYNC_TRANSPORT_CANDIDATE_OFFER_SIGNATURE_DOMAIN',
        'SYNC_TRANSPORT_CANDIDATE_OFFER_ID_DOMAIN',
        'SYNC_TRANSPORT_CANDIDATE_ACCEPTANCE_SIGNATURE_DOMAIN',
        'pub struct SignedSyncTransportCandidateOffer',
        'pub struct SignedSyncTransportCandidateAcceptance',
        'pub fn verify_mutual_canonical_candidate_selection(',
        'fallback_after_selected_failure: false',
        'candidate_selection_substitutions_fail_closed'
    )) {
        if (-not $Capability.Contains($needle)) {
            throw "candidate capability source marker missing: $needle"
        }
    }
    foreach ($needle in @(
        'pub struct CanonicalSyncV2MutualCandidateSelection',
        'pub const fn active_transport_authorized(&self) -> bool',
        'pub fn verify_durable_mutual_canonical_candidate_selection(',
        'verify_signed_device_session_authorization_v1(authorization)?',
        'CanonicalDeviceCapabilityV1::SyncHistory',
        'MutualCandidateAuthorizationMismatch',
        'MutualCandidateDurableIdentityMismatch',
        'mutual_candidate_selection_binds_ticket_authorities_and_both_durable_adapters'
    )) {
        if (-not $Adapter.Contains($needle)) {
            throw "durable selection source marker missing: $needle"
        }
    }
    if (-not $SessionCargo.Contains(
            'kilogram-sync-capability = { path = "../kilogram-sync-capability" }')) {
        throw 'session crate does not declare the capability verifier dependency'
    }
    foreach ($inactiveSource in @($Wire, $Transport, $Cli)) {
        if ($inactiveSource.Contains('SignedSyncTransportCandidateOffer') -or
            $inactiveSource.Contains('verify_durable_mutual_canonical_candidate_selection') -or
            $inactiveSource.Contains('CanonicalSyncV2MutualCandidateSelection')) {
            throw 'dormant mutual candidate selection leaked into active wire/transport/CLI code'
        }
    }
    $normalizedRfc = [regex]::Replace($Rfc, '\s+', ' ')
    foreach ($needle in @(
        'Three signed links',
        'Both peers must also supply valid canonical Device session authorizations',
        'active_transport_authorized=false',
        'without registering or dialling the candidate ALPN'
    )) {
        if ($normalizedRfc.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
            throw "mutual candidate RFC is missing '$needle'"
        }
    }
}

foreach ($path in @(
    $manifestPath, $migrationPath, $transportManifestPath, $compatibilityPath,
    $coveragePath, $capabilityPath, $adapterPath, $sessionCargoPath, $wirePath,
    $transportPath, $cliPath, $rfcPath
)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "mutual candidate selection file is missing: $path"
    }
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$migration = Get-Content -LiteralPath $migrationPath -Raw | ConvertFrom-Json
$transportManifest = Get-Content -LiteralPath $transportManifestPath -Raw | ConvertFrom-Json
$compatibility = Get-Content -LiteralPath $compatibilityPath -Raw | ConvertFrom-Json
$coverage = Get-Content -LiteralPath $coveragePath -Raw | ConvertFrom-Json
$capability = Get-Content -LiteralPath $capabilityPath -Raw
$adapter = Get-Content -LiteralPath $adapterPath -Raw
$sessionCargo = Get-Content -LiteralPath $sessionCargoPath -Raw
$wire = Get-Content -LiteralPath $wirePath -Raw
$transport = Get-Content -LiteralPath $transportPath -Raw
$cli = Get-Content -LiteralPath $cliPath -Raw
$rfc = Get-Content -LiteralPath $rfcPath -Raw

Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
    $capability $adapter $sessionCargo $wire $transport $cli $rfc

if ($SelfTest) {
    $premature = Copy-JsonObject $manifest
    $premature.activation.selection_authorizes_transport_activation = $true
    Assert-Rejected {
        Assert-Contract $premature $migration $transportManifest $compatibility $coverage `
            $capability $adapter $sessionCargo $wire $transport $cli $rfc
    } 'premature_transport_activation'

    $fallback = Copy-JsonObject $manifest
    $fallback.fail_closed.fallback_after_selected_failure = $false
    Assert-Rejected {
        Assert-Contract $fallback $migration $transportManifest $compatibility $coverage `
            $capability $adapter $sessionCargo $wire $transport $cli $rfc
    } 'fallback_after_selected_failure'

    $missingAuthority = Copy-JsonObject $manifest
    $missingAuthority.offer_bindings = @(
        @($missingAuthority.offer_bindings) |
            Where-Object { [string]$_ -cne 'requester-authority-revision' }
    )
    Assert-Rejected {
        Assert-Contract $missingAuthority $migration $transportManifest $compatibility $coverage `
            $capability $adapter $sessionCargo $wire $transport $cli $rfc
    } 'missing_requester_authority_binding'

    $sharedDomain = Copy-JsonObject $manifest
    $sharedDomain.signature_domains.listener_acceptance =
        $sharedDomain.signature_domains.requester_offer
    Assert-Rejected {
        Assert-Contract $sharedDomain $migration $transportManifest $compatibility $coverage `
            $capability $adapter $sessionCargo $wire $transport $cli $rfc
    } 'shared_offer_acceptance_signature_domain'

    Assert-Rejected {
        Assert-Contract $manifest $migration $transportManifest $compatibility $coverage `
            $capability $adapter $sessionCargo $wire `
            ($transport + ' verify_durable_mutual_canonical_candidate_selection') $cli $rfc
    } 'selection_leaked_into_active_transport'

    Write-Output 'canonical_sync_v2_mutual_candidate_selection_self_test=passed'
}

Write-Output 'canonical_sync_v2_mutual_candidate_selection_boundary=verified'
Write-Output 'listener_capability_signature=true'
Write-Output 'requester_offer_signature=true'
Write-Output 'listener_acceptance_signature=true'
Write-Output 'both_canonical_authorities_verified=true'
Write-Output 'both_durable_adapter_identities_bound=true'
Write-Output 'selection_authorizes_transport_activation=false'
Write-Output 'candidate_identifier_registered=false'
Write-Output 'candidate_identifier_dialed=false'
Write-Output 'active_runtime_changed=false'
Write-Output 'network_executed=false'
