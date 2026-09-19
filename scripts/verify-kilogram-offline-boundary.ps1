$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $manifestPath = Join-Path $workspace 'apps\kilogram-offline\Cargo.toml'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw
    if ($manifest -cnotmatch '(?m)^blake3\s*=\s*\{[^\r\n]*workspace\s*=\s*true[^\r\n]*features\s*=\s*\[[^\r\n]*"pure"') {
        throw 'kilogram-offline must enable the pinned blake3 pure feature.'
    }

    $tree = @(& cargo tree --offline --locked -p kilogram-offline --target x86_64-pc-windows-msvc --edges normal --prefix none --format '{p}' | Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) {
        throw 'cargo tree failed while inspecting the offline dependency boundary'
    }
    $featureTree = @(& cargo tree --offline --locked -p kilogram-offline --target x86_64-pc-windows-msvc --edges features -i blake3)
    if ($LASTEXITCODE -ne 0 -or
        -not ($featureTree | Where-Object { $_ -cmatch 'blake3 feature "pure"$' })) {
        throw 'kilogram-offline resolved graph does not enable blake3 pure code generation.'
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
    Write-Output 'blake3_codegen=pure-rust-intrinsics'
    Write-Output 'linked_blake3_native_objects=false'
    Write-Output 'network_dependencies=false'
    Write-Output 'runtime_dependencies=false'
    Write-Output "normal_dependency_records=$($tree.Count)"
}
finally {
    Pop-Location
}
