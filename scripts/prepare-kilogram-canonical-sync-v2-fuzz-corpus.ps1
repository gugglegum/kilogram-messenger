[CmdletBinding()]
param(
    [string]$OutputDirectory,
    [switch]$VerifyOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$manifestPath = Join-Path $workspace 'CANONICAL-SYNC-V2-FUZZ-CAMPAIGN.json'
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $workspace '.tmp\fuzz\corpus\canonical_sync_v2_decoders'
}
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)

function ConvertFrom-StrictLowerHex {
    param([Parameter(Mandatory = $true)][string]$Hex)

    if (($Hex.Length % 2) -ne 0 -or $Hex -cnotmatch '^[0-9a-f]*$') {
        throw 'Fuzz seed part is not strict lowercase hexadecimal.'
    }
    $bytes = [byte[]]::new($Hex.Length / 2)
    for ($index = 0; $index -lt $bytes.Length; $index++) {
        $bytes[$index] = [Convert]::ToByte($Hex.Substring($index * 2, 2), 16)
    }
    return $bytes
}

function Get-JsonPointerValue {
    param(
        [Parameter(Mandatory = $true)]$Document,
        [Parameter(Mandatory = $true)][string]$Pointer
    )

    if (-not $Pointer.StartsWith('/')) {
        throw "JSON pointer must start with '/': $Pointer"
    }
    $current = $Document
    foreach ($encoded in $Pointer.Substring(1).Split('/')) {
        $name = $encoded.Replace('~1', '/').Replace('~0', '~')
        if ($current -is [System.Array]) {
            $arrayIndex = 0
            if (-not [int]::TryParse($name, [ref]$arrayIndex) -or
                $arrayIndex -lt 0 -or $arrayIndex -ge $current.Count) {
                throw "JSON pointer array index is invalid: $Pointer"
            }
            $current = $current[$arrayIndex]
            continue
        }
        $property = $current.PSObject.Properties[$name]
        if ($null -eq $property) {
            throw "JSON pointer property is missing: $Pointer"
        }
        $current = $property.Value
    }
    return $current
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Fuzz campaign manifest is missing: $manifestPath"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ([string]$manifest.milestone -cne 'M1.1.73' -or
    [string]$manifest.status -cne 'coverage-guided-fuzz-harness-verified-campaign-not-run' -or
    [string]$manifest.target -cne 'canonical_sync_v2_decoders' -or
    [int]$manifest.selector.prefix_bytes -ne 1 -or
    [int]$manifest.selector.target_count -ne 13 -or
    [int]$manifest.seed_count -ne 20 -or
    [bool]$manifest.observed.sustained_campaign_completed -or
    [bool]$manifest.activation.active_transport_authorized) {
    throw 'Fuzz campaign manifest is inconsistent or prematurely activated.'
}

$allowedDocuments = @(
    'CANONICAL-SYNC-HISTORY-V2-VECTORS.json',
    'CANONICAL-SYNC-V2-VECTORS.json',
    'CANONICAL-SYNC-SESSION-V2-VECTORS.json',
    'CANONICAL-SYNC-RESPONDER-SESSION-V2-VECTORS.json'
)
$documents = @{}
$seenIds = @{}
$totalBytes = 0L
if (-not $VerifyOnly) {
    [System.IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null
}

foreach ($seed in @($manifest.seeds)) {
    $id = [string]$seed.id
    if ($id -cnotmatch '^[a-z0-9]+(?:-[a-z0-9]+)*$' -or $seenIds.ContainsKey($id)) {
        throw "Fuzz seed ID is invalid or duplicated: $id"
    }
    $seenIds[$id] = $true
    $selector = [int]$seed.selector
    if ($selector -lt 0 -or $selector -ge 13) {
        throw "Fuzz seed selector is outside the closed target set: $id"
    }

    $payload = [System.Collections.Generic.List[byte]]::new()
    foreach ($part in @($seed.parts)) {
        $documentName = [string]$part.document
        if ($documentName -cnotin $allowedDocuments) {
            throw "Fuzz seed references a forbidden document: $documentName"
        }
        if (-not $documents.ContainsKey($documentName)) {
            $documentPath = Join-Path $workspace $documentName
            $documents[$documentName] = Get-Content -LiteralPath $documentPath -Raw | ConvertFrom-Json
        }
        $hex = [string](Get-JsonPointerValue `
            -Document $documents[$documentName] `
            -Pointer ([string]$part.pointer))
        $payload.AddRange([byte[]](ConvertFrom-StrictLowerHex -Hex $hex))
    }

    if ($payload.Count -ne [int]$seed.payload_bytes -or
        ($payload.Count + 1) -ne [int]$seed.input_bytes -or
        ($payload.Count + 1) -gt [int]$manifest.campaign_contract.maximum_input_bytes) {
        throw "Fuzz seed byte count drifted: $id"
    }
    $input = [byte[]]::new($payload.Count + 1)
    $input[0] = [byte]$selector
    $payload.CopyTo($input, 1)
    $totalBytes += $input.Length

    if (-not $VerifyOnly) {
        $seedPath = Join-Path $OutputDirectory $id
        [System.IO.File]::WriteAllBytes($seedPath, $input)
    }
    $sha256Provider = [System.Security.Cryptography.SHA256]::Create()
    try {
        $sha256Bytes = $sha256Provider.ComputeHash($input)
    }
    finally {
        $sha256Provider.Dispose()
    }
    $sha256 = (($sha256Bytes | ForEach-Object { $_.ToString('x2') }) -join '')
    Write-Output "fuzz_seed=$id selector=$selector bytes=$($input.Length) sha256=$sha256"
}

if ($seenIds.Count -ne 20) {
    throw "Expected 20 fuzz seeds, got $($seenIds.Count)"
}
Write-Output 'canonical_sync_v2_fuzz_seed_corpus=verified'
Write-Output "seed_count=$($seenIds.Count)"
Write-Output "seed_total_bytes=$totalBytes"
Write-Output "corpus_materialized=$(([bool](-not $VerifyOnly)).ToString().ToLowerInvariant())"
Write-Output 'network_executed=false'
Write-Output 'coverage_guided_campaign_executed=false'

