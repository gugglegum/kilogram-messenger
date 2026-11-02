param([Parameter(Mandatory)][string]$Step)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=(Get-Location).Path
. (Join-Path $root 'scripts/invoke-migration-s54-whole.ps1') -Purpose FeasibilityF
$base=Get-S54Path (Join-Path $root '.tmp/s54-block4f-path-implementation-01')
function Write-OwnedPath([string]$Name,$Value){
    $p=Get-S54Path (Join-Path $base $Name) -AllowMissing
    if(-not $p.StartsWith($base+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $p)){throw 'Fresh canonical owned administration output.'}
    Write-KilogramFrozenJson $p $Value
}
Write-OwnedPath ($Step+'.source-pins.json') @{sources=@('scripts/invoke-migration-s54-whole.ps1','scripts/test-migration-s54-admission.ps1'|ForEach-Object {Get-S54Pin (Join-Path $root $_) 1048576});administration=@('owner.ps1','work.ps1','make-fixtures.py'|ForEach-Object {Get-S54Pin (Join-Path $base $_) 1048576})}
if($Step -like 'path-*'){
    $old=Read-S54Json (Get-S54Pin (Join-Path $root '.tmp/s54-block4d-preparation-order-repair-01/compile-02.result.json') 1048576) 1048576
    $baseline=Read-S54Json (Get-S54Pin (Join-Path $base 'baseline.json') 16384) 16384
    Write-OwnedPath ($Step+'.fixture-index.json') @{classification='public_path_primitive_fixture_set';reference=$baseline.reference;captures=@($old.normal,$old.tests)}
    $evidence=Get-S54Path (Join-Path $base $Step) -AllowMissing
    if(Test-Path $evidence){throw 'Fresh path case directory.'};$null=New-Item -ItemType Directory -Path $evidence
    & (Join-Path $root 'scripts/test-migration-s54-admission.ps1') -PathOnly -FixtureIndex (Join-Path $base ($Step+'.fixture-index.json')) -EvidenceDirectory $evidence
    exit 0
}
if($Step -like 'index-*'){
    $value=[Environment]::GetEnvironmentVariable('TOKIO_WORKER_THREADS','Process');$available=[Environment]::ProcessorCount
    Write-OwnedPath ($Step+'.spec.json') @{helpers=$script:S54Helpers;embedded=$script:S54Embedded;images=$script:S54Images;P=(Get-S54Purpose WholeP);F=(Get-S54Purpose FeasibilityF);environment=@(Get-KilogramFrozenEnvironment -Entries @(Get-ChildItem Env:));workers=@{present=($null -ne $value);value=$value;available=$available;effective=$(if($null -eq $value){$available}else{[int]$value})}}
    & 'C:/Users/Paul/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' (Join-Path $base 'make-fixtures.py') $Step
    if($LASTEXITCODE){throw 'Fresh public construction failed.'}
    $evidence=Get-S54Path (Join-Path $base $Step) -AllowMissing
    if(Test-Path $evidence){throw 'Fresh index evidence directory.'};$null=New-Item -ItemType Directory -Path $evidence
    & (Join-Path $root 'scripts/test-migration-s54-admission.ps1') -CompilerIndexOnly -FixtureIndex (Join-Path $base ($Step+'.fixture-set.json')) -EvidenceDirectory $evidence
    exit 0
}
if($Step -like 'measure-*'){. (Join-Path $base 'measure.ps1');exit 0}
throw 'Unsupported step.'
