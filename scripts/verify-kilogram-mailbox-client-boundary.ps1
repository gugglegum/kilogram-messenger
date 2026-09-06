$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $clientTree = @(& cargo tree --locked -p kilogram-mailbox-client --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while inspecting the mailbox-client dependency boundary'
    }

    $forbidden = @(
        'image',
        'iroh',
        'qrcode',
        'rqrr',
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
        $clientTree | Where-Object { $_ -match ('^' + [regex]::Escape($package) + ' v') }
    }
    if ($violations) {
        throw "mailbox-client dependency boundary contains forbidden packages:`n$($violations -join "`n")"
    }
    foreach ($required in @('kilogram-crypto', 'kilogram-mailbox', 'redb', 'reqwest')) {
        if (-not ($clientTree | Where-Object { $_ -match ('^' + [regex]::Escape($required) + ' v') })) {
            throw "mailbox-client dependency boundary is missing required package '$required'"
        }
    }

    $sourceFiles = Get-ChildItem -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox-client\src') -File -Filter '*.rs'
    $applicationIdentifiers = $sourceFiles | Select-String -CaseSensitive -Pattern @(
        '\bAccountId\b',
        '\bDeviceId\b',
        '\bConversationId\b',
        '\bEventId\b',
        '\bMessageId\b'
    )
    if ($applicationIdentifiers) {
        throw "mailbox-client API contains forbidden application identifiers:`n$($applicationIdentifiers -join "`n")"
    }

    $manifest = Get-Content -LiteralPath (Join-Path $workspace 'crates\kilogram-mailbox-client\Cargo.toml') -Raw
    if ($manifest -match '(?m)^\s*\[\[bin\]\]') {
        throw 'mailbox-client unexpectedly defines another executable'
    }

    $serviceTree = @(& cargo tree --locked -p kilogram-ticket-store --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while checking the existing opaque service integration'
    }
    if (-not ($serviceTree | Where-Object { $_ -match '^kilogram-mailbox v' })) {
        throw 'existing opaque service does not include the blind-mailbox contract'
    }

    Write-Output 'mailbox_client_boundary=verified'
    Write-Output 'new_executable=false'
    Write-Output 'application_identifiers=false'
    Write-Output 'runtime_dependencies=false'
    Write-Output 'https_dependency=reqwest'
    Write-Output 'durable_ledger_dependency=redb'
    Write-Output 'server_process=kilogram-ticket-store'
    Write-Output "normal_dependency_records=$($clientTree.Count)"
}
finally {
    Pop-Location
}
