$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $tree = @(& cargo tree --locked -p kilogram-mailbox-provisioning --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while inspecting the mailbox-provisioning dependency boundary'
    }

    $forbidden = @(
        'image',
        'iroh',
        'qrcode',
        'reqwest',
        'rqrr',
        'tokio',
        'kilogram-bootstrap-contract',
        'kilogram-protocol',
        'kilogram-ratchet',
        'kilogram-runtime-ipc',
        'kilogram-session',
        'kilogram-state',
        'kilogram-store',
        'kilogram-transport-iroh'
    )
    $violations = foreach ($package in $forbidden) {
        $tree | Where-Object { $_ -match ('^' + [regex]::Escape($package) + ' v') }
    }
    if ($violations) {
        throw "mailbox-provisioning dependency boundary contains forbidden packages:`n$($violations -join "`n")"
    }
    foreach ($required in @('kilogram-crypto', 'kilogram-identity', 'kilogram-mailbox', 'url')) {
        if (-not ($tree | Where-Object { $_ -match ('^' + [regex]::Escape($required) + ' v') })) {
            throw "mailbox-provisioning dependency boundary is missing required package '$required'"
        }
    }

    $sourceFiles = Get-ChildItem -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox-provisioning\src') -File -Filter '*.rs'
    $runtimeIdentifiers = $sourceFiles | Select-String -CaseSensitive -Pattern @(
        '\bConversationId\b',
        '\bEventId\b',
        '\bMessageId\b',
        '\bRuntimeContactId\b',
        '\bRuntimeQueueId\b'
    )
    if ($runtimeIdentifiers) {
        throw "mailbox-provisioning API contains forbidden runtime identifiers:`n$($runtimeIdentifiers -join "`n")"
    }

    $manifest = Get-Content -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox-provisioning\Cargo.toml') -Raw
    if ($manifest -match '(?m)^\s*\[\[bin\]\]') {
        throw 'mailbox-provisioning unexpectedly defines another executable'
    }
    if ($manifest -match '(?m)^\s*(reqwest|redb|tokio|iroh)(\.workspace)?\s*=') {
        throw 'mailbox-provisioning unexpectedly owns a direct network, runtime, or storage dependency'
    }

    Write-Output 'mailbox_provisioning_boundary=verified'
    Write-Output 'new_executable=false'
    Write-Output 'network_dependencies=false'
    Write-Output 'runtime_dependencies=false'
    Write-Output 'direct_storage_dependencies=false'
    Write-Output 'recipient_identity_binding=kilogram-identity'
    Write-Output 'capability_contract=kilogram-mailbox'
    Write-Output "normal_dependency_records=$($tree.Count)"
}
finally {
    Pop-Location
}
