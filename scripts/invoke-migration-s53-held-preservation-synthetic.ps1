[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('\AC-s53-cold-b0c-[a-z0-9-]{1,64}\z')][string]$Attempt)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
$workspace=Split-Path -Parent $PSScriptRoot
$corePath=Join-Path $PSScriptRoot 'migration-s53-cold-preservation.ps1'
$transportPath=Join-Path $PSScriptRoot 'migration-s53-typed-lock-worker.ps1'
$partialPath=Join-Path $PSScriptRoot 'migration-s53-held-preservation.cs'
$coreBytes=[IO.File]::ReadAllBytes($corePath)
if($coreBytes.Length -ne 82999 -or [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($coreBytes)).ToLowerInvariant() -cne 'eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f'){throw 'Frozen Core pin mismatch'}
$coreText=[Text.UTF8Encoding]::new($false,$true).GetString($coreBytes)
$match=[regex]::Match($coreText,'(?s)\$source = @\x27\r?\n(?<cs>.*?)\r?\n\x27@')
if(-not $match.Success){throw 'Frozen heredoc boundary'}
$original=$match.Groups['cs'].Value
$composed=$original
$edits=@(
 @('public sealed class Core : IDisposable {','public sealed partial class Core : IDisposable {'),
 @('    void Empty(Owned item) {','    void Empty(Owned item) { if(IsHeldTyped(item)){HeldEmpty(item);return;}'),
 @('    byte[] Hash(Owned item,long length) {','    byte[] Hash(Owned item,long length) { if(IsHeldTyped(item))return HeldHash(item,length);')
)
foreach($edit in $edits){
 if(([regex]::Matches($composed,[regex]::Escape($edit[0]))).Count -ne 1){throw 'Frozen sentinel count'}
 $composed=$composed.Replace($edit[0],$edit[1])
}
$restored=$composed
foreach($edit in $edits){$restored=$restored.Replace($edit[1],$edit[0])}
if($restored -cne $original){throw 'Unexpected Core substitution'}
$transport=[IO.File]::ReadAllText($transportPath)
$t=[regex]::Match($transport,'(?s)Add-Type -Language CSharp -TypeDefinition @\x27\r?\n(?<cs>.*?)\r?\n\x27@')
if(-not $t.Success){throw 'Transport heredoc boundary'}
$typed=$t.Groups['cs'].Value
$marker='public static partial class S53TypedLockWorker {'
if(([regex]::Matches($typed,[regex]::Escape($marker))).Count -ne 1){throw 'Transport partial boundary'}
$at=$typed.IndexOf($marker,[StringComparison]::Ordinal)
foreach($line in ($typed.Substring(0,$at) -split '\r?\n')){
 if([string]::IsNullOrWhiteSpace($line)){continue}
 if(-not $line.StartsWith('using ',[StringComparison]::Ordinal) -or -not $original.Contains($line)){throw 'Transport using not supplied by frozen Core'}
}
$definition=$composed+[Environment]::NewLine+$typed.Substring($at)+[Environment]::NewLine+[IO.File]::ReadAllText($partialPath)
$composition=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($definition))).ToLowerInvariant()
$hash=[Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
try{
 foreach($path in @($PSCommandPath,$transportPath,$corePath,$partialPath)){
  $bytes=[IO.File]::ReadAllBytes($path)
  if($bytes.Length -gt 256KB){throw 'Composition source cap'}
  $hash.AppendData([BitConverter]::GetBytes([long]$bytes.Length));$hash.AppendData($bytes)
 }
 $source=[Convert]::ToHexString($hash.GetHashAndReset()).ToLowerInvariant()
}finally{$hash.Dispose()}
Add-Type -Language CSharp -TypeDefinition $definition
exit ([S53TypedLockWorker]::RunHeld($source,$Attempt,$composition))
