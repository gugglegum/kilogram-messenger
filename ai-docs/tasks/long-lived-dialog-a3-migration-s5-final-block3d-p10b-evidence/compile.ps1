param([Parameter(Mandatory)][string]$Capture)
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
$env:CARGO_NET_OFFLINE='true'
& (Join-Path $workspace 'scripts/run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -Profile verification-scale -CargoJobs 2 -SkipCacheStatus -CargoTarget x86_64-pc-windows-msvc -StableArtifactsDirectory (Join-Path $workspace 'target/verification-scale/deps') -BuildEvidencePath $Capture
exit $LASTEXITCODE
