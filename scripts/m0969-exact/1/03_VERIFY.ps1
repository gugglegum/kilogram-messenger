. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0969Kit
$null = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'bob-complete.marker') 1800 'Bob completion marker'
[IO.File]::WriteAllText(
    (Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'),
    "stop`n"
)
$null = Wait-M0969File `
    (Join-Path $script:SharedDirectory 'providers-stopped.marker') 120 'providers stopped marker'
$boundaryDestination = Join-Path $script:EvidenceDirectory '08-boundaries.log'
$boundarySource = Join-Path $script:KitRoot 'BOUNDARIES.log'
if (Test-Path -LiteralPath $boundaryDestination) {
    throw "Final boundary evidence already exists; refusing a resumed clean run: $boundaryDestination"
}
Copy-Item -LiteralPath $boundarySource -Destination $boundaryDestination
& (Join-Path $script:KitRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
    -EvidenceDirectory $script:EvidenceDirectory
Write-Host 'M0.9.69 CLEAN EXACT-LOCATOR FIELD TEST COMPLETED SUCCESSFULLY.'
