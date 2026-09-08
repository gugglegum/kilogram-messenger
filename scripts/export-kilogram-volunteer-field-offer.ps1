[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet('provider1', 'provider2')] [string] $ProviderName,
    [Parameter(Mandatory)] [string] $EvidenceDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
$logPath = Join-Path $evidence "01-$ProviderName.log"
$offerPath = Join-Path $evidence "01-$ProviderName.offer"
if (-not (Test-Path -LiteralPath $logPath -PathType Leaf)) {
    throw "provider runtime log is missing: $logPath"
}
if (Test-Path -LiteralPath $offerPath) {
    throw "provider offer evidence already exists and will not be overwritten: $offerPath"
}
$text = Get-Content -LiteralPath $logPath -Raw
foreach ($required in @('runtime_volunteer_storage=serving', 'status=runtime-listening')) {
    if (-not [regex]::IsMatch($text, "(?m)^$([regex]::Escape($required))$")) {
        throw "provider runtime log is missing '$required'"
    }
}
$matches = [regex]::Matches($text, '(?m)^runtime_volunteer_storage_offer=([A-Za-z0-9_-]+)$')
if ($matches.Count -lt 1) {
    throw 'provider runtime log contains no complete store-signed offer'
}
$offer = $matches[$matches.Count - 1].Groups[1].Value
if ($offer.Length -gt 8192) {
    throw 'provider offer is unexpectedly large'
}
[IO.File]::WriteAllText($offerPath, "$offer`n", [Text.UTF8Encoding]::new($false))
Write-Output "provider_offer_file=$offerPath"
Write-Output "provider_offer_generation=$($matches.Count)"
Write-Output 'status=volunteer-field-offer-exported'
