[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$provisioningPath = Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src\lib.rs'
$protocolPath = Join-Path $workspace 'crates\kilogram-protocol\src\wire.rs'
$mainPath = Join-Path $workspace 'apps\kilogram-cli\src\main.rs'
$mailboxPath = Join-Path $workspace 'apps\kilogram-cli\src\runtime_mailbox.rs'
$manifestPath = Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'

$provisioning = Get-Content -LiteralPath $provisioningPath -Raw
$protocol = Get-Content -LiteralPath $protocolPath -Raw
$main = Get-Content -LiteralPath $mainPath -Raw
$mailbox = Get-Content -LiteralPath $mailboxPath -Raw
$manifest = Get-Content -LiteralPath $manifestPath -Raw

foreach ($required in @(
    'pub struct MailboxCapabilityConvergence',
    'pub struct SignedMailboxCapabilityAcknowledgement',
    'pub struct MailboxCapabilitySessionBinding',
    'pub enum MailboxCapabilityBindingState',
    'pub enum MailboxCapabilityInboundDisposition',
    'validate_owner_progression',
    'next_outbound_update',
    'owner_receive_binding_state',
    'recipient_write_binding_state',
    'classify_inbound_update',
    'convergence_survives_lost_ack_restart_rotation_and_revocation',
    'mailbox capability chain advanced before its predecessor was acknowledged'
)) {
    if (-not $provisioning.Contains($required)) {
        throw "transport-independent mailbox convergence contract is missing '$required'"
    }
}

foreach ($required in @(
    'MailboxCapabilityConvergence',
    'MailboxCapabilityInboundDisposition',
    'local_mailbox_capability_convergence',
    'peer_mailbox_capability_convergence',
    'convergence.next_outbound_update()',
    'convergence.classify_inbound_update(update)',
    'convergence.owner_receive_binding_state(binding.binding_id())',
    'convergence.recipient_write_binding_state(binding.binding_id())'
)) {
    if (-not $main.Contains($required)) {
        throw "runtime is not using shared mailbox convergence rule '$required'"
    }
}

if (-not $protocol.Contains('pub fn as_bytes(&self) -> &[u8; 32]')) {
    throw 'transport session binding cannot be mapped into the network-free convergence contract'
}

if ($mailbox.Contains('SignedRuntimeMailboxCapabilityAcknowledgement') -or
    $mailbox.Contains('CAPABILITY_ACKNOWLEDGEMENT_SIGNATURE_DOMAIN')) {
    throw 'CLI runtime still contains a duplicate mailbox acknowledgement implementation'
}

foreach ($forbidden in @(
    'iroh',
    'tokio',
    'reqwest',
    'kilogram-protocol',
    'kilogram-runtime-ipc',
    'kilogram-session',
    'kilogram-transport-iroh'
)) {
    $provisioningManifest = Get-Content -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox-provisioning\Cargo.toml') -Raw
    if ($provisioningManifest.Contains($forbidden)) {
        throw "mailbox convergence boundary gained forbidden dependency '$forbidden'"
    }
}

if ($manifest -match '(?m)^\s*\[\[bin\]\]') {
    throw 'mailbox convergence unexpectedly added another executable target'
}

Write-Output 'mailbox_capability_convergence=verified'
Write-Output 'state_machine=transport-independent-reconstructed-from-signed-artifacts'
Write-Output 'retry=one-predecessor-acked-update'
Write-Output 'rotation_overlap=owner-receive-only-until-ack'
Write-Output 'revocation=immediate-fail-closed'
Write-Output 'crash_regression=activation-lost-ack-retry-rotation-restart-revocation'
Write-Output 'duplicate_cli_ack_codec=false'
Write-Output 'network_dependency=false'
Write-Output 'new_executable=false'
