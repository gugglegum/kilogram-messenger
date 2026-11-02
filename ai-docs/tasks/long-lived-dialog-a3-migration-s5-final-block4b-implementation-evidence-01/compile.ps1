param([Parameter(Mandatory)][string]$Capture)
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
$env:CARGO_NET_OFFLINE='true'
. (Join-Path $workspace 'scripts/invoke-migration-s54-whole.ps1')
$before=Get-S54EmbeddedSnapshot $workspace
Write-KilogramFrozenJson ($Capture+'.embedded-before.json') @{classification='public exact embedded inputs captured before build';inputs=$before}

& (Join-Path $workspace 'scripts/run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -Profile verification-scale -CargoJobs 2 -SkipCacheStatus -CargoTarget x86_64-pc-windows-msvc -StableArtifactsDirectory (Join-Path $workspace 'target/verification-scale/deps') -BuildEvidencePath $Capture
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
$after=Get-S54EmbeddedSnapshot $workspace
for($n=0;$n -lt $before.Count;$n++){Assert-S54PinEqual $before[$n] $after[$n]}
$build=ConvertFrom-S54Strict (Read-S54Public $Capture 8388608) 8388608
$deps=Get-S54DepInfoPins $build $false
$sourcePaths=Get-S54SourcePaths $workspace
$sources=@($sourcePaths|ForEach-Object {Get-S54Pin $_ 8388608})
Assert-S54EmbeddedDependencies $workspace $deps $sources
Write-KilogramFrozenJson ($Capture+'.embedded.json') @{classification='public exact embedded inputs before/after compile-only and six selected rustc dep-info pins';before=$before;after=$after;dep_info=$deps;sources=$sources;normal_cli_built=$false}
exit 0

