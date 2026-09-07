[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$preflightPath = Join-Path $PSScriptRoot 'test-kilogram-mailbox-store-preflight.ps1'
$driverPath = Join-Path $PSScriptRoot 'invoke-kilogram-m1-acceptance-step.ps1'
$builderPath = Join-Path $PSScriptRoot 'new-kilogram-m1-acceptance-kit.ps1'
$preflight = Get-Content -LiteralPath $preflightPath -Raw
$driver = Get-Content -LiteralPath $driverPath -Raw
$builder = Get-Content -LiteralPath $builderPath -Raw

foreach ($required in @(
    "`$uri.Scheme -ne 'https'",
    '$handler.AllowAutoRedirect = $false',
    'trusted-default-windows-store',
    'storage_format=opaque-redb-v1',
    'blind_mailbox_transport=reverse-proxy-https-required',
    'blind_mailbox_store_key=',
    'mailbox health response must be exactly ok followed by LF',
    'mailbox startup log exposes forbidden application metadata'
)) {
    if ($preflight.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "mailbox HTTPS preflight is missing '$required'"
    }
}
foreach ($forbidden in @(
    'ServerCertificateCustomValidationCallback',
    'DangerousAcceptAnyServerCertificateValidator',
    'CertificatePolicy',
    'TrustAllCertsPolicy',
    'AllowAutoRedirect = $true'
)) {
    if ($preflight.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "mailbox HTTPS preflight contains a trust/redirect bypass: $forbidden"
    }
}

foreach ($required in @(
    "'bin/kilogram-cli.exe'",
    "'bin/kilogram-windows.exe'",
    'Get-FileHash',
    'BUILD-INFO.json',
    "profile -cne 'debug'",
    'archive -ne $false',
    'network_executed -ne $false',
    'must remain in a private local directory',
    'requires earlier evidence file',
    'store-preflight must run on Bob',
    "'store-preflight'",
    "'bob-01-start-lost-ack'",
    "'alice-05-start-receive'",
    "'copy-boundaries'",
    "'verify'"
)) {
    if ($driver.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "portable acceptance driver is missing '$required'"
    }
}
foreach ($forbidden in @('cargo build', '--release', 'Compress-Archive', '.zip')) {
    if ($driver.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "portable acceptance driver unexpectedly builds or packages artifacts: $forbidden"
    }
}

foreach ($required in @(
    'git status --porcelain --untracked-files=normal',
    'cargo build --jobs $cargoJobsResolved --locked --package kilogram-cli --package kilogram-windows',
    "profile = 'debug'",
    'archive = $false',
    'network_executed = $false',
    "'bin/kilogram-cli.exe'",
    "'bin/kilogram-windows.exe'",
    "'BOUNDARIES.log'",
    "'manifest.example.json'",
    "'alice.local.example.psd1'",
    "'bob.local.example.psd1'"
)) {
    if ($builder.IndexOf($required, [StringComparison]::Ordinal) -lt 0) {
        throw "acceptance kit builder is missing '$required'"
    }
}
foreach ($forbidden in @('--release', 'Compress-Archive', 'ZipFile', '.zip')) {
    if ($builder.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "acceptance kit builder unexpectedly creates a release/archive: $forbidden"
    }
}
if ([regex]::IsMatch($builder, '(?i)Copy-Item[^\r\n]+(?:ProfileFile|IpcFile|state-|device-secret|seed)')) {
    throw 'acceptance kit builder must not copy local profile, IPC, state or secret material.'
}

$manifestPaths = @(
    (Join-Path $workspace 'apps\kilogram-cli\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-windows\Cargo.toml'),
    (Join-Path $workspace 'apps\kilogram-ticket-store\Cargo.toml')
)
foreach ($manifestPath in $manifestPaths) {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw
    if ($manifest -match '(?m)^\s*\[\[bin\]\]\s*$') {
        throw "acceptance work unexpectedly added an explicit executable target: $manifestPath"
    }
}

$selfTestOutput = & powershell -NoProfile -ExecutionPolicy Bypass -File $preflightPath -SelfTest 2>&1
if ($LASTEXITCODE -ne 0 -or
    'mailbox_store_preflight_self_test=passed' -notin @($selfTestOutput | ForEach-Object { [string]$_ })) {
    throw "mailbox store preflight self-test failed.`n$($selfTestOutput -join [Environment]::NewLine)"
}
$stepOutput = & powershell -NoProfile -ExecutionPolicy Bypass -File $driverPath -ListSteps 2>&1
if ($LASTEXITCODE -ne 0 -or $stepOutput.Count -ne 23 -or
    [string]$stepOutput[0] -cne '01=store-preflight' -or
    [string]$stepOutput[22] -cne '23=verify') {
    throw "portable acceptance step inventory is incomplete.`n$($stepOutput -join [Environment]::NewLine)"
}

Write-Output 'm1_acceptance_kit_boundary=verified'
Write-Output 'mailbox_preflight=https-default-trust-no-redirect'
Write-Output 'portable_driver=stable-debug-exe-hash-checked'
Write-Output 'private_state_copy=forbidden'
Write-Output 'release_build=false'
Write-Output 'archive_created=false'
Write-Output 'network_executed=false'
Write-Output 'new_executable=false'
