$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $tree = @(& cargo tree --locked -p kilogram-mailbox --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while inspecting the blind-mailbox dependency boundary'
    }

    $forbidden = @(
        'image',
        'iroh',
        'qrcode',
        'reqwest',
        'rqrr',
        'tokio',
        'kilogram-bootstrap-contract',
        'kilogram-identity',
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
        throw "blind-mailbox dependency boundary contains forbidden packages:`n$($violations -join "`n")"
    }
    foreach ($required in @('kilogram-crypto', 'redb')) {
        if (-not ($tree | Where-Object { $_ -match ('^' + [regex]::Escape($required) + ' v') })) {
            throw "blind-mailbox dependency boundary is missing required package '$required'"
        }
    }

    $sourceFiles = Get-ChildItem -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox\src') -File -Filter '*.rs'
    $applicationIdentifiers = $sourceFiles | Select-String -CaseSensitive -Pattern @(
        '\bAccountId\b',
        '\bDeviceId\b',
        '\bConversationId\b',
        '\bEventId\b',
        '\bMessageId\b'
    )
    if ($applicationIdentifiers) {
        throw "blind-mailbox API contains forbidden application identifiers:`n$($applicationIdentifiers -join "`n")"
    }

    Write-Output 'blind_mailbox_boundary=verified'
    Write-Output 'network_dependencies=false'
    Write-Output 'runtime_dependencies=false'
    Write-Output 'application_identifiers=false'
    Write-Output 'ciphertext_dependency=kilogram-crypto'
    Write-Output 'durable_store_dependency=redb'
    Write-Output "normal_dependency_records=$($tree.Count)"
}
finally {
    Pop-Location
}
