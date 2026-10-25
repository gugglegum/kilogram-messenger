[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
Push-Location $repo
try {
    $baseline='e6f9b8ff6c7e9305d7216a94b142f0220b0c4a25'
    $actual=(& git rev-parse HEAD).Trim()
    if ($actual -ne $baseline) {throw 'Unexpected HEAD; source capture requires the accepted baseline.'}
    $tracked=@(& git diff --name-only -- '*.rs')
    $untracked=@(& git ls-files --others --exclude-standard) | Where-Object {$_ -match '\.rs$|s3-differential\.json$'}
    $paths=@($tracked)+@($untracked) | Sort-Object -Unique
    $sources=@($paths | ForEach-Object {
        $path=$_
        $entry=@(& git ls-tree $baseline -- $path)
        $before=$null
        if ($entry.Count -eq 1 -and $entry[0] -match '^\d+ blob ([0-9a-f]+)\s') {$before=$Matches[1]}
        $file=Get-Item -LiteralPath $path
        [ordered]@{path=$path;bytes=$file.Length;lines=(Get-Content -LiteralPath $path).Count;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant();baseline_blob=$before}
    })
    [ordered]@{baseline=$baseline;source_count=$sources.Count;sources=$sources} | ConvertTo-Json -Depth 7 |
        Set-Content -LiteralPath (Join-Path $PSScriptRoot 'source-manifest.json') -Encoding utf8
    $patch=@(& git diff --no-ext-diff --binary -- $tracked)
    foreach ($path in $untracked) {
        $part=@(& git diff --no-ext-diff --no-index -- /dev/null $path)
        if ($LASTEXITCODE -notin @(0,1)) {throw "git diff failed for $path"}
        $patch += $part
    }
    $patch | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'implementation-source.diff') -Encoding utf8
    $initial=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'baseline.json') -Raw | ConvertFrom-Json
    $protection=@($initial.protected_files | ForEach-Object {
        $current=(Get-FileHash -LiteralPath $_.path -Algorithm SHA256).Hash.ToLowerInvariant()
        $rootOwned=$_.path -in @('ai-docs/README.md','ai-docs/chat-coordination.md','ai-docs/tasks/long-lived-dialog-a3-protocol-s3-implementation-task.md')
        if (-not $rootOwned -and $current -ne $_.sha256) {throw "Frozen file changed: $($_.path)"}
        [ordered]@{path=$_.path;before_sha256=$_.sha256;after_sha256=$current;equal=($current -eq $_.sha256);root_owned=$rootOwned}
    })
    $protection | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'protected-files.json') -Encoding utf8
    $same=@('Cargo.lock','crates/kilogram-protocol/Cargo.toml','crates/kilogram-ratchet/Cargo.toml','crates/kilogram-state/Cargo.toml','apps/kilogram-cli/Cargo.toml','crates/kilogram-protocol/src/ordered_dialog_v1.rs','crates/kilogram-ratchet/src/ordered.rs','apps/kilogram-cli/src/ordered_dialog/ratchet.rs','apps/kilogram-cli/src/ordered_dialog/state.rs','crates/kilogram-state/src/vault/indexed_dialog/history_types.rs','ai-docs/tasks/long-lived-dialog-a3-protocol-s3-plan.md','ai-docs/tasks/long-lived-dialog-a3-protocol-s3-plan-review-1.md','ai-docs/tasks/long-lived-dialog-a3-protocol-s3-plan-review-2.md','ai-docs/tasks/long-lived-dialog-a3-protocol-s3-plan-accepted-files.json')
    & git diff --exit-code $baseline -- $same
    if ($LASTEXITCODE -ne 0) {throw 'Protected source/document diff detected.'}
    & git diff --cached --exit-code
    if ($LASTEXITCODE -ne 0) {throw 'Index has staged changes.'}
    & git diff --check
    if ($LASTEXITCODE -ne 0) {throw 'Whitespace errors.'}
    [ordered]@{baseline=$baseline;head=$actual;source_count=$sources.Count;protected_source_diff='empty';index='empty';whitespace='passed';root_owned_changes='preserved'} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'source-review.json') -Encoding utf8
    "sources=$($sources.Count) protected_source_diff=empty index=empty whitespace=passed"
} finally {Pop-Location}
