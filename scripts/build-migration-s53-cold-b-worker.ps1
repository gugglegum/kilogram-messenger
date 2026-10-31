[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$w=Split-Path -Parent $PSScriptRoot
$out=Join-Path $w 'target/verification-scale/s53-cold-worker'
if(Test-Path -LiteralPath $out){throw 'Materialized artifact collision; preserve previous build before a new explicit attempt'}
$paths=@($PSCommandPath,(Join-Path $PSScriptRoot 'migration-s53-cold-preservation.ps1'),(Join-Path $PSScriptRoot 'migration-s53-typed-lock-worker.ps1'),(Join-Path $PSScriptRoot 'migration-s53-held-preservation.cs'),(Join-Path $PSScriptRoot 'migration-s53-cold-b-private.cs'),(Join-Path $PSScriptRoot 'migration-s53-root-copy.ps1'),(Join-Path $PSScriptRoot 'invoke-migration-s53-cold-b-worker.ps1'))
$paths+=@(Join-Path $PSScriptRoot 'migration-s53-held-continuation.cs')
$core=[IO.File]::ReadAllBytes($paths[1])
if($core.Length -ne 82999 -or (Get-FileHash -LiteralPath $paths[1] -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f'){throw 'Frozen Core pin'}
$m=[regex]::Match([Text.UTF8Encoding]::new($false,$true).GetString($core),'(?s)\$source = @\x27\r?\n(?<cs>.*?)\r?\n\x27@')
if(-not $m.Success){throw 'Frozen heredoc boundary'}
$original=$m.Groups['cs'].Value;$composed=$original
$edits=@(
 @('public sealed class Core : IDisposable {','public sealed partial class Core : IDisposable {'),
 @('    void Empty(Owned item) {','    void Empty(Owned item) { if(IsHeldTyped(item)){HeldEmpty(item);return;}'),
 @('    byte[] Hash(Owned item,long length) {','    byte[] Hash(Owned item,long length) { if(IsHeldTyped(item))return HeldHash(item,length);')
)
foreach($edit in $edits){if(([regex]::Matches($composed,[regex]::Escape($edit[0]))).Count -ne 1){throw 'Frozen sentinel count'};$composed=$composed.Replace($edit[0],$edit[1])}
$restored=$composed;foreach($edit in $edits){$restored=$restored.Replace($edit[1],$edit[0])};if($restored -cne $original){throw 'Unexpected frozen edit'}
$bodies=@();$headers=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach($pair in @(@($paths[2],'public static partial class S53TypedLockWorker {'),@($paths[5],'public static class S53RootNativeR38 {'))){
 $text=[IO.File]::ReadAllText($pair[0]);$m=[regex]::Match($text,'(?s)Add-Type -(?:Language CSharp -TypeDefinition|TypeDefinition) @\x27\r?\n(?<cs>.*?)\r?\n\x27@')
 if(-not $m.Success){throw 'Helper heredoc boundary'};$cs=$m.Groups['cs'].Value;$at=$cs.IndexOf($pair[1],[StringComparison]::Ordinal)
 if($at -lt 0 -or ([regex]::Matches($cs,[regex]::Escape($pair[1]))).Count -ne 1){throw 'Helper type boundary'}
 foreach($line in ($cs.Substring(0,$at) -split '\r?\n')){if([string]::IsNullOrWhiteSpace($line)){continue};if($line -cnotmatch '\Ausing [A-Za-z0-9_.]+;\z'){throw 'Helper header'};if(-not $original.Contains($line)){$null=$headers.Add($line)}}
 $bodies+=@($cs.Substring($at))
}
$definition=(@($headers|Sort-Object)+@($composed)+@($bodies)+@([IO.File]::ReadAllText($paths[3]),[IO.File]::ReadAllText($paths[4]),[IO.File]::ReadAllText($paths[7]))) -join [Environment]::NewLine
$hash=[Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
try{foreach($path in $paths){$bytes=[IO.File]::ReadAllBytes($path);if($bytes.Length -gt 256KB){throw 'Source cap'};$hash.AppendData([BitConverter]::GetBytes([long]$bytes.Length));$hash.AppendData($bytes)};$graph=[Convert]::ToHexString($hash.GetHashAndReset()).ToLowerInvariant()}finally{$hash.Dispose()}
$null=New-Item -ItemType Directory -Path $out
[IO.File]::WriteAllText((Join-Path $out 'worker.cs'),$definition,[Text.UTF8Encoding]::new($false))
Add-Type -Language CSharp -TypeDefinition $definition -OutputAssembly (Join-Path $out 'worker.dll')
function Pin([string]$path){[ordered]@{path=$path;bytes=(Get-Item -LiteralPath $path).Length;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}}
$receipt=[ordered]@{source_graph_sha256=$graph;sources=@($paths|ForEach-Object{Pin $_});composed=(Pin (Join-Path $out 'worker.cs'));assembly=(Pin (Join-Path $out 'worker.dll'));frozen_core_edits=3;private_activated=$false}
[IO.File]::WriteAllText((Join-Path $out 'artifact.json'),($receipt|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
$receipt|ConvertTo-Json -Depth 6
