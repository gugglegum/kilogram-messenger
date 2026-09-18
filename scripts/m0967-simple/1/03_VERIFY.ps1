. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'bob-complete.marker') 1800 'Bob completion marker'
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'), "stop`n")
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'providers-stopped.marker') 120 'providers stopped marker'
$boundaryDestination = Join-Path $script:EvidenceDirectory '06-boundaries.log'
$boundarySource = Join-Path $script:KitRoot 'BOUNDARIES.log'
if (Test-Path -LiteralPath $boundaryDestination) {
    if ((Get-FileHash -LiteralPath $boundaryDestination -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath $boundarySource -Algorithm SHA256).Hash) {
        throw "Existing boundary evidence differs from the tested kit: $boundaryDestination"
    }
    Write-Host 'RESUMING FINAL VERIFICATION WITH THE EXISTING IDENTICAL BOUNDARY EVIDENCE.'
}
else {
    Copy-Item -LiteralPath $boundarySource -Destination $boundaryDestination
}
& (Join-Path $script:KitRoot 'verify-kilogram-volunteer-field-evidence.ps1') -EvidenceDirectory $script:EvidenceDirectory
Write-Host 'M0.9.67 SIMPLE THREE-FOLDER TEST COMPLETED SUCCESSFULLY.'
