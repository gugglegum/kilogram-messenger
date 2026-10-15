[CmdletBinding()]
param(
    [switch]$SelfTest,
    [string]$AcceptedTag
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$scriptsDirectory = [System.IO.Path]::GetFullPath($PSScriptRoot)
$candidateVerifier = Join-Path $scriptsDirectory 'verify-kilogram-m1-candidate.ps1'
$acceptedTagVerifier = Join-Path $scriptsDirectory 'verify-kilogram-m1-tag-acceptance.ps1'
$cargoWrapper = Join-Path $scriptsDirectory 'invoke-cargo-friendly.ps1'

$expectedGates = [ordered]@{
    'verify-kilogram-bootstrap-safe-provider-selection-boundary.ps1' = 'bootstrap_safe_provider_selection_boundary=verified'
    'verify-kilogram-canonical-authority-boundary.ps1' = 'canonical_authority_boundary=verified'
    'verify-kilogram-canonical-account-boundary.ps1' = 'canonical_account_boundary=verified'
    'verify-kilogram-canonical-account-recovery-boundary.ps1' = 'canonical_account_recovery_boundary=verified'
    'verify-kilogram-canonical-account-recovery-policy-boundary.ps1' = 'canonical_account_recovery_policy_boundary=verified'
    'verify-kilogram-canonical-mailbox-boundary.ps1' = 'canonical_mailbox_boundary=verified'
    'verify-kilogram-canonical-mailbox-gossip-boundary.ps1' = 'canonical_mailbox_gossip_boundary=verified'
    'verify-kilogram-canonical-mailbox-provisioning-boundary.ps1' = 'canonical_mailbox_provisioning_boundary=verified'
    'verify-kilogram-canonical-runtime-mailbox-boundary.ps1' = 'canonical_runtime_mailbox_boundary=verified'
    'verify-kilogram-canonical-runtime-ipc-boundary.ps1' = 'canonical_runtime_ipc_boundary=verified'
    'verify-kilogram-canonical-runtime-ipc-read-boundary.ps1' = 'canonical_runtime_ipc_read_boundary=verified'
    'verify-kilogram-canonical-runtime-ipc-contact-boundary.ps1' = 'canonical_runtime_ipc_contact_boundary=verified'
    'verify-kilogram-canonical-runtime-ipc-directory-boundary.ps1' = 'canonical_runtime_ipc_directory_boundary=verified'
    'verify-kilogram-canonical-mailbox-transport-boundary.ps1' = 'canonical_mailbox_transport_boundary=verified'
    'verify-kilogram-canonical-device-link-boundary.ps1' = 'canonical_device_link_boundary=verified'
    'verify-kilogram-canonical-endpoint-announcement-boundary.ps1' = 'canonical_endpoint_announcement_boundary=verified'
    'verify-kilogram-canonical-endpoint-announcement-transport-boundary.ps1' = 'canonical_endpoint_announcement_transport_boundary=verified'
    'verify-kilogram-canonical-event-boundary.ps1' = 'canonical_event_boundary=verified'
    'verify-kilogram-canonical-prekey-selection-boundary.ps1' = 'canonical_prekey_selection_boundary=verified'
    'verify-kilogram-canonical-publication-conflict-boundary.ps1' = 'canonical_publication_conflict_boundary=verified'
    'verify-kilogram-canonical-conformance-boundary.ps1' = 'canonical_conformance_boundary=verified'
    'verify-kilogram-canonical-coverage-boundary.ps1' = 'canonical_coverage_boundary=verified'
    'verify-kilogram-canonical-migration-boundary.ps1' = 'canonical_migration_boundary=verified'
    'verify-kilogram-canonical-ratchet-boundary.ps1' = 'canonical_ratchet_boundary=verified'
    'verify-kilogram-canonical-recovery-boundary.ps1' = 'canonical_recovery_boundary=verified'
    'verify-kilogram-canonical-recovery-exchange-boundary.ps1' = 'canonical_recovery_exchange_boundary=verified'
    'verify-kilogram-canonical-rewrap-boundary.ps1' = 'canonical_history_rewrap_boundary=verified'
    'verify-kilogram-canonical-sync-boundary.ps1' = 'canonical_sync_boundary=verified'
    'verify-kilogram-canonical-sync-session-boundary.ps1' = 'canonical_sync_session_boundary=verified'
    'verify-kilogram-canonical-ticket-publication-boundary.ps1' = 'canonical_ticket_publication_boundary=verified'
    'verify-kilogram-canonical-wire-boundary.ps1' = 'canonical_wire_boundary=verified'
    'verify-kilogram-independent-builder-boundary.ps1' = 'independent_builder_boundary=verified'
    'verify-kilogram-m0967-simple-kit-boundary.ps1' = 'm0967_simple_kit_boundary=verified'
    'verify-kilogram-m0969-exact-locator-kit-boundary.ps1' = 'm0969_exact_locator_kit_boundary=verified'
    'verify-kilogram-m0972-no-https-kit-boundary.ps1' = 'm0972_no_https_kit_boundary=verified'
    'verify-kilogram-m0973-no-https-kit-boundary.ps1' = 'm0973_no_https_kit_boundary=verified'
    'verify-kilogram-m0974-no-https-kit-boundary.ps1' = 'm0974_no_https_kit_boundary=verified'
    'verify-kilogram-m0976-service-free-v2-kit-boundary.ps1' = 'm0976_service_free_v2_kit_boundary=verified'
    'verify-kilogram-m0987-independent-provider-kit-boundary.ps1' = 'm0987_independent_provider_kit_boundary=verified'
    'verify-kilogram-m1-acceptance-kit-boundary.ps1' = 'm1_acceptance_kit_boundary=verified'
    'verify-kilogram-m1-candidate-boundary.ps1' = 'm1_candidate_boundary=verified'
    'verify-kilogram-m1-readiness-boundary.ps1' = 'm1_readiness_boundary=verified'
    'verify-kilogram-mailbox-boundary.ps1' = 'blind_mailbox_boundary=verified'
    'verify-kilogram-mailbox-client-boundary.ps1' = 'mailbox_client_boundary=verified'
    'verify-kilogram-mailbox-field-test-boundary.ps1' = 'mailbox_field_test_boundary=verified'
    'verify-kilogram-mailbox-https-retirement-boundary.ps1' = 'mailbox_https_retirement_boundary=verified'
    'verify-kilogram-mailbox-legacy-upgrade-boundary.ps1' = 'mailbox_legacy_upgrade_boundary=verified'
    'verify-kilogram-mailbox-provisioning-boundary.ps1' = 'mailbox_provisioning_boundary=verified'
    'verify-kilogram-offline-boundary.ps1' = 'offline_boundary=verified'
    'verify-kilogram-protocol-compatibility-boundary.ps1' = 'protocol_compatibility_boundary=verified'
    'verify-kilogram-provider-admission-work-boundary.ps1' = 'provider_admission_work_boundary=verified'
    'verify-kilogram-provider-colocation-avoidance-boundary.ps1' = 'provider_colocation_avoidance_boundary=verified'
    'verify-kilogram-provider-observation-boundary.ps1' = 'provider_observation_boundary=verified'
    'verify-kilogram-provider-path-domain-boundary.ps1' = 'provider_path_domain_boundary=verified'
    'verify-kilogram-volunteer-field-kit-boundary.ps1' = 'volunteer_field_kit_boundary=verified'
    'verify-kilogram-volunteer-iroh-boundary.ps1' = 'volunteer_iroh_boundary=verified'
    'verify-kilogram-volunteer-provider-gossip-boundary.ps1' = 'volunteer_provider_gossip_boundary=verified'
    'verify-kilogram-volunteer-provider-selection-boundary.ps1' = 'volunteer_provider_selection_boundary=verified'
    'verify-kilogram-volunteer-replica-locator-boundary.ps1' = 'volunteer_replica_set_locator_boundary=verified'
    'verify-kilogram-volunteer-replication-boundary.ps1' = 'volunteer_replication_boundary=verified'
    'verify-kilogram-volunteer-retrieval-boundary.ps1' = 'volunteer_retrieval_boundary=verified'
    'verify-kilogram-volunteer-storage-boundary.ps1' = 'volunteer_storage_boundary=verified'
    'verify-kilogram-windows-ci-boundary.ps1' = 'windows_ci_boundary=verified'
    'verify-publication-conflict-boundary.ps1' = 'publication_conflict_boundary=verified'
}

function Assert-ExactStringSet {
    param(
        [string[]]$Actual,
        [string[]]$Expected,
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

function Assert-ExactMarker {
    param(
        [string[]]$Lines,
        [string]$Marker,
        [string]$Label
    )

    if ($Marker -notin $Lines) {
        throw "$Label did not emit exact success marker '$Marker'"
    }
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

if ($expectedGates.Count -ne 64) {
    throw "M1 readiness boundary inventory must contain exactly 64 gates, got $($expectedGates.Count)"
}

if ($SelfTest) {
    Assert-ExactMarker @('example=verified') 'example=verified' 'synthetic gate'
    Assert-Rejected {
        Assert-ExactMarker @('example=failed') 'example=verified' 'synthetic gate'
    } 'missing_success_marker'
    Assert-ExactStringSet @('a.ps1', 'b.ps1') @('b.ps1', 'a.ps1') 'synthetic inventory'
    Assert-Rejected {
        Assert-ExactStringSet @('a.ps1', 'c.ps1') @('a.ps1', 'b.ps1') 'synthetic inventory'
    } 'changed_boundary_inventory'
    Write-Output 'm1_readiness_self_test=passed'
    Write-Output 'network_executed=false'
    Write-Output 'test_executables_started=false'
    Write-Output 'release_build=false'
    Write-Output 'archive_created=false'
    Write-Output 'git_tag_created=false'
    exit 0
}

$actualGateNames = @(
    Get-ChildItem -LiteralPath $scriptsDirectory -Filter 'verify-*-boundary.ps1' -File |
        ForEach-Object { $_.Name }
)
Assert-ExactStringSet $actualGateNames @($expectedGates.Keys) 'M1 readiness boundary inventory'

$passedGates = 0
foreach ($entry in $expectedGates.GetEnumerator()) {
    $gatePath = Join-Path $scriptsDirectory $entry.Key
    $gateOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $gatePath 2>&1)
    $gateExitCode = $LASTEXITCODE
    $gateLines = @($gateOutput | ForEach-Object { [string]$_ })
    if ($gateExitCode -ne 0) {
        throw "M1 readiness gate '$($entry.Key)' failed with exit code $gateExitCode.`n$($gateLines -join [Environment]::NewLine)"
    }
    Assert-ExactMarker $gateLines $entry.Value $entry.Key
    $passedGates++
    Write-Output "m1_readiness_gate=$($entry.Key):passed"
}

$verificationMode = 'current-candidate'
if ([string]::IsNullOrWhiteSpace($AcceptedTag)) {
    $candidateOutput = @(& powershell -NoProfile -ExecutionPolicy Bypass -File $candidateVerifier 2>&1)
    $candidateExitCode = $LASTEXITCODE
    $candidateLines = @($candidateOutput | ForEach-Object { [string]$_ })
    if ($candidateExitCode -ne 0) {
        throw "M1 candidate production verification failed with exit code $candidateExitCode.`n$($candidateLines -join [Environment]::NewLine)"
    }
    Assert-ExactMarker $candidateLines 'm1_candidate_status=verified' 'M1 candidate production verifier'
}
else {
    $tagOutput = @(
        & powershell -NoProfile -ExecutionPolicy Bypass `
            -File $acceptedTagVerifier `
            -Tag $AcceptedTag 2>&1
    )
    $tagExitCode = $LASTEXITCODE
    $tagLines = @($tagOutput | ForEach-Object { [string]$_ })
    if ($tagExitCode -ne 0) {
        throw "Accepted M1 tag verification failed with exit code $tagExitCode.`n$($tagLines -join [Environment]::NewLine)"
    }
    Assert-ExactMarker $tagLines 'm1_tag_acceptance_status=verified' 'accepted M1 tag verifier'
    $verificationMode = 'accepted-tag'
}

Push-Location $workspace
try {
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) {
        throw "cargo fmt failed with exit code $LASTEXITCODE"
    }

    & $cargoWrapper `
        -CargoCommand clippy `
        -CargoJobs 2 `
        -CargoArguments @(
            '--workspace', '--all-targets', '--all-features', '--locked',
            '--offline', '--', '-D', 'warnings'
        )
    $clippyExitCode = $LASTEXITCODE
    if ($clippyExitCode -ne 0) {
        throw "offline strict Clippy failed with exit code $clippyExitCode"
    }
}
finally {
    Pop-Location
}

Write-Output 'm1_readiness_status=verified'
Write-Output "boundary_gates=$passedGates/$($expectedGates.Count)"
Write-Output "verification_mode=$verificationMode"
if ($verificationMode -ceq 'current-candidate') {
    Write-Output 'candidate_status=verified'
}
else {
    Write-Output 'accepted_m1_tag_status=verified'
    Write-Output "accepted_m1_tag=$AcceptedTag"
}
Write-Output 'format_status=verified'
Write-Output 'offline_strict_clippy=verified'
Write-Output 'test_executables_started=false'
Write-Output 'network_executed=false'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'git_tag_created=false'
Write-Output 'strict_independent_provider_field_acceptance=pending'
Write-Output 'public_security_release=false'
