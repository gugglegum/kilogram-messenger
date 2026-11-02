function New-HDirectory([string]$Name){
    $p=Get-S54Path (Join-Path $base $Name) -AllowMissing
    if(-not $p.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh canonical H test directory required.'}
    $null=New-Item -ItemType Directory -Path $p;return $p
}
$evidence=New-HDirectory $Step
$value=[Environment]::GetEnvironmentVariable('TOKIO_WORKER_THREADS','Process');$available=[Environment]::ProcessorCount
Write-H ($Step+'.spec.json') @{helpers=$script:S54Helpers;embedded=$script:S54Embedded;images=$script:S54Images;P=(Get-S54Purpose WholeP);F=(Get-S54Purpose FeasibilityF);environment=@(Get-KilogramFrozenEnvironment -Entries @(Get-ChildItem Env:));workers=@{present=($null -ne $value);value=$value;available=$available;effective=$(if($null -eq $value){$available}else{[int]$value})}}
& 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' (Join-Path $base 'make-fixtures.py') $Step
if($LASTEXITCODE){throw 'Fresh HEAD admission fixture construction failed.'}
$cases=Read-S54Json (Get-S54Pin (Join-Path $base ($Step+'.fixture-set.json')) 16384) 16384
$f=@($cases.cases|Where-Object name -CEQ 'positive-F')[0];$p=@($cases.cases|Where-Object name -CEQ 'positive-P')[0]
Write-H ($Step+'.head-index.json') @{classification='fresh_public_HEAD_checks';Fixture=$f.index;owned_root=$f.root}
foreach($part in @('head','purpose','order')){$null=New-HDirectory ($Step+'/'+$part)}
& (Join-Path $root 'scripts/test-migration-s54-admission.ps1') -HeadOnly -FixtureIndex (Join-Path $base ($Step+'.head-index.json')) -EvidenceDirectory (Join-Path $evidence 'head')
& (Join-Path $root 'scripts/test-migration-s54-feasibility.ps1') -EvidenceDirectory (Join-Path $evidence 'purpose')
& (Join-Path $root 'scripts/test-migration-s54-preparation-order.ps1') -EvidenceDirectory (Join-Path $evidence 'order') -FixtureIndex $f.index -PFixtureIndex $p.index
Write-H ($Step+'.summary.json') @{head=(Get-S54Pin (Join-Path $evidence 'head/head-results.json') 1048576);purpose=(Get-S54Pin (Join-Path $evidence 'purpose/result.json') 1048576);order=(Get-S54Pin (Join-Path $evidence 'order/result.json') 1048576);historical_fixtures_written=$false;actual_production_Prepare=$false;runtime='not_established'}
