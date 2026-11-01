param([Parameter(Mandatory)][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
$base=Join-Path $workspace '.tmp/s54-block3e-p11a4-01'
$env:CARGO_NET_OFFLINE='true'
$selection=Get-Content -LiteralPath (Join-Path $base 'test-selection.json') -Raw|ConvertFrom-Json
$skip=@($selection.skip)
. (Join-Path $workspace 'scripts/cargo-frozen-artifacts.ps1')
$capturePath=Join-Path $base 'compile-01.capture.json'
$capture=Get-Content -LiteralPath $capturePath -Raw|ConvertFrom-Json -AsHashtable
$expectedPath=Join-Path $base "$Step.expected.json"
$current=Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
Assert-KilogramFrozenInputs $capture.build_inputs_before $current
Assert-KilogramFrozenInputs $capture.build_inputs_after $current
$expected=[ordered]@{format_version=1;classification='frozen-scale-test-artifacts';profile='verification-scale';target='x86_64-pc-windows-msvc';stable_directory=(Join-Path $workspace 'target/verification-scale/deps');build_inputs=$current;units_identity=@(Get-KilogramFrozenUnitIdentity $capture.units);artifacts=$capture.stable_artifacts}
Write-KilogramFrozenJson $expectedPath $expected
$sha=(Get-FileHash -LiteralPath $expectedPath -Algorithm SHA256).Hash.ToLowerInvariant()
& (Join-Path $workspace 'scripts/run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -Profile verification-scale -CargoJobs 2 -SkipCacheStatus -CargoTarget x86_64-pc-windows-msvc -StableArtifactsDirectory (Join-Path $workspace 'target/verification-scale/deps') -BuildEvidencePath (Join-Path $base "$Step.capture.json") -ExpectedArtifactsManifest $expectedPath -ExpectedArtifactsSha256 $sha -Filter s54_ -SkipTest $skip -Run -NoCapture
exit $LASTEXITCODE
