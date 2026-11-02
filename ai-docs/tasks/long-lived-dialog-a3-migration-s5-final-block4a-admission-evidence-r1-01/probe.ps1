$ErrorActionPreference='Stop'
. 'E:/AI-Projects/kilogram-p2p-messenger/scripts/invoke-migration-s54-whole.ps1'
$index=ConvertFrom-S54Strict (Read-S54Public 'E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block4a-r1-01/pure-02.fixture-index.json' 16384) 16384
$policy=New-S54Policy -FixtureRoot $index.root
$i=Read-S54Json $index.inputs.inventory 2097152
$capture=Read-S54Json $i.compiler.tests 8388608
Assert-S54Compiler $i $policy $capture $false
Write-Output 'baseline_compiler_accepted=true'
$capture.units[0].profile.debuginfo=@('line-tables-only')
$raw=ConvertFrom-S54Strict ([Text.Encoding]::UTF8.GetBytes($capture.raw_compiler_artifact_json[0])) 131072
$raw.profile.debuginfo=@('line-tables-only')
$capture.raw_compiler_artifact_json[0]=ConvertTo-Json -InputObject $raw -Depth 100 -Compress
Assert-S54Compiler $i $policy $capture $false
Write-Output 'unexpected_array_debuginfo_accepted=true; diagnostic_reproduction_only; no files changed'
