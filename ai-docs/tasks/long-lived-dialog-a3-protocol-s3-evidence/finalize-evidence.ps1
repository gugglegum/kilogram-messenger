[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
Push-Location $repo
try {
    $e='ai-docs/tasks/long-lived-dialog-a3-protocol-s3-evidence'
    & "$e/capture-source.ps1"
    & "$e/collect-verification.ps1"
    $recorded=@(Get-ChildItem -LiteralPath $e -Filter '*-result.json' -File | Sort-Object Name | ForEach-Object {
        $result=Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json
        if($null -ne $result.exit_code -and [int]$result.exit_code -ne 0) {
            [ordered]@{file=$_.Name;exit_code=$result.exit_code;seconds=$result.seconds}
        }
    })
    $early=@('check-initial.log','pure-1.log','compile-2.log')
    foreach($name in $early) {
        $raw=Get-Content -LiteralPath (Join-Path $e $name) -Raw
        if($raw -notmatch '(?m)^error(\[|:)') {throw "Expected early failure missing: $name"}
    }
    [ordered]@{recorded_nonzero_results=$recorded;early_compile_failure_logs_without_result_json=$early;note='Intermediate failures retained; not part of the final 12-command aggregate. Early diagnostics lack complete final provenance.'} |
        ConvertTo-Json -Depth 6 | Set-Content -LiteralPath "$e/failed-runs.json" -Encoding utf8
    $normative=@(
        @{path='ai-docs/tasks/long-lived-dialog-a3-protocol-s3-read-budget-amendment-1.md';sha256='9a5109f5234fbe49f3c0f407d97f3903c829bb981d9b21c1c13ea54c4e71f9fe'},
        @{path='ai-docs/tasks/long-lived-dialog-a3-protocol-s3-provenance-test-clarification-1.md';sha256='b169a7b0c8786df8f7ac1d6cc211d45f197194db8692e9d950ffd86b7fd389e4'},
        @{path='ai-docs/tasks/long-lived-dialog-a3-protocol-s3-time-budget-amendment-1.md';sha256='69d3166a4481540273501273ae2a77a63491934de89616a13afcaa8e69d796e6'}
    )
    foreach($row in $normative) {
        if((Get-FileHash -LiteralPath $row.path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $row.sha256) {throw "Amendment hash changed: $($row.path)"}
    }
    $manifest=Get-Content -LiteralPath "$e/source-manifest.json" -Raw | ConvertFrom-Json
    $summary=Get-Content -LiteralPath "$e/verification-summary.json" -Raw | ConvertFrom-Json
    $clippy=Get-Content -LiteralPath "$e/clippy-final-2-result.json" -Raw | ConvertFrom-Json
    $clippySources=@(Get-Content -LiteralPath "$e/clippy-final-2-source.json" -Raw | ConvertFrom-Json)
    if($clippy.exit_code -ne 0 -or $clippySources.Count -ne $manifest.sources.Count) {throw 'Final Clippy result/source count mismatch'}
    foreach($row in $manifest.sources) {
        $actual=@($clippySources | Where-Object {$_.path -eq $row.path})
        if($actual.Count -ne 1 -or $actual[0].sha256 -ne $row.sha256) {throw "Clippy source mismatch: $($row.path)"}
    }
    if($summary.named_harness_passes -ne 55 -or $summary.runs.Count -ne 12 -or $summary.aggregate_seconds -gt 1200) {throw 'Final verification count/budget mismatch'}
    $static=Get-Content -LiteralPath "$e/static-final-result.json" -Raw | ConvertFrom-Json
    foreach($property in $static.checks.PSObject.Properties) {if([int]$property.Value -ne 0) {throw "Static check failed: $($property.Name)"}}
    $parseErrors=@()
    foreach($script in Get-ChildItem -LiteralPath $e -Filter '*.ps1' -File) {
        $tokens=$null
        $errors=$null
        [void][System.Management.Automation.Language.Parser]::ParseFile($script.FullName,[ref]$tokens,[ref]$errors)
        $parseErrors+=@($errors)
    }
    if($parseErrors.Count -ne 0) {throw 'Evidence helper PowerShell parse failure'}
    $report='ai-docs/tasks/long-lived-dialog-a3-protocol-s3-implementation.md'
    [ordered]@{
        baseline=$manifest.baseline
        source_count=$manifest.sources.Count
        current_source_hashes='matched'
        final_runner_commands=$summary.runs.Count
        final_top_level_passes=$summary.named_harness_passes
        final_runner_aggregate_seconds=$summary.aggregate_seconds
        original_600_second_limit_met=$false
        amended_1200_second_limit_met=$true
        clippy_final='passed; exact final source snapshot'
        static_checks=$static.checks
        evidence_helper_parse_errors=$parseErrors.Count
        frozen_protected_files='matched; see protected-files.json and source-review.json'
        normative_amendments=$normative
        index='empty'
        commit_push='not performed; awaiting explicit root acceptance'
        root_owned_changes='preserved; excluded from executor candidate paths'
        scope='S3 component libraries and cfg(test) CLI; production actor/transport/migration excluded'
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$e/final-integrity.json" -Encoding utf8
    $evidencePaths=@(Get-ChildItem -LiteralPath $e -Recurse -File | ForEach-Object {$_.FullName.Substring($repo.Length+1).Replace('\','/')})
    $evidencePaths=@($evidencePaths)+@("$e/review-files.json","$e/artifact-hashes.json") | Sort-Object -Unique
    $candidate=[ordered]@{
        status='candidate_for_review_not_an_accepted_publication_manifest'
        baseline=$manifest.baseline
        source_paths=@($manifest.sources.path | Sort-Object)
        report_path=$report
        normative_amendment_paths=@($normative.path | Sort-Object)
        evidence_paths=$evidencePaths
        preserved_root_owned_paths=@('ai-docs/README.md','ai-docs/chat-coordination.md','ai-docs/tasks/long-lived-dialog-a3-protocol-s3-implementation-task.md')
        note='Root decides the publication allowlist after code review. No staging or Git writes.'
    }
    $candidate | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath "$e/review-files.json" -Encoding utf8
    $inventoryPaths=@($candidate.source_paths)+@($candidate.report_path)+@($candidate.normative_amendment_paths)+@($candidate.evidence_paths | Where-Object {$_ -ne "$e/artifact-hashes.json"}) | Sort-Object -Unique
    function Write-S3Inventory {
        $rows=@($inventoryPaths | ForEach-Object {
            $file=Get-Item -LiteralPath $_
            [ordered]@{path=$_;bytes=$file.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
        })
        [ordered]@{baseline=$manifest.baseline;self_excluded="$e/artifact-hashes.json";file_count=$rows.Count;files=$rows} |
            ConvertTo-Json -Depth 6 | Set-Content -LiteralPath "$e/artifact-hashes.json" -Encoding utf8
    }
    Write-S3Inventory
    $reportFile=(Resolve-Path -LiteralPath $report).Path
    $doc=Get-Content -LiteralPath $reportFile -Raw
    $missing=@()
    $count=0
    foreach($hit in [regex]::Matches($doc,'\[[^\]]+\]\(([^)]+)\)')) {
        $target=$hit.Groups[1].Value
        if($target -match '^https?://|^#') {continue}
        $count++
        $target=($target -split '#',2)[0]
        $resolved=[IO.Path]::GetFullPath((Join-Path (Split-Path $reportFile -Parent) $target))
        if(-not (Test-Path -LiteralPath $resolved)) {$missing+=$target}
    }
    $trailing=[regex]::Matches($doc,'(?m)[ \t]+\r?$').Count
    if($missing.Count -gt 0 -or $trailing -ne 0) {throw 'Report link/whitespace failure'}
    [ordered]@{checked_links=$count;missing=$missing;trailing_whitespace=$trailing} | ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath "$e/report-links.json" -Encoding utf8
    Write-S3Inventory
    & git diff --check
    if($LASTEXITCODE -ne 0) {throw 'Final whitespace failure'}
    & git diff --cached --quiet
    if($LASTEXITCODE -ne 0) {throw 'Index changed'}
    $inventory=Get-Content -LiteralPath "$e/artifact-hashes.json" -Raw | ConvertFrom-Json
    foreach($row in $inventory.files) {
        if((Get-FileHash -LiteralPath $row.path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $row.sha256) {throw "Inventory mismatch: $($row.path)"}
    }
    @($report,"$e/artifact-hashes.json","$e/review-files.json","$e/verification-summary.json") | ForEach-Object {
        $file=Get-Item -LiteralPath $_
        [ordered]@{path=$_;bytes=$file.Length;sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()}
    } | ConvertTo-Json -Depth 4
    "candidate_source_paths=$($candidate.source_paths.Count) evidence_paths=$($candidate.evidence_paths.Count) hashed_files=$($inventory.file_count) links=$count final_integrity=passed"
} finally {Pop-Location}
