. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$build = Assert-M0969Kit
$noHttpsCompatibility = [string]$build.milestone -cne 'M0.9.69'
$serviceFreeV2 = [string]$build.milestone -ceq 'M0.9.76'
$labelPrefix = switch ([string]$build.milestone) {
    'M0.9.76' { 'm0976' }
    'M0.9.74' { 'm0974' }
    'M0.9.73' { 'm0973' }
    default { 'm0972' }
}
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
if ($serviceFreeV2) {
    & (Join-Path $script:KitRoot 'verify-kilogram-m0976-service-free-v2-evidence.ps1') `
        -EvidenceDirectory $script:EvidenceDirectory
    Write-Host 'M0.9.76 SERVICE-FREE V2 VOLUNTEER DELIVERY TEST COMPLETED SUCCESSFULLY.'
} elseif ($noHttpsCompatibility) {
    & (Join-Path $script:KitRoot 'verify-kilogram-m0972-no-https-evidence.ps1') `
        -EvidenceDirectory $script:EvidenceDirectory -LabelPrefix $labelPrefix
    Write-Host "$([string]$build.milestone) CLEAN NO-HTTPS VOLUNTEER DELIVERY TEST COMPLETED SUCCESSFULLY."
} else {
    & (Join-Path $script:KitRoot 'verify-kilogram-m0969-exact-locator-evidence.ps1') `
        -EvidenceDirectory $script:EvidenceDirectory
    Write-Host 'M0.9.69 CLEAN EXACT-LOCATOR FIELD TEST COMPLETED SUCCESSFULLY.'
}
