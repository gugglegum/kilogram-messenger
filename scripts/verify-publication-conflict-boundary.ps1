$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $tree = @(& cargo tree --locked -p kilogram-publication-conflict --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while inspecting the publication-conflict dependency boundary'
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
        throw "publication-conflict dependency boundary contains forbidden packages:`n$($violations -join "`n")"
    }

    $sharedSource = [System.IO.Path]::GetFullPath((Join-Path $workspace 'crates\kilogram-publication-conflict\src\lib.rs'))
    $otherRustSources = Get-ChildItem -LiteralPath (Join-Path $workspace 'apps'), (Join-Path $workspace 'crates') -Recurse -File -Filter '*.rs' |
        Where-Object { [System.IO.Path]::GetFullPath($_.FullName) -ne $sharedSource }
    $duplicateDefinitions = $otherRustSources | Select-String -Pattern @(
        '^\s*(pub\s+)?struct SignedTicketPublication\s*\{',
        '^\s*(pub\s+)?struct SignedTicketPublicationObservation\s*\{',
        '^\s*(pub\s+)?struct SignedPublicationConflictProof\s*\{',
        '^\s*(pub\s+)?struct SignedPublicationConflictResolutionRequest\s*\{',
        '^\s*(pub\s+)?struct RootSignedPublicationConflictResolution\s*\{',
        '^\s*(pub\s+)?struct PublicationConflictResolutionResponse\s*\{',
        '^\s*(pub\s+)?struct PublicationConflictQrClaim\s*\{',
        'publication-conflict-confirmation:v1'
    )
    if ($duplicateDefinitions) {
        throw "publication-conflict wire implementation is duplicated outside the shared crate:`n$($duplicateDefinitions -join "`n")"
    }

    Write-Output 'publication_conflict_boundary=verified'
    Write-Output 'network_dependencies=false'
    Write-Output 'runtime_dependencies=false'
    Write-Output 'image_dependencies=false'
    Write-Output 'duplicate_wire_implementations=false'
    Write-Output "normal_dependency_records=$($tree.Count)"
}
finally {
    Pop-Location
}
