[CmdletBinding()]
param([Parameter(Mandatory)][string]$InputDirectory,[Parameter(Mandatory)][string]$OutputDirectory,
 [Parameter(Mandatory)][string]$OwnedLockOne,[Parameter(Mandatory)][string]$OwnedLockTwo)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$workspace=Split-Path -Parent $PSScriptRoot
function NormalPath([string]$Path){$full=[IO.Path]::GetFullPath($Path);if($full.StartsWith('\\?\')){$full=$full.Substring(4)};$full}
$inputRoot=NormalPath $InputDirectory
$outputRoot=NormalPath $OutputDirectory
$allowed=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-storage'))+[IO.Path]::DirectorySeparatorChar
$preserved=[IO.Path]::GetFullPath((Join-Path $workspace '.tmp/migration-s5-preserved'))+[IO.Path]::DirectorySeparatorChar
if(-not $inputRoot.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase) -or -not $outputRoot.StartsWith($preserved,[StringComparison]::OrdinalIgnoreCase)){throw 'Private preservation outside owned roots.'}
if(Test-Path -LiteralPath $outputRoot){throw 'Preserved input is immutable.'}
$owned=@((NormalPath $OwnedLockOne),(NormalPath $OwnedLockTwo))
$expected=@((NormalPath (Join-Path $inputRoot 'one/state/.kilogram-state.lock')),(NormalPath (Join-Path $inputRoot 'two/state/.kilogram-state.lock')))
for($i=0;$i -lt 2;$i++){if(-not $owned[$i].Equals($expected[$i],[StringComparison]::OrdinalIgnoreCase)){throw 'Missing/extraneous exact held-lock ownership.'}}
function Check-OwnedMetadata {
 foreach($path in $owned){$item=Get-Item -Force -LiteralPath $path;if($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $item.Length -ne 0){throw 'Held lock metadata is not a regular empty file.'};[ordered]@{path=$path;bytes=$item.Length;created_ticks=$item.CreationTimeUtc.Ticks;written_ticks=$item.LastWriteTimeUtc.Ticks;attributes=[int]$item.Attributes}}
}
function Inventory([string]$Root,[bool]$Source){@(
 Get-ChildItem -LiteralPath $Root -Recurse -Force | Sort-Object FullName | ForEach-Object {
  if($_.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Fixture reparse point refused.'}
  if(-not $_.PSIsContainer){
   $derived=$false
   if($Source -and $_.Name -ceq '.kilogram-state.lock'){
    if($owned -notcontains $_.FullName){throw 'Unlisted synchronization artifact refused.'}
    if($_.Length -ne 0){throw 'Nonempty OS-lock artifact refused; no content inference.'}
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([byte[]]@())).ToLowerInvariant()
    $derived=$true
   }else{$hash=(Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()}
   [ordered]@{path=[IO.Path]::GetRelativePath($Root,$_.FullName).Replace('\','/');bytes=$_.Length;sha256=$hash;digest_provenance=$(if($derived){'held-lock length checked; empty-byte digest derived'}else{'actual file byte read'})}
  }
 }
)}
$ownedBefore=@(Check-OwnedMetadata)
$before=Inventory $inputRoot $true
[void](New-Item -ItemType Directory -Path $outputRoot)
foreach($item in $before){
 $target=Join-Path $outputRoot $item.path
 [void][IO.Directory]::CreateDirectory((Split-Path -Parent $target))
 if($owned -contains (NormalPath (Join-Path $inputRoot $item.path))){
  # A held Windows range lock rejects ReadFile even for a zero-length file.
  # Only this verified-empty synchronization artifact is reconstructed empty;
  # every state/Root/Native byte is copied and hashed normally.
  $empty=[IO.File]::Open($target,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
  $empty.Dispose()
 }else{Copy-Item -LiteralPath (Join-Path $inputRoot $item.path) -Destination $target}
}
$after=Inventory $inputRoot $true;$copy=Inventory $outputRoot $false
$ownedAfter=@(Check-OwnedMetadata)
if((ConvertTo-Json -InputObject $ownedBefore -Compress) -cne (ConvertTo-Json -InputObject $ownedAfter -Compress)){throw 'Held synchronization metadata changed.'}
function ByteIdentity($Rows){ConvertTo-Json -InputObject @($Rows|ForEach-Object {[ordered]@{path=$_.path;bytes=$_.bytes;sha256=$_.sha256}}) -Compress}
$canonical=ByteIdentity $before
if($canonical -cne (ByteIdentity $after) -or $canonical -cne (ByteIdentity $copy)){throw 'Stopped-input bytes changed during preservation.'}
$manifest=Join-Path $outputRoot 'preservation-inventory.json'
[ordered]@{source=$inputRoot;preserved=$outputRoot;classification='private-stopped-copy-never-opened';held_lock_paths=$owned;os_lock_rule='Only two exact retained-handle paths: source metadata length checked before/after; empty digest derived. Destinations CreateNew and actual-byte hashed. All other state bytes actual read/copy/hash.';source_files=$before;source_after_files=$after;destination_files=$copy}|ConvertTo-Json -Depth 7 | Set-Content -LiteralPath $manifest -Encoding utf8
[ordered]@{source=$inputRoot;preserved=$outputRoot;inventory_path=$manifest;inventory_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $manifest).Hash.ToLowerInvariant();files=$before.Count;source_unchanged=$true;copy_matches=$true}|ConvertTo-Json -Compress
