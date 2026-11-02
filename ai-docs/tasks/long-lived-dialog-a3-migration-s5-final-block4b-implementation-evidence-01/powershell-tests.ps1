param([Parameter(Mandatory)][string]$Step)
$ErrorActionPreference='Stop'
$base='E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block4b-01'
$index=Join-Path $base 'pure-06.fixture-index.json'
$evidence=Join-Path $base ($Step+'.public-evidence')
$null=New-Item -ItemType Directory -Path $evidence -ErrorAction Stop
try{& 'E:/AI-Projects/kilogram-p2p-messenger/scripts/test-migration-s54-whole.ps1' -FixtureIndex $index -EvidenceDirectory $evidence}
catch{[Console]::Error.WriteLine($_.ScriptStackTrace);throw}
