[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../..')).Path
$trackedPaths = @('apps/kilogram-cli/src/main.rs', 'crates/kilogram-ratchet/src/lib.rs')
$newPaths = @('apps/kilogram-cli/src/ordered_ingress.rs', 'apps/kilogram-cli/src/ordered_ingress_tests.rs')
$patchPath = Join-Path $PSScriptRoot 'reviewable.patch'

Push-Location $workspace
try {
    $tracked = @(& git diff --no-ext-diff --binary -- @trackedPaths)
    if ($LASTEXITCODE -ne 0) { throw 'Could not export tracked source diff.' }
    $builder = [System.Text.StringBuilder]::new()
    [void]$builder.Append(($tracked -join "`n") + "`n")
    foreach ($relative in $newPaths) {
        $content = [System.IO.File]::ReadAllText((Join-Path $workspace $relative)).Replace("`r`n", "`n")
        if (-not $content.EndsWith("`n")) { throw "Source lacks final newline: $relative" }
        $lines = $content.Substring(0, $content.Length - 1).Split("`n")
        [void]$builder.Append("diff --git a/$relative b/$relative`nnew file mode 100644`n--- /dev/null`n+++ b/$relative`n@@ -0,0 +1,$($lines.Length) @@`n")
        foreach ($line in $lines) { [void]$builder.Append("+$line`n") }
    }
    [System.IO.File]::WriteAllText($patchPath, $builder.ToString(), [System.Text.UTF8Encoding]::new($false))
    & git apply --reverse --check --whitespace=error $patchPath
    if ($LASTEXITCODE -ne 0) { throw 'Export does not match the current working source.' }
    & git apply --stat $patchPath
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect patch.' }
    Write-Output 'reviewable_patch_reverse_check=passed'
    Get-FileHash -Algorithm SHA256 -LiteralPath $patchPath | Select-Object Hash,Path
}
finally {
    Pop-Location
}
