$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $tree = @(& cargo tree --locked -p kilogram-offline --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while inspecting the offline dependency boundary'
    }

    $forbidden = @(
        'iroh',
        'tokio',
        'reqwest',
        'kilogram-bootstrap-contract',
        'kilogram-protocol',
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
        throw "offline dependency boundary contains forbidden packages:`n$($violations -join "`n")"
    }
    if (-not ($tree | Where-Object { $_ -match '^kilogram-publication-conflict v' })) {
        throw 'offline dependency boundary is missing the shared publication-conflict crate'
    }

    Write-Output 'offline_boundary=verified'
    Write-Output 'shared_publication_conflict_codec=true'
    Write-Output 'network_dependencies=false'
    Write-Output 'runtime_dependencies=false'
    Write-Output "normal_dependency_records=$($tree.Count)"
}
finally {
    Pop-Location
}
