. (Join-Path (Split-Path -Parent $PSScriptRoot) 'common.ps1')
$null = Assert-M0967Kit
$run = Get-M0967Run
$privateBase = Get-M0967PrivateRoot 'providers' ([string]$run.run_id)
$profiles = @{}
foreach ($name in @('provider1', 'provider2')) {
    $profile = Join-Path $privateBase "$name\runtime-profile.json"
    if (-not (Test-Path -LiteralPath $profile -PathType Leaf)) {
        throw "Existing provider profile is missing: $profile"
    }
    $profiles[$name] = $profile
}

$stop = Join-Path $script:SharedDirectory 'STOP-PROVIDERS.marker'
$stopped = Join-Path $script:SharedDirectory 'providers-stopped.marker'
if (-not (Test-Path -LiteralPath $stopped -PathType Leaf)) {
    [IO.File]::WriteAllText($stop, "stop-for-wire-fix`n")
    $null = Wait-M0967File $stopped 120 'old providers stopped marker'
}
Remove-Item -LiteralPath $stop -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $stopped -Force -ErrorAction SilentlyContinue

$oldArtifacts = [Collections.Generic.List[string]]::new()
foreach ($name in @('provider1', 'provider2')) {
    $oldArtifacts.Add((Join-Path $script:EvidenceDirectory "01-$name.log"))
    $oldArtifacts.Add((Join-Path $script:EvidenceDirectory "01-$name.offer"))
}
Move-M0967FailedAttemptAside $oldArtifacts.ToArray() 'pre-wire-fix'

$processes = @{}
try {
    foreach ($name in @('provider1', 'provider2')) {
        $log = Join-Path $script:EvidenceDirectory "01-$name.log"
        $process = Start-M0967Process `
            $script:CliPath @('runtime-from-profile', '--profile-file', $profiles[$name]) $log
        $processes[$name] = $process
        $text = Wait-M0967LogPattern $log '^status=runtime-listening$' $process 180
        $offerMatches = [regex]::Matches(
            $text,
            '(?m)^runtime_volunteer_storage_offer=([A-Za-z0-9_-]+)$'
        )
        if ($offerMatches.Count -lt 1) { throw "$name did not publish a volunteer offer" }
        [IO.File]::WriteAllText(
            (Join-Path $script:EvidenceDirectory "01-$name.offer"),
            ($offerMatches[$offerMatches.Count - 1].Groups[1].Value + "`n"),
            [Text.UTF8Encoding]::new($false)
        )
    }
    [IO.File]::WriteAllText(
        (Join-Path $script:SharedDirectory 'providers-ready.marker'),
        "ready-after-wire-fix`n"
    )
    Write-Host 'FIXED PROVIDERS ARE READY. Leave this window open until final verification.'
    while (-not (Test-Path -LiteralPath $stop -PathType Leaf)) {
        foreach ($entry in $processes.GetEnumerator()) {
            if ($entry.Value.HasExited) { throw "$($entry.Key) stopped unexpectedly" }
        }
        Start-Sleep -Seconds 2
    }
}
finally {
    foreach ($process in $processes.Values) { Stop-M0967Process $process }
    [IO.File]::WriteAllText($stopped, "stopped`n")
}
Write-Host 'PROVIDERS STOPPED.'
