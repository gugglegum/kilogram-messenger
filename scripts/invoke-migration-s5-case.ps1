[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[CF]-[a-z0-9-]+$')][string]$Case,
    [Parameter(Mandatory)][string]$Filter,
    [ValidateSet('C','F')][string]$Set='F',
    [string[]]$Package=@('kilogram-cli'),
    [string]$FixtureDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$evidence=Join-Path $workspace 'ai-docs/tasks/long-lived-dialog-a3-migration-s5-storage-evidence'
$caseJson=Join-Path $evidence "$Case.json"
if(Test-Path -LiteralPath $caseJson){throw 'Every contributing attempt requires a new case ID.'}
$clock=[Diagnostics.Stopwatch]::StartNew()
$ledgerPath=Join-Path $evidence 'contributing-cases.json'
$previousCases=@()
if(Test-Path -LiteralPath $ledgerPath){$previousCases=@((Get-Content -LiteralPath $ledgerPath -Raw|ConvertFrom-Json).cases)}
$fCeiling=1320
$budgetAmendment=[ordered]@{scope='R4 verification only';review='long-lived-dialog-a3-migration-s5-storage-implementation-review-2.md';review_sha256='bb07f171f3a82b8bf6d864cac40af286374b33461be91697cd1b634aa31299d1';original_F=1200;additional_F=120;rule='Explicit root amendment before R4 execution; cumulative contributors retained, no reset or retroactive PASS'}
$caseExit=0;$caseError=$null
$previousOffline=$env:CARGO_NET_OFFLINE
$previousFixture=$env:KILOGRAM_S5_FIXTURE_DIR
$sourceName="$Case-source-files.json"
$fieldPaths=@('target/compatibility/long-lived-dialog-migration/kilogram-cli.exe','target/compatibility/long-lived-dialog-migration/kilogram-l1-control-peer.exe','target/compatibility/long-lived-dialog-baseline/kilogram-cli.exe','target/compatibility/long-lived-dialog-runtime/kilogram-cli.exe','target/compatibility/long-lived-dialog-predecessor-s3/kilogram-state-predecessor.exe')
$fieldImages=@($fieldPaths|Where-Object {Test-Path -LiteralPath (Join-Path $workspace $_)}|ForEach-Object {[ordered]@{path=$_;bytes=(Get-Item -LiteralPath (Join-Path $workspace $_)).Length;sha256=(Get-FileHash -LiteralPath (Join-Path $workspace $_) -Algorithm SHA256).Hash.ToLowerInvariant()}})
try {
    $stableRoot=[IO.Path]::GetFullPath((Join-Path $workspace 'target/verification/deps'))+[IO.Path]::DirectorySeparatorChar
    $live=@(Get-CimInstance Win32_Process | Where-Object {
        $_.ExecutablePath -and $_.ExecutablePath.StartsWith($stableRoot,[StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($_.ExecutablePath) -match '^kilogram-(cli|state|ratchet)-.*-tests\.exe$'
    } | Select-Object ProcessId,ExecutablePath)
    if($live.Count){throw 'Live stable harness prevents wrapper recopy.'}
    $sourcePaths=@(& git -C $workspace diff --name-only | Where-Object {$_ -match '\.(rs|ps1)$'})
    $sourcePaths+=@(& git -C $workspace ls-files --others --exclude-standard | Where-Object {$_ -match '\.(rs|ps1)$'})
    $sourcePaths+=@('Cargo.lock','scripts/run-cargo-tests-stable.ps1','scripts/invoke-cargo-friendly.ps1')
    $hashes=@($sourcePaths|Sort-Object -Unique|ForEach-Object {
        $full=Join-Path $workspace $_
        [ordered]@{path=$_;bytes=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
    [ordered]@{baseline=(& git -C $workspace rev-parse HEAD);files=$hashes;stable_images=$live} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidence $sourceName) -Encoding utf8
    $env:CARGO_NET_OFFLINE='true'
    if($FixtureDirectory){$env:KILOGRAM_S5_FIXTURE_DIR=[IO.Path]::GetFullPath($FixtureDirectory)}
    & (Join-Path $PSScriptRoot 'run-cargo-tests-stable.ps1') -Package $Package -CargoJobs 12 -Filter $Filter -Run -NoCapture *> (Join-Path $evidence "$Case.log")
    foreach($f in $hashes){if((Get-FileHash -LiteralPath (Join-Path $workspace $f.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $f.sha256){throw "Source changed during contributing case: $($f.path)"}}
    foreach($f in $fieldImages){if((Get-FileHash -LiteralPath (Join-Path $workspace $f.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $f.sha256){throw "Fixed field image changed during case: $($f.path)"}}
} catch {
    $caseExit=1;$caseError=$_.Exception.Message
    $_|Out-String|Add-Content -LiteralPath (Join-Path $evidence "$Case.log")
} finally {
    $env:CARGO_NET_OFFLINE=$previousOffline
    $env:KILOGRAM_S5_FIXTURE_DIR=$previousFixture
}
$images=@(Get-ChildItem -LiteralPath (Join-Path $workspace 'target/verification/deps') -File | Where-Object {$_.Name -match '^kilogram-(cli|state|ratchet)-.*-tests\.exe$'} | ForEach-Object {
    [ordered]@{path=$_.FullName;bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$caseRecord=[ordered]@{case=$Case;contributing_set=$Set;elapsed_seconds=$clock.Elapsed.TotalSeconds;exit_code=$caseExit;error=$caseError;filter=$Filter;source_manifest=$sourceName;seed_case=if($FixtureDirectory){'F-feasibility-01'}else{$null};executable_inventory=$images;field_images_before_after_equal=$fieldImages;rule='Complete runner through log/inventory/ledger aggregation; includes cached compilation, staging, fixtures/copies/recovery and cleanup; final timing serialization excluded; failures retained'}
$caseRecord | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath $caseJson -Encoding utf8
$cases=@(Get-ChildItem -LiteralPath $evidence -Filter '*.json'|Where-Object Name -ne 'contributing-cases.json'|ForEach-Object {
    $v=Get-Content -LiteralPath $_.FullName -Raw|ConvertFrom-Json
    if($v.PSObject.Properties.Name -contains 'case' -and $v.PSObject.Properties.Name -contains 'contributing_set'){$v}
})
# Preserve annotations added to the cumulative ledger without rewriting any old
# per-case artifact (including the first runner's postprocess failure note).
foreach($entry in $cases){
    $previous=@($previousCases|Where-Object {$_.case -eq $entry.case})
    if($previous.Count -gt 1){throw 'Duplicate historical contributor.'}
    if($previous.Count -eq 1){
        foreach($property in $previous[0].PSObject.Properties){
            if($entry.PSObject.Properties.Name -notcontains $property.Name){$entry|Add-Member -NotePropertyName $property.Name -NotePropertyValue $property.Value}
        }
    }
}
Get-Content -LiteralPath (Join-Path $evidence "$Case.log") -Tail 65
$clock.Stop()
$caseRecord.elapsed_seconds=$clock.Elapsed.TotalSeconds
$caseRecord | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath $caseJson -Encoding utf8
($cases|Where-Object {$_.case -eq $Case}).elapsed_seconds=$clock.Elapsed.TotalSeconds
$ct=0.0;$ft=0.0
foreach($entry in $cases){
    if($entry.contributing_set -eq 'C'){$ct+=[double]$entry.elapsed_seconds}
    elseif($entry.contributing_set -eq 'F'){$ft+=[double]$entry.elapsed_seconds}
}
[ordered]@{baseline='3547cf528f15c15a21cdb9e30b1d7ca35a01297a';status='implementation-in-progress';ceilings_seconds=@{C=600;F=$fCeiling;P=2400};budget_amendment=$budgetAmendment;charged_elapsed_seconds=@{C=[double]$ct;F=[double]$ft;P=0};cases=$cases;large511='not_run';wholeP5='not_run';scope='S5.1 R4 storage repair only; live indexed L1/handoff pending'} | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath (Join-Path $evidence 'contributing-cases.json') -Encoding utf8
if($ct -gt 600 -or $ft -gt $fCeiling){throw 'Contributing ceiling exceeded: preserve evidence and callback root before more runs.'}
exit $caseExit
