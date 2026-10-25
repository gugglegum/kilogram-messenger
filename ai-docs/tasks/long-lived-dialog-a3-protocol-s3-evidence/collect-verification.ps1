[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$repo=(Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
Push-Location $repo
try {
    $manifest=Get-Content (Join-Path $PSScriptRoot 'source-manifest.json') -Raw | ConvertFrom-Json
    $final=@{}
    foreach($row in $manifest.sources){
        $sha=(Get-FileHash -LiteralPath $row.path -Algorithm SHA256).Hash.ToLowerInvariant()
        if($sha -ne $row.sha256){throw "Final source changed: $($row.path)"}
        $final[$row.path]=$sha
    }
    $names=@('n01-final','n02-final','bounded-final','regression-s2-history','regression-s2-cache','regression-s2-private','regression-s1-ledger','regression-s2-cursor','regression-codec1','regression-guard1','regression-byte-adapter','regression-codec1-bounds')
    $runs=@()
    foreach($name in $names){
        $result=Get-Content (Join-Path $PSScriptRoot "$name-result.json") -Raw | ConvertFrom-Json
        if($result.exit_code -ne 0){throw "Final run failed: $name"}
        $source=Get-Content (Join-Path $PSScriptRoot "$name-source.json") -Raw | ConvertFrom-Json
        $differences=@($source | Where-Object {$final[$_.path] -ne $_.sha256} | ForEach-Object {$_.path})
        if($name -eq 'n01-final'){
            $allowed=@('apps/kilogram-cli/src/ordered_dialog/protocol2_failure_tests.rs','crates/kilogram-protocol/src/ordered_dialog_v2/tests/golden.rs')
            if(@($differences | Where-Object {$_ -notin $allowed}).Count -gt 0){throw 'Unexpected N01 source reuse difference'}
        } elseif($differences.Count -ne 0){throw "Source mismatch for $name"}
        $harnesses=@()
        $current=$null
        $compile=@()
        foreach($line in Get-Content (Join-Path $PSScriptRoot "$name.log")){
            if($line -match '^running_test_executable=(.*)$'){$current=$Matches[1]}
            if($line -match '^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;.*finished in ([0-9.]+)s'){
                $harnesses += [ordered]@{path=$current;passed=[int]$Matches[1];failed=[int]$Matches[2];ignored=[int]$Matches[3];seconds=[double]$Matches[4]}
            }
            if($line -match '^\s*Finished .* in ([0-9.]+)s$'){$compile += [double]$Matches[1]}
        }
        # Child-process cursor output can contain an inner libtest summary. The
        # last result for each stable executable is its top-level harness result.
        $top=@{}
        foreach($h in $harnesses){$top[$h['path']]=$h}
        $outer=@()
        foreach($exe in $result.executables){if($top.ContainsKey($exe.path)){$outer+=$top[$exe.path]}}
        $runs += [ordered]@{name=$name;exit_code=$result.exit_code;runner_seconds=$result.seconds;source_differences_from_final=$differences;compile_seconds=$compile;harnesses=$outer;all_libtest_summaries=$harnesses;executables=$result.executables}
    }
    $sum=0.0
    foreach($run in $runs){$sum += [double]$run['runner_seconds']}
    if($sum -gt 1200){throw "Amended aggregate exceeded: $sum"}
    $passes=0
    foreach($run in $runs){foreach($h in $run.harnesses){$passes+=$h.passed}}
    [ordered]@{baseline=$manifest.baseline;aggregate_seconds=$sum;aggregate_limit=1200;original_limit=600;original_limit_met=$false;named_harness_passes=$passes;runs=$runs} |
        ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'verification-summary.json') -Encoding utf8
    $n1=$runs[0]
    $n2=$runs[1]
    $correspondence=@($n1.executables | ForEach-Object {
        $first=$_
        $second=$n2.executables | Where-Object {$_.path -eq $first.path} | Select-Object -First 1
        $h1=$n1.harnesses | Where-Object {$_.path -eq $first.path} | Select-Object -First 1
        $h2=$n2.harnesses | Where-Object {$_.path -eq $first.path} | Select-Object -First 1
        [ordered]@{path=$first.path;n01_sha256=$first.sha256;n02_sha256=$second.sha256;equal=($first.sha256 -eq $second.sha256);n01_passed=$h1.passed;n02_passed=$h2.passed}
    })
    [ordered]@{n01_reuse='Production sources, shared helpers and N01 driver byte-identical to final; only unrelated G02 fixture and protocol differential assertions changed';n01_source_differences=$n1.source_differences_from_final;artifacts=$correspondence} |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'source-exe-correspondence.json') -Encoding utf8
    "final_aggregate_seconds=$sum named_passes=$passes source_correspondence=passed"
} finally {Pop-Location}
