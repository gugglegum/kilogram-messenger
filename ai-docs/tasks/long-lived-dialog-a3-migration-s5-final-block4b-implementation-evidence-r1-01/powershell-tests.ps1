param([Parameter(Mandatory)][string]$Step)
$ErrorActionPreference='Stop'
$base='E:/AI-Projects/kilogram-p2p-messenger/.tmp/s54-block4b-r1-01'
$evidence=Join-Path $base ($Step+'.public-evidence')
$null=New-Item -ItemType Directory -Path $evidence -ErrorAction Stop
try{& 'E:/AI-Projects/kilogram-p2p-messenger/scripts/test-migration-s54-repair.ps1' -FixtureIndex (Join-Path $base 'pure-02.fixture-index.json') -EvidenceDirectory $evidence}catch{[Console]::Error.WriteLine($_.ScriptStackTrace);throw}
