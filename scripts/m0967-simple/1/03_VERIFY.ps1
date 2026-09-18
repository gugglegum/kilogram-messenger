. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'bob-complete.marker') 1800 'Bob completion marker'
[IO.File]::WriteAllText((Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'), "stop`n")
$null = Wait-M0967File (Join-Path $script:SharedDirectory 'providers-stopped.marker') 120 'providers stopped marker'
$boundaryDestination = Join-Path $script:EvidenceDirectory '06-boundaries.log'
if (Test-Path -LiteralPath $boundaryDestination) { throw "Boundary evidence exists: $boundaryDestination" }
Copy-Item -LiteralPath (Join-Path $script:KitRoot 'BOUNDARIES.log') -Destination $boundaryDestination
& (Join-Path $script:KitRoot 'verify-kilogram-volunteer-field-evidence.ps1') -EvidenceDirectory $script:EvidenceDirectory
if ($LASTEXITCODE -ne 0) { throw 'M0.9.67 evidence verification failed.' }
Write-Host 'M0.9.67 SIMPLE THREE-FOLDER TEST COMPLETED SUCCESSFULLY.'

