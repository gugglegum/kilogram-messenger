[CmdletBinding()]
param([Parameter(Mandatory)][string]$InputDirectory,[Parameter(Mandatory)][string]$OutputManifest)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
$root=[IO.Path]::GetFullPath($InputDirectory)
$expectedRoot=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-l1/F-exhausted-origin-01'))
$output=[IO.Path]::GetFullPath($OutputManifest)
$allowedOutput=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-storage'))+[IO.Path]::DirectorySeparatorChar
if(-not $root.Equals($expectedRoot,[StringComparison]::OrdinalIgnoreCase) -or -not $output.StartsWith($allowedOutput,[StringComparison]::OrdinalIgnoreCase)){throw 'Unexpected origin or private verification output.'}
if(Test-Path -LiteralPath $output){throw 'Origin verification record is immutable.'}
$manifestPath=Join-Path $root 'frozen-origin.json'
$manifest=Get-Content -LiteralPath $manifestPath -Raw|ConvertFrom-Json
if($manifest.classification -cne 'genuine-stopped-origin-provenance-not-case-verdict' -or $manifest.producer_sha256 -cne 'd9b7ff82469211390df3b084f52e42414e780897253396ace9886c00824793b1' -or $manifest.asymmetric_text_counts.A -ne 511 -or $manifest.asymmetric_text_counts.B -ne 16 -or @($manifest.processes|Where-Object {-not $_.has_exited}).Count){throw 'Origin producer/count/stopped provenance mismatch.'}
if(-not ([IO.Path]::GetFullPath($manifest.frozen_original)).Equals($root,[StringComparison]::OrdinalIgnoreCase) -or (Get-FileHash -LiteralPath $manifest.driver_path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $manifest.driver_sha256){throw 'Origin path/producer script binding mismatch.'}
$expected=@{}
foreach($row in $manifest.files){
 if([IO.Path]::IsPathRooted($row.path) -or $expected.ContainsKey($row.path) -or $row.path -ceq 'frozen-origin.json'){throw 'Invalid/duplicate original inventory path.'}
 $full=[IO.Path]::GetFullPath((Join-Path $root $row.path))
 if(-not $full.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Original inventory escapes root.'}
 $expected[$row.path]=$row
}
$actual=@(Get-ChildItem -LiteralPath $root -Recurse -Force|Sort-Object FullName|ForEach-Object {
 if($_.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Origin reparse point refused.'}
 if(-not $_.PSIsContainer -and $_.FullName -cne $manifestPath){
  $relative=[IO.Path]::GetRelativePath($root,$_.FullName).Replace('\','/')
  $hash=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  if(-not $expected.ContainsKey($relative) -or $expected[$relative].bytes -ne $_.Length -or $expected[$relative].sha256 -cne $hash){throw "Frozen origin byte mismatch: $relative"}
  [ordered]@{path=$relative;bytes=$_.Length;sha256=$hash}
 }
})
if($actual.Count -ne $expected.Count){throw 'Frozen origin inventory incomplete.'}
$manifestHash=(Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
[ordered]@{classification='actual-byte-verified-stopped-origin-no-repository-open';origin=$root;frozen_manifest_sha256=$manifestHash;files=$actual}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $output -Encoding utf8
[ordered]@{origin=$root;verified_files=$actual.Count;frozen_manifest_sha256=$manifestHash;verification_path=$output;verification_sha256=(Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant();original_unchanged=$true}|ConvertTo-Json -Compress
