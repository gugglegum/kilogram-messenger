param([Parameter(Mandatory)][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace='E:\AI-Projects\kilogram-p2p-messenger'
$base=Join-Path $workspace '.tmp/s54-block4b-r1-01'
$env:CARGO_NET_OFFLINE='true'
$env:KILOGRAM_S54_ADMISSION_FIXTURE_EXPORT=Join-Path $base ($Step+'.fixture-index.json')
. (Join-Path $workspace 'scripts/invoke-migration-s54-whole.ps1')
$embedded=ConvertFrom-S54Strict (Read-S54Public (Join-Path $base 'compile-02.capture.json.embedded.json') 2097152) 2097152
$before=Get-S54EmbeddedSnapshot $workspace
for($n=0;$n -lt $before.Count;$n++){Assert-S54PinEqual $before[$n] $embedded.before[$n];Assert-S54PinEqual $before[$n] $embedded.after[$n]}
Write-KilogramFrozenJson (Join-Path $base ($Step+'.embedded-before.json')) @{classification='public exact embedded inputs before frozen-run build';inputs=$before}

$capture=Get-Content -LiteralPath (Join-Path $base 'compile-02.capture.json') -Raw|ConvertFrom-Json -AsHashtable
$current=Get-KilogramFrozenBuildInputs -Workspace $workspace -Profile verification-scale -Target x86_64-pc-windows-msvc -Package @('kilogram-cli','kilogram-transport-iroh') -CargoFeatures @()
Assert-KilogramFrozenInputs $capture.build_inputs_before $current
Assert-KilogramFrozenInputs $capture.build_inputs_after $current
$expectedPath=Join-Path $base "$Step.expected.json"
$expected=[ordered]@{format_version=1;classification='frozen-scale-test-artifacts';profile='verification-scale';target='x86_64-pc-windows-msvc';stable_directory=(Join-Path $workspace 'target/verification-scale/deps');build_inputs=$current;units_identity=@(Get-KilogramFrozenUnitIdentity $capture.units);artifacts=$capture.stable_artifacts}
Write-KilogramFrozenJson $expectedPath $expected
$sha=(Get-FileHash -LiteralPath $expectedPath -Algorithm SHA256).Hash.ToLowerInvariant()
$selection=Get-Content -LiteralPath (Join-Path $base 'test-selection.json') -Raw|ConvertFrom-Json
$inventory=@();$skip=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach($artifact in $capture.stable_artifacts){
 $lines=@(& $artifact.path '--list' '--format' 'terse');if($LASTEXITCODE -ne 0){throw 'Stable test inventory failed.'}
 $names=@($lines|ForEach-Object {if($_ -match '^(.+): (test|benchmark)$'){$Matches[1]}})
 foreach($name in $names){$prefix=if($name.Contains('::')){$name.Split('::')[0]+'::'}else{$name};[void]$skip.Add($prefix)}
 $inventory+=@{path=$artifact.path;names=$names}
}
$allNames=@($inventory|ForEach-Object {$_.names})
foreach($name in $selection.expected_names){if(@($allNames|Where-Object {$_ -ceq $name}).Count -ne 1){throw "Target must resolve exactly once: $name"}}
foreach($name in $allNames){if(-not @($skip|Where-Object {$name.Contains($_)}).Count){throw 'Aggregate skip coverage incomplete.'}}
Write-KilogramFrozenJson (Join-Path $base "$Step.inventory.json") @{artifacts=$inventory;aggregate_skips=@($skip|Sort-Object);selected=$selection.expected_names}
& (Join-Path $workspace 'scripts/run-cargo-tests-stable.ps1') -Package @('kilogram-cli','kilogram-transport-iroh') -Profile verification-scale -CargoJobs 2 -SkipCacheStatus -CargoTarget x86_64-pc-windows-msvc -StableArtifactsDirectory (Join-Path $workspace 'target/verification-scale/deps') -BuildEvidencePath (Join-Path $base "$Step.capture.json") -ExpectedArtifactsManifest $expectedPath -ExpectedArtifactsSha256 $sha -IsolatedTestsFile (Join-Path $base 'isolated-tests.json') -IsolatedTestOrder BeforeAggregate -SkipTest @($skip) -Run -NoCapture
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
$after=Get-S54EmbeddedSnapshot $workspace
for($n=0;$n -lt $before.Count;$n++){Assert-S54PinEqual $before[$n] $after[$n]}
$build=ConvertFrom-S54Strict (Read-S54Public (Join-Path $base ($Step+'.capture.json')) 8388608) 8388608
$deps=Get-S54DepInfoPins $build $false
Assert-S54EmbeddedDependencies $workspace $deps $embedded.sources
Write-KilogramFrozenJson (Join-Path $base ($Step+'.embedded.json')) @{classification='public exact embedded inputs before/after frozen-run build and six selected rustc dep-info pins';before=$before;after=$after;dep_info=$deps;normal_cli_built=$false}
exit 0

